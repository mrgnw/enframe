pub struct Migration {
    pub version: i64,
    pub description: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "create tables",
        sql: include_str!("../../database.sql"),
    },
    Migration {
        version: 2,
        description: "add rotation to frames",
        sql: "ALTER TABLE frames ADD COLUMN rotation REAL NOT NULL DEFAULT 0;",
    },
    Migration {
        version: 3,
        description: "add destination to frames",
        sql: "ALTER TABLE frames ADD COLUMN destination TEXT;",
    },
    Migration {
        version: 4,
        description: "add is_auto to frames",
        sql: "ALTER TABLE frames ADD COLUMN is_auto INTEGER NOT NULL DEFAULT 0;",
    },
    Migration {
        version: 5,
        description: "persistent undo history",
        sql: "CREATE TABLE IF NOT EXISTS history (
	id INTEGER PRIMARY KEY,
	photo_id INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
	seq INTEGER NOT NULL,
	snapshot TEXT NOT NULL,
	created_at TEXT DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_history_photo ON history(photo_id);
ALTER TABLE photos ADD COLUMN history_pos INTEGER NOT NULL DEFAULT 0;",
    },
    Migration {
        version: 6,
        description: "carved versions",
        sql: "ALTER TABLE photos ADD COLUMN source_id INTEGER REFERENCES photos(id) ON DELETE CASCADE;
ALTER TABLE photos ADD COLUMN carve TEXT;",
    },
];
