//! YouTube download queue: up to two concurrent yt-dlp workers, JSON-line
//! progress parsing, retry with backoff, pause/resume via --continue, and a
//! library handoff (finished files appear in Recent). Queue state persists
//! to youtube_downloads.json; in-flight items resume as "paused" on launch.

use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const DEFAULT_CONCURRENT: usize = 2;
const MAX_RETRIES: u32 = 5;
const RETRY_BACKOFF: Duration = Duration::from_secs(20);
const QUEUE_FILE_NAME: &str = "youtube_downloads.json";
const UPDATE_EVENT: &str = "youtube_download_update";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct DownloadItem {
    pub(crate) id: String,
    pub(crate) url: String,
    pub(crate) title: String,
    /// queued | downloading | paused | failed | done | cancelled
    pub(crate) status: String,
    pub(crate) progress_percent: f64,
    pub(crate) speed_bps: f64,
    pub(crate) eta_seconds: f64,
    pub(crate) dest_dir: String,
    pub(crate) file_path: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) retries: u32,
    pub(crate) added_at: i64,
    // Download options (kept so retry/resume re-runs identically).
    pub(crate) quality_max_height: Option<u32>,
    pub(crate) container: String,
    pub(crate) audio_only: bool,
    pub(crate) audio_format: String,
    pub(crate) embed_subs: bool,
    pub(crate) sub_langs: String,
    pub(crate) embed_thumbnail: bool,
    pub(crate) embed_chapters: bool,
    /// Non-fatal outcome of the subtitle pass ("Saved subtitles", or why
    /// not) — the video is already complete either way.
    pub(crate) subtitle_note: Option<String>,
}

impl Default for DownloadItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            url: String::new(),
            title: String::new(),
            status: "queued".to_string(),
            progress_percent: 0.0,
            speed_bps: 0.0,
            eta_seconds: 0.0,
            dest_dir: String::new(),
            file_path: None,
            error: None,
            retries: 0,
            added_at: 0,
            quality_max_height: Some(1080),
            container: "mp4".to_string(),
            audio_only: false,
            audio_format: "mp3".to_string(),
            embed_subs: false,
            sub_langs: "en".to_string(),
            embed_thumbnail: true,
            embed_chapters: true,
            subtitle_note: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AddDownloadPayload {
    url: String,
    title: String,
    #[serde(default)]
    quality_max_height: Option<u32>,
    #[serde(default)]
    container: Option<String>,
    #[serde(default)]
    audio_only: bool,
    #[serde(default)]
    audio_format: Option<String>,
    #[serde(default)]
    embed_subs: bool,
    #[serde(default)]
    sub_langs: Option<String>,
    #[serde(default = "default_true")]
    embed_thumbnail: bool,
    #[serde(default = "default_true")]
    embed_chapters: bool,
    #[serde(default)]
    dest_dir: Option<String>,
    /// true = jump the queue ("Download now")
    #[serde(default)]
    front: bool,
}

fn default_true() -> bool {
    true
}

static QUEUE: OnceLock<Mutex<Vec<DownloadItem>>> = OnceLock::new();
static RUNNING_PIDS: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();
static LOADED: OnceLock<()> = OnceLock::new();

fn queue() -> &'static Mutex<Vec<DownloadItem>> {
    QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

fn running_pids() -> &'static Mutex<HashMap<String, u32>> {
    RUNNING_PIDS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn queue_file(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(crate::store::storage_paths::app_data_dir(app)?.join(QUEUE_FILE_NAME))
}

fn ensure_loaded(app: &AppHandle) {
    LOADED.get_or_init(|| {
        let items: Vec<DownloadItem> = queue_file(app)
            .ok()
            .and_then(|path| crate::store::json_io::read_json_or_default(&path).ok())
            .unwrap_or_default();
        if let Ok(mut guard) = queue().lock() {
            *guard = items
                .into_iter()
                .map(|mut item| {
                    // Anything mid-flight when the app closed resumes paused.
                    if item.status == "downloading" || item.status == "queued" {
                        item.status = "paused".to_string();
                    }
                    item
                })
                .collect();
        }
    });
}

fn persist(app: &AppHandle) {
    let Ok(guard) = queue().lock() else { return };
    let items = guard.clone();
    drop(guard);
    if let Ok(path) = queue_file(app) {
        let _ = crate::store::json_io::write_json(&path, &items);
    }
}

