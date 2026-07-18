use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{
    build_load_file_command_args_with_options, json_value_to_string, mpv_command_checked,
    mpv_set_option_string_checked, with_mpv, AppState, OpenFileState,
};

#[tauri::command]
pub(crate) fn mpv_run_command(
    state: tauri::State<'_, AppState>,
    args: Vec<serde_json::Value>,
) -> Result<(), String> {
    let args_owned: Vec<String> = args
        .iter()
        .cloned()
        .map(json_value_to_string)
        .collect::<Result<Vec<_>, _>>()?;
    let args_str: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();
    with_mpv(&state, |mpv_guard| {
        mpv_command_checked(mpv_guard, &args_str)
    })
}

#[tauri::command]
pub(crate) fn mpv_set_option_string(
    state: tauri::State<'_, AppState>,
    name: String,
    value: serde_json::Value,
) -> Result<(), String> {
    let value_str = json_value_to_string(value)?;
    with_mpv(&state, |mpv_guard| {
        mpv_set_option_string_checked(mpv_guard, &name, &value_str)
    })
}

/// Read an mpv property as a string. Returns `None` when the property is
/// currently unavailable (e.g. no file loaded, or metadata not yet produced)
/// so callers can treat "not ready" differently from a hard error.
#[tauri::command]
pub(crate) fn mpv_get_property_string(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<Option<String>, String> {
    with_mpv(&state, |mpv_guard| Ok(mpv_guard.get_property_string(&name).ok()))
}

/// Read a local image file and return it as a base64 `data:` URL so the webview
/// can display it without the asset protocol. Used for Favourites thumbnails.
/// Returns `None` for missing files or non-image extensions.
#[tauri::command]
pub(crate) fn read_image_data_url(path: String) -> Result<Option<String>, String> {
    use base64::Engine as _;

    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let file_path = std::path::Path::new(trimmed);
    let mime = match file_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => return Ok(None),
    };
    if !file_path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(file_path).map_err(|error| error.to_string())?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(Some(format!("data:{mime};base64,{encoded}")))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoadFilePayload {
    url: String,
    resume_position: Option<f64>,
    auto_play: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoadFileResult {
    title: Option<String>,
    is_live_playback: bool,
}

fn resume_playback(mpv_guard: &crate::mpv::MpvHandle) -> Result<(), String> {
    mpv_command_checked(mpv_guard, &["set", "pause", "no"])
}

fn restart_from_beginning_after_eof(mpv_guard: &crate::mpv::MpvHandle) -> Result<(), String> {
    mpv_command_checked(mpv_guard, &["seek", "0", "absolute", "exact"])?;
    resume_playback(mpv_guard)
}

fn escape_mpv_load_option_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace(',', "\\,")
}

#[tauri::command]
pub(crate) async fn load_file(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    payload: LoadFilePayload,
) -> Result<LoadFileResult, String> {
    let resume_position = payload.resume_position.unwrap_or(0.0);
    let auto_play = payload.auto_play.unwrap_or(true);
    let resolved_media = crate::mpv::try_resolve_with_ytdlp(&app, &payload.url).await;
    let playback_url = resolved_media
        .as_ref()
        .map(|resolved| resolved.url.as_str())
        .unwrap_or(&payload.url);
    let mut load_options = vec![];
    if let Some(title) = resolved_media
        .as_ref()
        .and_then(|resolved| resolved.title.as_deref())
        .map(str::trim)
        .filter(|title| !title.is_empty())
    {
        load_options.push(format!(
            "force-media-title={}",
            escape_mpv_load_option_value(title)
        ));
    }
    let command_args =
        build_load_file_command_args_with_options(&playback_url, resume_position, &load_options);
    let command_refs: Vec<&str> = command_args.iter().map(String::as_str).collect();
    with_mpv(&state, |mpv_guard| {
        mpv_command_checked(mpv_guard, &command_refs)?;
        mpv_command_checked(
            mpv_guard,
            &["set", "pause", if auto_play { "no" } else { "yes" }],
        )?;
        Ok(())
    })?;
    Ok(LoadFileResult {
        title: resolved_media
            .as_ref()
            .and_then(|resolved| resolved.title.clone()),
        is_live_playback: resolved_media
            .as_ref()
            .map(|resolved| resolved.is_live_playback)
            .unwrap_or(false),
    })
}

#[tauri::command]
pub(crate) fn cycle_pause(state: tauri::State<'_, AppState>) -> Result<(), String> {
    with_mpv(&state, |mpv_guard| {
        if mpv_guard.eof_reached() {
            return restart_from_beginning_after_eof(mpv_guard);
        }

        mpv_command_checked(mpv_guard, &["cycle", "pause"])
    })?;
    Ok(())
}

#[tauri::command]
pub(crate) fn seek_video(state: tauri::State<'_, AppState>, position: f64) -> Result<(), String> {
    let position_str = position.to_string();
    with_mpv(&state, |mpv_guard| {
        mpv_command_checked(mpv_guard, &["seek", &position_str, "absolute"])?;

        if mpv_guard.eof_reached() {
            resume_playback(mpv_guard)?;
        }

        Ok(())
    })?;
    Ok(())
}

fn sanitize_screenshot_stem(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_control() {
                return ' ';
            }
            match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
                _ => c,
            }
        })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let stem: String = collapsed.chars().take(64).collect();
    let stem = stem.trim().trim_end_matches('.').to_string();
    if stem.is_empty() {
        "Screenshot".to_string()
    } else {
        stem
    }
}

