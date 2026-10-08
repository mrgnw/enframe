mod db;
mod pinframe;
mod roots;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use roots::Roots;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};
use tower::ServiceExt as _;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

const DEFAULT_PORT: u16 = 5911;
const MAX_BODY: usize = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 5_000;

// ponytail: one connection behind a mutex — a single browser client serialises fine.
// Swap for r2d2_sqlite if concurrent tabs ever contend.
#[derive(Clone)]
struct AppState {
    conn: Arc<Mutex<Connection>>,
    inbox: PathBuf,
    browse: PathBuf,
    roots: Arc<Roots>,
    pinframe: Arc<pinframe::Pinframe>,
}

type ApiResult<T> = Result<T, (StatusCode, String)>;

fn fail(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

fn denied(message: String) -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, message)
}

fn text(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

async fn blocking<T, F>(work: F) -> ApiResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(fail)?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}

#[derive(Deserialize)]
struct SqlBody {
    sql: String,
    #[serde(default)]
    values: Vec<Value>,
}

async fn sql_select(
    State(state): State<AppState>,
    Json(body): Json<SqlBody>,
) -> ApiResult<Json<Value>> {
    blocking(move || {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        db::select(&conn, &body.sql, &body.values)
    })
    .await
    .map(Json)
}

async fn sql_execute(
    State(state): State<AppState>,
    Json(body): Json<SqlBody>,
) -> ApiResult<Json<Value>> {
    blocking(move || {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        db::execute(&conn, &body.sql, &body.values)
    })
    .await
    .map(Json)
}

// Tauri renames camelCase invoke args to snake_case; these mirror that mapping.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CropArgs {
    source_path: String,
    output_path: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    quality: u8,
    format: String,
}

#[derive(Deserialize)]
struct SaveImageArgs {
    path: String,
    data: String,
}

#[derive(Deserialize)]
struct DirectoryArgs {
    directory: String,
}

// One path, whether it arrives as a query string or in a JSON body.
#[derive(Deserialize)]
struct PathArg {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumAssetsArgs {
    album_id: String,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize)]
struct SearchArgs {
    query: String,
    limit: Option<i64>,
}

#[derive(Deserialize)]
struct LimitArgs {
    limit: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssetIdsArgs {
    asset_ids: Vec<String>,
}

fn args<T: serde::de::DeserializeOwned>(body: Value) -> Result<T, String> {
    serde_json::from_value(body).map_err(|e| format!("bad arguments: {e}"))
}

fn encode<T: Serialize>(result: Result<T, String>) -> Result<Value, String> {
    serde_json::to_value(result?).map_err(|e| e.to_string())
}

fn dispatch(command: &str, body: Value) -> Result<Value, String> {
    use app_lib::commands::{
        crop_and_export, get_screenshot_mode, raw_thumbnail_blocking, save_image,
    };
    use app_lib::{inbox, lightroom};

    match command {
        "get_screenshot_mode" => encode(Ok(get_screenshot_mode())),
        "raw_thumbnail" => {
            let a: PathArg = args(body)?;
            encode(raw_thumbnail_blocking(&a.path))
        }
        "save_image" => {
            let a: SaveImageArgs = args(body)?;
            encode(save_image(a.path, a.data))
        }
        "crop_and_export" => {
            let a: CropArgs = args(body)?;
            encode(crop_and_export(
                a.source_path,
                a.output_path,
                a.x,
                a.y,
                a.width,
                a.height,
                a.quality,
                a.format,
            ))
        }
        "scan_inbox" => {
            let a: DirectoryArgs = args(body)?;
            encode(inbox::scan_inbox(a.directory))
        }
        "get_lightroom_library_path" => encode(lightroom::get_lightroom_library_path()),
        "list_lr_albums" => encode(lightroom::list_lr_albums()),
        "list_lr_album_assets" => {
            let a: AlbumAssetsArgs = args(body)?;
            encode(lightroom::list_lr_album_assets(
                a.album_id, a.limit, a.offset,
            ))
        }
        "search_lr_assets" => {
            let a: SearchArgs = args(body)?;
            encode(lightroom::search_lr_assets(a.query, a.limit))
        }
        "list_lr_recent" => {
            let a: LimitArgs = args(body)?;
            encode(lightroom::list_lr_recent(a.limit))
        }
        "get_lr_asset_paths" => {
            let a: AssetIdsArgs = args(body)?;
            encode(lightroom::get_lr_asset_paths(a.asset_ids))
        }
        other => Err(format!("unknown command: {other}")),
    }
}

// Roots are the only boundary. There was a second one — routes that pick a path on the
// server answered loopback only — and it could not survive an app connecting to an
// instance on another machine: to serve, that app is an off-loopback caller, so it would
// have browsed the library and then been refused every export. Without identity there is
// no telling that app from a stranger, and this project has ruled out authentication.
//
// What is left is what always did the real work: canonicalised read and write roots that
// no `..` or symlink escapes, SQLITE_LIMIT_ATTACHED at zero, an authorizer that denies
// ATTACH and PRAGMA, and a deadline on image reads. What is given up is the guarantee
// that a network caller cannot make the server write anywhere at all, even inside a write
// root. Who can reach the port is the operator's business, and stating one boundary
// plainly beats keeping two that disagree.
enum Access {
    Read,
    Write,
}

// Replaces a caller-supplied path with the checked, canonical one before the command
// ever sees it, so every path-carrying command is confined in one place.
fn confine(roots: &Roots, body: &mut Value, key: &str, access: Access) -> ApiResult<()> {
    let requested = body
        .get(key)
        .and_then(Value::as_str)
        .ok_or((StatusCode::BAD_REQUEST, format!("missing {key}")))?;
    let checked = match access {
        Access::Read => roots.check_read(requested),
        Access::Write => roots.check_write(requested),
    }
    .map_err(denied)?;
    body[key] = Value::String(text(checked));
    Ok(())
}

// The single list of commands that write to a path the caller chose, so every such write
// is checked against the write roots in one place.
fn write_key(command: &str) -> Option<&'static str> {
    match command {
        "save_image" => Some("path"),
        "crop_and_export" => Some("outputPath"),
        _ => None,
    }
}

