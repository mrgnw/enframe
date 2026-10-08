use crate::{ApiResult, AppState, blocking, denied};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::process::Command;
use tokio::sync::mpsc;

const PHOTO_EXTENSIONS: [&str; 7] = ["avif", "heic", "heif", "jxl", "jpg", "jpeg", "png"];
const SUPERSEDED: &str = "superseded by a newer run for this folder";

// The tracker, called on a blocking thread with each progress line it reports. Tests
// stand in for it.
pub type Run = fn(::stabilize::Job, &mut dyn FnMut(&str)) -> Result<::stabilize::Sidecar, String>;

// Tracks, then writes the clip and sidecar when the job has an output.
pub fn run(
    job: ::stabilize::Job,
    log: &mut dyn FnMut(&str),
) -> Result<::stabilize::Sidecar, String> {
    let tracked = ::stabilize::track(&job, log)?;
    match job.output {
        Some(_) => tracked.write(log),
        None => Ok(tracked.sidecar(&::stabilize::run_id(), None)),
    }
}

// One run per folder. Tickets are global, so a ticket is never reused once a folder's
// entry is dropped, and only the newest ticket for a folder may write or run.
pub struct Stabilize {
    run: Run,
    tickets: AtomicU64,
    folders: Mutex<HashMap<PathBuf, Folder>>,
}

// Setting `cancel` stops the run holding the other end at its next photo.
struct Folder {
    ticket: u64,
    cancel: Arc<AtomicBool>,
}

// Dropping the request's future, when the client leaves, stops its run.
struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

type Point = [f64; 2];

// One feature's marks. The document the marking UI edits is a list of named features,
// saved beside the folder as `<folder>.pin.json`; each feature is one tracker run. The
// marks' fields are the tracker's: `points` is the first photo's, `at` and `near` map a
// photo to its mark and soft mark, `skip` lists hidden photos, `order` is the order.
#[derive(Deserialize, Serialize)]
pub struct Marks {
    points: Vec<Point>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    at: BTreeMap<String, Mark>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    near: BTreeMap<String, Mark>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    skip: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order: Option<Order>,
}

#[derive(Deserialize, Serialize)]
struct Feature {
    name: String,
    marks: Marks,
}

#[derive(Deserialize, Serialize)]
struct Mark {
    points: Vec<Point>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zoom: Option<f64>,
}

#[derive(Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Order {
    Time,
    Name,
}

#[derive(Deserialize)]
struct RunArgs {
    folder: String,
    features: Vec<Feature>,
    feature: String,
    pad: Option<String>,
    out: Option<String>,
    fps: Option<f64>,
    frames: Option<String>,
    vf: Option<String>,
}

fn bad(message: impl Into<String>) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, message.into())
}

fn superseded() -> (StatusCode, String) {
    (StatusCode::CONFLICT, SUPERSEDED.to_string())
}

fn check_marks(marks: &Marks) -> Result<(), String> {
    if marks.points.is_empty() {
        return Err("points: needs at least one point".to_string());
    }
    for (key, map) in [("at", &marks.at), ("near", &marks.near)] {
        for (file, mark) in map {
            if mark.points.is_empty() {
                return Err(format!("{key} {file}: needs at least one point"));
            }
            if mark.zoom.is_some_and(|zoom| zoom <= 0.0) {
                return Err(format!("{key} {file}: zoom must be a positive number"));
            }
        }
    }
    Ok(())
}

// A name ends up in the default clip's file name, so it is plain and unique.
fn check_features(features: &[Feature], feature: &str) -> ApiResult<usize> {
    for (i, Feature { name, marks }) in features.iter().enumerate() {
        if name.trim().is_empty() || name.contains('/') {
            return Err(bad(format!(
                "feature {name:?}: needs a name, without a slash"
            )));
        }
        if features[..i].iter().any(|other| other.name == *name) {
            return Err(bad(format!("feature {name}: the name is taken")));
        }
        check_marks(marks).map_err(|e| bad(format!("feature {name}: {e}")))?;
    }
    features
        .iter()
        .position(|other| other.name == feature)
        .ok_or_else(|| bad(format!("feature {feature}: not in the document")))
}