fn emit_item(app: &AppHandle, item: &DownloadItem) {
    let _ = app.emit(UPDATE_EVENT, item.clone());
}

fn update_item<F: FnOnce(&mut DownloadItem)>(
    app: &AppHandle,
    id: &str,
    mutate: F,
) -> Option<DownloadItem> {
    let mut guard = queue().lock().ok()?;
    let item = guard.iter_mut().find(|item| item.id == id)?;
    mutate(item);
    let snapshot = item.clone();
    drop(guard);
    emit_item(app, &snapshot);
    Some(snapshot)
}

fn setting(app: &AppHandle, label: &str) -> Option<String> {
    crate::store::ui_state_store::load_setting_value(app, label)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Settings → YouTube → Download Folder, falling back to Videos\YouTube.
fn default_dest_dir(app: &AppHandle) -> String {
    if let Some(configured) = setting(app, "YOUTUBE_DOWNLOAD_DIR") {
        return configured;
    }
    app.path()
        .video_dir()
        .map(|dir| dir.join("YouTube"))
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "YouTube".to_string())
}

fn max_concurrent(app: &AppHandle) -> usize {
    setting(app, "YOUTUBE_DOWNLOAD_CONCURRENCY")
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| (1..=4).contains(value))
        .unwrap_or(DEFAULT_CONCURRENT)
}

/// 0 (or unset) means unlimited; otherwise MB/s for yt-dlp's --limit-rate.
fn rate_limit(app: &AppHandle) -> Option<String> {
    setting(app, "YOUTUBE_DOWNLOAD_RATE_LIMIT")
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| *value > 0.0)
        .map(|value| format!("{value}M"))
}

// --- worker -----------------------------------------------------------------

/// Starts workers until the concurrency cap is reached.
fn pump(app: &AppHandle) {
    let limit = max_concurrent(app);
    loop {
        let next_id = {
            let Ok(mut guard) = queue().lock() else { return };
            let active = guard
                .iter()
                .filter(|item| item.status == "downloading")
                .count();
            if active >= limit {
                return;
            }
            let Some(item) = guard.iter_mut().find(|item| item.status == "queued") else {
                return;
            };
            item.status = "downloading".to_string();
            item.error = None;
            let snapshot = item.clone();
            drop(guard);
            emit_item(app, &snapshot);
            snapshot.id
        };
        let app_clone = app.clone();
        std::thread::spawn(move || run_download(&app_clone, &next_id));
    }
}

fn item_snapshot(id: &str) -> Option<DownloadItem> {
    queue()
        .lock()
        .ok()?
        .iter()
        .find(|item| item.id == id)
        .cloned()
}

fn build_output(item: &DownloadItem) -> String {
    format!("{}/%(title)s [%(id)s].%(ext)s", item.dest_dir.replace('\\', "/"))
}