fn confine_command(roots: &Roots, command: &str, body: &mut Value) -> ApiResult<()> {
    match command {
        "crop_and_export" => confine(roots, body, "sourcePath", Access::Read)?,
        "scan_inbox" => confine(roots, body, "directory", Access::Read)?,
        "raw_thumbnail" => confine(roots, body, "path", Access::Read)?,
        // The Lightroom commands take ids and queries, and read the library through
        // a root that has to be allowlisted for them to work at all.
        _ => {}
    }
    match write_key(command) {
        Some(key) => confine(roots, body, key, Access::Write),
        None => Ok(()),
    }
}

async fn api(
    State(state): State<AppState>,
    Path(command): Path<String>,
    Json(mut body): Json<Value>,
) -> ApiResult<Json<Value>> {
    confine_command(&state.roots, &command, &mut body)?;
    blocking(move || dispatch(&command, body)).await.map(Json)
}

// ponytail: the root check runs on the async thread — it is one realpath(2).
// Move it into spawn_blocking if roots ever point at a network mount.
async fn read_text(
    State(state): State<AppState>,
    Query(query): Query<PathArg>,
) -> ApiResult<String> {
    let path = state.roots.check_read(&query.path).map_err(denied)?;
    blocking(move || std::fs::read_to_string(&path).map_err(|e| e.to_string())).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Stat {
    is_directory: bool,
}

async fn stat(
    State(state): State<AppState>,
    Query(query): Query<PathArg>,
) -> ApiResult<Json<Stat>> {
    let path = state.roots.check_read(&query.path).map_err(denied)?;
    blocking(move || {
        let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        Ok(Stat {
            is_directory: meta.is_dir(),
        })
    })
    .await
    .map(Json)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    name: String,
    path: String,
    is_directory: bool,
    is_image: bool,
}

#[derive(Serialize)]
struct Listing {
    path: String,
    parent: Option<String>,
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct LsQuery {
    dir: Option<String>,
}

fn is_image(path: &FsPath) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| app_lib::inbox::IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
}

fn list_dir(roots: &Roots, dir: PathBuf) -> Result<Listing, String> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let is_directory = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        entries.push(Entry {
            name,
            is_image: !is_directory && is_image(&path),
            path: path.to_string_lossy().into_owned(),
            is_directory,
        });
        if entries.len() >= MAX_ENTRIES {
            break;
        }
    }
    entries.sort_by(|a, b| {
        b.is_directory
            .cmp(&a.is_directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    // Only offer an "up" link the picker is actually allowed to open.
    let parent = dir
        .parent()
        .filter(|parent| {
            roots
                .read_roots()
                .iter()
                .any(|root| parent.starts_with(root))
        })
        .map(|parent| parent.to_string_lossy().into_owned());

    Ok(Listing {
        parent,
        path: text(dir),
        entries,
    })
}

async fn ls(
    State(state): State<AppState>,
    Query(query): Query<LsQuery>,
) -> ApiResult<Json<Listing>> {
    let dir = match query.dir.filter(|dir| !dir.is_empty()) {
        Some(dir) => state.roots.check_read(&dir).map_err(denied)?,
        None => state.browse.clone(),
    };
    blocking(move || list_dir(&state.roots, dir))
        .await
        .map(Json)
}

#[derive(Serialize)]
struct Uploaded {
    path: String,
}

// Browser drops carry bytes, not paths, so land them in the inbox and import by path.
async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Json<Uploaded>> {
    let filename = headers
        .get("x-filename")
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            percent_encoding::percent_decode_str(v)
                .decode_utf8_lossy()
                .into_owned()
        })
        .map(|v| v.rsplit('/').next().unwrap_or(&v).to_string())
        .filter(|v| !v.is_empty() && v != "." && v != "..")
        .ok_or((StatusCode::BAD_REQUEST, "missing x-filename header".into()))?;

    std::fs::create_dir_all(&state.inbox).map_err(fail)?;
    let path = state
        .roots
        .check_write(&text(state.inbox.join(&filename)))
        .map_err(denied)?;

    blocking(move || {
        std::fs::write(&path, &body).map_err(|e| e.to_string())?;
        Ok(Uploaded { path: text(path) })
    })
    .await
    .map(Json)
}

// A read that never returns leaves the browser tab spinning forever, which is how a
// launchd-managed server without Full Disk Access fails on an iCloud path. Bound it:
// slow enough for a genuine iCloud download, finite either way.
const IMAGE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(60);

async fn deadline<T>(work: impl Future<Output = T>) -> ApiResult<T> {
    tokio::time::timeout(IMAGE_DEADLINE, work)
        .await
        .map_err(|_| {
            (
                StatusCode::GATEWAY_TIMEOUT,
                "timed out reading the image; the server may lack permission to read it"
                    .to_string(),
            )
        })
}

