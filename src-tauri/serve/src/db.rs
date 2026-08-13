use app_lib::migrations::MIGRATIONS;
use rusqlite::Connection;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::limits::Limit;
use rusqlite::types::{ToSqlOutput, Value as SqlValue, ValueRef};
use serde_json::{Map, Value};
use sha2::{Digest, Sha384};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const MAX_ROWS: usize = 100_000;

// The desktop app stores its database under the Tauri app-config dir for the
// bundle identifier, so read the identifier from the same config the app builds from.
pub fn default_path() -> Result<PathBuf, String> {
    let config: Value = serde_json::from_str(include_str!("../../tauri.conf.json"))
        .map_err(|e| format!("tauri.conf.json: {e}"))?;
    let identifier = config["identifier"]
        .as_str()
        .ok_or("tauri.conf.json has no identifier")?;
    let home = PathBuf::from(std::env::var("HOME").map_err(|e| format!("HOME: {e}"))?);

    // ponytail: macOS and Linux only, matching where this app actually runs.
    let base = if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else {
        home.join(".local/share")
    };
    Ok(base.join(identifier).join("enframe.db"))
}

pub fn open(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let mut conn = Connection::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
        .map_err(|e| e.to_string())?;
    migrate(&mut conn)?;
    confine(&conn);
    Ok(conn)
}

// /sql takes arbitrary statements, and SQLite can reach outside its own database:
// ATTACH creates a file anywhere, and a virtual table module can open one. Both are
// shut off here, after the migrations have run their DDL.
// rusqlite's `load_extension` feature is not enabled, so sqlite3_load_extension is
// not compiled in and cannot be reached from SQL.
fn confine(conn: &Connection) {
    conn.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0);
    conn.authorizer(Some(|context: AuthContext<'_>| match context.action {
        AuthAction::Attach { .. }
        | AuthAction::Detach { .. }
        | AuthAction::CreateVtable { .. }
        | AuthAction::DropVtable { .. }
        | AuthAction::Pragma { .. }
        | AuthAction::Unknown { .. } => Authorization::Deny,
        _ => Authorization::Allow,
    }));
}

// Mirrors the sqlx bookkeeping tauri-plugin-sql uses, so a database migrated here
// is accepted by the desktop app and vice versa.
fn migrate(conn: &mut Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    success BOOLEAN NOT NULL,
    checksum BLOB NOT NULL,
    execution_time BIGINT NOT NULL
);",
    )
    .map_err(|e| format!("migrations table: {e}"))?;

    let applied: HashSet<i64> = {
        let mut stmt = conn
            .prepare("SELECT version FROM _sqlx_migrations")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };

    for migration in MIGRATIONS {
        if applied.contains(&migration.version) {
            continue;
        }
        let checksum = Sha384::digest(migration.sql.as_bytes()).to_vec();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(migration.sql)
            .map_err(|e| format!("migration {}: {e}", migration.version))?;
        tx.execute(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
             VALUES (?1, ?2, TRUE, ?3, 0)",
            rusqlite::params![migration.version, migration.description, checksum],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    Ok(())
}

// The frontend writes tauri-plugin-sql's `$1` placeholders; SQLite reads `?1` as the
// same positional binding. ponytail: no string-literal awareness, no app query has a `$`.
fn to_positional(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek().is_some_and(char::is_ascii_digit) {
            out.push('?');
        } else {
            out.push(c);
        }
    }
    out
}

