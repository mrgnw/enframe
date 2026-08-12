use enigo::{Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use image::ImageEncoder;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use std::{env, fs, thread};
use xcap::Window;

const APP_DATA_DIR: &str = "Library/Application Support/com.enframe.app";
const DB_FILENAME: &str = "enframe.db";
const WINDOW_TITLE: &str = "Enframe";
const WINDOW_TIMEOUT: Duration = Duration::from_secs(20);
const SETTLE_TIME: Duration = Duration::from_millis(1500);
const ACTION_DELAY: Duration = Duration::from_millis(600);

struct AppGuard {
    child: Child,
    real_db_backup: Option<PathBuf>,
    db_path: PathBuf,
}

impl Drop for AppGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();

        // Restore real DB
        if let Some(ref backup) = self.real_db_backup {
            if backup.exists() {
                let _ = fs::rename(backup, &self.db_path);
                eprintln!("Restored real DB from backup");
            }
        }
    }
}

fn repo_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).parent().unwrap().to_path_buf()
}

fn app_data_dir() -> PathBuf {
    dirs_or_home().join(APP_DATA_DIR)
}

fn dirs_or_home() -> PathBuf {
    PathBuf::from(env::var("HOME").expect("HOME not set"))
}

fn find_enframe_binary() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Check release binary first
    let release = manifest_dir.join("target/release/enframe");
    if release.exists() {
        return release;
    }

    // Check debug
    let debug = manifest_dir.join("target/debug/enframe");
    if debug.exists() {
        return debug;
    }

    eprintln!("Enframe binary not found. Building release...");
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&manifest_dir)
        .status()
        .expect("Failed to run cargo build");

    if !status.success() {
        panic!("cargo build --release failed");
    }

    assert!(release.exists(), "Binary not found after build");
    release
}

fn setup_fixture_db() -> (PathBuf, Option<PathBuf>) {
    let root = repo_root();
    let fixture_db = root.join("tests/fixtures/test.db");
    assert!(
        fixture_db.exists(),
        "Fixture DB not found at {fixture_db:?}. Run: python3 scripts/seed_fixture.py"
    );

    let data_dir = app_data_dir();
    fs::create_dir_all(&data_dir).expect("Failed to create app data dir");

    let db_path = data_dir.join(DB_FILENAME);

    // Backup real DB if it exists
    let backup_path = data_dir.join(format!("{DB_FILENAME}.screenshots-backup"));
    let real_db_backup = if db_path.exists() {
        fs::copy(&db_path, &backup_path).expect("Failed to backup real DB");
        eprintln!("Backed up real DB to {backup_path:?}");
        Some(backup_path)
    } else {
        None
    };

    // Copy fixture DB
    fs::copy(&fixture_db, &db_path).expect("Failed to copy fixture DB");
    eprintln!("Installed fixture DB to {db_path:?}");

    (db_path, real_db_backup)
}

fn launch_app(binary: &Path) -> Child {
    Command::new(binary)
        .env("SCREENSHOT_PHOTO_ID", "1")
        .spawn()
        .expect("Failed to launch Enframe")
}

fn wait_for_window(timeout: Duration) -> Window {
    let start = Instant::now();
    loop {
        if start.elapsed() > timeout {
            panic!("Timed out waiting for {WINDOW_TITLE} window after {timeout:?}");
        }

        if let Ok(windows) = Window::all() {
            for w in windows {
                let title = w.title().unwrap_or_default();
                if title.contains(WINDOW_TITLE) && w.width().unwrap_or(0) > 100 {
                    eprintln!(
                        "Found window: {title} ({}x{})",
                        w.width().unwrap_or(0),
                        w.height().unwrap_or(0)
                    );
                    return w;
                }
            }
        }

        thread::sleep(Duration::from_millis(500));
    }
}

fn capture(_window: &Window, out_dir: &Path, name: &str) {
    // Re-find the window to get fresh content
    let fresh = Window::all()
        .expect("Failed to list windows")
        .into_iter()
        .find(|w| w.title().unwrap_or_default().contains(WINDOW_TITLE))
        .expect("Window disappeared");

    let img = fresh.capture_image().expect("Failed to capture window");
    let path = out_dir.join(format!("{name}.webp"));

    // Encode as WebP
    let mut buf: Vec<u8> = Vec::new();
    let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut buf);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .expect("WebP encode failed");

    fs::write(&path, &buf).expect("Failed to write screenshot");
    let kb = buf.len() / 1024;
    eprintln!("  Captured: {} ({kb}KB)", path.display());
}