async fn img(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: Request,
) -> ApiResult<Response> {
    let absolute = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    let file = state.roots.check_read(&absolute).map_err(denied)?;

    let source = file.clone();
    let raw = deadline(blocking(move || {
        app_lib::commands::raw_as_jpeg(&text(source))
    }))
    .await??;

    // ponytail: decoded on every request; the immutable Cache-Control keeps the browser
    // from asking twice. Add a keyed cache if cold loads of a large library ever hurt.
    if let Some(jpeg) = raw {
        return Ok((
            [
                (header::CONTENT_TYPE, "image/jpeg"),
                (
                    header::CACHE_CONTROL,
                    "private, max-age=31536000, immutable",
                ),
            ],
            jpeg,
        )
            .into_response());
    }

    deadline(ServeFile::new(file).oneshot(request))
        .await?
        .map(IntoResponse::into_response)
        .map_err(fail)
}

struct Options {
    host: String,
    port: u16,
    database: PathBuf,
    web: PathBuf,
    inbox: PathBuf,
    read_roots: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,
    pinframe: Option<PathBuf>,
}

fn flag(args: &[String], name: &str, env: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
        .or_else(|| std::env::var(env).ok())
}

// Repeatable flag plus a colon-separated env var, matching how PATH-shaped config reads.
fn flags(args: &[String], name: &str, env: &str) -> Vec<PathBuf> {
    let mut values: Vec<PathBuf> = args
        .windows(2)
        .filter(|pair| pair[0] == name)
        .map(|pair| PathBuf::from(&pair[1]))
        .collect();
    if let Ok(list) = std::env::var(env) {
        values.extend(list.split(':').filter(|v| !v.is_empty()).map(PathBuf::from));
    }
    values
}

// ponytail: a default root that is absent is dropped, not fatal — a machine without
// ~/Pictures should still boot. An explicitly configured root must exist.
fn existing(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.into_iter().filter(|path| path.exists()).collect()
}

fn options() -> Result<Options, String> {
    let args: Vec<String> = std::env::args().collect();
    let database = match flag(&args, "--db", "ENFRAME_DB") {
        Some(path) => PathBuf::from(path),
        None => db::default_path()?,
    };
    let inbox = match flag(&args, "--inbox", "ENFRAME_INBOX") {
        Some(path) => PathBuf::from(path),
        None => database
            .parent()
            .ok_or("database path has no parent")?
            .join("inbox"),
    };
    Ok(Options {
        host: flag(&args, "--host", "ENFRAME_HOST").unwrap_or_else(|| "127.0.0.1".into()),
        port: match flag(&args, "--port", "ENFRAME_PORT") {
            Some(port) => port.parse().map_err(|e| format!("bad port: {e}"))?,
            None => DEFAULT_PORT,
        },
        web: PathBuf::from(flag(&args, "--web", "ENFRAME_WEB").unwrap_or_else(|| "build".into())),
        read_roots: flags(&args, "--read-root", "ENFRAME_READ_ROOTS"),
        write_roots: flags(&args, "--write-root", "ENFRAME_WRITE_ROOTS"),
        pinframe: flag(&args, "--pinframe", "ENFRAME_PINFRAME").map(PathBuf::from),
        database,
        inbox,
    })
}

// Photos and the Lightroom library both live under ~/Pictures, and exports default to
// the directory of the source photo, so that one tree is both readable and writable.
fn pictures() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|e| format!("HOME: {e}"))?;
    Ok(PathBuf::from(home).join("Pictures"))
}

// Photos live wherever the user keeps them: iCloud Drive, an external volume, ~/Pictures.
// Deriving roots from the library itself keeps serve able to read what the app imported,
// without widening the boundary to everything the desktop app could touch.
fn library_roots(connection: &Connection) -> Vec<PathBuf> {
    let Ok(mut statement) = connection.prepare("SELECT DISTINCT file_path FROM photos") else {
        return Vec::new();
    };
    let Ok(rows) = statement.query_map([], |row| row.get::<_, String>(0)) else {
        return Vec::new();
    };
    let mut roots: Vec<PathBuf> = Vec::new();
    for path in rows.flatten() {
        let Some(parent) = FsPath::new(&path).parent() else {
            continue;
        };
        if !is_specific_enough(parent) || roots.iter().any(|root| root == parent) {
            continue;
        }
        roots.push(parent.to_path_buf());
    }
    roots
}

// A photo sitting directly in / or ~ would otherwise turn the whole tree into a root.
fn is_specific_enough(path: &FsPath) -> bool {
    if path.components().count() < 4 {
        return false;
    }
    match std::env::var("HOME") {
        Ok(home) => path != FsPath::new(&home),
        Err(_) => true,
    }
}

fn build_roots(
    options: &Options,
    pictures: &FsPath,
    library: Vec<PathBuf>,
) -> Result<Roots, String> {
    let database = options
        .database
        .parent()
        .ok_or("database path has no parent")?
        .to_path_buf();
    let read = existing(
        [
            vec![
                pictures.to_path_buf(),
                database,
                options.inbox.clone(),
                options.web.clone(),
            ],
            library.clone(),
        ]
        .concat(),
    );
    let write = existing([vec![options.inbox.clone(), pictures.to_path_buf()], library].concat());
    Roots::new(
        [read, options.read_roots.clone()].concat(),
        [write, options.write_roots.clone()].concat(),
    )
}

