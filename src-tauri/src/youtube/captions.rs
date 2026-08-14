//! Captions for streamed playback: list what a video offers and fetch a
//! track as an .srt that mpv can load as an external subtitle. Files are
//! cached per video+language so switching back is instant.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};

use super::ytdlp;

/// Emitted while a caption fetch is waiting out a rate limit.
pub(crate) const CAPTION_PROGRESS_EVENT: &str = "youtube://caption-progress";

/// Shared wording so the UI always points at the same fix.
pub(crate) const SIGN_IN_HINT: &str =
    "YouTube wants a signed-in account for this video. Export a cookies.txt \
     from a private browser window and set it in Settings → YouTube → \
     Cookies file.";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptionProgress {
    video_id: String,
    language: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptionTrack {
    pub(crate) code: String,
    pub(crate) name: String,
    /// Machine-generated (or auto-translated) rather than author-provided.
    pub(crate) auto: bool,
}

static TRACK_CACHE: OnceLock<Mutex<HashMap<String, Vec<CaptionTrack>>>> = OnceLock::new();

fn track_cache() -> &'static Mutex<HashMap<String, Vec<CaptionTrack>>> {
    TRACK_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn watch_url(video_id: &str) -> String {
    format!("https://www.youtube.com/watch?v={video_id}")
}

/// Lists the caption tracks a video offers, author-provided first.
#[tauri::command]
pub(crate) async fn youtube_caption_tracks(
    app: AppHandle,
    video_id: String,
) -> Result<Vec<CaptionTrack>, String> {
    let video_id = video_id.trim().to_string();
    if video_id.is_empty() {
        return Ok(Vec::new());
    }
    if let Ok(guard) = track_cache().lock() {
        if let Some(tracks) = guard.get(&video_id) {
            return Ok(tracks.clone());
        }
    }
    let cached_id = video_id.clone();
    let tracks = tauri::async_runtime::spawn_blocking(move || {
        // The metadata dump carries both caption maps without downloading.
        // --dump-single-json still runs format selection, and this call
        // wants none of it: a video whose formats are all unavailable can
        // still have its 157 caption tracks, and without the override the
        // whole dump exits 1 and the menu reads "no subtitles".
        let value = ytdlp::run_json_with(
            &app,
            &watch_url(&video_id),
            &[
                "--skip-download".to_string(),
                "--ignore-no-formats-error".to_string(),
            ],
        )?;
        let mut tracks = Vec::new();
        collect_tracks(value.get("subtitles"), false, &mut tracks);
        collect_tracks(value.get("automatic_captions"), true, &mut tracks);
        // YouTube returns these keyed by language code; order them the way
        // they read on screen, with author-provided tracks first.
        tracks.sort_by(|a, b| {
            a.auto
                .cmp(&b.auto)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok::<Vec<CaptionTrack>, String>(tracks)
    })
    .await
    .map_err(|error| format!("Caption worker failed: {error}"))??;

    if let Ok(mut guard) = track_cache().lock() {
        guard.insert(cached_id, tracks.clone());
    }
    Ok(tracks)
}

fn collect_tracks(map: Option<&Value>, auto: bool, out: &mut Vec<CaptionTrack>) {
    let Some(map) = map.and_then(Value::as_object) else {
        return;
    };
    for (code, formats) in map {
        // Live-chat replays are not subtitles.
        if code == "live_chat" {
            continue;
        }
        let name = formats
            .as_array()
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("name"))
            .and_then(Value::as_str)
            .unwrap_or(code)
            .to_string();
        out.push(CaptionTrack {
            code: code.clone(),
            name,
            auto,
        });
    }
}

/// Downloads one caption track as .srt and returns its path for `sub-add`.
#[tauri::command]
pub(crate) async fn youtube_caption_file(
    app: AppHandle,
    video_id: String,
    language: String,
) -> Result<String, String> {
    let video_id = video_id.trim().to_string();
    let language = language.trim().to_string();
    if video_id.is_empty() || language.is_empty() {
        return Err("Missing video or language".to_string());
    }

    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("Cache dir unavailable: {error}"))?
        .join("yt_captions");
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("Cache dir create failed: {error}"))?;
    let expected = dir.join(format!("{video_id}.{language}.srt"));
    if expected.is_file() {
        return Ok(expected.to_string_lossy().into_owned());
    }

    let output = dir.join(format!("{video_id}.%(ext)s"));
    let settings = crate::mpv::resolve_ytdlp_settings(&app);
    let Some(ytdl_path) = settings.binary.path else {
        return Err("yt-dlp is not available".to_string());
    };
    let url = watch_url(&video_id);
    let expected_clone = expected.clone();

    tauri::async_runtime::spawn_blocking(move || {
        // Caption endpoints rate-limit far harder than media ones, so a 429
        // on the first try is routine and usually clears within a minute.
        const BACKOFFS: [u64; 3] = [0, 30, 120];
        let mut last_stderr = String::new();
        for (attempt, wait) in BACKOFFS.iter().enumerate() {
            if *wait > 0 {
                let _ = app.emit(
                    CAPTION_PROGRESS_EVENT,
                    CaptionProgress {
                        video_id: video_id.clone(),
                        language: language.clone(),
                        message: format!(
                            "YouTube is rate-limiting captions — retrying in {wait}s ({}/{})",
                            attempt + 1,
                            BACKOFFS.len() - 1
                        ),
                    },
                );
                std::thread::sleep(std::time::Duration::from_secs(*wait));
            }

            let mut command = crate::mpv::ytdlp_base_command(&ytdl_path);
            command
                .arg("--no-playlist")
                .arg("--skip-download")
                .arg("--write-subs")
                .arg("--write-auto-subs")
                .arg("--sub-langs")
                .arg(&language)
                .arg("--convert-subs")
                .arg("srt")
                .arg("--sleep-subtitles")
                .arg("2")
                .arg("--retries")
                .arg("5")
                .arg("--retry-sleep")
                .arg("5")
                .arg("-o")
                .arg(&output)
                .arg(&url)
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            settings.cookies.apply(&mut command);
            if let Ok(result) = command.output() {
                last_stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
            }

            if let Some(path) = find_written_srt(&expected_clone, &video_id) {
                return Ok(path);
            }
            // A sign-in wall will not clear by waiting.
            if needs_sign_in(&last_stderr) {
                break;
            }
        }
        Err(caption_error(&last_stderr))
    })
    .await
    .map_err(|error| format!("Caption worker failed: {error}"))?
}

/// Locates the .srt yt-dlp wrote — it may label the file with a regional
/// variant (en-US, en-orig…) rather than the code we asked for.
fn find_written_srt(expected: &std::path::Path, video_id: &str) -> Option<String> {
    if expected.is_file() {
        return Some(expected.to_string_lossy().into_owned());
    }
    let prefix = format!("{video_id}.");
    std::fs::read_dir(expected.parent()?)
        .ok()?
        .filter_map(|entry| entry.ok())
        .find_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            (name.starts_with(&prefix) && name.ends_with(".srt"))
                .then(|| entry.path().to_string_lossy().into_owned())
        })
}

fn needs_sign_in(stderr: &str) -> bool {
    let text = stderr.to_lowercase();
    text.contains("sign in to confirm")
        || text.contains("age-restricted")
        || text.contains("cookies are no longer valid")
        || text.contains("members-only")
}

fn caption_error(stderr: &str) -> String {
    if needs_sign_in(stderr) {
        return SIGN_IN_HINT.to_string();
    }
    "YouTube didn't return that caption track — it is rate-limiting caption \
     requests. Try again in a few minutes, or set a cookies.txt file in \
     Settings → YouTube to raise the limit."
        .to_string()
}