fn format_screenshot_position(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    format!(
        "{:02}.{:02}.{:02}",
        total / 3600,
        (total % 3600) / 60,
        total % 60
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScreenshotResult {
    path: String,
    file_name: String,
}

#[tauri::command]
pub(crate) async fn take_screenshot(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    include_subtitles: Option<bool>,
) -> Result<ScreenshotResult, String> {
    use tauri::Manager;

    // A user-configured folder (Settings → Playback → Screenshot Folder) takes
    // precedence. This lets users avoid OneDrive-redirected Pictures folders,
    // where mpv's screenshot writer can fail on the reparse-point path.
    let custom_dir = crate::store::ui_state_store::load_setting_value(&app, "SCREENSHOT_DIR")
        .ok()
        .flatten()
        .map(PathBuf::from);

    let target_dir = match custom_dir {
        Some(dir) => dir,
        None => app
            .path()
            .picture_dir()
            .map(|dir| dir.join("Lumo Screenshots"))
            .or_else(|_| {
                app.path()
                    .app_local_data_dir()
                    .map(|dir| dir.join("screenshots"))
            })
            .map_err(|error| format!("Failed to resolve screenshot directory: {error}"))?,
    };
    std::fs::create_dir_all(&target_dir)
        .map_err(|error| format!("Failed to create screenshot folder: {error}"))?;

    let mode = if include_subtitles.unwrap_or(true) {
        "subtitles"
    } else {
        "video"
    };
    let mpv_player = state.mpv_player.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mpv_guard = mpv_player.lock().map_err(|error| error.to_string())?;
        let title = mpv_guard
            .get_property_string("media-title")
            .unwrap_or_default();
        let position = mpv_guard
            .get_property_string("time-pos")
            .ok()
            .and_then(|raw| raw.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        let stem = format!(
            "{} {}",
            sanitize_screenshot_stem(&title),
            format_screenshot_position(position)
        );

        // Prefer lossless PNG, but some libmpv/ffmpeg builds ship without the
        // PNG encoder (screenshot-to-file then fails with "Could not open
        // libavcodec encoder"). Fall back to JPG, which these builds support.
        let mut last_error = "Failed to write screenshot".to_string();
        for extension in ["png", "jpg"] {
            let mut file_path = target_dir.join(format!("{stem}.{extension}"));
            let mut counter = 2;
            while file_path.exists() {
                file_path = target_dir.join(format!("{stem} ({counter}).{extension}"));
                counter += 1;
            }
            let file_path_str = file_path.to_string_lossy().to_string();
            match mpv_command_checked(&mpv_guard, &["screenshot-to-file", &file_path_str, mode]) {
                Ok(()) => {
                    let file_name = file_path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| file_path_str.clone());
                    return Ok(ScreenshotResult {
                        path: file_path_str,
                        file_name,
                    });
                }
                Err(error) => {
                    // Remove any empty/partial file the failed attempt may have left.
                    let _ = std::fs::remove_file(&file_path);
                    last_error = error;
                }
            }
        }
        Err(last_error)
    })
    .await
    .map_err(|error| format!("Screenshot task failed: {error}"))?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnhanceSuggestion {
    brightness: f64,
    contrast: f64,
    saturation: f64,
    temperature: f64,
    tint: f64,
}

// "Auto Enhance": sample the current frame, analyse its histogram + colour
// balance, and return balanced correction values. Auto-levels (contrast),
// exposure (brightness), a dull-frame saturation bump, and gray-world white
// balance (temperature/tint). Values are in the -100..100 slider space.
#[tauri::command]
pub(crate) async fn analyze_frame_for_enhance(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<EnhanceSuggestion, String> {
    use tauri::Manager;

    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    let sample = cache_dir.join("enhance_sample.jpg");
    let sample_str = sample.to_string_lossy().to_string();

    let mpv_player = state.mpv_player.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _ = std::fs::remove_file(&sample_str);
        {
            let mpv_guard = mpv_player.lock().map_err(|e| e.to_string())?;
            mpv_command_checked(
                &mpv_guard,
                &["screenshot-to-file", &sample_str, "video"],
            )?;
        }

        let file = std::fs::File::open(&sample_str)
            .map_err(|e| format!("Could not read frame sample: {e}"))?;
        let decoder = image::codecs::jpeg::JpegDecoder::new(std::io::BufReader::new(file))
            .map_err(|e| format!("Could not decode frame: {e}"))?;
        let rgb = image::DynamicImage::from_decoder(decoder)
            .map_err(|e| format!("Could not read frame pixels: {e}"))?
            .to_rgb8();
        let _ = std::fs::remove_file(&sample_str);

        let (w, h) = rgb.dimensions();
        if w == 0 || h == 0 {
            return Err("Empty frame".to_string());
        }
        // Subsample to ~150k pixels for speed.
        let total = (w as u64) * (h as u64);
        let step = ((total / 150_000).max(1)) as usize;

        let mut hist = [0u32; 256];
        let (mut sum_r, mut sum_g, mut sum_b, mut sum_sat) = (0f64, 0f64, 0f64, 0f64);
        let mut count = 0u64;
        for (i, px) in rgb.pixels().enumerate() {
            if i % step != 0 {
                continue;
            }
            let (r, g, b) = (px[0] as f64, px[1] as f64, px[2] as f64);
            let luma = 0.299 * r + 0.587 * g + 0.114 * b;
            hist[(luma.round().clamp(0.0, 255.0)) as usize] += 1;
            sum_r += r;
            sum_g += g;
            sum_b += b;
            let mx = r.max(g).max(b);
            let mn = r.min(g).min(b);
            if mx > 0.0 {
                sum_sat += (mx - mn) / mx;
            }
            count += 1;
        }
        if count == 0 {
            return Err("No pixels sampled".to_string());
        }

        let n = count as f64;
        let (mean_r, mean_g, mean_b) = (sum_r / n, sum_g / n, sum_b / n);
        let avg_sat = sum_sat / n;

        let percentile = |p: f64| -> f64 {
            let target = (n * p) as u64;
            let mut acc = 0u64;
            for (v, &c) in hist.iter().enumerate() {
                acc += c as u64;
                if acc >= target {
                    return v as f64;
                }
            }
            255.0
        };
        let black = percentile(0.01);
        let white = percentile(0.99);
        let median = percentile(0.5);
        let range = (white - black).max(1.0);

        // Balanced strength; conservative caps so a good source isn't wrecked.
        let s = 0.6;
        let contrast = (((255.0 / range) - 1.0) * 60.0 * s).clamp(0.0, 55.0);
        let brightness = ((120.0 - median) * 0.5 * s).clamp(-40.0, 40.0);
        let saturation = ((0.32 - avg_sat) * 180.0 * s).clamp(0.0, 40.0);
        let temperature = ((mean_b - mean_r) * 0.5 * s).clamp(-45.0, 45.0);
        let tint = (((mean_r + mean_b) / 2.0 - mean_g) * 0.5 * s).clamp(-40.0, 40.0);

        Ok(EnhanceSuggestion {
            brightness,
            contrast,
            saturation,
            temperature,
            tint,
        })
    })
    .await
    .map_err(|e| format!("Auto-enhance task failed: {e}"))?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeVersions {
    soia_version: String,
    mpv_version: Option<String>,
    ffmpeg_version: Option<String>,
}

fn normalize_mpv_version(value: Option<String>) -> Option<String> {
    value.map(|raw| raw.trim().to_string()).and_then(|trimmed| {
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.strip_prefix("mpv ").unwrap_or(&trimmed).to_string())
        }
    })
}