// The native shell loads from its own origin, so every call it makes to the instance it
// connected to is cross-origin — even on loopback. Naming the app's own origins lets the
// shell through while a stranger's web page still cannot read the library over loopback.
// The last one is the dev server this project's frontend runs on.
const APP_ORIGINS: [&str; 3] = [
    "tauri://localhost",
    "http://tauri.localhost",
    "http://localhost:5910",
];

fn cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(
            APP_ORIGINS
                .iter()
                .map(|origin| HeaderValue::from_static(origin))
                .collect::<Vec<_>>(),
        )
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, HeaderName::from_static("x-filename")])
}

fn router(state: AppState, web: &FsPath) -> Router {
    let spa = ServeDir::new(web).fallback(ServeFile::new(web.join("index.html")));
    Router::new()
        .route("/sql/select", post(sql_select))
        .route("/sql/execute", post(sql_execute))
        .route("/api/read_text", get(read_text))
        .route("/api/stat", get(stat))
        .route("/api/ls", get(ls))
        .route("/api/upload", post(upload))
        .route("/api/pinframe/track", post(pinframe::track))
        .route("/api/pinframe/export", post(pinframe::export))
        .route("/api/pinframe/reveal", post(pinframe::reveal))
        .route("/api/{command}", post(api))
        .route("/img/{*path}", get(img))
        .fallback_service(spa)
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .layer(cors())
        .with_state(state)
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let options = options()?;
    // Before anything opens the database: one writer, and the loser is told where to connect.
    let _ownership = db::take_ownership(&options.database, options.port)?;
    let connection = db::open(&options.database)?;
    std::fs::create_dir_all(&options.inbox)
        .map_err(|e| format!("create {}: {e}", options.inbox.display()))?;

    let pictures = pictures()?;
    let roots = build_roots(&options, &pictures, library_roots(&connection))?;
    let browse = match pictures.canonicalize() {
        Ok(pictures) if roots.check_read(&text(pictures.clone())).is_ok() => pictures,
        _ => roots.read_roots()[0].clone(),
    };

    let state = AppState {
        conn: Arc::new(Mutex::new(connection)),
        inbox: options.inbox.clone(),
        browse,
        roots: Arc::new(roots),
        pinframe: Arc::new(pinframe::Pinframe::new(options.pinframe.clone())),
    };

    let app = router(state.clone(), &options.web);

    let address = format!("{}:{}", options.host, options.port);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|e| format!("bind {address}: {e}"))?;
    println!("enframe serving http://{address}");
    println!("  database {}", options.database.display());
    println!("  web      {}", options.web.display());
    println!("  inbox    {}", options.inbox.display());
    for root in state.roots.read_roots() {
        println!("  read     {}", root.display());
    }
    for root in state.roots.write_roots() {
        println!("  write    {}", root.display());
    }
    match state.pinframe.binary() {
        Some(binary) => println!("  pinframe {}", binary.display()),
        None => println!("  pinframe unset"),
    }

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::ConnectInfo;

    // write_key is confinement's list alone now, and naming a command here is what puts
    // its destination through check_write.
    #[test]
    fn the_caller_chosen_writes_are_the_ones_checked_against_the_write_roots() {
        assert_eq!(write_key("save_image"), Some("path"));
        assert_eq!(write_key("crop_and_export"), Some("outputPath"));
        assert_eq!(write_key("scan_inbox"), None);
        assert_eq!(write_key("raw_thumbnail"), None);
        assert_eq!(write_key("list_lr_albums"), None);
    }

    struct Sandbox {
        base: PathBuf,
    }

    impl Sandbox {
        fn new(name: &str) -> Self {
            let base = std::env::temp_dir()
                .canonicalize()
                .unwrap()
                .join(format!("enframe_serve_{}_{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            std::fs::create_dir_all(&base).unwrap();
            Self { base }
        }

        fn app(&self) -> Router {
            self.app_with(None)
        }

        fn app_with(&self, pinframe: Option<PathBuf>) -> Router {
            let inbox = self.base.join("inbox");
            std::fs::create_dir_all(&inbox).unwrap();
            let connection = db::open(&self.base.join("enframe.db")).unwrap();
            let roots = Roots::new(vec![self.base.clone()], vec![inbox.clone()]).unwrap();
            let state = AppState {
                conn: Arc::new(Mutex::new(connection)),
                browse: self.base.clone(),
                roots: Arc::new(roots),
                pinframe: Arc::new(pinframe::Pinframe::new(pinframe)),
                inbox,
            };
            router(state, &self.base)
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    // The peer address still reaches the router — main serves with connect info — and is
    // now deliberately consulted by nothing. Every test below hands the router an
    // off-loopback peer, so a reinstated address test would fail these outright.
    fn from(peer: &str, mut request: Request) -> Request {
        request
            .extensions_mut()
            .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        request
    }

    async fn status(app: &Router, request: Request) -> StatusCode {
        app.clone().oneshot(request).await.unwrap().status()
    }

    fn get_ls(dir: &str) -> Request {
        let query = if dir.is_empty() {
            String::new()
        } else {
            format!("?dir={dir}")
        };
        Request::builder()
            .uri(format!("/api/ls{query}"))
            .body(Body::empty())
            .unwrap()
    }

    fn post_json(uri: &str, body: &str) -> Request {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    // The file browser used to answer loopback only, which meant an app connected to
    // another machine could never pick a file on it. It answers anyone now, and the read
    // root is what stops the walk.
    #[tokio::test]
    async fn the_file_browser_answers_a_remote_caller_inside_the_read_roots() {
        let sandbox = Sandbox::new("browser");
        let app = sandbox.app();
        let inside = text(sandbox.base.clone());

        assert_eq!(
            status(&app, from("100.64.0.7:52001", get_ls(""))).await,
            StatusCode::OK
        );
        assert_eq!(
            status(&app, from("192.168.1.20:52001", get_ls(&inside))).await,
            StatusCode::OK
        );
        // No connection info at all — an unknown peer is no longer a refused one.
        assert_eq!(status(&app, get_ls("")).await, StatusCode::OK);

        let outside = text(std::env::temp_dir().canonicalize().unwrap());
        assert_eq!(
            status(&app, from("100.64.0.7:52001", get_ls(&outside))).await,
            StatusCode::FORBIDDEN
        );
    }

    // A remote write lands where the write roots allow and nowhere else. This is the whole
    // of the trade: exporting from a connected app works, and the roots carry the boundary
    // that the peer address used to share.
    #[tokio::test]
    async fn a_remote_write_lands_in_a_write_root_and_nowhere_else() {
        let sandbox = Sandbox::new("write");
        let app = sandbox.app();

        let target = sandbox.base.join("inbox/saved.png");
        let allowed = format!("{{\"path\":{:?},\"data\":\"\"}}", text(target.clone()));
        assert_eq!(
            status(
                &app,
                from("100.64.0.7:52001", post_json("/api/save_image", &allowed))
            )
            .await,
            StatusCode::OK
        );
        assert!(target.exists());

        // Readable but not writable: the same caller cannot climb out of the write root.
        let escape = sandbox.base.join("outside_the_inbox.png");
        let refused = format!("{{\"path\":{:?},\"data\":\"\"}}", text(escape.clone()));
        assert_eq!(
            status(
                &app,
                from("100.64.0.7:52001", post_json("/api/save_image", &refused))
            )
            .await,
            StatusCode::FORBIDDEN
        );
        assert!(!escape.exists(), "a refused write must not touch the disk");
    }

    // Listing a bookmarked folder is a read, so it answers remotely — but only inside a
    // read root. Confinement, not the peer address, is what bounds where a caller can look.
    #[tokio::test]
    async fn a_remote_folder_scan_is_confined_to_a_read_root() {
        let sandbox = Sandbox::new("scan");
        let app = sandbox.app();

        let inside = format!("{{\"directory\":{:?}}}", text(sandbox.base.join("inbox")));
        assert_eq!(
            status(
                &app,
                from("100.64.0.7:52001", post_json("/api/scan_inbox", &inside))
            )
            .await,
            StatusCode::OK
        );

        let outside = std::env::temp_dir().canonicalize().unwrap();
        let escape = format!("{{\"directory\":{:?}}}", text(outside));
        assert_eq!(
            status(
                &app,
                from("100.64.0.7:52001", post_json("/api/scan_inbox", &escape))
            )
            .await,
            StatusCode::FORBIDDEN
        );
    }

    // RAW tiles were placeholders for every client but the desktop app, because only the
    // app had the command. Serve has it now, and confinement bounds which files it decodes.
    #[tokio::test]
    async fn a_thumbnail_is_confined_to_a_read_root() {
        let sandbox = Sandbox::new("thumb");
        let app = sandbox.app();

        // A file that exists but sits outside every root, so the refusal is confinement
        // rather than a path that simply could not be resolved.
        let outside = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("enframe_outside_{}.arw", std::process::id()));
        std::fs::write(&outside, b"not really a raw").unwrap();
        let escape = format!("{{\"path\":{:?}}}", text(outside.clone()));
        assert_eq!(
            status(&app, post_json("/api/raw_thumbnail", &escape)).await,
            StatusCode::FORBIDDEN
        );
        let _ = std::fs::remove_file(&outside);

        // Inside a read root the same command answers; a non-RAW extension thumbnails
        // to null rather than failing.
        let inside = sandbox.base.join("tile.jpeg");
        std::fs::write(&inside, b"x").unwrap();
        let allowed = format!("{{\"path\":{:?}}}", text(inside));
        assert_eq!(
            status(&app, post_json("/api/raw_thumbnail", &allowed)).await,
            StatusCode::OK
        );
    }

    // The native shell is cross-origin to the instance it connects to, so its preflight
    // has to clear; a stranger's page must get no such clearance.
    #[tokio::test]
    async fn only_the_app_origins_clear_a_preflight() {
        let sandbox = Sandbox::new("cors");
        let app = sandbox.app();

        let preflight = |origin: &str| {
            Request::builder()
                .method("OPTIONS")
                .uri("/sql/select")
                .header(header::ORIGIN, origin)
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
                .body(Body::empty())
                .unwrap()
        };

        for origin in APP_ORIGINS {
            let response = app.clone().oneshot(preflight(origin)).await.unwrap();
            assert_eq!(
                response
                    .headers()
                    .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                    .map(|v| v.to_str().unwrap().to_string()),
                Some(origin.to_string())
            );
        }

        let response = app
            .clone()
            .oneshot(preflight("https://stranger.example"))
            .await
            .unwrap();
        assert!(
            !response
                .headers()
                .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        );
    }

    #[tokio::test]
    async fn the_database_stays_open_to_remote_callers() {
        let sandbox = Sandbox::new("sql");
        let app = sandbox.app();

        let select = post_json("/sql/select", "{\"sql\":\"SELECT 1 AS one\",\"values\":[]}");
        assert_eq!(
            status(&app, from("100.64.0.7:52001", select)).await,
            StatusCode::OK
        );

        let execute = post_json(
            "/sql/execute",
            "{\"sql\":\"CREATE TABLE IF NOT EXISTS t (a)\",\"values\":[]}",
        );
        assert_eq!(
            status(&app, from("100.64.0.7:52001", execute)).await,
            StatusCode::OK
        );
    }

    async fn respond(app: Router, request: Request) -> (StatusCode, String) {
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    // Stands in for pinframe: sleeps long enough to be superseded, then writes a sidecar
    // whose run is the --points it was given, so a test can tell which request it answers.
    const FAKE_PINFRAME: &str = r#"#!/bin/sh
while [ $# -gt 0 ]; do
    case "$1" in
        --out) out=$2; shift ;;
        --points) points=$2; shift ;;
    esac
    shift
done
sleep 0.4
printf '{"run":"%s","clip":null,"photos":[{"file":"a.png"}]}' "$points" > "${out%.*}.json"
echo "a.png  10x20  anchor 1,2  scale 1.00  your mark"
echo "clip 10x20"
"#;

    impl Sandbox {
        fn fake_pinframe(&self) -> PathBuf {
            use std::os::unix::fs::PermissionsExt;
            let path = self.base.join("pinframe");
            std::fs::write(&path, FAKE_PINFRAME).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        }

        // A series inside the write root, so the document beside it is writable.
        fn series(&self) -> PathBuf {
            let folder = self.base.join("inbox/series");
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join("a.png"), b"x").unwrap();
            std::fs::write(folder.join("b.png"), b"x").unwrap();
            folder
        }
    }

    fn track_request(folder: &FsPath, marks: &str) -> Request {
        let body = format!(
            "{{\"folder\":{:?},\"marks\":{marks}}}",
            text(folder.to_path_buf())
        );
        post_json("/api/pinframe/track", &body)
    }

    #[tokio::test]
    async fn pinframe_rejects_bad_requests_before_anything_runs() {
        let sandbox = Sandbox::new("pinframe_bad");
        let app = sandbox.app_with(Some(sandbox.fake_pinframe()));
        let folder = text(sandbox.series());

        let bad_marks = [
            "{}",
            "{\"points\":[]}",
            "{\"points\":[[1]]}",
            "{\"points\":[[1,\"2\"]]}",
            "{\"points\":[[1,2]],\"at\":{\"b.png\":{\"points\":[]}}}",
            "{\"points\":[[1,2]],\"near\":{\"b.png\":{\"points\":[[1,2]],\"zoom\":0}}}",
            "{\"points\":[[1,2]],\"skip\":\"b.png\"}",
            "{\"points\":[[1,2]],\"order\":\"sideways\"}",
        ];
        for marks in bad_marks {
            let body = format!("{{\"folder\":{folder:?},\"marks\":{marks}}}");
            let (status, error) =
                respond(app.clone(), post_json("/api/pinframe/track", &body)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{marks}: {error}");
        }

        let marks = "{\"points\":[[1,2]]}";
        let inside = format!("{folder}/clip.avif");
        let outside = text(std::env::temp_dir().canonicalize().unwrap());
        let unwritable = sandbox.base.join("unwritable");
        std::fs::create_dir_all(&unwritable).unwrap();
        let unwritable = text(unwritable);
        let beside_root = text(sandbox.base.join("clip.avif"));
        let rejected = [
            (
                "track",
                format!("{{\"folder\":{folder:?},\"marks\":{marks},\"pad\":\"red\"}}"),
                StatusCode::BAD_REQUEST,
            ),
            (
                "track",
                format!("{{\"marks\":{marks}}}"),
                StatusCode::BAD_REQUEST,
            ),
            (
                "export",
                format!("{{\"folder\":{folder:?},\"marks\":{marks},\"fps\":0}}"),
                StatusCode::BAD_REQUEST,
            ),
            (
                "export",
                format!("{{\"folder\":{folder:?},\"marks\":{marks},\"out\":{inside:?}}}"),
                StatusCode::BAD_REQUEST,
            ),
            (
                "track",
                format!("{{\"folder\":{outside:?},\"marks\":{marks}}}"),
                StatusCode::FORBIDDEN,
            ),
            (
                "track",
                format!("{{\"folder\":{unwritable:?},\"marks\":{marks}}}"),
                StatusCode::FORBIDDEN,
            ),
            (
                "export",
                format!("{{\"folder\":{folder:?},\"marks\":{marks},\"out\":{beside_root:?}}}"),
                StatusCode::FORBIDDEN,
            ),
        ];
        for (route, body, expected) in rejected {
            let request = post_json(&format!("/api/pinframe/{route}"), &body);
            let (status, error) = respond(app.clone(), request).await;
            assert_eq!(status, expected, "{body}: {error}");
        }
        assert!(
            !sandbox.base.join("inbox/series.pin.json").exists(),
            "a rejected request must not write the document"
        );
    }

    #[tokio::test]
    async fn pinframe_answers_a_clear_error_when_the_binary_is_unset_or_missing() {
        let sandbox = Sandbox::new("pinframe_unset");
        let folder = sandbox.series();
        let marks = "{\"points\":[[1,2]]}";

        let (status, error) = respond(sandbox.app(), track_request(&folder, marks)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(error.contains("--pinframe"), "{error}");

        let missing = sandbox.app_with(Some(sandbox.base.join("absent")));
        let (status, error) = respond(missing, track_request(&folder, marks)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(error.contains("absent"), "{error}");
    }

    #[tokio::test]
    async fn a_newer_track_supersedes_the_one_in_flight() {
        let sandbox = Sandbox::new("pinframe_supersede");
        let app = sandbox.app_with(Some(sandbox.fake_pinframe()));
        let folder = sandbox.series();

        let first = tokio::spawn(respond(
            app.clone(),
            track_request(&folder, "{\"points\":[[1,1]]}"),
        ));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let (status, body) = respond(app, track_request(&folder, "{\"points\":[[2,2]]}")).await;

        assert_eq!(first.await.unwrap().0, StatusCode::CONFLICT);
        assert_eq!(status, StatusCode::OK, "{body}");
        let sidecar: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(sidecar["run"], "2,2");
        assert_eq!(sidecar["photos"][0]["width"], 10);
        assert_eq!(sidecar["photos"][0]["note"], "your mark");

        let document = std::fs::read_to_string(sandbox.base.join("inbox/series.pin.json")).unwrap();
        let document: Value = serde_json::from_str(&document).unwrap();
        assert_eq!(document["marks"]["points"], serde_json::json!([[2.0, 2.0]]));
        assert_eq!(document["photos"], serde_json::json!(["a.png", "b.png"]));
    }

    #[tokio::test]
    async fn rapid_marks_only_ever_answer_with_the_newest_result() {
        let sandbox = Sandbox::new("pinframe_rapid");
        let app = sandbox.app_with(Some(sandbox.fake_pinframe()));
        let folder = sandbox.series();

        let mut runs = Vec::new();
        for mark in 0..6 {
            let marks = format!("{{\"points\":[[{mark},{mark}]]}}");
            runs.push(tokio::spawn(respond(
                app.clone(),
                track_request(&folder, &marks),
            )));
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let mut answers = Vec::new();
        for run in runs {
            answers.push(run.await.unwrap());
        }

        let (last, older) = answers.split_last().unwrap();
        for (status, body) in older {
            assert_eq!(*status, StatusCode::CONFLICT, "{body}");
        }
        assert_eq!(last.0, StatusCode::OK, "{}", last.1);
        let sidecar: Value = serde_json::from_str(&last.1).unwrap();
        assert_eq!(sidecar["run"], "5,5");
    }

    fn export_request(folder: &FsPath, marks: &str) -> Request {
        let body = format!(
            "{{\"folder\":{:?},\"marks\":{marks},\"out\":{:?}}}",
            text(folder.to_path_buf()),
            format!("{}.avif", folder.display())
        );
        post_json("/api/pinframe/export", &body)
    }

    #[tokio::test]
    async fn export_streams_each_line_then_the_result() {
        let sandbox = Sandbox::new("pinframe_export");
        let app = sandbox.app_with(Some(sandbox.fake_pinframe()));
        let folder = sandbox.series();

        let (status, body) = respond(app, export_request(&folder, "{\"points\":[[3,3]]}")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let progress = body.find("event: progress\ndata: {\"line\":\"a.png  10x20");
        let clip = body.find("event: progress\ndata: {\"line\":\"clip 10x20\"}");
        let done = body.find("event: done\n");
        assert!(
            progress < clip && clip < done && progress.is_some(),
            "{body}"
        );
        let data = body[done.unwrap()..].lines().nth(1).unwrap();
        let done: Value = serde_json::from_str(data.strip_prefix("data: ").unwrap()).unwrap();
        assert_eq!(done["out"], format!("{}.avif", folder.display()));
        assert_eq!(done["sidecar"]["run"], "3,3");
        assert_eq!(done["sidecar"]["photos"][0]["width"], 10);
    }

    #[tokio::test]
    async fn a_newer_run_ends_the_export_stream_with_superseded() {
        let sandbox = Sandbox::new("pinframe_export_supersede");
        let app = sandbox.app_with(Some(sandbox.fake_pinframe()));
        let folder = sandbox.series();

        let first = tokio::spawn(respond(
            app.clone(),
            export_request(&folder, "{\"points\":[[1,1]]}"),
        ));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let (status, body) = respond(app, track_request(&folder, "{\"points\":[[2,2]]}")).await;
        assert_eq!(status, StatusCode::OK, "{body}");

        let (status, body) = first.await.unwrap();
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body.contains("event: error\n"), "{body}");
        assert!(body.contains("\"superseded\":true"), "{body}");
        assert!(!body.contains("event: done"), "{body}");
    }

    #[tokio::test]
    async fn reveal_only_shows_files_inside_the_read_roots() {
        let sandbox = Sandbox::new("pinframe_reveal");
        let app = sandbox.app();
        let outside = text(std::env::temp_dir().canonicalize().unwrap());
        let expected = if cfg!(target_os = "macos") {
            [StatusCode::BAD_REQUEST, StatusCode::FORBIDDEN]
        } else {
            [StatusCode::NOT_IMPLEMENTED; 2]
        };
        let requests = ["{}".to_string(), format!("{{\"path\":{outside:?}}}")];
        for (body, expected) in requests.iter().zip(expected) {
            let (status, error) =
                respond(app.clone(), post_json("/api/pinframe/reveal", body)).await;
            assert_eq!(status, expected, "{body}: {error}");
        }
    }

    // The done state shows the clip and links its sidecar through /img.
    #[tokio::test]
    async fn img_serves_a_clip_and_its_sidecar_with_their_types() {
        let sandbox = Sandbox::new("pinframe_clip");
        let app = sandbox.app();
        for (name, kind) in [
            ("clip.avif", "image/avif"),
            ("clip.mp4", "video/mp4"),
            ("clip.webm", "video/webm"),
            ("clip.mov", "video/quicktime"),
            ("clip.json", "application/json"),
        ] {
            let path = sandbox.base.join(name);
            std::fs::write(&path, b"x").unwrap();
            let uri = format!("/img/{}", text(path).trim_start_matches('/'));
            let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{name}");
            assert_eq!(response.headers()[header::CONTENT_TYPE], kind, "{name}");
        }
    }

    const PINFRAME: &str = "/Users/user/dev/pinframe/rust/target/release/pinframe";
    const TRAIN_CLOUDS: &str = "/Users/user/dev/pinframe/samples/train clouds";
    const TRAIN_CLOUDS_MARKS: &str = r#"{
        "points": [[1267.5, 510], [1782.5, 510], [1267.5, 710], [1782.5, 710]],
        "at": {
            "IMG_4680.avif": { "points": [[485, 1100], [655, 1100], [485, 1180], [655, 1180]] },
            "IMG_4684.avif": { "points": [[855, 695], [1005, 695], [855, 845], [1005, 845]], "zoom": 3 },
            "IMG_4695.avif": { "points": [[1240, 675], [1360, 675], [1240, 745], [1360, 745]], "zoom": 1 },
            "IMG_4697.avif": { "points": [[1230, 655], [1350, 655], [1230, 725], [1350, 725]], "zoom": 1 },
            "IMG_4698.avif": { "points": [[940, 637.5], [1140, 637.5], [940, 812.5], [1140, 812.5]] },
            "IMG_4703.avif": { "points": [[846, 641.5], [1046, 641.5], [846, 816.5], [1046, 816.5]] },
            "IMG_4706.avif": { "points": [[960, 705], [1040, 705], [960, 755], [1040, 755]], "zoom": 1 },
            "IMG_4708.avif": { "points": [[870, 705], [1050, 705], [870, 795], [1050, 795]], "zoom": 1 },
            "IMG_4713.avif": { "points": [[1160, 645], [1320, 645], [1160, 795], [1320, 795]] },
            "IMG_4716.avif": { "points": [[750, 707.5], [950, 707.5], [750, 782.5], [950, 782.5]] },
            "IMG_4718.avif": { "points": [[535, 985], [805, 985], [535, 1075], [805, 1075]], "zoom": 1 },
            "IMG_4766.avif": { "points": [[900, 165], [1200, 165], [900, 335], [1200, 335]], "zoom": 1 },
            "IMG_4767.avif": { "points": [[1000, 200]] },
            "IMG_4768.avif": { "points": [[1000, 230]] },
            "IMG_4769.avif": { "points": [[1000, 400]] },
            "IMG_4770.avif": { "points": [[1000, 330]] },
            "IMG_4772.avif": { "points": [[1000, 380]] },
            "IMG_4773.avif": { "points": [[650, 600]] },
            "IMG_4774.avif": { "points": [[700, 400]] },
            "IMG_4780.avif": { "points": [[1000, 300]] },
            "IMG_4791.avif": { "points": [[675, 345], [985, 345], [675, 595], [985, 595]], "zoom": 1 },
            "IMG_4796.avif": { "points": [[1620, 60]] }
        }
    }"#;

    // The marking loop's budget: once one run has warmed the feature cache, a re-run with
    // one more mark answers through the endpoint in about 0.15 s on train clouds.
    #[tokio::test]
    async fn a_warm_rerun_on_train_clouds_is_fast() {
        let samples = FsPath::new(TRAIN_CLOUDS);
        if !FsPath::new(PINFRAME).exists() || !samples.exists() {
            eprintln!("skipped: {PINFRAME} or {TRAIN_CLOUDS} is absent");
            return;
        }
        let sandbox = Sandbox::new("pinframe_real");
        let app = sandbox.app_with(Some(PathBuf::from(PINFRAME)));
        let folder = sandbox.base.join("inbox/train clouds");
        std::fs::create_dir_all(&folder).unwrap();
        for entry in std::fs::read_dir(samples).unwrap().flatten() {
            let target = folder.join(entry.file_name());
            if std::fs::hard_link(entry.path(), &target).is_err() {
                std::fs::copy(entry.path(), &target).unwrap();
            }
        }

        let (status, body) = respond(app.clone(), track_request(&folder, TRAIN_CLOUDS_MARKS)).await;
        assert_eq!(status, StatusCode::OK, "{body}");

        let mut marks: Value = serde_json::from_str(TRAIN_CLOUDS_MARKS).unwrap();
        marks["at"]["IMG_4798.avif"] = serde_json::json!({ "points": [[1000, 400]] });
        let started = std::time::Instant::now();
        let (status, body) = respond(app, track_request(&folder, &marks.to_string())).await;
        let elapsed = started.elapsed();
        assert_eq!(status, StatusCode::OK, "{body}");

        let sidecar: Value = serde_json::from_str(&body).unwrap();
        let photos = sidecar["photos"].as_array().unwrap();
        assert_eq!(photos.len(), 57);
        assert!(photos.iter().all(|photo| photo["note"].is_string()));
        eprintln!("warm re-run through the endpoint: {elapsed:?}");
        assert!(elapsed < std::time::Duration::from_secs(1), "{elapsed:?}");
    }
}
