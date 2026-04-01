#!/usr/bin/env python3
"""
Test: Create a duplicate asset in Lightroom CC's local catalog with a different crop.

Usage:
        1. Close Lightroom CC completely
        2. Run: uv run --with msgpack scripts/lr_test_duplicate.py
        3. Open Lightroom CC and check if the new cropped copy appears

Dry run (no changes):
        uv run --with msgpack scripts/lr_test_duplicate.py --dry-run

Undo (restore from latest backup):
        uv run --with msgpack scripts/lr_test_duplicate.py --undo
"""

import argparse
import hashlib
import os
import re
import shutil
import sqlite3
import sys
import uuid
from datetime import datetime, timezone
from pathlib import Path

try:
    import msgpack
except ImportError:
    print("Missing dependency. Run with: uv run --with msgpack")
    sys.exit(1)


LR_LIBRARY = Path.home() / "Pictures" / "Lightroom Library.lrlibrary"
CATALOG_GUID = "00000000000000000000000000000000"
CATALOG_DIR = LR_LIBRARY / CATALOG_GUID
MCAT_PATH = CATALOG_DIR / "Managed Catalog.mcat"
SETTINGS_DIR = CATALOG_DIR / "settings"
BACKUP_DIR = Path(__file__).parent / "lr_backup"

# Source photo: IMG_0001.ARW (6000x4000, currently cropped to 3224x3999)
SOURCE_ASSET_ID = "00a0493cc09f42b48043b22c97647d15"

# Test crop: portrait crop from center-right area
# Original is 6000x4000 (landscape), this gives a 2400x3200 portrait region
TEST_CROP = {
    "CropTop": "0.1",
    "CropLeft": "0.3",
    "CropBottom": "0.9",
    "CropRight": "0.7",
    "CropAngle": "0",
    "CropConstrainToWarp": "0",
    "CropConstrainToUnitSquare": "1",
    "HasCrop": "True",
}


def backup_catalog():
    BACKUP_DIR.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    backup_subdir = BACKUP_DIR / timestamp
    backup_subdir.mkdir(exist_ok=True)

    for suffix in ["", "-shm", "-wal"]:
        src = Path(str(MCAT_PATH) + suffix)
        if src.exists():
            dst = backup_subdir / src.name
            shutil.copy2(src, dst)
            print(f"  Backed up {src.name} ({src.stat().st_size:,} bytes)")

    print(f"  Saved to {backup_subdir}")
    return backup_subdir


def restore_from_latest_backup():
    if not BACKUP_DIR.exists():
        print("No backups found")
        return False

    backups = sorted(BACKUP_DIR.iterdir())
    if not backups:
        print("No backups found")
        return False

    latest = backups[-1]
    print(f"Restoring from {latest}")

    for f in latest.iterdir():
        dst = CATALOG_DIR / f.name
        shutil.copy2(f, dst)
        print(f"  Restored {f.name}")

    print("Done")
    return True


def read_source_asset(conn):
    cur = conn.cursor()
    cur.execute(
        """
		SELECT d.fullDocId, d.localDocId, d.annotation, r.revId, r.content, r.sequence
		FROM docs d JOIN revs r ON r.sequence = d.winningRevSequence
		WHERE CAST(d.fullDocId AS TEXT) = ?
	""",
        (SOURCE_ASSET_ID,),
    )

    row = cur.fetchone()
    if not row:
        print(f"Source asset {SOURCE_ASSET_ID} not found")
        sys.exit(1)

    return {
        "fullDocId": row[0].decode("utf-8") if isinstance(row[0], bytes) else row[0],
        "localDocId": row[1],
        "annotation": msgpack.unpackb(row[2], raw=False) if row[2] else {},
        "revId": row[3].decode("utf-8") if isinstance(row[3], bytes) else row[3],
        "content": msgpack.unpackb(row[4], raw=False),
        "sequence": row[5],
    }