fn window_center(window: &Window) -> (i32, i32) {
    let x = window.x().unwrap_or(0);
    let y = window.y().unwrap_or(0);
    let w = window.width().unwrap_or(800) as i32;
    let h = window.height().unwrap_or(600) as i32;
    (x + w / 2, y + h / 2 + 20)
}

fn click_at(enigo: &mut Enigo, x: i32, y: i32) {
    let _ = enigo.move_mouse(x, y, Coordinate::Abs);
    thread::sleep(Duration::from_millis(50));
    let _ = enigo.button(Button::Left, Direction::Click);
}

fn alt_click_at(enigo: &mut Enigo, x: i32, y: i32) {
    let _ = enigo.move_mouse(x, y, Coordinate::Abs);
    thread::sleep(Duration::from_millis(50));
    let _ = enigo.key(Key::Alt, Direction::Press);
    thread::sleep(Duration::from_millis(50));
    let _ = enigo.button(Button::Left, Direction::Click);
    thread::sleep(Duration::from_millis(50));
    let _ = enigo.key(Key::Alt, Direction::Release);
}

fn press_key(enigo: &mut Enigo, c: char) {
    let _ = enigo.key(Key::Unicode(c), Direction::Click);
}

fn list_windows() {
    match Window::all() {
        Ok(windows) => {
            eprintln!("Found {} windows:", windows.len());
            for w in &windows {
                let title = w.title().unwrap_or_default();
                let width = w.width().unwrap_or(0);
                let height = w.height().unwrap_or(0);
                if width > 0 {
                    eprintln!("  '{}' ({}x{})", title, width, height);
                }
            }
        }
        Err(e) => eprintln!("Error listing windows: {e}"),
    }
}

fn main() {
    let root = repo_root();

    // Parse args
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--list-windows") {
        list_windows();
        return;
    }

    let out_dir = if let Some(pos) = args.iter().position(|a| a == "--out-dir") {
        PathBuf::from(args.get(pos + 1).expect("--out-dir requires a path"))
    } else {
        root.join("docs/screenshots")
    };
    fs::create_dir_all(&out_dir).expect("Failed to create output dir");

    eprintln!("=== Enframe Screenshot Tests ===");
    eprintln!("Output: {}", out_dir.display());

    // Setup fixture DB (copies to app data dir, backs up real DB)
    let (db_path, real_db_backup) = setup_fixture_db();

    // Find and launch app
    let binary = find_enframe_binary();
    eprintln!("Binary: {}", binary.display());

    let child = launch_app(&binary);
    let _guard = AppGuard {
        child,
        real_db_backup,
        db_path,
    };

    // Wait for window
    eprintln!("Waiting for window...");
    let window = wait_for_window(WINDOW_TIMEOUT);

    // Let the app fully load and navigate to the edit page
    eprintln!("Waiting for app to settle...");
    thread::sleep(SETTLE_TIME);
    thread::sleep(SETTLE_TIME); // Extra time for edit page to load

    let mut enigo = Enigo::new(&Settings::default()).expect("Failed to create Enigo");
    let (cx, cy) = window_center(&window);

    // === Scenarios ===

    eprintln!("\n--- Scenario: Editor loaded ---");
    capture(&window, &out_dir, "01-editor-loaded");

    eprintln!("\n--- Scenario: Add focal point ---");
    click_at(&mut enigo, cx, cy);
    thread::sleep(ACTION_DELAY);
    capture(&window, &out_dir, "02-focal-point");

    eprintln!("\n--- Scenario: Auto dual frame (Alt+click) ---");
    alt_click_at(&mut enigo, cx - 100, cy - 50);
    thread::sleep(ACTION_DELAY);
    capture(&window, &out_dir, "03-auto-dual-frame");

    eprintln!("\n--- Scenario: Panel open ---");
    press_key(&mut enigo, 't');
    thread::sleep(ACTION_DELAY);
    capture(&window, &out_dir, "04-panel-open");

    // Select a frame first (press . to select next frame)
    press_key(&mut enigo, '.');
    thread::sleep(Duration::from_millis(200));

    let guides = [
        "05-guide-thirds",
        "06-guide-phi",
        "07-guide-spiral",
        "08-guide-triangle",
        "09-guide-diagonal",
    ];

    for name in guides {
        eprintln!("\n--- Scenario: {name} ---");
        press_key(&mut enigo, 'g');
        thread::sleep(ACTION_DELAY);
        capture(&window, &out_dir, name);
    }

    eprintln!(
        "\n=== Done! Saved {} screenshots to {} ===",
        5 + guides.len(),
        out_dir.display()
    );
}
