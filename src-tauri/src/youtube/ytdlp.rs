//! Minimal yt-dlp JSON runner for the YouTube browser (search fallback and,
//! later, trending/channel/playlist feeds). Follows the same recipe as
//! `mpv::ytdlp_resolver`: hidden console window, piped output drained on
//! reader threads, hard deadline with kill.

use log::info;
use serde_json::Value;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::AppHandle;

const YTDLP_JSON_TIMEOUT: Duration = Duration::from_secs(45);

fn quiet_command(program: &str) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    command
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YtdlpStatus {
    pub(crate) version: String,
    pub(crate) path: String,
    /// True when running from a native Python module rather than the
    /// bundled x64 executable.
    pub(crate) native: bool,
}

/// Reports which yt-dlp is in use and its version (shown in Settings).
#[tauri::command]
pub(crate) async fn youtube_ytdlp_status(app: AppHandle) -> Result<YtdlpStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = crate::mpv::resolve_ytdlp_settings(&app);
        let path = settings
            .binary
            .path
            .ok_or_else(|| "yt-dlp is not available".to_string())?;
        let output = crate::mpv::ytdlp_base_command(&path)
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .map_err(|error| format!("yt-dlp failed to start: {error}"))?;
        Ok(YtdlpStatus {
            version: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            native: path.to_ascii_lowercase().ends_with("python.exe"),
            path,
        })
    })
    .await
    .map_err(|error| format!("yt-dlp status worker failed: {error}"))?
}

/// Updates yt-dlp in place: `pip install -U` for a native Python install,
/// `yt-dlp -U` for the bundled executable.
#[tauri::command]
pub(crate) async fn youtube_ytdlp_update(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = crate::mpv::resolve_ytdlp_settings(&app);
        let path = settings
            .binary
            .path
            .ok_or_else(|| "yt-dlp is not available".to_string())?;
        let native = path.to_ascii_lowercase().ends_with("python.exe");
        let mut command = quiet_command(&path);
        if native {
            command
                .arg("-m")
                .arg("pip")
                .arg("install")
                .arg("-U")
                .arg("yt-dlp");
        } else {
            command.arg("-U");
        }
        let output = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| format!("Update failed to start: {error}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(stderr.trim().chars().take(200).collect());
        }
        // pip/yt-dlp are chatty; report a short outcome rather than the log.
        let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
        let message = if stdout.contains("already satisfied")
            || stdout.contains("is up to date")
            || stdout.contains("up-to-date")
        {
            "Already up to date"
        } else if stdout.contains("successfully installed")
            || stdout.contains("updated to")
            || stdout.contains("downloading")
        {
            "Updated to the latest version"
        } else {
            "Update check finished"
        };
        Ok(message.to_string())
    })
    .await
    .map_err(|error| format!("yt-dlp update worker failed: {error}"))?
}

/// Runs `yt-dlp --version` and waits for it. The very first yt-dlp spawn on
/// a machine pays antivirus scanning + x64-emulation warm-up (tens of
/// seconds); the warm-up command runs this off the critical path so a later
/// search fallback or playback resolve doesn't.
pub(super) fn run_version(ytdl_path: &str) -> Result<(), String> {
    let mut child = crate::mpv::ytdlp_base_command(ytdl_path)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("yt-dlp failed to start: {error}"))?;
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        if child
            .try_wait()
            .map_err(|error| format!("yt-dlp wait failed: {error}"))?
            .is_some()
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("yt-dlp warm-up timed out".to_string());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Runs `yt-dlp --dump-single-json --flat-playlist <extra_args…> <target>`
/// with the user's cookie/proxy settings applied, returning the parsed JSON.
pub(super) fn run_flat_json(app: &AppHandle, target: &str) -> Result<Value, String> {
    run_flat_json_with(app, target, &[])
}

pub(super) fn run_flat_json_with(
    app: &AppHandle,
    target: &str,
    extra_args: &[String],
) -> Result<Value, String> {
    let settings = crate::mpv::resolve_ytdlp_settings(app);
    let Some(ytdl_path) = settings.binary.path else {
        return Err("yt-dlp is not available".to_string());
    };
    let proxy_url = crate::network::proxy::current_proxy_key(app)?;

    let mut command = crate::mpv::ytdlp_base_command(&ytdl_path);
    command
        .arg("--dump-single-json")
        .arg("--flat-playlist")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(proxy_url) = proxy_url.as_deref() {
        command.arg("--proxy").arg(proxy_url);
    }
    if let Some(browser) = settings.cookies.browser.as_deref() {
        command.arg("--cookies-from-browser").arg(browser);
    }
    for arg in extra_args {
        command.arg(arg);
    }
    command.arg(target);

    info!("youtube: yt-dlp flat-json for {target}");
    let mut child = command
        .spawn()
        .map_err(|error| format!("yt-dlp failed to start: {error}"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "yt-dlp stdout pipe is unavailable".to_string())?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "yt-dlp stderr pipe is unavailable".to_string())?;
    let stdout_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });

    let deadline = Instant::now() + YTDLP_JSON_TIMEOUT;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("yt-dlp wait failed: {error}"))?
        {
            let stdout = stdout_reader
                .join()
                .map_err(|_| "yt-dlp stdout reader panicked".to_string())?
                .map_err(|error| format!("yt-dlp stdout read failed: {error}"))?;
            let stderr = stderr_reader
                .join()
                .map_err(|_| "yt-dlp stderr reader panicked".to_string())?
                .map_err(|error| format!("yt-dlp stderr read failed: {error}"))?;
            if !status.success() {
                let stderr = String::from_utf8_lossy(&stderr);
                return Err(format!(
                    "yt-dlp exited with status {}: {}",
                    status,
                    stderr.trim().chars().take(300).collect::<String>()
                ));
            }
            return serde_json::from_slice(&stdout)
                .map_err(|error| format!("yt-dlp returned invalid JSON: {error}"));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(format!(
                "yt-dlp timed out after {}s",
                YTDLP_JSON_TIMEOUT.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