fn run_download(app: &AppHandle, id: &str) {
    let Some(item) = item_snapshot(id) else { return };
    let settings = crate::mpv::resolve_ytdlp_settings(app);
    let Some(ytdl_path) = settings.binary.path else {
        finish_failed(app, id, "yt-dlp is not available".to_string());
        return;
    };
    let _ = std::fs::create_dir_all(&item.dest_dir);

    let mut command = crate::mpv::ytdlp_base_command(&ytdl_path);
    command
        .arg("--no-playlist")
        .arg("--newline")
        .arg("--progress-template")
        .arg("%(progress)j")
        // Progress lines name the fragment being fetched (video/audio are
        // separate streams); this prints the real file once merged/remuxed.
        .arg("--print")
        .arg("after_move:filepath")
        // --print implies quiet, which would also silence progress.
        .arg("--progress")
        .arg("--continue")
        .arg("--retries")
        .arg("5")
        .arg("--fragment-retries")
        .arg("10")
        .arg("--embed-metadata")
        .arg("-o")
        .arg(build_output(&item));

    if item.audio_only {
        command
            .arg("-x")
            .arg("--audio-format")
            .arg(&item.audio_format)
            .arg("--audio-quality")
            .arg("0");
    } else {
        let selector = match item.quality_max_height {
            Some(height) => format!("bv*[height<={height}]+ba/b[height<={height}]/bv*+ba/b"),
            None => "bv*+ba/b".to_string(),
        };
        command.arg("-f").arg(selector);
        command.arg("--remux-video").arg(&item.container);
    }
    // Subtitles are fetched in a separate pass (see fetch_subtitles): doing
    // them here would let a rate-limited caption endpoint fail the video,
    // and --ignore-errors would hide real download errors.
    if item.embed_thumbnail {
        command.arg("--embed-thumbnail");
    }
    if item.embed_chapters {
        command.arg("--embed-chapters");
    }
    settings.cookies.apply(&mut command);
    if let Some(limit) = rate_limit(app) {
        command.arg("--limit-rate").arg(limit);
    }
    if let Ok(Some(proxy)) = crate::network::proxy::current_proxy_key(app) {
        command.arg("--proxy").arg(proxy);
    }
    if let Some(ffmpeg) = crate::mpv::find_ffmpeg(
        crate::store::ui_state_store::load_setting_value(app, "FFMPEG_PATH")
            .ok()
            .flatten()
            .as_deref(),
    ) {
        command.arg("--ffmpeg-location").arg(ffmpeg);
    }
    command.arg(&item.url);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    info!("youtube dl: starting {}", item.id);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            finish_failed(app, id, format!("yt-dlp failed to start: {error}"));
            return;
        }
    };
    if let Ok(mut pids) = running_pids().lock() {
        pids.insert(id.to_string(), child.id());
    }

    // Progress lines arrive on stdout as JSON (one per --newline tick).
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    // Keep the last real ERROR; warnings (impersonation, container hints)
    // are noise and must never be shown as the failure reason.
    let stderr_reader = std::thread::spawn(move || {
        let mut error_line = String::new();
        let mut fallback = String::new();
        if let Some(stderr) = stderr {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed.starts_with("ERROR") || trimmed.contains("ERROR:") {
                    error_line = trimmed.to_string();
                } else if !trimmed.starts_with("WARNING") {
                    fallback = trimmed.to_string();
                }
            }
        }
        if error_line.is_empty() {
            fallback
        } else {
            error_line
        }
    });

    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let mut last_filename: Option<String> = None;
    let mut final_path: Option<String> = None;
    if let Some(stdout) = stdout {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(progress) = serde_json::from_str::<Value>(&line) else {
                // Not progress JSON: the --print line with the final path.
                let trimmed = line.trim();
                if trimmed.len() > 3 && std::path::Path::new(trimmed).is_absolute() {
                    final_path = Some(trimmed.to_string());
                }
                continue;
            };
            if let Some(filename) = progress.get("filename").and_then(Value::as_str) {
                last_filename = Some(filename.to_string());
            }
            let downloaded = progress
                .get("downloaded_bytes")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let total = progress
                .get("total_bytes")
                .and_then(Value::as_f64)
                .or_else(|| progress.get("total_bytes_estimate").and_then(Value::as_f64))
                .unwrap_or(0.0);
            let speed = progress.get("speed").and_then(Value::as_f64).unwrap_or(0.0);
            let eta = progress.get("eta").and_then(Value::as_f64).unwrap_or(0.0);
            if last_emit.elapsed() >= Duration::from_millis(400) {
                last_emit = Instant::now();
                update_item(app, id, |item| {
                    item.progress_percent = if total > 0.0 {
                        (downloaded / total * 100.0).clamp(0.0, 100.0)
                    } else {
                        item.progress_percent
                    };
                    item.speed_bps = speed;
                    item.eta_seconds = eta;
                });
            }
        }
    }

    let status = child.wait();
    if let Ok(mut pids) = running_pids().lock() {
        pids.remove(id);
    }
    let stderr_tail = stderr_reader.join().unwrap_or_default();

    // Pause/cancel kill the process; those statuses are already set.
    let current_status = item_snapshot(id).map(|item| item.status);
    if matches!(current_status.as_deref(), Some("paused") | Some("cancelled")) {
        persist(app);
        pump(app);
        return;
    }

    // Try every way of naming the finished file and keep the first that
    // actually exists — a reported path may be stale or mis-encoded, and
    // the id lookup also covers "already downloaded" runs that print
    // nothing at all.
    let resolved = [
        final_path.clone(),
        last_filename.clone().and_then(|name| resolve_final_file(&name)),
        find_by_video_id(&item.dest_dir, &item.url),
    ]
    .into_iter()
    .flatten()
    .find(|path| std::path::Path::new(path).is_file());
    // The finished file on disk is the only real proof of success: a
    // non-zero exit often just means an optional extra (usually subtitles,
    // rate-limited) failed.
    let succeeded = resolved.is_some();

    match status {
        _ if succeeded => {
            if item.embed_subs {
                if let Some(path) = resolved.clone() {
                    // Off the worker slot: caption endpoints rate-limit hard
                    // and the retries can take a while.
                    let app_subs = app.clone();
                    let item_subs = item.clone();
                    std::thread::spawn(move || {
                        fetch_subtitles_with_retries(&app_subs, &item_subs, &path);
                    });
                }
            }
            update_item(app, id, |item| {
                item.status = "done".to_string();
                item.progress_percent = 100.0;
                item.speed_bps = 0.0;
                item.eta_seconds = 0.0;
                item.file_path = resolved.clone();
            });
            register_in_library(app, id, resolved);
        }
        _ => {
            warn!(
                "youtube dl: {} failed (exit {:?}); stderr tail: {}",
                item.id,
                status.as_ref().map(|exit| exit.code()).unwrap_or(None),
                if stderr_tail.is_empty() {
                    "<empty>"
                } else {
                    stderr_tail.as_str()
                }
            );
            let message = if !stderr_tail.is_empty() {
                stderr_tail.chars().take(300).collect()
            } else if matches!(&status, Ok(exit) if exit.success()) {
                // Clean exit but nothing on disk to point at.
                "Finished without producing a file — check the download folder"
                    .to_string()
            } else {
                "yt-dlp exited with an error".to_string()
            };
            let snapshot = item_snapshot(id);
            let retries = snapshot.map(|item| item.retries).unwrap_or(MAX_RETRIES);
            if retries < MAX_RETRIES {
                update_item(app, id, |item| {
                    item.retries += 1;
                    item.status = "failed".to_string();
                    item.error = Some(format!(
                        "{message} — retrying ({}/{MAX_RETRIES}) in {}s",
                        retries + 1,
                        RETRY_BACKOFF.as_secs()
                    ));
                });
                let app_retry = app.clone();
                let id_retry = id.to_string();
                std::thread::spawn(move || {
                    std::thread::sleep(RETRY_BACKOFF);
                    // Only auto-requeue if the user hasn't intervened.
                    let requeued = update_item(&app_retry, &id_retry, |item| {
                        if item.status == "failed" {
                            item.status = "queued".to_string();
                        }
                    });
                    if requeued.is_some() {
                        persist(&app_retry);
                        pump(&app_retry);
                    }
                });
            } else {
                update_item(app, id, |item| {
                    item.status = "failed".to_string();
                    item.error = Some(message);
                });
            }
        }
    }
    persist(app);
    pump(app);
}

