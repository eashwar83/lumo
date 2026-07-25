//! Lossless merge / split of media files via an external ffmpeg.
//!
//! The bundled ffmpeg is playback-only (no muxers), so — exactly like clip
//! export — these operations need a *full* ffmpeg located at runtime with
//! [`crate::mpv::find_ffmpeg`]. `ffprobe` (its usual sibling) is used to read
//! duration / chapters / stream parameters; when it's missing we degrade
//! gracefully rather than fail.
//!
//!   * **Merge** uses the concat *demuxer* (`-c copy`) when every input shares
//!     the same codec / resolution / pixel format — truly lossless and instant.
//!     When they differ, an opt-in re-encode path normalises each input with the
//!     concat *filter* so the merge still succeeds.
//!   * **Split** uses the segment muxer (`-c copy`). Cuts snap to the nearest
//!     keyframe — the inherent price of not re-encoding.

use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// ffmpeg/ffprobe are console apps: launched normally on Windows they flash a
/// console window over the video. Every spawn goes through this.
fn quiet_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    command
}

/// Locate ffprobe next to the chosen ffmpeg, else on PATH.
fn find_ffprobe(ffmpeg: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" };
    let runnable = |path: &Path| -> bool {
        quiet_command(path)
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    if let Some(dir) = ffmpeg.parent() {
        let sibling = dir.join(name);
        if sibling.exists() && runnable(&sibling) {
            return Some(sibling);
        }
    }
    let bare = PathBuf::from("ffprobe");
    if runnable(&bare) {
        return Some(bare);
    }
    None
}

/// The subset of a file's properties we need for compatibility checks and for
/// building a normalising re-encode.
#[derive(Clone, Default)]
struct MediaProbe {
    duration: f64,
    width: u32,
    height: u32,
    fps: f64,
    has_video: bool,
    has_audio: bool,
    /// Codec/resolution/pixfmt signature used to judge concat compatibility.
    signature: String,
    chapters: Vec<f64>,
}

fn probe_media(ffprobe: &Path, path: &str) -> Result<MediaProbe, String> {
    let output = quiet_command(ffprobe)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            "-show_chapters",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {e}"))?;
    if !output.status.success() {
        return Err("ffprobe could not read this file".to_string());
    }
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Invalid ffprobe output: {e}"))?;

    let mut probe = MediaProbe::default();

    probe.duration = json
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let mut vcodec = String::new();
    let mut acodec = String::new();
    let mut pix_fmt = String::new();
    let mut sample_rate = String::new();
    let mut channels = String::new();

    if let Some(streams) = json.get("streams").and_then(Value::as_array) {
        for stream in streams {
            let kind = stream
                .get("codec_type")
                .and_then(Value::as_str)
                .unwrap_or("");
            match kind {
                "video" if !probe.has_video => {
                    probe.has_video = true;
                    probe.width = stream
                        .get("width")
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as u32;
                    probe.height = stream
                        .get("height")
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as u32;
                    vcodec = stream
                        .get("codec_name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    pix_fmt = stream
                        .get("pix_fmt")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    probe.fps = stream
                        .get("r_frame_rate")
                        .and_then(Value::as_str)
                        .and_then(parse_rational)
                        .unwrap_or(0.0);
                }
                "audio" if !probe.has_audio => {
                    probe.has_audio = true;
                    acodec = stream
                        .get("codec_name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    sample_rate = stream
                        .get("sample_rate")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    channels = stream
                        .get("channels")
                        .and_then(Value::as_u64)
                        .map(|c| c.to_string())
                        .unwrap_or_default();
                }
                _ => {}
            }
        }
    }

    probe.signature = format!(
        "{vcodec}|{}x{}|{pix_fmt}|{acodec}|{sample_rate}|{channels}",
        probe.width, probe.height
    );

    if let Some(chapters) = json.get("chapters").and_then(Value::as_array) {
        for chapter in chapters {
            if let Some(start) = chapter
                .get("start_time")
                .and_then(Value::as_str)
                .and_then(|s| s.parse::<f64>().ok())
            {
                if start > 0.0 {
                    probe.chapters.push(start);
                }
            }
        }
    }

    Ok(probe)
}

/// Parse ffprobe's "30000/1001" style frame-rate strings.
fn parse_rational(s: &str) -> Option<f64> {
    let (num, den) = s.split_once('/')?;
    let num: f64 = num.parse().ok()?;
    let den: f64 = den.parse().ok()?;
    if den == 0.0 {
        None
    } else {
        Some(num / den)
    }
}

fn resolve_ffmpeg(configured: Option<&str>) -> Result<PathBuf, String> {
    crate::mpv::find_ffmpeg(configured).ok_or_else(|| {
        "This needs ffmpeg. Install it, or set its path in Settings → Advanced."
            .to_string()
    })
}

// --- info surfaced to the UI ------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaEditInfo {
    duration: f64,
    /// Chapter start times in seconds (empty if none / no ffprobe).
    chapters: Vec<f64>,
    has_video: bool,
    has_audio: bool,
    /// True when ffprobe was available and the file could be read.
    probed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MergeInspect {
    /// All inputs share codec / resolution / pixel format → losslessly mergeable.
    compatible: bool,
    /// ffprobe was available so the verdict is real (not a guess).
    probed: bool,
    /// Human-readable reason when not compatible / not verifiable.
    note: String,
}

// --- commands ---------------------------------------------------------------

const FFMPEG_PATH_SETTING: &str = "FFMPEG_PATH";

fn configured_ffmpeg(app: &tauri::AppHandle) -> Option<String> {
    crate::store::ui_state_store::load_setting_value(app, FFMPEG_PATH_SETTING)
        .ok()
        .flatten()
}

/// Duration + chapters for a file the user wants to split (chapters need
/// ffprobe; duration falls back to 0 when it isn't available).
#[tauri::command]
pub(crate) async fn media_edit_info(
    app: tauri::AppHandle,
    path: String,
) -> Result<MediaEditInfo, String> {
    let configured = configured_ffmpeg(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let ffmpeg = resolve_ffmpeg(configured.as_deref())?;
        let Some(ffprobe) = find_ffprobe(&ffmpeg) else {
            return Ok(MediaEditInfo {
                duration: 0.0,
                chapters: vec![],
                has_video: true,
                has_audio: true,
                probed: false,
            });
        };
        let probe = probe_media(&ffprobe, &path)?;
        Ok(MediaEditInfo {
            duration: probe.duration,
            chapters: probe.chapters,
            has_video: probe.has_video,
            has_audio: probe.has_audio,
            probed: true,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Judge whether a set of files can be merged losslessly (same stream params).
#[tauri::command]
pub(crate) async fn inspect_merge(
    app: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<MergeInspect, String> {
    let configured = configured_ffmpeg(&app);
    tauri::async_runtime::spawn_blocking(move || {
        if paths.len() < 2 {
            return Err("Pick at least two files to merge".to_string());
        }
        let ffmpeg = resolve_ffmpeg(configured.as_deref())?;
        let Some(ffprobe) = find_ffprobe(&ffmpeg) else {
            return Ok(MergeInspect {
                compatible: false,
                probed: false,
                note: "Couldn't verify the files match (ffprobe not found). A \
lossless merge only works if they share the same format."
                    .to_string(),
            });
        };
        let mut signatures = Vec::new();
        for path in &paths {
            signatures.push(probe_media(&ffprobe, path)?.signature);
        }
        let first = &signatures[0];
        let compatible = signatures.iter().all(|s| s == first);
        Ok(MergeInspect {
            compatible,
            probed: true,
            note: if compatible {
                String::new()
            } else {
                "These files have different formats (codec / resolution), so a \
lossless merge isn't possible. Re-encoding will combine them into one file."
                    .to_string()
            },
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Merge `paths` in order into `output`. When `reencode` is false, a lossless
/// concat-demuxer stream copy is attempted; when true, inputs are normalised and
/// re-encoded so mismatched files still combine.
#[tauri::command]
pub(crate) async fn merge_files(
    app: tauri::AppHandle,
    paths: Vec<String>,
    output: String,
    reencode: bool,
) -> Result<String, String> {
    use tauri::Manager;

    if paths.len() < 2 {
        return Err("Pick at least two files to merge".to_string());
    }
    let configured = configured_ffmpeg(&app);
    let work = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("Failed to resolve cache dir: {e}"))?
        .join("merge_work");

    tauri::async_runtime::spawn_blocking(move || {
        let ffmpeg = resolve_ffmpeg(configured.as_deref())?;
        let out = PathBuf::from(&output);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create output folder: {e}"))?;
        }
        let _ = std::fs::remove_file(&out);
        if reencode {
            let ffprobe = find_ffprobe(&ffmpeg);
            merge_reencode(&ffmpeg, ffprobe.as_deref(), &paths, &out)?;
        } else {
            merge_copy(&ffmpeg, &paths, &out, &work)?;
        }
        confirm_output(&out)?;
        Ok(output)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Split `path` at `times` (seconds, ascending) into `out_dir/<base>-NNN.<ext>`.
/// Returns the produced file paths.
#[tauri::command]
pub(crate) async fn split_file(
    app: tauri::AppHandle,
    path: String,
    out_dir: String,
    base: String,
    times: Vec<f64>,
) -> Result<Vec<String>, String> {
    let configured = configured_ffmpeg(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let ffmpeg = resolve_ffmpeg(configured.as_deref())?;
        split_segments(&ffmpeg, &path, &out_dir, &base, &times)
    })
    .await
    .map_err(|e| e.to_string())?
}

// --- ffmpeg drivers ---------------------------------------------------------

/// Escape a path for the concat demuxer list file (single-quoted).
fn concat_escape(path: &str) -> String {
    format!("file '{}'", path.replace('\'', "'\\''"))
}

fn merge_copy(
    ffmpeg: &Path,
    paths: &[String],
    output: &Path,
    work_dir: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(work_dir)
        .map_err(|e| format!("Failed to create work folder: {e}"))?;
    let list_path = work_dir.join("concat.txt");
    let list: String = paths
        .iter()
        .map(|p| concat_escape(p))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&list_path, list)
        .map_err(|e| format!("Failed to write merge list: {e}"))?;

    let mut command = quiet_command(ffmpeg);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-fflags")
        .arg("+genpts")
        .arg("-i")
        .arg(&list_path)
        .arg("-map")
        .arg("0")
        .arg("-c")
        .arg("copy");
    add_faststart_if_mp4(&mut command, output);
    command.arg(output);

    let result = run(command)?;
    let _ = std::fs::remove_dir_all(work_dir);
    if !result.0 {
        return Err(format!(
            "Lossless merge failed: {}. The files may not share the same format \
— try the re-encode option.",
            result.1
        ));
    }
    Ok(())
}

fn merge_reencode(
    ffmpeg: &Path,
    ffprobe: Option<&Path>,
    paths: &[String],
    output: &Path,
) -> Result<(), String> {
    // Probe each input to choose a common canvas; fall back to 1080p/30 when
    // ffprobe is unavailable.
    let probes: Vec<MediaProbe> = paths
        .iter()
        .map(|p| ffprobe.and_then(|fp| probe_media(fp, p).ok()).unwrap_or_default())
        .collect();

    let mut target_w = probes.iter().map(|p| p.width).max().unwrap_or(0);
    let mut target_h = probes.iter().map(|p| p.height).max().unwrap_or(0);
    if target_w == 0 || target_h == 0 {
        target_w = 1920;
        target_h = 1080;
    }
    // H.264 needs even dimensions.
    target_w += target_w % 2;
    target_h += target_h % 2;

    let mut target_fps = probes
        .iter()
        .map(|p| p.fps)
        .fold(0.0_f64, f64::max);
    if !(target_fps > 0.0) {
        target_fps = 30.0;
    }
    target_fps = target_fps.min(60.0);

    // Only interleave audio when every input actually has an audio track, so the
    // concat filter's stream counts line up. Missing ffprobe → assume present.
    let all_have_audio = probes
        .iter()
        .all(|p| p.has_audio || p.signature.is_empty());

    let mut command = quiet_command(ffmpeg);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y");
    for path in paths {
        command.arg("-i").arg(path);
    }

    let n = paths.len();
    let mut filter = String::new();
    let mut concat_inputs = String::new();
    for i in 0..n {
        filter.push_str(&format!(
            "[{i}:v:0]scale={target_w}:{target_h}:force_original_aspect_ratio=decrease,\
pad={target_w}:{target_h}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={target_fps},format=yuv420p[v{i}];"
        ));
        concat_inputs.push_str(&format!("[v{i}]"));
        if all_have_audio {
            filter.push_str(&format!(
                "[{i}:a:0]aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo[a{i}];"
            ));
            concat_inputs.push_str(&format!("[a{i}]"));
        }
    }
    if all_have_audio {
        filter.push_str(&format!("{concat_inputs}concat=n={n}:v=1:a=1[v][a]"));
    } else {
        filter.push_str(&format!("{concat_inputs}concat=n={n}:v=1:a=0[v]"));
    }

    command.arg("-filter_complex").arg(&filter);
    command.arg("-map").arg("[v]");
    if all_have_audio {
        command.arg("-map").arg("[a]");
    }
    command
        .arg("-c:v")
        .arg("libx264")
        .arg("-crf")
        .arg("18")
        .arg("-preset")
        .arg("veryfast")
        .arg("-pix_fmt")
        .arg("yuv420p");
    if all_have_audio {
        command.arg("-c:a").arg("aac").arg("-b:a").arg("192k");
    }
    add_faststart_if_mp4(&mut command, output);
    command.arg(output);

    let result = run(command)?;
    if !result.0 {
        return Err(format!("Re-encode merge failed: {}", result.1));
    }
    Ok(())
}

fn split_segments(
    ffmpeg: &Path,
    input: &str,
    out_dir: &str,
    base: &str,
    times: &[f64],
) -> Result<Vec<String>, String> {
    let mut cuts: Vec<f64> = times.iter().copied().filter(|t| *t > 0.0).collect();
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    cuts.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    if cuts.is_empty() {
        return Err("No valid split points".to_string());
    }

    let dir = PathBuf::from(out_dir);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create output folder: {e}"))?;

    let ext = Path::new(input)
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
        .unwrap_or("mp4")
        .to_lowercase();
    // '%' is the only printf-special character in the segment pattern.
    let safe_base = base.replace('%', "_");
    let pattern = dir.join(format!("{safe_base}-%03d.{ext}"));

    let segment_times = cuts
        .iter()
        .map(|t| format!("{t:.3}"))
        .collect::<Vec<_>>()
        .join(",");

    let mut command = quiet_command(ffmpeg);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(input)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a:0?")
        .arg("-c")
        .arg("copy")
        .arg("-f")
        .arg("segment")
        .arg("-segment_times")
        .arg(&segment_times)
        .arg("-segment_start_number")
        .arg("1")
        .arg("-reset_timestamps")
        .arg("1")
        .arg("-avoid_negative_ts")
        .arg("make_zero")
        .arg(&pattern);

    let result = run(command)?;
    if !result.0 {
        return Err(format!("Split failed: {}", result.1));
    }

    // Collect the produced pieces in numeric order.
    let mut produced: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("Failed to read output folder: {e}"))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            name.starts_with(&format!("{safe_base}-"))
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case(&ext))
                    .unwrap_or(false)
        })
        .collect();
    produced.sort();
    if produced.is_empty() {
        return Err("Split produced no output".to_string());
    }
    Ok(produced
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

fn add_faststart_if_mp4(command: &mut Command, output: &Path) {
    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if matches!(ext.as_str(), "mp4" | "m4v" | "mov") {
        command.arg("-movflags").arg("+faststart");
    }
}

/// Run a command; return (success, last stderr line).
fn run(mut command: Command) -> Result<(bool, String), String> {
    let output = command
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {e}"))?;
    if output.status.success() {
        return Ok((true, String::new()));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("unknown error")
        .trim()
        .to_string();
    Ok((false, detail))
}

fn confirm_output(output: &Path) -> Result<(), String> {
    match std::fs::metadata(output) {
        Ok(meta) if meta.len() > 0 => Ok(()),
        _ => {
            let _ = std::fs::remove_file(output);
            Err("ffmpeg produced no output".to_string())
        }
    }
}
