//! Lifecycle for the bundled bgutil PO-token server (Node.js). PO tokens
//! make YouTube treat our yt-dlp requests like an attested browser: no
//! stream-start throttling ramp and fewer "unavailable" videos. Everything
//! degrades silently to the tokenless path when Node or the bundle is
//! missing — the provider can only make things better.

use log::{info, warn};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager};

const POT_SERVER_ADDR: &str = "127.0.0.1:4416";
const POT_SERVER_ZIP: &str = "binaries/bgutil-pot-server.zip";

/// The server ships as one zip (NSIS/Tauri resource globs flatten deep
/// trees) and is unpacked into app data on first use.
fn installed_server_script(app: &AppHandle) -> Option<std::path::PathBuf> {
    let data_dir = app.path().app_local_data_dir().ok()?;
    let root = data_dir.join("bgutil-pot-server");
    let script = root.join("build").join("main.js");
    if script.is_file() {
        return Some(script);
    }
    let zip = app
        .path()
        .resolve(POT_SERVER_ZIP, tauri::path::BaseDirectory::Resource)
        .ok()
        .filter(|path| path.is_file())?;
    info!("pot: unpacking token server (first use)");
    let mut command = Command::new("powershell");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    let status = command
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(format!(
            "Expand-Archive -Force -LiteralPath '{}' -DestinationPath '{}'",
            zip.display(),
            root.display()
        ))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Ok(status) if status.success() && script.is_file() => Some(script),
        Ok(status) => {
            warn!("pot: unpack failed (status {status})");
            None
        }
        Err(error) => {
            warn!("pot: unpack failed: {error}");
            None
        }
    }
}

static SERVER: OnceLock<Mutex<Option<Child>>> = OnceLock::new();

fn server_slot() -> &'static Mutex<Option<Child>> {
    SERVER.get_or_init(|| Mutex::new(None))
}

fn ping() -> bool {
    let Ok(address) = POT_SERVER_ADDR.parse() else {
        return false;
    };
    let Ok(mut stream) =
        std::net::TcpStream::connect_timeout(&address, Duration::from_millis(400))
    else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(700)));
    if stream
        .write_all(b"GET /ping HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut buffer = [0u8; 32];
    matches!(stream.read(&mut buffer), Ok(read) if read > 0)
}

/// Starts the token server if it isn't already answering. Cheap to call
/// repeatedly; the yt-dlp plugin discovers it on its default port.
pub(crate) fn ensure_pot_server(app: &AppHandle) {
    if ping() {
        return;
    }
    let Ok(mut guard) = server_slot().lock() else {
        return;
    };
    if let Some(child) = guard.as_mut() {
        // Still booting (spawned but not answering yet) — leave it alone.
        if child.try_wait().ok().flatten().is_none() {
            return;
        }
    }
    let Some(script) = installed_server_script(app) else {
        info!("pot: token server not bundled; using tokenless path");
        return;
    };
    let mut command = Command::new("node");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    match command
        .arg(&script)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => {
            info!("pot: token server starting");
            *guard = Some(child);
        }
        Err(error) => {
            warn!("pot: token server unavailable (is Node installed?): {error}");
        }
    }
}

pub(crate) fn shutdown_pot_server() {
    if let Ok(mut guard) = server_slot().lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