fn to_sql(value: &Value) -> ToSqlOutput<'static> {
    let owned = match value {
        Value::Null => SqlValue::Null,
        Value::Bool(b) => SqlValue::Integer(*b as i64),
        Value::Number(n) => match n.as_i64() {
            Some(i) => SqlValue::Integer(i),
            None => SqlValue::Real(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => SqlValue::Text(s.clone()),
        other => SqlValue::Text(other.to_string()),
    };
    ToSqlOutput::Owned(owned)
}

fn from_sql(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::from(i),
        ValueRef::Real(f) => Value::from(f),
        ValueRef::Text(t) => Value::from(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Value::from(b.to_vec()),
    }
}

pub fn select(conn: &Connection, sql: &str, values: &[Value]) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(&to_positional(sql))
        .map_err(|e| e.to_string())?;
    let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
    let params = rusqlite::params_from_iter(values.iter().map(to_sql));

    let mut rows = stmt.query(params).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        if out.len() >= MAX_ROWS {
            return Err(format!("query returned more than {MAX_ROWS} rows"));
        }
        let mut object = Map::with_capacity(columns.len());
        for (i, name) in columns.iter().enumerate() {
            object.insert(
                name.clone(),
                from_sql(row.get_ref(i).map_err(|e| e.to_string())?),
            );
        }
        out.push(Value::Object(object));
    }
    Ok(Value::Array(out))
}

pub fn execute(conn: &Connection, sql: &str, values: &[Value]) -> Result<Value, String> {
    let params = rusqlite::params_from_iter(values.iter().map(to_sql));
    let rows_affected = conn
        .execute(&to_positional(sql), params)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "rowsAffected": rows_affected,
        "lastInsertId": conn.last_insert_rowid(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_become_positional() {
        assert_eq!(
            to_positional("SELECT * FROM photos WHERE id = $1 AND x > $2"),
            "SELECT * FROM photos WHERE id = ?1 AND x > ?2"
        );
        assert_eq!(to_positional("SELECT '$' || a"), "SELECT '$' || a");
    }

    #[test]
    fn select_returns_column_keyed_objects() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER, name TEXT, ratio REAL);")
            .unwrap();
        execute(
            &conn,
            "INSERT INTO t (id, name, ratio) VALUES ($1, $2, $3)",
            &[Value::from(7), Value::from("a"), Value::from(0.5)],
        )
        .unwrap();
        let rows = select(&conn, "SELECT * FROM t WHERE id = $1", &[Value::from(7)]).unwrap();
        assert_eq!(
            rows,
            serde_json::json!([{ "id": 7, "name": "a", "ratio": 0.5 }])
        );
    }

    #[test]
    fn migrations_are_idempotent() {
        let dir = std::env::temp_dir().join(format!("enframe_serve_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("enframe.db");
        open(&path).unwrap();
        let conn = open(&path).unwrap();
        let applied: i64 = conn
            .query_row("SELECT count(*) FROM _sqlx_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(applied as usize, MIGRATIONS.len());
        assert_eq!(
            select(&conn, "SELECT count(*) AS n FROM photos", &[]).unwrap(),
            serde_json::json!([{ "n": 0 }])
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn attach_is_denied_and_app_queries_still_run() {
        let dir = std::env::temp_dir().join(format!("enframe_serve_attach_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let conn = open(&dir.join("enframe.db")).unwrap();

        let pwn = dir.join("pwn.db");
        let attach = format!("ATTACH DATABASE '{}' AS pwn", pwn.display());
        assert!(execute(&conn, &attach, &[]).is_err());
        assert!(execute(&conn, "DETACH DATABASE pwn", &[]).is_err());
        assert!(execute(&conn, "PRAGMA journal_mode = DELETE", &[]).is_err());
        assert!(!pwn.exists());

        // The four statement shapes src/lib/db.ts actually sends.
        execute(
            &conn,
            "INSERT INTO photos (file_path, title) VALUES ($1, $2)",
            &[Value::from("/photo.jpg"), Value::from("a")],
        )
        .unwrap();
        execute(
            &conn,
            "UPDATE photos SET title = $1 WHERE id = $2",
            &[Value::from("b"), Value::from(1)],
        )
        .unwrap();
        assert_eq!(
            select(&conn, "SELECT count(*) AS n FROM photos", &[]).unwrap(),
            serde_json::json!([{ "n": 1 }])
        );
        execute(&conn, "DELETE FROM photos WHERE id = $1", &[Value::from(1)]).unwrap();

        let _ = std::fs::remove_dir_all(&dir);
    }
}
