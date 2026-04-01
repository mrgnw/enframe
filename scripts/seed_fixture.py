#!/usr/bin/env python3
"""Seed the screenshot test fixture database."""

import hashlib
import os
import sqlite3
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FIXTURE_DIR = os.path.join(REPO, 'tests', 'fixtures')
DB_PATH = os.path.join(FIXTURE_DIR, 'test.db')
SAMPLE_JPG = os.path.join(FIXTURE_DIR, 'sample.jpg')

SCHEMA_SQL = """
CREATE TABLE IF NOT EXISTS _sqlx_migrations (
	version BIGINT PRIMARY KEY,
	description TEXT NOT NULL,
	installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
	success BOOLEAN NOT NULL,
	checksum BLOB NOT NULL,
	execution_time BIGINT NOT NULL
);

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
CREATE INDEX IF NOT EXISTS idx_photos_lr_asset ON photos(lr_asset_id);

CREATE TABLE IF NOT EXISTS frames (
	id INTEGER PRIMARY KEY,
	photo_id INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
	label TEXT,
	x REAL NOT NULL DEFAULT 0,
	y REAL NOT NULL DEFAULT 0,
	width REAL NOT NULL DEFAULT 1,
	height REAL NOT NULL DEFAULT 1,
	is_default BOOLEAN NOT NULL DEFAULT 0,
	created_at TEXT DEFAULT (datetime('now')),
	rotation REAL NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_frames_photo ON frames(photo_id);

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
CREATE INDEX IF NOT EXISTS idx_focal_points_photo ON focal_points(photo_id);
CREATE INDEX IF NOT EXISTS idx_focal_points_frame ON focal_points(frame_id);
"""

MIGRATION_V1_SQL = open(os.path.join(REPO, 'database.sql')).read()
MIGRATION_V2_SQL = 'ALTER TABLE frames ADD COLUMN rotation REAL NOT NULL DEFAULT 0;'


def main():
	os.makedirs(FIXTURE_DIR, exist_ok=True)

	if not os.path.exists(SAMPLE_JPG):
		print(f'ERROR: {SAMPLE_JPG} does not exist')
		print(
			'Create it first (e.g. sips -z 600 800 -s format jpeg <source> --out tests/fixtures/sample.jpg)'
		)
		raise SystemExit(1)

	if os.path.exists(DB_PATH):
		os.remove(DB_PATH)

	db = sqlite3.connect(DB_PATH)
	db.executescript(SCHEMA_SQL)

	# Insert migration records so Tauri's migration runner skips them
	now = time.strftime('%Y-%m-%d %H:%M:%S')
	for version, desc, sql in [
		(1, 'create tables', MIGRATION_V1_SQL),
		(2, 'add rotation to frames', MIGRATION_V2_SQL),
	]:
		checksum = hashlib.sha384(sql.encode()).digest()
		db.execute(
			'INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) VALUES (?, ?, ?, 1, ?, 0)',
			(version, desc, now, checksum),
		)

	# Insert test photo with absolute path
	db.execute(
		'INSERT INTO photos (id, file_path, width, height, title) VALUES (1, ?, 800, 600, ?)',
		(SAMPLE_JPG, 'Test Photo'),
	)

	db.commit()
	db.close()
	print(f'Seeded {DB_PATH}')
	print(f'  photo: {SAMPLE_JPG}')


if __name__ == '__main__':
	main()