fn parse_pad(raw: Option<String>) -> ApiResult<Option<String>> {
    let Some(pad) = raw.filter(|pad| pad != "none") else {
        return Ok(None);
    };
    let colour = pad
        .strip_prefix('#')
        .is_some_and(|hex| hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()));
    if colour || ["transparent", "black", "white"].contains(&pad.as_str()) {
        return Ok(Some(pad));
    }
    Err(bad("pad must be transparent, black, white or #rrggbb"))
}

fn points(points: &[Point]) -> Vec<(f64, f64)> {
    points.iter().map(|&[x, y]| (x, y)).collect()
}

fn tracker_mark(mark: &Mark) -> ::stabilize::Mark {
    ::stabilize::Mark {
        points: points(&mark.points),
        zoom: mark.zoom,
    }
}

fn tracker_marks(marks: &Marks) -> ::stabilize::Marks {
    let mut tracker = ::stabilize::Marks::new(&points(&marks.points));
    for (file, mark) in &marks.at {
        tracker = tracker.with_at(file, &tracker_mark(mark));
    }
    for (file, mark) in &marks.near {
        tracker = tracker.with_near(file, &tracker_mark(mark));
    }
    for file in &marks.skip {
        tracker = tracker.with_skip(file);
    }
    tracker
}

fn tracker_order(order: Option<Order>) -> ::stabilize::Order {
    match order {
        Some(Order::Name) => ::stabilize::Order::Name,
        Some(Order::Time) | None => ::stabilize::Order::Time,
    }
}

// Symlinks are not followed: a link inside a read root may point anywhere.
fn list_photos(folder: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut photos = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_file = entry.file_type().is_ok_and(|t| t.is_file());
        let extension = Path::new(&name)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if is_file && !name.starts_with('.') && PHOTO_EXTENSIONS.contains(&extension.as_str()) {
            photos.push(entry.path());
        }
    }
    photos.sort();
    if photos.len() < 2 {
        return Err(format!(
            "{}: need at least two photos (AVIF, HEIC, JPEG XL, JPEG or PNG)",
            folder.display()
        ));
    }
    Ok(photos)
}

// `IMG_4669.avif  2048x1536  anchor 1040,725  scale 2.50  38 features`: the line the
// tracker reports per photo. Columns are two spaces apart, so a name may hold single spaces.
fn photo_line(line: &str) -> Option<(&str, u64, u64, &str)> {
    let (file, rest) = line.split_once("  ")?;
    let (size, rest) = rest.split_once("  ")?;
    let (width, height) = size.split_once('x')?;
    let (_, rest) = rest
        .strip_prefix("anchor ")?
        .split_once(char::is_whitespace)?;
    let (_, note) = rest
        .trim_start()
        .strip_prefix("scale ")?
        .split_once(char::is_whitespace)?;
    Some((
        file,
        width.parse().ok()?,
        height.parse().ok()?,
        note.trim_start(),
    ))
}

// The sidecar carries neither a photo's oriented size nor the tracker's note; the log does.
fn annotate(mut sidecar: Value, log: &str) -> Value {
    let rows: HashMap<&str, (u64, u64, &str)> = log
        .lines()
        .filter_map(photo_line)
        .map(|(file, width, height, note)| (file, (width, height, note)))
        .collect();
    let photos = sidecar.get_mut("photos").and_then(Value::as_array_mut);
    for photo in photos.into_iter().flatten() {
        let row = photo
            .get("file")
            .and_then(Value::as_str)
            .and_then(|file| rows.get(file))
            .copied();
        if let Some((width, height, note)) = row {
            photo["width"] = json!(width);
            photo["height"] = json!(height);
            photo["note"] = json!(note);
        }
    }
    sidecar
}