def read_xmp_settings(sha256):
    path = SETTINGS_DIR / sha256
    if not path.exists():
        print(f"XMP settings not found: {path}")
        sys.exit(1)
    return path.read_text()


def modify_xmp_crop(xmp_text, crop_settings):
    modified = xmp_text
    for key, value in crop_settings.items():
        pattern = rf'crs:{key}="[^"]*"'
        replacement = f'crs:{key}="{value}"'
        if re.search(pattern, modified):
            modified = re.sub(pattern, replacement, modified)
        else:
            # Insert before HasSettings if the key doesn't exist yet
            modified = modified.replace(
                "crs:HasSettings=", f'crs:{key}="{value}"\n   crs:HasSettings='
            )
    return modified


def compute_sha256(content):
    return hashlib.sha256(content.encode("utf-8")).hexdigest()


def write_xmp_settings(sha256, content):
    path = SETTINGS_DIR / sha256
    if path.exists():
        print(f"  XMP already exists: {sha256[:24]}...")
        return path
    path.write_text(content)
    print(f"  Wrote XMP: {sha256[:24]}... ({len(content)} bytes)")
    return path


def calculate_cropped_dimensions(original_width, original_height, crop):
    left = float(crop["CropLeft"])
    right = float(crop["CropRight"])
    top = float(crop["CropTop"])
    bottom = float(crop["CropBottom"])
    return (
        int(original_width * (right - left)),
        int(original_height * (bottom - top)),
    )


def create_asset_doc(conn, source, new_xmp_sha256, cropped_w, cropped_h):
    new_doc_id = uuid.uuid4().hex
    new_rev_id = uuid.uuid4().hex
    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.000Z")

    # Clone source content, update develop settings
    content = dict(source["content"])
    content["userCreated"] = now
    content["userUpdated"] = now
    content["changedAtTime"] = now
    content["changedOnDevice"] = "enframe"

    dev = dict(content.get("develop", {}))
    dev["croppedWidth"] = cropped_w
    dev["croppedHeight"] = cropped_h
    dev["userUpdated"] = now
    dev["device"] = "enframe"
    dev["xmpCameraRaw"] = {"sha256": new_xmp_sha256}
    content["develop"] = dev

    # Strip fields LR CC should recalculate
    for key in ("aesthetics", "reviews", "ratings"):
        content.pop(key, None)

    content_bytes = msgpack.packb(content, use_bin_type=True)

    # Build annotation with local file reference
    xmp_path = str(SETTINGS_DIR / new_xmp_sha256)
    xmp_size = os.path.getsize(xmp_path)
    annotation = {
        "_localOnly": {
            "files": {
                "xmp_develop": {
                    "fileSize": xmp_size,
                    "path": xmp_path,
                    "sha256": new_xmp_sha256,
                }
            }
        }
    }
    annotation_bytes = msgpack.packb(annotation, use_bin_type=True)

    cur = conn.cursor()

    # Insert doc
    cur.execute(
        """
		INSERT INTO docs (fullDocId, annotation, type, subtype, deleted, hasConflicts)
		VALUES (?, ?, 'asset', 'image', 0, 0)
	""",
        (new_doc_id, annotation_bytes),
    )
    local_doc_id = cur.lastrowid

    # Insert revision
    cur.execute(
        """
		INSERT INTO revs (localDocId, revId, current, deleted, content)
		VALUES (?, ?, 1, 0, ?)
	""",
        (local_doc_id, new_rev_id, content_bytes),
    )
    rev_sequence = cur.lastrowid

    # Link winning revision
    cur.execute(
        """
		UPDATE docs SET winningRevSequence = ?, mostRecentRevSequence = ?
		WHERE localDocId = ?
	""",
        (rev_sequence, rev_sequence, local_doc_id),
    )

    print(f"  Doc ID:    {new_doc_id}")
    print(f"  Local ID:  {local_doc_id}")
    print(f"  Rev ID:    {new_rev_id}")
    print(f"  Sequence:  {rev_sequence}")

    return new_doc_id, local_doc_id, rev_sequence