fn normalize_generic_version(value: Option<String>) -> Option<String> {
    value.map(|raw| raw.trim().to_string()).and_then(|trimmed| {
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

#[tauri::command]
pub(crate) fn get_runtime_versions(
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeVersions, String> {
    let (mpv_version, ffmpeg_version) = with_mpv(&state, |mpv_guard| {
        Ok((
            mpv_guard.get_property_string("mpv-version").ok(),
            mpv_guard.get_property_string("ffmpeg-version").ok(),
        ))
    })?;

    Ok(RuntimeVersions {
        soia_version: env!("CARGO_PKG_VERSION").to_string(),
        mpv_version: normalize_mpv_version(mpv_version),
        ffmpeg_version: normalize_generic_version(ffmpeg_version),
    })
}

fn resolve_local_media_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("file://") {
        let parsed = url::Url::parse(trimmed).ok()?;
        if parsed.scheme() == "file" {
            return parsed.to_file_path().ok();
        }
        return None;
    }
    Some(PathBuf::from(trimmed))
}

#[tauri::command]
pub(crate) fn get_media_file_size(path: String) -> Result<Option<u64>, String> {
    let Some(local_path) = resolve_local_media_path(&path) else {
        return Ok(None);
    };
    if !local_path.is_file() {
        return Ok(None);
    }
    match std::fs::metadata(local_path) {
        Ok(metadata) => Ok(Some(metadata.len())),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub(crate) fn list_local_media_siblings(path: String) -> Result<Vec<String>, String> {
    let Some(local_path) = resolve_local_media_path(&path) else {
        return Ok(Vec::new());
    };
    if !local_path.is_file() {
        return Ok(Vec::new());
    }
    let Some(parent) = local_path.parent() else {
        return Ok(Vec::new());
    };

    let mut files: Vec<PathBuf> = std::fs::read_dir(parent)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|entry_path| entry_path.is_file())
        .collect();

    files.sort_by(|left, right| {
        let left_name = left
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_lowercase();
        let right_name = right
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_lowercase();
        left_name.cmp(&right_name)
    });

    Ok(files
        .into_iter()
        .map(|entry_path| entry_path.to_string_lossy().into_owned())
        .collect())
}

#[tauri::command]
pub(crate) fn consume_pending_open_files(
    state: tauri::State<'_, OpenFileState>,
) -> Result<Vec<String>, String> {
    let mut pending = state.pending_paths.lock().map_err(|e| e.to_string())?;
    Ok(std::mem::take(&mut *pending))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResolveYoutubePlaylistPayload {
    url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResolvedYoutubePlaylistEntry {
    url: String,
    title: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResolvedYoutubePlaylist {
    playlist_title: Option<String>,
    entries: Vec<ResolvedYoutubePlaylistEntry>,
}

#[tauri::command]
pub(crate) async fn resolve_youtube_playlist(
    app: tauri::AppHandle,
    payload: ResolveYoutubePlaylistPayload,
) -> Result<ResolvedYoutubePlaylist, String> {
    let resolved = crate::mpv::resolve_ytdlp_playlist(&app, &payload.url).await?;
    Ok(ResolvedYoutubePlaylist {
        playlist_title: resolved.title,
        entries: resolved
            .entries
            .into_iter()
            .map(|entry| ResolvedYoutubePlaylistEntry {
                url: entry.url,
                title: entry.title,
            })
            .collect(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParsePlaylistFilePayload {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParsePlaylistSourcePayload {
    source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParsedPlaylistEntry {
    path: String,
    title: Option<String>,
    icon: Option<String>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParsedPlaylistMetadata {
    has_end_list: bool,
    playlist_type: Option<String>,
    target_duration: Option<f64>,
    has_hls_tags: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParsedPlaylistFile {
    entries: Vec<ParsedPlaylistEntry>,
    metadata: ParsedPlaylistMetadata,
}

enum PlaylistBase<'a> {
    Local(&'a Path),
    Remote(url::Url),
}

fn is_absolute_local_path(candidate: &str) -> bool {
    let path = Path::new(candidate);
    if path.is_absolute() {
        return true;
    }
    let bytes = candidate.as_bytes();
    bytes.len() >= 3
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
        && bytes[0].is_ascii_alphabetic()
}

fn resolve_playlist_entry_path(base: &PlaylistBase<'_>, raw: &str) -> Option<String> {
    let candidate = raw.trim();
    if candidate.is_empty() {
        return None;
    }
    if let Ok(parsed) = url::Url::parse(candidate) {
        if !parsed.scheme().is_empty() {
            return Some(candidate.to_string());
        }
    }
    if is_absolute_local_path(candidate) {
        return Some(candidate.to_string());
    }
    match base {
        PlaylistBase::Local(base_dir) => {
            Some(base_dir.join(candidate).to_string_lossy().into_owned())
        }
        PlaylistBase::Remote(base_url) => {
            base_url.join(candidate).ok().map(|item| item.to_string())
        }
    }
}

fn parse_extinf_attribute(line: &str, key: &str) -> Option<String> {
    let key_prefix = format!("{}=", key);
    let lower_line = line.to_ascii_lowercase();
    let start = lower_line.find(&key_prefix)? + key_prefix.len();
    let rest = &line[start..];
    let value = if let Some(stripped) = rest.strip_prefix('"') {
        let end = stripped.find('"')?;
        &stripped[..end]
    } else {
        rest.split_whitespace().next().unwrap_or_default()
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_extinf_title(line: &str) -> Option<String> {
    let mut in_quotes = false;
    let mut comma_index = None;
    for (index, ch) in line.char_indices() {
        if ch == '"' {
            in_quotes = !in_quotes;
            continue;
        }
        if ch == ',' && !in_quotes {
            comma_index = Some(index);
            break;
        }
    }
    let comma_index = comma_index?;
    let title = line[(comma_index + 1)..].trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn parse_extinf_icon(line: &str, base: &PlaylistBase<'_>) -> Option<String> {
    parse_extinf_attribute(line, "tvg-logo")
        .or_else(|| parse_extinf_attribute(line, "logo"))
        .and_then(|raw| resolve_playlist_entry_path(base, &raw))
}

fn parse_playlist_type(line: &str) -> Option<String> {
    let value = line.split_once(':')?.1.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_ascii_uppercase())
    }
}

fn parse_target_duration(line: &str) -> Option<f64> {
    line.split_once(':')?.1.trim().parse::<f64>().ok()
}

fn parse_m3u_playlist(content: &str, base: &PlaylistBase<'_>) -> ParsedPlaylistFile {
    let mut entries = Vec::new();
    let mut pending_title: Option<String> = None;
    let mut pending_icon: Option<String> = None;
    let mut metadata = ParsedPlaylistMetadata::default();

    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim().trim_start_matches('\u{feff}');
        if trimmed.is_empty() {
            log::debug!("playlist parse: skip empty line {}", index + 1);
            continue;
        }
        if let Some(stripped) = trimmed.strip_prefix("#EXTINF:") {
            pending_title = parse_extinf_title(stripped);
            pending_icon = parse_extinf_icon(stripped, base);
            log::debug!(
                "playlist parse: line {} extinf title={:?} icon={:?}",
                index + 1,
                pending_title,
                pending_icon
            );
            continue;
        }
        if trimmed.starts_with("#EXT-X-") {
            metadata.has_hls_tags = true;
            if trimmed.eq_ignore_ascii_case("#EXT-X-ENDLIST") {
                metadata.has_end_list = true;
            } else if trimmed
                .to_ascii_uppercase()
                .starts_with("#EXT-X-PLAYLIST-TYPE:")
            {
                metadata.playlist_type = parse_playlist_type(trimmed);
            } else if trimmed
                .to_ascii_uppercase()
                .starts_with("#EXT-X-TARGETDURATION:")
            {
                metadata.target_duration = parse_target_duration(trimmed);
            }
        }
        if trimmed.starts_with('#') {
            log::debug!("playlist parse: line {} comment/metadata", index + 1);
            continue;
        }

        let Some(path) = resolve_playlist_entry_path(base, trimmed) else {
            log::debug!("playlist parse: line {} unresolved entry", index + 1);
            continue;
        };
        let title = pending_title.take();
        let icon = pending_icon.take();
        log::debug!(
            "playlist parse: line {} entry path={} title={:?} icon={:?}",
            index + 1,
            path,
            title,
            icon
        );
        entries.push(ParsedPlaylistEntry { path, title, icon });
    }

    ParsedPlaylistFile { entries, metadata }
}

fn parse_playlist_source_inner(
    app: &tauri::AppHandle,
    source: &str,
) -> Result<ParsedPlaylistFile, String> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err("Playlist source is empty".to_string());
    }

    if let Ok(url) = url::Url::parse(trimmed) {
        if matches!(url.scheme(), "http" | "https") {
            log::info!("playlist parse: start url={}", url);
            let client = crate::network::proxy::configure_blocking_client_builder(
                app,
                reqwest::blocking::Client::builder().timeout(Duration::from_secs(20)),
            )?
            .build()
                .map_err(|e| e.to_string())?;
            let response = client.get(url.clone()).send().map_err(|e| e.to_string())?;
            let status = response.status();
            if !status.is_success() {
                log::warn!("playlist parse: fetch failed url={} status={}", url, status);
                return Err(format!("Playlist request failed: {}", status));
            }
            let content = response.text().map_err(|e| e.to_string())?;
            log::debug!(
                "playlist parse: loaded {} bytes from {}",
                content.len(),
                url
            );
            let parsed = parse_m3u_playlist(&content, &PlaylistBase::Remote(url.clone()));
            log::info!(
                "playlist parse: done url={} entries={} has_hls_tags={}",
                url,
                parsed.entries.len(),
                parsed.metadata.has_hls_tags
            );
            return Ok(parsed);
        }
    }

    let input_path = PathBuf::from(trimmed);
    log::info!(
        "playlist parse: start path={}",
        input_path.to_string_lossy()
    );
    if !input_path.is_file() {
        log::warn!(
            "playlist parse: file not found path={}",
            input_path.to_string_lossy()
        );
        return Err("Playlist file not found".to_string());
    }
    let content = std::fs::read_to_string(&input_path).map_err(|e| e.to_string())?;
    log::debug!(
        "playlist parse: loaded {} bytes from {}",
        content.len(),
        input_path.to_string_lossy()
    );
    let base_dir = input_path.parent().unwrap_or_else(|| Path::new(""));
    let parsed = parse_m3u_playlist(&content, &PlaylistBase::Local(base_dir));
    log::info!(
        "playlist parse: done path={} entries={} has_hls_tags={}",
        input_path.to_string_lossy(),
        parsed.entries.len(),
        parsed.metadata.has_hls_tags
    );
    Ok(parsed)
}

#[tauri::command]
pub(crate) fn parse_playlist_file(
    app: tauri::AppHandle,
    payload: ParsePlaylistFilePayload,
) -> Result<ParsedPlaylistFile, String> {
    parse_playlist_source_inner(&app, &payload.path)
}

#[tauri::command]
pub(crate) fn parse_playlist_source(
    app: tauri::AppHandle,
    payload: ParsePlaylistSourcePayload,
) -> Result<ParsedPlaylistFile, String> {
    parse_playlist_source_inner(&app, &payload.source)
}
