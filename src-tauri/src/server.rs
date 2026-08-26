use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const PORT: u16 = 5911;
const PROBE_TIMEOUT: Duration = Duration::from_millis(300);
const READY_TIMEOUT: Duration = Duration::from_secs(10);
const READY_POLL: Duration = Duration::from_millis(50);

fn address() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], PORT))
}

pub fn base_url() -> String {
    format!("http://127.0.0.1:{PORT}")
}

// A listening socket is enough: whatever answers here owns the database, because the
// server takes an exclusive lock on it before it binds.
fn answering() -> bool {
    TcpStream::connect_timeout(&address(), PROBE_TIMEOUT).is_ok()
}

// Tauri copies an externalBin next to the app binary with the target triple stripped,
// which is also where `cargo build` leaves it, so one lookup covers bundle and dev.
fn sidecar() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let path = exe
        .parent()
        .ok_or("the app binary has no directory")?
        .join("enframe-serve");
    if !path.exists() {
        return Err(format!("no bundled server at {}", path.display()));
    }
    Ok(path)
}

// Probe before spawning so an instance that is already running keeps the database it
// owns. The probe is racy on its own — the ownership lock in the server is what makes
// one writer structural — but it is what keeps the common case from spawning a second
// process only to watch it exit.
pub fn ensure() -> Result<Option<Child>, String> {
    if answering() {
        return Ok(None);
    }
    let mut child = Command::new(sidecar()?)
        .spawn()
        .map_err(|e| format!("spawn enframe-serve: {e}"))?;

    let deadline = Instant::now() + READY_TIMEOUT;
    while Instant::now() < deadline {
        if answering() {
            return Ok(Some(child));
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!("enframe-serve exited before serving: {status}"));
        }
        std::thread::sleep(READY_POLL);
    }
    let _ = child.kill();
    Err(format!(
        "enframe-serve did not answer on port {PORT} within {}s",
        READY_TIMEOUT.as_secs()
    ))
}
