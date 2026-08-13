mod db;
mod roots;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use roots::Roots;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};
use tower::ServiceExt as _;
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
    use app_lib::commands::{crop_and_export, get_screenshot_mode, save_image};
    use app_lib::{inbox, lightroom};

    match command {
        "get_screenshot_mode" => encode(Ok(get_screenshot_mode())),
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

fn confine_command(roots: &Roots, command: &str, body: &mut Value) -> ApiResult<()> {
    match command {
        "save_image" => confine(roots, body, "path", Access::Write),
        "crop_and_export" => {
            confine(roots, body, "sourcePath", Access::Read)?;
            confine(roots, body, "outputPath", Access::Write)
        }
        "scan_inbox" => confine(roots, body, "directory", Access::Read),
        // The Lightroom commands take ids and queries, and read the library through
        // a root that has to be allowlisted for them to work at all.
        _ => Ok(()),
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

#[derive(Deserialize)]
struct PathQuery {
    path: String,
}

// ponytail: the root check runs on the async thread — it is one realpath(2).
// Move it into spawn_blocking if roots ever point at a network mount.
async fn read_text(
    State(state): State<AppState>,
    Query(query): Query<PathQuery>,
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
    Query(query): Query<PathQuery>,
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
    ServeFile::new(file)
        .oneshot(request)
        .await
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

fn build_roots(options: &Options, pictures: &FsPath, library: Vec<PathBuf>) -> Result<Roots, String> {
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

#[tokio::main]
async fn main() -> Result<(), String> {
    let options = options()?;
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

    let spa = ServeDir::new(&options.web).fallback(ServeFile::new(options.web.join("index.html")));
    let app = Router::new()
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
        .with_state(state.clone());

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

    axum::serve(listener, app.into_make_service())
        .await
        .map_err(|e| e.to_string())
}