def main():
    parser = argparse.ArgumentParser(description="Test LR CC catalog duplication")
    parser.add_argument(
        "--undo", action="store_true", help="Restore from latest backup"
    )
    parser.add_argument(
        "--dry-run", action="store_true", help="Preview without writing"
    )
    args = parser.parse_args()

    if args.undo:
        restore_from_latest_backup()
        return

    if not MCAT_PATH.exists():
        print(f"Catalog not found: {MCAT_PATH}")
        sys.exit(1)

    # Warn if LR CC might be running
    lr_wal = Path(str(MCAT_PATH) + "-wal")
    if lr_wal.exists() and lr_wal.stat().st_size > 0:
        print("WARNING: WAL file is non-empty. Lightroom may be running.")
        print("Close Lightroom CC before proceeding.")
        response = input("Continue anyway? [y/N] ")
        if response.lower() != "y":
            return

    print("=== LR CC Catalog Test Duplication ===\n")

    # 1. Backup
    print("1. Backing up catalog...")
    backup_catalog()
    print()

    # 2. Read source
    print("2. Reading source asset...")
    conn = sqlite3.connect(str(MCAT_PATH))
    conn.text_factory = bytes

    source = read_source_asset(conn)
    imp = source["content"].get("importSource", {})
    dev = source["content"].get("develop", {})

    print(f"  File:     {imp.get('fileName')}")
    print(f"  Original: {imp.get('originalWidth')}x{imp.get('originalHeight')}")
    print(f"  Current:  {dev.get('croppedWidth')}x{dev.get('croppedHeight')}")
    print()

    # 3. Modify XMP
    print("3. Creating modified XMP...")
    src_xmp_sha = dev.get("xmpCameraRaw", {}).get("sha256")
    if not src_xmp_sha:
        print("  ERROR: Source has no XMP sha256")
        conn.close()
        sys.exit(1)

    src_xmp = read_xmp_settings(src_xmp_sha)
    new_xmp = modify_xmp_crop(src_xmp, TEST_CROP)
    new_xmp_sha = compute_sha256(new_xmp)

    orig_w = imp.get("originalWidth", 6000)
    orig_h = imp.get("originalHeight", 4000)
    cropped_w, cropped_h = calculate_cropped_dimensions(orig_w, orig_h, TEST_CROP)

    print(f"  Source XMP: {src_xmp_sha[:24]}...")
    print(f"  New XMP:    {new_xmp_sha[:24]}...")
    print(
        f"  Crop: T={TEST_CROP['CropTop']} L={TEST_CROP['CropLeft']} B={TEST_CROP['CropBottom']} R={TEST_CROP['CropRight']}"
    )
    print(f"  Result:     {cropped_w}x{cropped_h}")
    print()

    if args.dry_run:
        print("DRY RUN — no changes written")
        conn.close()
        return

    # 4. Write XMP file
    print("4. Writing XMP settings file...")
    write_xmp_settings(new_xmp_sha, new_xmp)
    print()

    # 5. Insert asset doc
    print("5. Inserting asset doc...")
    new_doc_id, local_doc_id, rev_seq = create_asset_doc(
        conn, source, new_xmp_sha, cropped_w, cropped_h
    )
    print()

    conn.commit()
    conn.close()

    print("=== Done ===\n")
    print(f"New asset: {new_doc_id}")
    print(f"New XMP:   settings/{new_xmp_sha}\n")
    print("Next:")
    print("  1. Open Lightroom CC")
    print("  2. Look for a duplicate of IMG_0001.ARW with a different crop")
    print(
        "  3. Check ~/Library/Application Support/Adobe/Lightroom CC/lrcc_console.log for errors\n"
    )
    print("Undo:")
    print("  uv run --with msgpack scripts/lr_test_duplicate.py --undo")


if __name__ == "__main__":
    main()
