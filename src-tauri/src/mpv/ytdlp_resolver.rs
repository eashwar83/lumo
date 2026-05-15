use log::{info, warn};
use serde_json::Value;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use url::Url;

const YTDLP_TIMEOUT: Duration = Duration::from_secs(60);
// const YTDLP_FORMAT: &str = "bv*[height<=1080][vcodec^=avc1]+ba/bv*[height<=1080][vcodec^=h264]+ba/bv*[height<=1080][vcodec^=hev1]+ba/bv*[height<=1080][vcodec^=hvc1]+ba/bv*[height<=1080][vcodec^=vp9]+ba/b[height<=1080]/bv*[height<=1080]+ba/bv*+ba/b";
const YTDLP_FORMAT: &str = "bv*[height<=1080][vcodec^=vp9]+ba/bv*[height<=1080][vcodec^=h264]+ba/bv*[height<=720][vcodec^=hev1]+ba/bv*[height<=720][vcodec^=hvc1]+ba/bv*[height<=720][vcodec^=vp9]+ba/b[height<=720]/bv*[height<=720]+ba/bv*+ba/b";
// const YTDLP_FORMAT: &str = "bv*[height<=1080]+ba/b[height<=1080]/bv*+ba/b";
// const YTDLP_FORMAT: &str = "bv*[height<=720]+ba/b[height<=720]/bv*+ba/b";
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

pub(crate) struct ResolvedMedia {
    pub(crate) url: String,
    pub(crate) title: Option<String>,
}

pub(crate) async fn resolve(app: &AppHandle, raw_url: &str) -> Result<Option<ResolvedMedia>, String> {
    if !is_http_url(raw_url) {
        return Ok(None);
    }
    if is_likely_direct_stream_url(raw_url) {
        return Ok(None);
    }

    let Some(ytdl_path) = crate::app_bootstrap::resolve_ytdl_path(app) else {
        return Ok(None);
    };

    let proxy_url = crate::network::proxy::current_proxy_key(app)?;
    let raw_url = raw_url.to_string();
    let output = tauri::async_runtime::spawn_blocking(move || {
        run_ytdlp_command(&ytdl_path, proxy_url.as_deref(), &raw_url)
    })
    .await
    .map_err(|error| format!("yt-dlp worker failed: {error}"))??;
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
    let playback_url = proxied_candidate_url(&candidate);
    let title = extract_media_title(&value);

    info!("yt-dlp: resolved url through stream proxy");
    Ok(Some(ResolvedMedia {
        url: playback_url,
        title,
    }))
}

pub(crate) async fn try_resolve(app: &AppHandle, raw_url: &str) -> Option<ResolvedMedia> {
    match resolve(app, raw_url).await {
        Ok(resolved) => resolved,
        Err(error) => {
            warn!("yt-dlp: resolve failed for {}: {error}", redact_url(raw_url));
            None
        }
    }
}

fn run_ytdlp_command(
    ytdl_path: &str,
    proxy_url: Option<&str>,
    raw_url: &str,
) -> Result<std::process::Output, String> {
    let mut command = Command::new(ytdl_path);
    let mut log_args = vec![
        "--dump-single-json".to_string(),
        "--no-playlist".to_string(),
        "-f".to_string(),
        YTDLP_FORMAT.to_string(),
        redact_url(raw_url),
    ];
    command
        .arg("--dump-single-json")
        .arg("--no-playlist")
        .arg("-f")
        .arg(YTDLP_FORMAT)
        .arg(raw_url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(proxy_url) = proxy_url {
        command.arg("--proxy").arg(proxy_url);
        log_args.push("--proxy".to_string());
        log_args.push(redact_url(proxy_url));
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
