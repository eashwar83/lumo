//! Materialises an A–B range of a YouTube video as a local file, so the
//! export pipeline built for local files — clip, GIF, AI description — can
//! run on it unchanged.
//!
//! A streamed video is two URLs stitched live by the player; ffmpeg cannot
//! consume the stitched form, and ffmpeg reading YouTube's streams directly
//! is starved (its open-ended reads are the shape YouTube now throttles —
//! a ten-second range took 160 s that way). The player already solved
//! that: its stream proxy serves those same streams at full speed in
//! bounded chunks. So the range is cut from the proxied streams of the
//! video being played — already resolved, already registered, at the
//! quality on screen — in one exact ffmpeg pass.

use log::{info, warn};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// Longer than this is a download, not an export.
const MAX_RANGE_SECS: f64 = 15.0 * 60.0;
const CUT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// The same range exported as a clip and then as a GIF should not be cut
/// twice, so a few recent ranges stay cached.
const KEEP_RANGES: usize = 5;

fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("Cache dir unavailable: {error}"))?
        .join("yt_ranges");
    std::fs::create_dir_all(&dir).map_err(|error| format!("Cache dir create failed: {error}"))?;
    Ok(dir)
}

/// A filename-safe handle for the video: the `v=` id, or the last path
/// segment for short links.
fn video_id(raw_url: &str) -> String {
    let parsed = url::Url::parse(raw_url).ok();
    let from_query = parsed.as_ref().and_then(|u| {
        u.query_pairs()
            .find(|(key, _)| key == "v")
            .map(|(_, value)| value.into_owned())
    });
    let from_path = parsed
        .as_ref()
        .and_then(|u| u.path_segments()?.last().map(str::to_string));
    from_query
        .or(from_path)
        .unwrap_or_else(|| "video".to_string())
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(24)
        .collect()
}

/// The proxied stream URLs inside what the resolver hands the player —
/// either one plain `http://127.0.0.1:<port>/stream/<token>` or an EDL
/// wrapping two of them, video first then audio.
fn proxied_streams(playback_url: &str) -> Vec<String> {
    let marker = "http://127.0.0.1:";
    let mut out = Vec::new();
    let mut rest = playback_url;
    while let Some(index) = rest.find(marker) {
        let candidate = &rest[index..];
        let end = candidate
            .find(|c: char| c == ';' || c == '"' || c.is_whitespace())
            .unwrap_or(candidate.len());
        out.push(candidate[..end].to_string());
        rest = &candidate[end..];
    }
    out
}

fn prune(dir: &Path, keep: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path != keep)
        .map(|path| {
            let mtime = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            (mtime, path)
        })
        .collect();
    // `keep` itself counts toward the cap.
    if files.len() + 1 <= KEEP_RANGES {
        return;
    }
    files.sort_by_key(|(mtime, _)| *mtime);
    let remove = files.len() + 1 - KEEP_RANGES;
    for (_, path) in files.into_iter().take(remove) {
        let _ = std::fs::remove_file(path);
    }
}