/// The output template ends with "[<video id>].<ext>", so a finished file
/// can always be found by id even when yt-dlp reported nothing.
fn find_by_video_id(dest_dir: &str, url: &str) -> Option<String> {
    let id = url
        .split(|c| c == '?' || c == '&')
        .find_map(|part| part.strip_prefix("v="))
        .or_else(|| url.rsplit('/').next())?
        .trim();
    if id.is_empty() {
        return None;
    }
    let marker = format!("[{id}]");
    let entries = std::fs::read_dir(dest_dir).ok()?;
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .find(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            name.contains(&marker)
                && !name.ends_with(".part")
                && !name.ends_with(".ytdl")
                && !name.ends_with(".srt")
                && !name.ends_with(".vtt")
                // Stream fragments carry a ".fNNN." marker.
                && !name.contains("].f")
        })
        .map(|entry| entry.path().to_string_lossy().into_owned())
}

/// Second pass that saves captions as a sidecar `.srt` beside the finished
/// video (Lumo loads sidecars automatically). Runs only after the video is
/// safely on disk and never affects the download's outcome — caption
/// endpoints rate-limit far more aggressively than media ones.
/// YouTube throttles caption requests far more aggressively than media, so
/// a 429 is common and usually temporary — retry with growing gaps, then
/// record the outcome on the item.
fn fetch_subtitles_with_retries(app: &AppHandle, item: &DownloadItem, video_path: &str) {
    const BACKOFFS: [u64; 3] = [0, 30, 120];
    for (attempt, wait) in BACKOFFS.iter().enumerate() {
        if *wait > 0 {
            std::thread::sleep(Duration::from_secs(*wait));
        }
        if subtitles_exist(video_path) {
            break;
        }
        fetch_subtitles(app, item, video_path);
        if subtitles_exist(video_path) {
            update_item(app, &item.id, |entry| {
                entry.subtitle_note = Some("subtitles saved".to_string());
            });
            persist(app);
            info!("youtube dl: subtitles saved for {}", item.id);
            return;
        }
        warn!(
            "youtube dl: subtitle attempt {} failed for {}",
            attempt + 1,
            item.id
        );
    }
    update_item(app, &item.id, |entry| {
        entry.subtitle_note =
            Some("subtitles unavailable (YouTube rate limit)".to_string());
    });
    persist(app);
}

