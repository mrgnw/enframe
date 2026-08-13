mod db;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
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
}

type ApiResult<T> = Result<T, (StatusCode, String)>;

fn fail(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
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

async fn sql_select(State(state): State<AppState>, Json(body): Json<SqlBody>) -> ApiResult<Json<Value>> {
    blocking(move || {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        db::select(&conn, &body.sql, &body.values)
    })
    .await
    .map(Json)
}

async fn sql_execute(State(state): State<AppState>, Json(body): Json<SqlBody>) -> ApiResult<Json<Value>> {
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
            encode(lightroom::list_lr_album_assets(a.album_id, a.limit, a.offset))
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

async fn api(Path(command): Path<String>, Json(body): Json<Value>) -> ApiResult<Json<Value>> {
    blocking(move || dispatch(&command, body)).await.map(Json)
}

#[derive(Deserialize)]
struct PathQuery {
    path: String,
}

async fn read_text(Query(query): Query<PathQuery>) -> ApiResult<String> {
    blocking(move || std::fs::read_to_string(&query.path).map_err(|e| e.to_string())).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Stat {
    is_directory: bool,
}

async fn stat(Query(query): Query<PathQuery>) -> ApiResult<Json<Stat>> {
    blocking(move || {
        let meta = std::fs::metadata(&query.path).map_err(|e| e.to_string())?;
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

fn list_dir(dir: Option<String>) -> Result<Listing, String> {
    let home = std::env::var("HOME").map_err(|e| format!("HOME: {e}"))?;
    let dir = PathBuf::from(dir.filter(|d| !d.is_empty()).unwrap_or(home));
    let dir = dir.canonicalize().map_err(|e| format!("{}: {e}", dir.display()))?;

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

    Ok(Listing {
        parent: dir.parent().map(|p| p.to_string_lossy().into_owned()),
        path: dir.to_string_lossy().into_owned(),
        entries,
    })
}

async fn ls(Query(query): Query<LsQuery>) -> ApiResult<Json<Listing>> {
    blocking(move || list_dir(query.dir)).await.map(Json)
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
        .map(|v| percent_encoding::percent_decode_str(v).decode_utf8_lossy().into_owned())
        .map(|v| v.rsplit('/').next().unwrap_or(&v).to_string())
        .filter(|v| !v.is_empty() && v != "." && v != "..")
        .ok_or((StatusCode::BAD_REQUEST, "missing x-filename header".into()))?;

    blocking(move || {
        std::fs::create_dir_all(&state.inbox).map_err(|e| e.to_string())?;
        let path = state.inbox.join(&filename);
        std::fs::write(&path, &body).map_err(|e| e.to_string())?;
        Ok(Uploaded {
            path: path.to_string_lossy().into_owned(),
        })
    })
    .await
    .map(Json)
}

async fn img(Path(path): Path<String>, request: Request) -> ApiResult<Response> {
    let absolute = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    ServeFile::new(absolute)
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
}

fn flag(args: &[String], name: &str, env: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
        .or_else(|| std::env::var(env).ok())
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
        database,
        inbox,
    })
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let options = options()?;
    let state = AppState {
        conn: Arc::new(Mutex::new(db::open(&options.database)?)),
        inbox: options.inbox.clone(),
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
        .with_state(state);

    let address = format!("{}:{}", options.host, options.port);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|e| format!("bind {address}: {e}"))?;
    println!("enframe serving http://{address}");
    println!("  database {}", options.database.display());
    println!("  web      {}", options.web.display());
    println!("  inbox    {}", options.inbox.display());

    axum::serve(listener, app.into_make_service())
        .await
        .map_err(|e| e.to_string())
}