// The Mac's file system ignores case, so a clip "beside" the folder may be inside it.
fn same_folder(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

fn document_text(
    folder: &Path,
    features: &[Feature],
    photos: &[PathBuf],
) -> Result<Vec<u8>, String> {
    let names: Vec<String> = photos
        .iter()
        .filter_map(|photo| photo.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    let document = json!({ "version": 2, "folder": folder, "features": features, "photos": names });
    let mut bytes = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
    let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, formatter);
    document
        .serialize(&mut serializer)
        .map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

// Written under a name of its own and renamed, so a reader never sees half a document.
fn save_document(path: &Path, ticket: u64, bytes: &[u8]) -> Result<(), String> {
    let partial = PathBuf::from(format!("{}.{ticket}.partial", path.display()));
    std::fs::write(&partial, bytes).map_err(|e| format!("{}: {e}", partial.display()))?;
    std::fs::rename(&partial, path).map_err(|e| format!("{}: {e}", path.display()))
}

struct Job {
    folder: PathBuf,
    document: PathBuf,
    features: Vec<Feature>,
    feature: usize,
    pad: Option<String>,
    output: Option<::stabilize::Output>,
}

// A claimed folder whose document is saved: what the tracker runs on.
struct Running {
    ticket: u64,
    cancel: CancelOnDrop,
    photos: Vec<PathBuf>,
}

impl Stabilize {
    pub fn new(run: Run) -> Self {
        Self {
            run,
            tickets: AtomicU64::new(0),
            folders: Mutex::new(HashMap::new()),
        }
    }

    // Taking a ticket cancels the folder's run in flight, before any await can reorder
    // two requests.
    fn claim(&self, folder: &Path) -> (u64, Arc<AtomicBool>) {
        let ticket = self.tickets.fetch_add(1, Ordering::Relaxed) + 1;
        let cancel = Arc::new(AtomicBool::new(false));
        let mut folders = self.folders.lock().unwrap_or_else(|e| e.into_inner());
        let entry = Folder {
            ticket,
            cancel: cancel.clone(),
        };
        if let Some(older) = folders.insert(folder.to_path_buf(), entry) {
            older.cancel.store(true, Ordering::Relaxed);
        }
        (ticket, cancel)
    }

    // ponytail: the document write happens under the lock, on the async thread — it is a
    // few hundred bytes, and holding the lock is what keeps an older document from
    // landing after a newer one.
    fn save(&self, job: &Job, ticket: u64, document: &[u8]) -> ApiResult<()> {
        let folders = self.folders.lock().unwrap_or_else(|e| e.into_inner());
        match folders.get(&job.folder) {
            Some(folder) if folder.ticket == ticket => {
                save_document(&job.document, ticket, document).map_err(crate::fail)
            }
            _ => Err(superseded()),
        }
    }

    // A run that finished just as a newer one claimed the folder is stale all the same.
    fn finish(&self, folder: &Path, ticket: u64) -> ApiResult<()> {
        let mut folders = self.folders.lock().unwrap_or_else(|e| e.into_inner());
        match folders.get(folder) {
            Some(entry) if entry.ticket == ticket => {
                folders.remove(folder);
                Ok(())
            }
            _ => Err(superseded()),
        }
    }

    // Claims the folder and saves the document. A request superseded before this point
    // answers 409 like any other.
    async fn start(&self, job: &Job) -> ApiResult<Running> {
        let (ticket, cancel) = self.claim(&job.folder);
        let cancel = CancelOnDrop(cancel);
        let folder = job.folder.clone();
        let photos = blocking(move || list_photos(&folder))
            .await
            .map_err(|(_, e)| bad(e))?;
        let document = document_text(&job.folder, &job.features, &photos).map_err(crate::fail)?;
        self.save(job, ticket, &document)?;
        Ok(Running {
            ticket,
            cancel,
            photos,
        })
    }

    // Runs the tracker on a blocking thread, handing every line it reports to `on_line`,
    // and answers with the sidecar, annotated from those lines. Dropping the future stops
    // the run.
    async fn wait(
        &self,
        what: &str,
        job: &Job,
        running: Running,
        mut on_line: impl FnMut(&str) + Send + 'static,
    ) -> ApiResult<Value> {
        let tracker_job = ::stabilize::Job {
            photos: running.photos,
            order: tracker_order(job.features[job.feature].marks.order),
            marks: tracker_marks(&job.features[job.feature].marks),
            pad: job.pad.clone(),
            output: job.output.clone(),
            cancel: Some(running.cancel.0.clone()),
            ..::stabilize::Job::default()
        };
        let run = self.run;
        let (result, log) = tokio::task::spawn_blocking(move || {
            let mut log = String::new();
            let result = run(tracker_job, &mut |line| {
                on_line(line);
                log.push_str(line);
                log.push('\n');
            });
            (result, log)
        })
        .await
        .map_err(crate::fail)?;
        self.finish(&job.folder, running.ticket)?;
        let sidecar = result.map_err(|e| crate::fail(format!("{what}: {e}")))?;
        let sidecar = serde_json::to_value(sidecar).map_err(crate::fail)?;
        Ok(annotate(sidecar, &log))
    }
}

// Every run saves the whole document and tracks the one feature it names.
fn parse_args(body: Value) -> ApiResult<(RunArgs, usize)> {
    let args: RunArgs =
        serde_json::from_value(body).map_err(|e| bad(format!("bad arguments: {e}")))?;
    let feature = check_features(&args.features, &args.feature)?;
    Ok((args, feature))
}

// The folder must be readable and the document beside it writable.
fn confine_folder(state: &AppState, folder: &str) -> ApiResult<(PathBuf, PathBuf)> {
    let folder = state.roots.check_read(folder).map_err(denied)?;
    let document = format!("{}.pin.json", folder.display());
    let document = state.roots.check_write(&document).map_err(denied)?;
    Ok((folder, document))
}

pub async fn track(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    let (args, feature) = parse_args(body)?;
    let pad = parse_pad(args.pad)?;
    let (folder, document) = confine_folder(&state, &args.folder)?;
    let job = Job {
        folder,
        document,
        features: args.features,
        feature,
        pad,
        output: None,
    };
    let running = state.stabilize.start(&job).await?;
    let sidecar = state.stabilize.wait("track", &job, running, |_| {}).await?;
    Ok(Json(sidecar))
}

fn export_output(
    state: &AppState,
    folder: &Path,
    out: PathBuf,
    args: &mut RunArgs,
) -> ApiResult<::stabilize::Output> {
    let mut output = ::stabilize::Output {
        out,
        vf: args.vf.take(),
        ..::stabilize::Output::default()
    };
    if let Some(fps) = args.fps {
        if fps <= 0.0 {
            return Err(bad("fps must be a positive number"));
        }
        output.fps = fps;
    }
    if let Some(frames) = args.frames.take() {
        let checked = state.roots.check_write(&frames).map_err(denied)?;
        if same_folder(&checked, folder) {
            return Err(bad(format!(
                "frames {frames}: the frames cannot be written inside the photo folder"
            )));
        }
        output.frames = Some(checked);
    }
    Ok(output)
}

// The clip defaults to `<folder>-<feature>.avif`, and never lands inside the folder
// itself: the next run would read it as a photo.
fn export_out(
    state: &AppState,
    folder: &Path,
    feature: &str,
    out: Option<String>,
) -> ApiResult<PathBuf> {
    let requested = out.unwrap_or_else(|| format!("{}-{feature}.avif", folder.display()));
    let checked = state.roots.check_write(&requested).map_err(denied)?;
    if checked
        .parent()
        .is_some_and(|parent| same_folder(parent, folder))
    {
        return Err(bad(format!(
            "out {requested}: the clip cannot be written inside the photo folder"
        )));
    }
    Ok(checked)
}

// ponytail: unbounded, since the tracker reports a line per photo and a few more.
struct Events(mpsc::UnboundedReceiver<Event>);

impl futures_core::Stream for Events {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.poll_recv(cx).map(|event| event.map(Ok))
    }
}

fn event(name: &str, data: Value) -> Event {
    Event::default().event(name).data(data.to_string())
}

fn ending(result: ApiResult<Value>, out: &Path) -> Event {
    match result {
        Ok(sidecar) => event("done", json!({ "out": out, "sidecar": sidecar })),
        Err((status, error)) if status == StatusCode::CONFLICT => {
            event("error", json!({ "error": error, "superseded": true }))
        }
        Err((_, error)) => event("error", json!({ "error": error })),
    }
}

// Bad requests and a run superseded before it starts answer with a status; once the
// tracker runs, the answer is server-sent events: `progress` {line} per line it reports,
// then `done` {out, sidecar} or `error` {error, superseded?}. A client that leaves stops it.
pub async fn export(State(state): State<AppState>, Json(body): Json<Value>) -> ApiResult<Response> {
    let (mut args, feature) = parse_args(body)?;
    let pad = parse_pad(args.pad.take())?;
    let (folder, document) = confine_folder(&state, &args.folder)?;
    let out = export_out(&state, &folder, &args.feature, args.out.take())?;
    let output = export_output(&state, &folder, out.clone(), &mut args)?;
    let job = Job {
        folder,
        document,
        features: args.features,
        feature,
        pad,
        output: Some(output),
    };
    let running = state.stabilize.start(&job).await?;
    let (events, receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let progress = events.clone();
        let run = state.stabilize.wait("export", &job, running, move |line| {
            let _ = progress.send(event("progress", json!({ "line": line })));
        });
        let result = tokio::select! {
            result = run => result,
            _ = events.closed() => return,
        };
        let _ = events.send(ending(result, &out));
    });
    Ok(Sse::new(Events(receiver)).into_response())
}

