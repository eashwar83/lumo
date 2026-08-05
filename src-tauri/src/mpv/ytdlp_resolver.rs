use log::{info, warn};
use serde_json::Value;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use url::Url;

/// yt-dlp is a console application: spawned normally on Windows it flashes a
/// console window over the player. Every spawn must go through this.
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

/// Builds the yt-dlp base command. When the configured path is a Python
/// interpreter (native ARM64 install), yt-dlp runs as `python -m yt_dlp`
/// - same CLI, ~7x faster than the emulated x64 exe.
pub(crate) fn ytdlp_base_command(ytdl_path: &str) -> Command {
    let mut command = quiet_command(ytdl_path);
    if ytdl_path.to_ascii_lowercase().ends_with("python.exe") {
        command.arg("-m").arg("yt_dlp");
    }
    // Windows pipes default to the legacy code page, which mangles every
    // non-ASCII character in titles and file paths (yt-dlp rewrites "|" as
    // the full-width "｜") into "?" - breaking the paths we read back.
    command.env("PYTHONIOENCODING", "utf-8").env("PYTHONUTF8", "1");
    // YouTube's `n` parameter is guarded by a JS challenge that a plain JS
    // runtime cannot solve; yt-dlp needs its own solver script, which it
    // only fetches when asked. Signed-in requests (a cookies file) always
    // hit this path, and without the solver YouTube returns nothing but
    // storyboard images — "Requested format is not available" for every
    // video. yt-dlp caches the script, so this costs one download.
    if supports_remote_components(ytdl_path) {
        command.arg("--remote-components").arg("ejs:github");
    }
    command
}

/// Whether this yt-dlp knows `--remote-components` (added mid-2026). An
/// older binary would abort on the unknown option, taking every call with
/// it, so probe once per path rather than assume.
fn supports_remote_components(ytdl_path: &str) -> bool {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, bool>>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(supported) = guard.get(ytdl_path) {
            return *supported;
        }
    }

    let mut probe = quiet_command(ytdl_path);
    if ytdl_path.to_ascii_lowercase().ends_with("python.exe") {
        probe.arg("-m").arg("yt_dlp");
    }
    let supported = probe
        .arg("--help")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout).contains("--remote-components")
        })
        .unwrap_or(false);

    info!("yt-dlp: --remote-components supported = {supported}");
    if let Ok(mut guard) = cache.lock() {
        guard.insert(ytdl_path.to_string(), supported);
    }
    supported
}

const YTDLP_TIMEOUT: Duration = Duration::from_secs(60);
const DIRECT_STREAM_EXTENSIONS: &[&str] = &[
    "m3u8", "mp4", "m4v", "mov", "mkv", "webm", "flv", "avi", "ts", "mp3", "m4a", "aac", "flac",
    "wav", "ogg", "opus",
];

#[derive(Clone)]
struct Candidate {
    url: String,
    headers: Vec<(String, String)>,
    format_id: Option<String>,
    protocol: Option<String>,
    resolution: Option<String>,
    score: i64,
}

#[derive(Clone)]
pub(crate) struct ResolvedMedia {
    pub(crate) url: String,
    pub(crate) title: Option<String>,
    pub(crate) is_live_playback: bool,
}

// Resolved streams stay valid for hours; caching them makes replays and
// quality switches skip the 5-19s yt-dlp extraction entirely. The proxied
// URLs stay usable because the stream-proxy backend registry is in-memory
// and lives for the whole app session.
const RESOLVE_CACHE_TTL: Duration = Duration::from_secs(40 * 60);
const RESOLVE_CACHE_MAX: usize = 40;

static RESOLVE_CACHE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, (Instant, ResolvedMedia)>>,
> = std::sync::OnceLock::new();

