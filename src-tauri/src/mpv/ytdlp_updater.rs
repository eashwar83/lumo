//! Keeps yt-dlp itself fresh, in the background.
//!
//! Why this exists: on 2026-08-18 YouTube walled off anonymous stream URLs
//! from the player clients yt-dlp's *stable* release used — every video
//! served ~4 MB and then 403'd. The counter-move shipped in yt-dlp's
//! *nightly* within a day; the outage on this machine lasted as long as it
//! did only because the installed yt-dlp was six weeks stale and pinned to
//! stable. yt-dlp is the arms race outsourced; consuming it with a lag
//! turns every YouTube change into days of dead playback. This check runs
//! once a day, off the startup path, and turns the next such wave into
//! "restart the app".

use log::{debug, info, warn};
use std::io::Read;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const CHANNEL_SETTING_LABEL: &str = "YTDLP_UPDATE_CHANNEL";
/// At most one attempt per day; the stamp is written even on failure so a
/// broken network doesn't retry on every launch.
const CHECK_INTERVAL: Duration = Duration::from_secs(23 * 60 * 60);
/// pip on a slow connection can genuinely take a couple of minutes.
const UPDATE_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(PartialEq)]
enum Channel {
    Nightly,
    Stable,
    Off,
}

fn channel(app: &AppHandle) -> Channel {
    match crate::store::ui_state_store::load_setting_value(app, CHANNEL_SETTING_LABEL)
        .ok()
        .flatten()
        .as_deref()
    {
        Some("Off") => Channel::Off,
        Some("Stable") => Channel::Stable,
        // Nightly by default: extractor fixes land there first, and an
        // extractor that lags is a player that doesn't play.
        _ => Channel::Nightly,
    }
}

fn stamp_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_local_data_dir()
        .ok()
        .map(|dir| dir.join("ytdlp_update_stamp"))
}

fn due(app: &AppHandle) -> bool {
    let Some(path) = stamp_path(app) else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let last = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok())
        .unwrap_or(0);
    now.saturating_sub(last) >= CHECK_INTERVAL.as_secs()
}

fn write_stamp(app: &AppHandle) {
    let Some(path) = stamp_path(app) else {
        return;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = std::fs::write(path, now.to_string());
}

/// Spawns the once-a-day check, delayed so it never competes with startup.
pub(crate) fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        let handle = app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || run(&handle)).await;
    });
}

fn run(app: &AppHandle) {
    let channel = channel(app);
    if channel == Channel::Off {
        debug!("yt-dlp updater: disabled by setting");
        return;
    }
    if !due(app) {
        debug!("yt-dlp updater: checked recently, skipping");
        return;
    }
    let settings = crate::mpv::resolve_ytdlp_settings(app);
    let Some(ytdl_path) = settings.binary.path else {
        debug!("yt-dlp updater: no yt-dlp configured");
        return;
    };

    // Attempts are stamped, not successes: a machine that is offline today
    // should not hammer PyPI on every launch, and tomorrow's check retries.
    write_stamp(app);

    let mut command;
    if ytdl_path.to_ascii_lowercase().ends_with("python.exe") {
        // pip install of the yt-dlp package. --pre is the nightly channel
        // (dev releases on PyPI). Note pip never downgrades on -U, so
        // switching the setting Nightly -> Stable keeps an installed
        // nightly until the next stable overtakes it — acceptable, and
        // better than force-reinstalling on every check.
        command = quiet_command(&ytdl_path);
        command
            .arg("-m")
            .arg("pip")
            .arg("install")
            .arg("--upgrade")
            .arg("--no-input")
            .arg("--disable-pip-version-check");
        if channel == Channel::Nightly {
            command.arg("--pre");
        }
        command.arg("yt-dlp");
    } else {
        // The standalone exe self-updates and can switch release channels.
        // A per-user install dir is writable; a system-wide one fails here
        // and is logged rather than escalated.
        command = quiet_command(&ytdl_path);
        command.arg("--update-to").arg(match channel {
            Channel::Nightly => "nightly",
            _ => "stable",
        });
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    info!("yt-dlp updater: checking for updates");
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            warn!("yt-dlp updater: failed to start: {error}");
            return;
        }
    };
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        if let Some(pipe) = stdout_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut stdout);
        }
        if let Some(pipe) = stderr_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut stderr);
        }
        (stdout, stderr)
    });

    let deadline = Instant::now() + UPDATE_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(error) => {
                warn!("yt-dlp updater: wait failed: {error}");
                break None;
            }
        }
    };
    let (stdout, stderr) = reader.join().unwrap_or_default();
    let Some(status) = status else {
        warn!(
            "yt-dlp updater: timed out after {}s",
            UPDATE_TIMEOUT.as_secs()
        );
        return;
    };

    let stdout = String::from_utf8_lossy(&stdout);
    if let Some(line) = stdout
        .lines()
        .find(|line| line.contains("Successfully installed") || line.contains("Updated yt-dlp to"))
    {
        info!("yt-dlp updater: {}", line.trim());
    } else if status.success() {
        info!("yt-dlp updater: already up to date");
    } else {
        let stderr = String::from_utf8_lossy(&stderr);
        warn!(
            "yt-dlp updater: exited with {}: {}",
            status,
            stderr.trim().chars().take(300).collect::<String>()
        );
    }
}

/// A console child spawned plainly on Windows flashes a window; every spawn
/// here goes through this. (Mirror of the resolver's private helper.)
fn quiet_command(program: &str) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    command
}
