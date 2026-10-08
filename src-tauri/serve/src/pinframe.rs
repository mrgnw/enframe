use crate::{ApiResult, AppState, blocking, denied, text};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::process::Command;
use tokio::sync::oneshot;

const PHOTO_EXTENSIONS: [&str; 7] = ["avif", "heic", "heif", "jxl", "jpg", "jpeg", "png"];
const SUPERSEDED: &str = "superseded by a newer run for this folder";

// One run per folder. Tickets are global, so a ticket is never reused once a folder's
// entry is dropped, and only the newest ticket for a folder may write or spawn.
pub struct Pinframe {
    binary: Option<PathBuf>,
    tickets: AtomicU64,
    folders: Mutex<HashMap<PathBuf, Folder>>,
}

// Dropping `cancel` is what stops the run holding the other end.
struct Folder {
    ticket: u64,
    cancel: Option<oneshot::Sender<()>>,
}

type Point = [f64; 2];

// The document the marking UI edits, saved beside the folder as `<folder>.pin.json`. Its
// fields are the CLI's flags: `points` is `--points`, `at` and `near` map a photo to
// `--at` and `--near`, `skip` is `--skip`, `order` is `--order`.
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
    marks: Value,
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

fn parse_marks(raw: Value) -> ApiResult<Marks> {
    let marks: Marks = serde_json::from_value(raw).map_err(|e| bad(format!("marks: {e}")))?;
    if marks.points.is_empty() {
        return Err(bad("points: needs at least one point"));
    }
    for (key, map) in [("at", &marks.at), ("near", &marks.near)] {
        for (file, mark) in map {
            if mark.points.is_empty() {
                return Err(bad(format!("{key} {file}: needs at least one point")));
            }
            if mark.zoom.is_some_and(|zoom| zoom <= 0.0) {
                return Err(bad(format!("{key} {file}: zoom must be a positive number")));
            }
        }
    }
    Ok(marks)
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

fn format_points(points: &[Point]) -> String {
    points
        .iter()
        .map(|[x, y]| format!("{x},{y}"))
        .collect::<Vec<_>>()
        .join("/")
}

fn cli_args(marks: &Marks) -> Vec<String> {
    let mut args = vec!["--points".to_string(), format_points(&marks.points)];
    for (flag, map) in [("--at", &marks.at), ("--near", &marks.near)] {
        for (file, mark) in map {
            let zoom = mark.zoom.map(|zoom| format!("@{zoom}")).unwrap_or_default();
            args.push(flag.to_string());
            args.push(format!("{file}={}{zoom}", format_points(&mark.points)));
        }
    }
    for file in &marks.skip {
        args.push("--skip".to_string());
        args.push(file.clone());
    }
    if let Some(order) = marks.order {
        args.push("--order".to_string());
        args.push(match order {
            Order::Time => "time".to_string(),
            Order::Name => "name".to_string(),
        });
    }
    args
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

// `IMG_4669.avif  2048x1536  anchor 1040,725  scale 2.50  38 features`: the line the CLI
// prints per photo. Columns are two spaces apart, so a name may hold single spaces.
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
fn annotate(mut sidecar: Value, stdout: &str) -> Value {
    let rows: HashMap<&str, (u64, u64, &str)> = stdout
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

fn document_text(folder: &Path, marks: &Marks, photos: &[PathBuf]) -> Result<Vec<u8>, String> {
    let names: Vec<String> = photos
        .iter()
        .filter_map(|photo| photo.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    let document = json!({ "version": 1, "folder": folder, "marks": marks, "photos": names });
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
    marks: Marks,
    out: PathBuf,
    extra: Vec<String>,
}

impl Pinframe {
    pub fn new(binary: Option<PathBuf>) -> Self {
        Self {
            binary,
            tickets: AtomicU64::new(0),
            folders: Mutex::new(HashMap::new()),
        }
    }

    pub fn binary(&self) -> Option<&Path> {
        self.binary.as_deref()
    }

    fn configured(&self) -> ApiResult<&Path> {
        self.binary().ok_or((
            StatusCode::SERVICE_UNAVAILABLE,
            "pinframe is not configured: start serve with --pinframe or ENFRAME_PINFRAME"
                .to_string(),
        ))
    }

    // Taking a ticket cancels the folder's run in flight, before any await can reorder
    // two requests.
    fn claim(&self, folder: &Path) -> u64 {
        let ticket = self.tickets.fetch_add(1, Ordering::Relaxed) + 1;
        let mut folders = self.folders.lock().unwrap_or_else(|e| e.into_inner());
        folders.insert(
            folder.to_path_buf(),
            Folder {
                ticket,
                cancel: None,
            },
        );
        ticket
    }

    // ponytail: the document write happens under the lock, on the async thread — it is a
    // few hundred bytes, and holding the lock is what keeps an older document from
    // landing after a newer one.
    fn start(
        &self,
        job: &Job,
        ticket: u64,
        document: &[u8],
        photos: &[PathBuf],
    ) -> ApiResult<(tokio::process::Child, oneshot::Receiver<()>)> {
        let binary = self.configured()?;
        let mut folders = self.folders.lock().unwrap_or_else(|e| e.into_inner());
        let folder = match folders.get_mut(&job.folder) {
            Some(folder) if folder.ticket == ticket => folder,
            _ => return Err(superseded()),
        };
        save_document(&job.document, ticket, document).map_err(crate::fail)?;
        let child = Command::new(binary)
            .args(photos)
            .args(cli_args(&job.marks))
            .arg("--out")
            .arg(&job.out)
            .args(&job.extra)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    format!("pinframe {}: {e}", binary.display()),
                )
            })?;
        let (cancel, cancelled) = oneshot::channel();
        folder.cancel = Some(cancel);
        Ok((child, cancelled))
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

    async fn run(&self, what: &str, job: &Job) -> ApiResult<String> {
        self.configured()?;
        let ticket = self.claim(&job.folder);
        let folder = job.folder.clone();
        let photos = blocking(move || list_photos(&folder))
            .await
            .map_err(|(_, e)| bad(e))?;
        let document = document_text(&job.folder, &job.marks, &photos).map_err(crate::fail)?;
        let (child, cancelled) = self.start(job, ticket, &document, &photos)?;
        let output = tokio::select! {
            output = child.wait_with_output() => output.map_err(crate::fail)?,
            _ = cancelled => return Err(superseded()),
        };
        self.finish(&job.folder, ticket)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| output.status.to_string());
            return Err(crate::fail(format!("{what}: {detail}")));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

fn parse_args(body: Value) -> ApiResult<RunArgs> {
    serde_json::from_value(body).map_err(|e| bad(format!("bad arguments: {e}")))
}

// The folder must be readable and the document beside it writable.
fn confine_folder(state: &AppState, folder: &str) -> ApiResult<(PathBuf, PathBuf)> {
    let folder = state.roots.check_read(folder).map_err(denied)?;
    let document = format!("{}.pin.json", folder.display());
    let document = state.roots.check_write(&document).map_err(denied)?;
    Ok((folder, document))
}

fn read_sidecar(path: &Path, stdout: &str) -> Result<Value, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let sidecar = serde_json::from_slice(&bytes).map_err(|e| format!("sidecar: {e}"))?;
    Ok(annotate(sidecar, stdout))
}

// JSON-only output lands in a file of the run's own, outside the photo folder, and is
// removed once read.
fn track_output() -> ApiResult<PathBuf> {
    static OUTPUTS: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join("enframe-pinframe");
    std::fs::create_dir_all(&dir).map_err(crate::fail)?;
    let id = OUTPUTS.fetch_add(1, Ordering::Relaxed);
    Ok(dir.join(format!("{}-{id}.json", std::process::id())))
}

pub async fn track(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    let args = parse_args(body)?;
    let marks = parse_marks(args.marks)?;
    let pad = parse_pad(args.pad)?;
    let (folder, document) = confine_folder(&state, &args.folder)?;
    let out = track_output()?;
    let extra = match pad {
        Some(pad) => vec!["--pad".to_string(), pad],
        None => Vec::new(),
    };
    let job = Job {
        folder,
        document,
        marks,
        out: out.clone(),
        extra,
    };
    let stdout = state.pinframe.run("track", &job).await;
    let sidecar = stdout.and_then(|stdout| read_sidecar(&out, &stdout).map_err(crate::fail));
    let _ = std::fs::remove_file(&out);
    sidecar.map(Json)
}

fn export_extra(state: &AppState, folder: &Path, args: RunArgs) -> ApiResult<Vec<String>> {
    let mut extra = Vec::new();
    if let Some(pad) = parse_pad(args.pad)? {
        extra.extend(["--pad".to_string(), pad]);
    }
    if let Some(fps) = args.fps {
        if fps <= 0.0 {
            return Err(bad("fps must be a positive number"));
        }
        extra.extend(["--fps".to_string(), fps.to_string()]);
    }
    if let Some(frames) = args.frames {
        let checked = state.roots.check_write(&frames).map_err(denied)?;
        if same_folder(&checked, folder) {
            return Err(bad(format!(
                "frames {frames}: the frames cannot be written inside the photo folder"
            )));
        }
        extra.extend(["--frames".to_string(), text(checked)]);
    }
    if let Some(vf) = args.vf {
        extra.extend(["--vf".to_string(), vf]);
    }
    Ok(extra)
}

// The clip defaults to `<folder>.avif`, and never lands inside the folder itself: the
// next run would read it as a photo.
fn export_out(state: &AppState, folder: &Path, out: Option<String>) -> ApiResult<PathBuf> {
    let requested = out.unwrap_or_else(|| format!("{}.avif", folder.display()));
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

pub async fn export(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    let mut args = parse_args(body)?;
    let marks = parse_marks(std::mem::take(&mut args.marks))?;
    let (folder, document) = confine_folder(&state, &args.folder)?;
    let out = export_out(&state, &folder, args.out.take())?;
    let extra = export_extra(&state, &folder, args)?;
    let job = Job {
        folder,
        document,
        marks,
        out: out.clone(),
        extra,
    };
    let stdout = state.pinframe.run("export", &job).await?;
    let sidecar = read_sidecar(&out.with_extension("json"), &stdout).map_err(crate::fail)?;
    Ok(Json(json!({ "out": out, "sidecar": sidecar })))
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
    fn marks_become_the_cli_flags() {
        let marks = parse_marks(json!({
            "points": [[675, 345], [985.5, 345]],
            "at": { "b.avif": { "points": [[1, 2]], "zoom": 2.5 } },
            "near": { "c.avif": { "points": [[3, 4]] } },
            "skip": ["d.avif"],
            "order": "name"
        }))
        .unwrap();
        assert_eq!(
            cli_args(&marks),
            [
                "--points",
                "675,345/985.5,345",
                "--at",
                "b.avif=1,2@2.5",
                "--near",
                "c.avif=3,4",
                "--skip",
                "d.avif",
                "--order",
                "name"
            ]
        );
    }
}