fn resolve_cache(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, (Instant, ResolvedMedia)>> {
    RESOLVE_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn resolve_cache_get(key: &str) -> Option<ResolvedMedia> {
    let guard = resolve_cache().lock().ok()?;
    let (stored_at, media) = guard.get(key)?;
    // Live streams get fresh manifests each time.
    (!media.is_live_playback && stored_at.elapsed() < RESOLVE_CACHE_TTL)
        .then(|| media.clone())
}

/// Drops every cached resolution for a video. Stream URLs are tied to the
/// IP that requested them, so a VPN hop or an expiry makes them 403 — the
/// cure is to resolve again rather than replay a dead URL.
pub(crate) fn forget_resolution(raw_url: &str) {
    if let Ok(mut guard) = resolve_cache().lock() {
        guard.retain(|key, _| !key.starts_with(raw_url));
    }
}

fn resolve_cache_put(key: String, media: &ResolvedMedia) {
    if media.is_live_playback {
        return;
    }
    let Ok(mut guard) = resolve_cache().lock() else {
        return;
    };
    guard.retain(|_, (stored_at, _)| stored_at.elapsed() < RESOLVE_CACHE_TTL);
    if guard.len() >= RESOLVE_CACHE_MAX {
        if let Some(oldest) = guard
            .iter()
            .min_by_key(|(_, (stored_at, _))| *stored_at)
            .map(|(cache_key, _)| cache_key.clone())
        {
            guard.remove(&oldest);
        }
    }
    guard.insert(key, (Instant::now(), media.clone()));
}

pub(crate) struct ResolvedPlaylistEntry {
    pub(crate) url: String,
    pub(crate) title: Option<String>,
}

pub(crate) struct ResolvedPlaylist {
    pub(crate) title: Option<String>,
    pub(crate) entries: Vec<ResolvedPlaylistEntry>,
}

pub(crate) async fn resolve_playlist(
    app: &AppHandle,
    raw_url: &str,
) -> Result<ResolvedPlaylist, String> {
    let settings = super::ytdlp_settings::resolve(app);
    let Some(ytdl_path) = settings.binary.path else {
        return Err("yt-dlp is not configured".to_string());
    };

    let proxy_url = crate::network::proxy::current_proxy_key(app)?;
    let cookies = settings.cookies;
    let raw_url = raw_url.to_string();
    let had_cookies = cookies.file.is_some() || cookies.browser.is_some();
    let proxy_clone = proxy_url.clone();
    let url_clone = raw_url.clone();
    let ytdl_clone = ytdl_path.clone();
    let output = tauri::async_runtime::spawn_blocking(move || {
        run_ytdlp_playlist_command(&ytdl_path, proxy_url.as_deref(), Some(&cookies), &raw_url)
    })
    .await
    .map_err(|error| format!("yt-dlp worker failed: {error}"))??;

    let output = if !output.status.success()
        && had_cookies
        && is_cookie_permission_error(&output.stderr)
    {
        warn!(
            "yt-dlp: cookies-from-browser failed due to permission error, retrying without cookies"
        );
        tauri::async_runtime::spawn_blocking(move || {
            run_ytdlp_playlist_command(&ytdl_clone, proxy_clone.as_deref(), None, &url_clone)
        })
        .await
        .map_err(|error| format!("yt-dlp worker failed: {error}"))??
    } else {
        output
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "yt-dlp exited with status {}: {}",
            output.status,
            stderr.trim()
        ));
    }

    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("yt-dlp returned invalid JSON: {error}"))?;

    let entries = extract_playlist_entries(&value);
    if entries.is_empty() {
        return Err("yt-dlp did not return any playlist entries".to_string());
    }

    let title = extract_media_title(&value);
    info!(
        "yt-dlp: resolved {} playlist entries title={:?}",
        entries.len(),
        title
    );
    Ok(ResolvedPlaylist { title, entries })
}

