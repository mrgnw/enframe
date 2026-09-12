mod db;
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
            let inbox = self.base.join("inbox");
            std::fs::create_dir_all(&inbox).unwrap();
            let connection = db::open(&self.base.join("enframe.db")).unwrap();
            let roots = Roots::new(vec![self.base.clone()], vec![inbox.clone()]).unwrap();
            let state = AppState {
                conn: Arc::new(Mutex::new(connection)),
                browse: self.base.clone(),
                roots: Arc::new(roots),
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
}
