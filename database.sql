-- Enframe SQLite Schema
-- All coordinates are normalized (0.0 to 1.0) to survive image resizing

CREATE TABLE IF NOT EXISTS photos (
	id INTEGER PRIMARY KEY,
	file_path TEXT NOT NULL UNIQUE,
	width INTEGER,
	height INTEGER,
	title TEXT,
	description TEXT,
	lr_asset_id TEXT,
	created_at TEXT DEFAULT (datetime('now'))
);

CREATE INDEX idx_photos_lr_asset ON photos(lr_asset_id);

CREATE TABLE IF NOT EXISTS frames (
	id INTEGER PRIMARY KEY,
	photo_id INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
	label TEXT,
	x REAL NOT NULL DEFAULT 0,
	y REAL NOT NULL DEFAULT 0,
	width REAL NOT NULL DEFAULT 1,
	height REAL NOT NULL DEFAULT 1,
	rotation REAL NOT NULL DEFAULT 0,
	is_default BOOLEAN NOT NULL DEFAULT 0,
	created_at TEXT DEFAULT (datetime('now'))
);

CREATE INDEX idx_frames_photo ON frames(photo_id);

CREATE TABLE IF NOT EXISTS focal_points (
	id INTEGER PRIMARY KEY,
	photo_id INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
	frame_id INTEGER REFERENCES frames(id) ON DELETE CASCADE,
	x REAL NOT NULL,
	y REAL NOT NULL,
	label TEXT,
	required BOOLEAN NOT NULL DEFAULT 1,
	created_at TEXT DEFAULT (datetime('now'))
);

CREATE INDEX idx_focal_points_photo ON focal_points(photo_id);
CREATE INDEX idx_focal_points_frame ON focal_points(frame_id);