fn run_ytdlp_playlist_command(
    ytdl_path: &str,
    proxy_url: Option<&str>,
    cookies: Option<&super::ytdlp_settings::YtdlpCookieSettings>,
    raw_url: &str,
) -> Result<std::process::Output, String> {
    let mut command = ytdlp_base_command(ytdl_path);
    let mut log_args = vec![
        "--dump-single-json".to_string(),
        "--flat-playlist".to_string(),
        redact_url(raw_url),
    ];
    command
        .arg("--dump-single-json")
        .arg("--flat-playlist")
        .arg(raw_url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(proxy_url) = proxy_url {
        command.arg("--proxy").arg(proxy_url);
        log_args.push("--proxy".to_string());
        log_args.push(redact_url(proxy_url));
    }

    if let Some(cookies) = cookies {
        cookies.apply(&mut command);
        log_args.extend(cookies.log_args());
    }

    info!(
        "yt-dlp: run {}",
        format_command_for_log(ytdl_path, &log_args)
    );

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
    let started_at = Instant::now();
    let deadline = Instant::now() + YTDLP_TIMEOUT;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("yt-dlp wait failed: {error}"))?
        {
            let elapsed = started_at.elapsed();
            info!("yt-dlp: playlist finished in {:.3}s", elapsed.as_secs_f64());
            let stdout = stdout_reader
                .join()
                .map_err(|_| "yt-dlp stdout reader panicked".to_string())?
                .map_err(|error| format!("yt-dlp stdout read failed: {error}"))?;
            let stderr = stderr_reader
                .join()
                .map_err(|_| "yt-dlp stderr reader panicked".to_string())?
                .map_err(|error| format!("yt-dlp stderr read failed: {error}"))?;
            let output = std::process::Output {
                status,
                stdout,
                stderr,
            };
            info!(
                "yt-dlp: playlist exit status={} stdout={}B stderr={}B",
                output.status,
                output.stdout.len(),
                output.stderr.len()
            );
            return Ok(output);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            warn!(
                "yt-dlp: playlist timed out after {:.3}s",
                started_at.elapsed().as_secs_f64()
            );
            return Err(format!(
                "yt-dlp timed out after {}s",
                YTDLP_TIMEOUT.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn extract_playlist_entries(value: &Value) -> Vec<ResolvedPlaylistEntry> {
    value
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let url = entry
                .get("url")
                .and_then(Value::as_str)
                .or_else(|| entry.get("webpage_url").and_then(Value::as_str))
                .filter(|url| !url.is_empty())?;
            let title = entry
                .get("title")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string);
            Some(ResolvedPlaylistEntry {
                url: url.to_string(),
                title,
            })
        })
        .collect()
}

pub(crate) async fn resolve(
    app: &AppHandle,
    raw_url: &str,
    max_height_override: Option<u32>,
) -> Result<Option<ResolvedMedia>, String> {
    if !is_http_url(raw_url) {
        return Ok(None);
    }
    if is_likely_direct_stream_url(raw_url) {
        return Ok(None);
    }

    let settings = super::ytdlp_settings::resolve(app);
    let Some(ytdl_path) = settings.binary.path else {
        return Ok(None);
    };

    let effective_max_height = max_height_override.unwrap_or(settings.format.max_height);
    let cache_key = format!("{raw_url}|{effective_max_height}");
    if let Some(cached) = resolve_cache_get(&cache_key) {
        info!("yt-dlp: resolve served from cache");
        return Ok(Some(cached));
    }

    let proxy_url = crate::network::proxy::current_proxy_key(app)?;
    let cookies = settings.cookies;
    let format_selector = super::ytdlp_settings::YtdlpFormatSettings {
        max_height: effective_max_height,
    }
    .selector();
    let raw_url = raw_url.to_string();
    let had_cookies = cookies.file.is_some() || cookies.browser.is_some();
    let proxy_clone = proxy_url.clone();
    let format_clone = format_selector.clone();
    let url_clone = raw_url.clone();
    let ytdl_clone = ytdl_path.clone();
    let output = tauri::async_runtime::spawn_blocking(move || {
        run_ytdlp_command(
            &ytdl_path,
            proxy_url.as_deref(),
            Some(&cookies),
            &format_selector,
            &raw_url,
        )
    })
    .await
    .map_err(|error| format!("yt-dlp worker failed: {error}"))??;
    let output = if !output.status.success()
        && had_cookies
        && is_cookie_permission_error(&output.stderr)
    {
        warn!(
            "yt-dlp: cookies-from-browser failed due to permission error, retrying without cookies"
        );
        tauri::async_runtime::spawn_blocking(move || {
            run_ytdlp_command(
                &ytdl_clone,
                proxy_clone.as_deref(),
                None,
                &format_clone,
                &url_clone,
            )
        })
        .await
        .map_err(|error| format!("yt-dlp worker failed: {error}"))??
    } else {
        output
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "yt-dlp exited with status {}: {}",
            output.status,
            stderr.trim()
        ));
    }

    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("yt-dlp returned invalid JSON: {error}"))?;
    let Some(candidate) = select_candidate(&value) else {
        return Err("yt-dlp did not return a playable URL".to_string());
    };
    log_selected_candidate("selected", &candidate);
    let is_live_playback = is_live_video(&value);
    let playback_url = proxied_candidate_url(&candidate);
    let title = extract_media_title(&value);

    info!("yt-dlp: resolved url through stream proxy");
    let resolved = ResolvedMedia {
        url: playback_url,
        title,
        is_live_playback,
    };
    resolve_cache_put(cache_key, &resolved);
    Ok(Some(resolved))
}