/// Cuts `[start, end]` of the YouTube video at `url` into a local MP4 and
/// returns its path. The file's own timeline runs from 0 to `end - start`.
#[tauri::command]
pub(crate) async fn youtube_fetch_range(
    app: AppHandle,
    url: String,
    start: f64,
    end: f64,
    max_height: Option<u32>,
) -> Result<String, String> {
    if !(end > start) {
        return Err("Invalid range".to_string());
    }
    if end - start > MAX_RANGE_SECS {
        return Err(format!(
            "The range is {:.0} minutes long; YouTube exports are capped at {:.0} minutes",
            (end - start) / 60.0,
            MAX_RANGE_SECS / 60.0
        ));
    }
    let dir = cache_dir(&app)?;
    let height = max_height.unwrap_or(2160).max(144);
    let target = dir.join(format!(
        "{}-{}-{}-{}p.mp4",
        video_id(&url),
        (start * 1000.0).round() as u64,
        (end * 1000.0).round() as u64,
        height
    ));
    if target.is_file() {
        info!("youtube range: cache hit {}", target.display());
        return Ok(target.to_string_lossy().into_owned());
    }

    let ffmpeg = crate::mpv::find_ffmpeg(
        crate::store::ui_state_store::load_setting_value(&app, "FFMPEG_PATH")
            .ok()
            .flatten()
            .as_deref(),
    );
    let Some(ffmpeg) = ffmpeg else {
        return Err(
            "Exporting a YouTube range needs ffmpeg — set its path in Settings → Advanced"
                .to_string(),
        );
    };

    // The video on screen is already resolved (and cached), so this is
    // instant in the common case; a cold resolve costs the usual seconds.
    let Some(resolved) = crate::mpv::try_resolve_with_ytdlp(&app, &url, Some(height)).await
    else {
        return Err("Could not resolve the video's streams".to_string());
    };
    let streams = proxied_streams(&resolved.url);
    let Some(video) = streams.first().cloned() else {
        return Err("The video's streams are not served through the proxy".to_string());
    };
    let audio = streams.get(1).cloned();

    let target_clone = target.clone();
    let dir_clone = dir.clone();
    let length = end - start;
    tauri::async_runtime::spawn_blocking(move || {
        let mut command = std::process::Command::new(&ffmpeg);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW
            command.creation_flags(0x0800_0000);
        }
        command
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-y");
        // `-ss` before each `-i` seeks the proxied stream by byte range to
        // the keyframe at or before `start`, then decodes forward to the
        // exact instant — the same accuracy as the local clip export.
        command
            .arg("-ss")
            .arg(format!("{start}"))
            .arg("-i")
            .arg(&video);
        if let Some(audio) = audio.as_deref() {
            command
                .arg("-ss")
                .arg(format!("{start}"))
                .arg("-i")
                .arg(audio);
        }
        command
            .arg("-t")
            .arg(format!("{length}"))
            .arg("-map")
            .arg("0:v:0");
        if audio.is_some() {
            command.arg("-map").arg("1:a:0?");
        } else {
            command.arg("-map").arg("0:a:0?");
        }
        command
            .arg("-c:v")
            .arg("libx264")
            .arg("-crf")
            .arg("18")
            .arg("-preset")
            .arg("veryfast")
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg("-c:a")
            .arg("aac")
            .arg("-b:a")
            .arg("192k")
            .arg("-movflags")
            .arg("+faststart")
            .arg(&target_clone)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        info!("youtube range: cutting {start:.2}-{end:.2} at <= {height}p");
        let started = Instant::now();
        let mut child = command
            .spawn()
            .map_err(|error| format!("ffmpeg failed to start: {error}"))?;
        let mut stderr_pipe = child.stderr.take();
        let reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(pipe) = stderr_pipe.as_mut() {
                let _ = pipe.read_to_end(&mut bytes);
            }
            bytes
        });
        let deadline = started + CUT_TIMEOUT;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(error) => return Err(format!("ffmpeg wait failed: {error}")),
            }
        };
        let stderr = String::from_utf8_lossy(&reader.join().unwrap_or_default()).to_string();
        let Some(status) = status else {
            let _ = std::fs::remove_file(&target_clone);
            return Err("Cutting the range timed out".to_string());
        };
        if !status.success() || !target_clone.is_file() {
            let _ = std::fs::remove_file(&target_clone);
            let reason: String = stderr
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("ffmpeg produced no file")
                .trim()
                .chars()
                .take(240)
                .collect();
            warn!("youtube range: failed: {reason}");
            return Err(reason);
        }
        prune(&dir_clone, &target_clone);
        info!(
            "youtube range: ready {} in {:.1}s",
            target_clone.display(),
            started.elapsed().as_secs_f64()
        );
        Ok(target_clone.to_string_lossy().into_owned())
    })
    .await
    .map_err(|error| format!("Range worker failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_both_proxied_streams_inside_an_edl() {
        let edl = "edl://!new_stream;!no_clip;!no_chapters;%46%http://127.0.0.1:7012/stream/01a01271-7d5e-7573-b671-d5d40b147fbb;!new_stream;!no_clip;!no_chapters;%46%http://127.0.0.1:7012/stream/01a01271-90d0-71f1-9f35-313341f14f75";
        let streams = proxied_streams(edl);
        assert_eq!(streams.len(), 2);
        assert!(streams[0].ends_with("/stream/01a01271-7d5e-7573-b671-d5d40b147fbb"));
        assert!(streams[1].ends_with("/stream/01a01271-90d0-71f1-9f35-313341f14f75"));

        let single = "http://127.0.0.1:7012/stream/01a01271-7d5e-7573-b671-d5d40b147fbb";
        assert_eq!(proxied_streams(single), vec![single.to_string()]);
    }
}