/// True once any sidecar subtitle sits next to the video.
fn subtitles_exist(video_path: &str) -> bool {
    let path = std::path::Path::new(video_path);
    let (Some(parent), Some(stem)) = (path.parent(), path.file_stem()) else {
        return false;
    };
    let stem = stem.to_string_lossy().to_string();
    std::fs::read_dir(parent)
        .map(|entries| {
            entries.filter_map(|entry| entry.ok()).any(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                name.starts_with(&stem) && (name.ends_with(".srt") || name.ends_with(".vtt"))
            })
        })
        .unwrap_or(false)
}

fn fetch_subtitles(app: &AppHandle, item: &DownloadItem, video_path: &str) {
    let settings = crate::mpv::resolve_ytdlp_settings(app);
    let Some(ytdl_path) = settings.binary.path else {
        return;
    };
    let path = std::path::Path::new(video_path);
    let Some(stem) = path.file_stem().map(|stem| stem.to_string_lossy()) else {
        return;
    };
    let Some(parent) = path.parent() else { return };
    let output = parent.join(format!("{stem}.%(ext)s"));

    let mut command = crate::mpv::ytdlp_base_command(&ytdl_path);
    command
        .arg("--no-playlist")
        .arg("--skip-download")
        .arg("--write-subs")
        .arg("--write-auto-subs")
        .arg("--sub-langs")
        .arg(&item.sub_langs)
        .arg("--convert-subs")
        .arg("srt")
        .arg("--sleep-subtitles")
        .arg("2")
        .arg("--retries")
        .arg("5")
        .arg("--retry-sleep")
        .arg("5")
        .arg("-o")
        .arg(output)
        .arg(&item.url)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    settings.cookies.apply(&mut command);
    let _ = command.status();
}

/// Fallback when `--print` gave nothing: progress reports per-stream files
/// like "video [id].f251.webm"; the merged result drops the `.fNNN` part and
/// may carry a different extension.
fn resolve_final_file(fragment: &str) -> Option<String> {
    let path = std::path::Path::new(fragment);
    if path.is_file() {
        let name = path.file_name()?.to_string_lossy();
        if !name.contains(".f") {
            return Some(fragment.to_string());
        }
    }
    let parent = path.parent()?;
    let name = path.file_name()?.to_string_lossy().to_string();
    // Strip the ".fNNN" stream marker plus the extension.
    let stem = name
        .rfind(".f")
        .map(|index| name[..index].to_string())
        .unwrap_or_else(|| name.clone());
    for extension in ["mp4", "mkv", "webm", "m4a", "mp3", "opus"] {
        let candidate = parent.join(format!("{stem}.{extension}"));
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    path.is_file().then(|| fragment.to_string())
}

fn finish_failed(app: &AppHandle, id: &str, message: String) {
    warn!("youtube dl: {id} failed: {message}");
    update_item(app, id, |item| {
        item.status = "failed".to_string();
        item.error = Some(message);
    });
    persist(app);
    pump(app);
}

/// Finished downloads become ordinary library entries (Recent, heartable).
fn register_in_library(app: &AppHandle, id: &str, file_path: Option<String>) {
    let Some(file_path) = file_path else { return };
    let title = item_snapshot(id)
        .map(|item| item.title)
        .unwrap_or_default();
    let entry = crate::store::play_history::PlayHistoryEntry {
        id: String::new(),
        path: file_path,
        title,
        last_position: 0.0,
        duration: 0.0,
        last_played_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as i64)
            .unwrap_or(0),
        is_pinned: false,
        is_live_playback: false,
        external_audio_tracks: Vec::new(),
        external_sub_tracks: Vec::new(),
    };
    if let Err(error) = crate::store::play_history::save_play_history_entry(app, entry) {
        warn!("youtube dl: library handoff failed: {error}");
    }
}