#[derive(Deserialize)]
struct RevealArgs {
    path: String,
}

// Shows a file in Finder on the machine serve runs on.
pub async fn reveal(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> ApiResult<StatusCode> {
    if !cfg!(target_os = "macos") {
        return Err((
            StatusCode::NOT_IMPLEMENTED,
            "reveal needs serve to run on macOS".to_string(),
        ));
    }
    let args: RevealArgs =
        serde_json::from_value(body).map_err(|e| bad(format!("bad arguments: {e}")))?;
    let path = state.roots.check_read(&args.path).map_err(denied)?;
    let status = Command::new("open")
        .arg("-R")
        .arg(&path)
        .status()
        .await
        .map_err(crate::fail)?;
    if !status.success() {
        return Err(crate::fail(format!("open -R: {status}")));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_line_gives_size_and_note() {
        let line =
            "Photo 1.avif  2048x1536  anchor 1040,725  scale 2.50  LOST (2 features), estimated";
        assert_eq!(
            photo_line(line),
            Some(("Photo 1.avif", 2048, 1536, "LOST (2 features), estimated"))
        );
        assert_eq!(
            photo_line("order: time: every photo has a capture time"),
            None
        );
    }

    #[test]
    fn marks_become_the_trackers() {
        let marks: Marks = serde_json::from_value(json!({
            "points": [[675, 345], [985.5, 345]],
            "at": { "b.avif": { "points": [[1, 2]], "zoom": 2.5 } },
            "near": { "c.avif": { "points": [[3, 4]] } },
            "skip": ["d.avif"],
            "order": "name"
        }))
        .unwrap();
        assert_eq!(
            tracker_marks(&marks),
            ::stabilize::Marks {
                points: Some("675,345/985.5,345".to_string()),
                at: vec!["b.avif=1,2@2.5".to_string()],
                skip: vec!["d.avif".to_string()],
                near: vec!["c.avif=3,4".to_string()],
            }
        );
        assert_eq!(tracker_order(marks.order), ::stabilize::Order::Name);
        assert_eq!(tracker_order(None), ::stabilize::Order::Time);
    }
}