pub(crate) async fn try_resolve(
    app: &AppHandle,
    raw_url: &str,
    max_height_override: Option<u32>,
) -> Option<ResolvedMedia> {
    try_resolve_reporting(app, raw_url, max_height_override)
        .await
        .0
}

/// Like [`try_resolve`] but keeps the failure text. Callers that surface an
/// error to the user need it: env_logger writes to stderr, which a windowed
/// build discards, so a swallowed cause is a cause nobody can ever read.
pub(crate) async fn try_resolve_reporting(
    app: &AppHandle,
    raw_url: &str,
    max_height_override: Option<u32>,
) -> (Option<ResolvedMedia>, Option<String>) {
    match resolve(app, raw_url, max_height_override).await {
        Ok(resolved) => (resolved, None),
        Err(error) => {
            warn!("yt-dlp: resolve failed for {}: {error}", redact_url(raw_url));
            (None, Some(error))
        }
    }
}

fn run_ytdlp_command(
    ytdl_path: &str,
    proxy_url: Option<&str>,
    cookies: Option<&super::ytdlp_settings::YtdlpCookieSettings>,
    format_selector: &str,
    raw_url: &str,
) -> Result<std::process::Output, String> {
    let mut command = ytdlp_base_command(ytdl_path);
    let mut log_args = vec![
        "--dump-single-json".to_string(),
        "--no-playlist".to_string(),
        // Lets yt-dlp solve JS challenges with the system Node (deno is its
        // only default), unlocking token-attested formats; a warning-level
        // no-op when Node is absent.
        "--js-runtimes".to_string(),
        "node".to_string(),
        "-f".to_string(),
        format_selector.to_string(),
        redact_url(raw_url),
    ];
    command
        .arg("--dump-single-json")
        .arg("--no-playlist")
        .arg("--js-runtimes")
        .arg("node")
        .arg("-f")
        .arg(format_selector)
        .arg(raw_url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(proxy_url) = proxy_url {
        command.arg("--proxy").arg(proxy_url);
        log_args.push("--proxy".to_string());
        log_args.push(redact_url(proxy_url));
    }

    if let Some(cookies) = cookies {
        cookies.apply(&mut command);
        log_args.extend(cookies.log_args());
    }

    info!(
        "yt-dlp: run {}",
        format_command_for_log(ytdl_path, &log_args)
    );

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
    let started_at = Instant::now();
    let deadline = Instant::now() + YTDLP_TIMEOUT;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("yt-dlp wait failed: {error}"))?
        {
            let elapsed = started_at.elapsed();
            info!("yt-dlp: finished in {:.3}s", elapsed.as_secs_f64());
            let stdout = stdout_reader
                .join()
                .map_err(|_| "yt-dlp stdout reader panicked".to_string())?
                .map_err(|error| format!("yt-dlp stdout read failed: {error}"))?;
            let stderr = stderr_reader
                .join()
                .map_err(|_| "yt-dlp stderr reader panicked".to_string())?
                .map_err(|error| format!("yt-dlp stderr read failed: {error}"))?;
            let output = std::process::Output {
                status,
                stdout,
                stderr,
            };
            info!(
                "yt-dlp: exit status={} stdout={}B stderr={}B",
                output.status,
                output.stdout.len(),
                output.stderr.len()
            );
            return Ok(output);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            warn!(
                "yt-dlp: timed out after {:.3}s",
                started_at.elapsed().as_secs_f64()
            );
            return Err(format!("yt-dlp timed out after {}s", YTDLP_TIMEOUT.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn format_command_for_log(program: &str, args: &[String]) -> String {
    std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .map(shell_quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '='))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn select_candidate(value: &Value) -> Option<Candidate> {
    let top_headers = parse_headers(value.get("http_headers"));

    // YouTube "ramps" fresh DASH URLs: the first byte is withheld until
    // `available_at`, a few seconds out. Preferring PO-token-attested HLS
    // skipped that wait, but an HLS stream has to be proxied playlist and
    // all — thousands of segment URLs rewritten on every refresh — and
    // that path cost far more than the seconds it saved. DASH is a single
    // URL the proxy passes straight through, so the ramp is the better
    // trade.

    if let Some(candidate) = select_requested_formats(value, &top_headers) {
        return Some(candidate);
    }

    if let Some(url) = value
        .get("url")
        .and_then(Value::as_str)
        .filter(|url| is_http_url(url))
    {
        return Some(Candidate {
            url: url.to_string(),
            headers: top_headers,
            format_id: None,
            protocol: value
                .get("protocol")
                .and_then(Value::as_str)
                .map(str::to_string),
            resolution: value
                .get("resolution")
                .and_then(Value::as_str)
                .map(str::to_string),
            score: i64::MAX,
        });
    }

    select_best_video_candidate(value, &top_headers)
        .or_else(|| select_best_combined_candidate(value, &top_headers))
}

fn select_requested_formats(value: &Value, top_headers: &[(String, String)]) -> Option<Candidate> {
    let requested_formats: Vec<Candidate> = value
        .get("requested_formats")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|format| format_candidate(format, top_headers))
        .collect();

    match requested_formats.as_slice() {
        [] => None,
        [candidate] => Some(candidate.clone()),
        candidates => {
            for candidate in candidates {
                log_selected_candidate("requested stream", candidate);
            }
            Some(Candidate {
                url: build_edl_url(candidates),
                headers: Vec::new(),
                format_id: value
                    .get("format_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                protocol: value
                    .get("protocol")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                resolution: value
                    .get("resolution")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                score: i64::MAX - 1,
            })
        }
    }
}

fn select_best_video_candidate(value: &Value, top_headers: &[(String, String)]) -> Option<Candidate> {
    value
        .get("formats")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|format| format_candidate(format, &top_headers))
        .filter(|candidate| candidate.score >= 10_000_000)
        .max_by_key(|candidate| candidate.score)
}

fn select_best_combined_candidate(value: &Value, top_headers: &[(String, String)]) -> Option<Candidate> {
    value
        .get("formats")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|format| format_candidate(format, &top_headers))
        .filter(|candidate| candidate.score >= 3_000_000 && candidate.score < 10_000_000)
        .max_by_key(|candidate| candidate.score)
}

fn proxied_candidate_url(candidate: &Candidate) -> String {
    super::stream_proxy::rewrite_stream_url_with_headers(&candidate.url, &candidate.headers)
        .unwrap_or_else(|| candidate.url.clone())
}

fn build_edl_url(candidates: &[Candidate]) -> String {
    let mut edl = String::from("edl://");
    for candidate in candidates {
        let url = proxied_candidate_url(candidate);
        edl.push_str(&format!(
            "!new_stream;!no_clip;!no_chapters;%{}%{};",
            url.len(),
            url
        ));
    }
    edl.trim_end_matches(';').to_string()
}

fn log_selected_candidate(label: &str, candidate: &Candidate) {
    info!(
        "yt-dlp: {label} format_id={} protocol={} resolution={} score={}",
        candidate.format_id.as_deref().unwrap_or("<top-level>"),
        candidate.protocol.as_deref().unwrap_or("<unknown>"),
        candidate.resolution.as_deref().unwrap_or("<unknown>"),
        candidate.score
    );
}

/// Liveness from yt-dlp's own metadata - protocol is no longer a proxy for
/// it now that VOD playback may legitimately use HLS.
fn is_live_video(value: &Value) -> bool {
    value.get("is_live").and_then(Value::as_bool).unwrap_or(false)
        || value
            .get("live_status")
            .and_then(Value::as_str)
            .is_some_and(|status| status == "is_live" || status == "post_live")
}


fn extract_media_title(value: &Value) -> Option<String> {
    value
        .get("title")
        .or_else(|| value.get("fulltitle"))
        .and_then(Value::as_str)
        .map(|title| title.trim())
        .filter(|title| !title.is_empty())
        .map(|title| title.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn format_candidate(format: &Value, top_headers: &[(String, String)]) -> Option<Candidate> {
    let url = format.get("url").and_then(Value::as_str)?;
    if !is_http_url(url) {
        return None;
    }
    if !is_playable_format(format) {
        return None;
    }

    let headers = merge_headers(top_headers, &parse_headers(format.get("http_headers")));
    Some(Candidate {
        url: url.to_string(),
        headers,
        format_id: format
            .get("format_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        protocol: format
            .get("protocol")
            .and_then(Value::as_str)
            .map(str::to_string),
        resolution: format
            .get("resolution")
            .and_then(Value::as_str)
            .map(str::to_string),
        score: score_format(format, url),
    })
}

fn is_playable_format(format: &Value) -> bool {
    let protocol = format
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let ext = format
        .get("ext")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let vcodec = format
        .get("vcodec")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let acodec = format
        .get("acodec")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    if matches!(ext.as_str(), "mhtml" | "jpg" | "webp" | "png") {
        return false;
    }
    if matches!(protocol.as_str(), "mhtml" | "images") {
        return false;
    }
    codec_name_is_present(&vcodec) || codec_name_is_present(&acodec)
}

fn score_format(format: &Value, url: &str) -> i64 {
    let protocol = format
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let ext = format
        .get("ext")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let vcodec = format
        .get("vcodec")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let acodec = format
        .get("acodec")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let height = format.get("height").and_then(Value::as_i64).unwrap_or(0);
    let tbr = format
        .get("tbr")
        .and_then(Value::as_f64)
        .map(|value| value as i64)
        .unwrap_or(0);

    let has_video = codec_name_is_present(&vcodec);
    let has_audio = codec_name_is_present(&acodec);
    let is_hls = protocol.contains("m3u8") || url.to_ascii_lowercase().contains(".m3u8");
    let is_direct_https = protocol == "https";
    let mut score = 0;
    if has_video && !has_audio {
        score += 10_000_000;
    } else if has_video && has_audio {
        score += 3_000_000;
    } else if has_audio && !has_video {
        score += 100_000;
    }

    if height > 0 && height <= 1080 {
        score += height * 10_000;
    } else if height > 1080 {
        score -= 1_000_000 + height * 1_000;
    }
    if is_direct_https {
        score += 50_000;
    } else if is_hls {
        score += 25_000;
    }
    if matches!(ext.as_str(), "mp4" | "m4a" | "webm") {
        score += 20_000;
    }
    score + height * 100 + tbr
}

fn codec_name_is_present(value: &str) -> bool {
    !value.is_empty() && value != "none"
}

fn parse_headers(value: Option<&Value>) -> Vec<(String, String)> {
    value
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|headers| headers.iter())
        .filter_map(|(name, value)| {
            value
                .as_str()
                .map(|value| (name.to_string(), value.to_string()))
        })
        .collect()
}

fn merge_headers(
    base: &[(String, String)],
    override_headers: &[(String, String)],
) -> Vec<(String, String)> {
    let mut merged = base.to_vec();
    for (name, value) in override_headers {
        if let Some((_, existing_value)) = merged
            .iter_mut()
            .find(|(existing_name, _)| existing_name.eq_ignore_ascii_case(name))
        {
            *existing_value = value.clone();
        } else {
            merged.push((name.clone(), value.clone()));
        }
    }
    merged
}

fn is_http_url(raw: &str) -> bool {
    Url::parse(raw)
        .map(|url| matches!(url.scheme(), "http" | "https"))
        .unwrap_or(false)
}

fn is_likely_direct_stream_url(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    let path = url.path().to_ascii_lowercase();
    DIRECT_STREAM_EXTENSIONS
        .iter()
        .any(|extension| path.ends_with(&format!(".{extension}")))
}

fn is_cookie_permission_error(stderr: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    text.contains("could not copy cookies")
        || text.contains("permission denied")
        || text.contains("failed to decrypt")
        || text.contains("could not read cookies")
        || text.contains("unable to get cookies")
        || (text.contains("cookie") && text.contains("error"))
}

fn redact_url(raw: &str) -> String {
    let Ok(mut url) = Url::parse(raw) else {
        return raw.to_string();
    };
    if !url.username().is_empty() {
        let _ = url.set_username("<user>");
        let _ = url.set_password(Some("<redacted>"));
    }
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