fn kill_running(id: &str) {
    let pid = running_pids()
        .lock()
        .ok()
        .and_then(|mut pids| pids.remove(id));
    #[cfg(windows)]
    if let Some(pid) = pid {
        // Kill the whole tree: yt-dlp spawns ffmpeg children.
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .status();
    }
    #[cfg(not(windows))]
    if let Some(pid) = pid {
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status();
    }
}

#[cfg(windows)]
use std::os::windows::process::CommandExt;

// --- commands -----------------------------------------------------------------

#[tauri::command]
pub(crate) fn youtube_download_add(
    app: AppHandle,
    payload: AddDownloadPayload,
) -> Result<Vec<DownloadItem>, String> {
    ensure_loaded(&app);
    let url = payload.url.trim().to_string();
    if url.is_empty() {
        return Err("Missing download URL".to_string());
    }
    let item = DownloadItem {
        id: format!(
            "dl_{}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_millis())
                .unwrap_or(0),
            queue().lock().map(|guard| guard.len()).unwrap_or(0)
        ),
        url,
        title: payload.title.trim().to_string(),
        status: "queued".to_string(),
        added_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as i64)
            .unwrap_or(0),
        quality_max_height: payload.quality_max_height,
        container: payload.container.unwrap_or_else(|| "mp4".to_string()),
        audio_only: payload.audio_only,
        audio_format: payload.audio_format.unwrap_or_else(|| "mp3".to_string()),
        embed_subs: payload.embed_subs,
        sub_langs: payload
            .sub_langs
            .unwrap_or_else(|| "en".to_string()),
        embed_thumbnail: payload.embed_thumbnail,
        embed_chapters: payload.embed_chapters,
        dest_dir: payload
            .dest_dir
            .filter(|dir| !dir.trim().is_empty())
            .unwrap_or_else(|| default_dest_dir(&app)),
        ..Default::default()
    };
    {
        let mut guard = queue().lock().map_err(|_| "Queue lock poisoned")?;
        if payload.front {
            guard.insert(0, item.clone());
        } else {
            guard.push(item.clone());
        }
    }
    emit_item(&app, &item);
    persist(&app);
    pump(&app);
    youtube_download_list(app)
}

#[tauri::command]
pub(crate) fn youtube_download_list(app: AppHandle) -> Result<Vec<DownloadItem>, String> {
    ensure_loaded(&app);
    Ok(queue().lock().map_err(|_| "Queue lock poisoned")?.clone())
}

#[tauri::command]
pub(crate) fn youtube_download_pause(app: AppHandle, id: String) -> Result<(), String> {
    update_item(&app, &id, |item| {
        if item.status == "downloading" || item.status == "queued" {
            item.status = "paused".to_string();
            item.speed_bps = 0.0;
        }
    });
    kill_running(&id);
    persist(&app);
    pump(&app);
    Ok(())
}

#[tauri::command]
pub(crate) fn youtube_download_resume(app: AppHandle, id: String) -> Result<(), String> {
    update_item(&app, &id, |item| {
        if item.status == "paused" || item.status == "failed" {
            item.status = "queued".to_string();
            item.error = None;
        }
    });
    persist(&app);
    pump(&app);
    Ok(())
}

#[tauri::command]
pub(crate) fn youtube_download_cancel(app: AppHandle, id: String) -> Result<(), String> {
    update_item(&app, &id, |item| {
        item.status = "cancelled".to_string();
        item.speed_bps = 0.0;
    });
    kill_running(&id);
    persist(&app);
    pump(&app);
    Ok(())
}

#[tauri::command]
pub(crate) fn youtube_download_remove(app: AppHandle, id: String) -> Result<Vec<DownloadItem>, String> {
    kill_running(&id);
    {
        let mut guard = queue().lock().map_err(|_| "Queue lock poisoned")?;
        guard.retain(|item| item.id != id);
    }
    persist(&app);
    pump(&app);
    youtube_download_list(app)
}

#[tauri::command]
pub(crate) fn youtube_download_clear_done(app: AppHandle) -> Result<Vec<DownloadItem>, String> {
    {
        let mut guard = queue().lock().map_err(|_| "Queue lock poisoned")?;
        guard.retain(|item| item.status != "done" && item.status != "cancelled");
    }
    persist(&app);
    youtube_download_list(app)
}

#[tauri::command]
pub(crate) fn youtube_download_open_folder(app: AppHandle) -> Result<(), String> {
    let dir = default_dest_dir(&app);
    let _ = std::fs::create_dir_all(&dir);
    crate::commands::persistence::open_directory(std::path::Path::new(&dir))
}
