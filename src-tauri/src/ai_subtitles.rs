//! AI subtitle generation (cloud).
//!
//! Pulls the audio with a full ffmpeg, transcribes it in ~10-minute chunks via
//! an OpenAI-compatible `/audio/transcriptions` endpoint (OpenAI Whisper, Groq,
//! or a custom base URL), optionally translates the lines through the chat AI,
//! and writes a timed `.srt` next to the video. Progress is streamed to the
//! frontend via `ai_subtitles_progress` events; a long run can be cancelled.

use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{Emitter, Manager};

/// Audio is transcribed in chunks so uploads stay small and memory stays bounded.
const CHUNK_SECS: f64 = 600.0;
/// Lines translated per chat request (keeps each round-trip reasonable).
const TRANSLATE_BATCH: usize = 40;
const REQUEST_TIMEOUT_SECS: u64 = 300;
const FFMPEG_PATH_SETTING: &str = "FFMPEG_PATH";

/// Set while a job runs; a `cancel_ai_subtitles` call flips it and the pipeline
/// bails out between chunks.
static CANCEL: AtomicBool = AtomicBool::new(false);

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Segment {
    start: f64,
    end: f64,
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubtitleResult {
    srt_path: String,
    file_name: String,
    line_count: usize,
}

/// ffmpeg is a console app; keep its window from flashing on Windows.
fn quiet_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command
}

/// Normalise an API base that may be a root (…/v1) or a full endpoint URL.
fn api_root(base: &str) -> String {
    let trimmed = base.trim().trim_end_matches('/');
    trimmed
        .strip_suffix("/audio/transcriptions")
        .or_else(|| trimmed.strip_suffix("/chat/completions"))
        .unwrap_or(trimmed)
        .to_string()
}

fn srt_time(t: f64) -> String {
    let ms = (t.max(0.0) * 1000.0).round() as i64;
    format!(
        "{:02}:{:02}:{:02},{:03}",
        ms / 3_600_000,
        (ms % 3_600_000) / 60_000,
        (ms % 60_000) / 1000,
        ms % 1000
    )
}

fn build_srt(segments: &[Segment]) -> String {
    let mut out = String::new();
    for (i, seg) in segments.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            srt_time(seg.start),
            srt_time(seg.end.max(seg.start + 0.4)),
            seg.text.trim()
        ));
    }
    out
}

/// Parse `HH:MM:SS,mmm` into seconds.
fn parse_srt_time(s: &str) -> Option<f64> {
    let s = s.trim();
    // Milliseconds after ',' or '.'; models sometimes use either (or omit them).
    let (hms, milli) = match s.rsplit_once(',').or_else(|| s.rsplit_once('.')) {
        Some((a, b)) => (a, b.trim().parse::<f64>().unwrap_or(0.0)),
        None => (s, 0.0),
    };
    // Colon fields, seconds-last. Accept HH:MM:SS, MM:SS, or SS — some models
    // drop the hours on short clips (e.g. "00:03,000" instead of "00:00:03,000").
    let parts: Vec<f64> = hms
        .split(':')
        .map(|p| p.trim().parse::<f64>().ok())
        .collect::<Option<Vec<_>>>()?;
    let secs = match parts.as_slice() {
        [h, m, s] => h * 3600.0 + m * 60.0 + s,
        [m, s] => m * 60.0 + s,
        [s] => *s,
        _ => return None,
    };
    Some(secs + milli / 1000.0)
}

/// Parse an existing .srt into segments (so new ranges can merge into it).
fn parse_srt(content: &str) -> Vec<Segment> {
    let normalized = content.replace("\r\n", "\n");
    let mut segments = Vec::new();
    for block in normalized.split("\n\n") {
        let lines: Vec<&str> = block.trim().lines().collect();
        let Some(ti) = lines.iter().position(|l| l.contains("-->")) else {
            continue;
        };
        let Some((start_s, end_s)) = lines[ti].split_once("-->") else {
            continue;
        };
        let (Some(start), Some(end)) = (parse_srt_time(start_s), parse_srt_time(end_s))
        else {
            continue;
        };
        let text = lines[ti + 1..].join("\n").trim().to_string();
        if text.is_empty() {
            continue;
        }
        segments.push(Segment { start, end, text });
    }
    segments
}

fn emit_progress(app: &tauri::AppHandle, stage: &str, done: usize, total: usize) {
    let _ = app.emit(
        "ai_subtitles_progress",
        json!({ "stage": stage, "done": done, "total": total }),
    );
}

fn cancelled() -> bool {
    CANCEL.load(Ordering::SeqCst)
}

/// Extract a mono 16 kHz AAC clip for `[start, start+dur)`.
/// Human-readable language name for a language code, for LLM prompts. Falls back
/// to the code itself when unknown.
fn language_name(code: &str) -> String {
    let c = code.trim().to_ascii_lowercase();
    let name = match c.as_str() {
        "en" => "English",
        "hi" => "Hindi",
        "te" => "Telugu",
        "ta" => "Tamil",
        "kn" => "Kannada",
        "ml" => "Malayalam",
        "mr" => "Marathi",
        "bn" => "Bengali",
        "gu" => "Gujarati",
        "pa" => "Punjabi",
        "ur" => "Urdu",
        "or" | "od" => "Odia",
        "as" => "Assamese",
        "ne" => "Nepali",
        "si" => "Sinhala",
        "ar" => "Arabic",
        "zh" => "Chinese",
        "ja" => "Japanese",
        "ko" => "Korean",
        "fr" => "French",
        "de" => "German",
        "es" => "Spanish",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "it" => "Italian",
        "nl" => "Dutch",
        "tr" => "Turkish",
        "vi" => "Vietnamese",
        "th" => "Thai",
        "id" => "Indonesian",
        "fa" => "Persian",
        "pl" => "Polish",
        "uk" => "Ukrainian",
        "he" => "Hebrew",
        "el" => "Greek",
        _ => return code.trim().to_string(),
    };
    name.to_string()
}

fn extract_chunk(
    ffmpeg: &Path,
    input: &str,
    start: f64,
    dur: f64,
    out: &Path,
) -> Result<(), String> {
    // Pick the encoder from the output extension: Gemini wants a natively-typed
    // audio (flac); the OpenAI-shaped and Sarvam endpoints take aac/m4a.
    let ext = out
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let codec: &[&str] = match ext.as_str() {
        "flac" => &["-c:a", "flac"],
        "wav" => &["-c:a", "pcm_s16le"], // whisper.cpp wants 16-bit PCM WAV
        "mp3" => &["-c:a", "libmp3lame", "-b:a", "64k"],
        _ => &["-c:a", "aac", "-b:a", "64k"],
    };
    let result = quiet_command(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-ss"])
        .arg(format!("{start}"))
        .arg("-t")
        .arg(format!("{dur}"))
        .arg("-i")
        .arg(input)
        .args(["-vn", "-ac", "1", "-ar", "16000"])
        .args(codec)
        .arg(out)
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {e}"))?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let detail = stderr.lines().last().unwrap_or("unknown error").trim();
        return Err(format!("Audio extraction failed: {detail}"));
    }
    Ok(())
}

/// Pull the seconds value after a `silencedetect` key (e.g. "silence_start:").
fn parse_silence_seconds(line: &str, key: &str) -> Option<f64> {
    let idx = line.find(key)?;
    line[idx + key.len()..]
        .split_whitespace()
        .next()?
        .trim_end_matches('|')
        .parse::<f64>()
        .ok()
}

/// Detect silence *midpoints* across [start,end] of the source audio, returned as
/// absolute seconds. These are natural cut positions: breaking chunks here (rather
/// than on a blind fixed grid) keeps a song's tail and the dialogue that follows
/// in separate chunks, so an untimed dialogue chunk starts at the real pause
/// instead of bleeding ~20s earlier.
fn detect_silence_points(ffmpeg: &Path, input: &str, start: f64, end: f64) -> Vec<f64> {
    let dur = end - start;
    if dur <= 0.0 {
        return vec![];
    }
    let output = quiet_command(ffmpeg)
        .args(["-hide_banner", "-nostats", "-ss"])
        .arg(format!("{start}"))
        .arg("-t")
        .arg(format!("{dur}"))
        .arg("-i")
        .arg(input)
        .args(["-vn", "-af", "silencedetect=noise=-30dB:d=0.35", "-f", "null", "-"])
        .output();
    let Ok(out) = output else {
        return vec![];
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut points = Vec::new();
    let mut pending: Option<f64> = None;
    for line in stderr.lines() {
        if let Some(v) = parse_silence_seconds(line, "silence_start:") {
            pending = Some(v);
        }
        if let Some(v) = parse_silence_seconds(line, "silence_end:") {
            let s = pending.take().unwrap_or((v - 0.3).max(0.0));
            points.push((s + v) / 2.0 + start); // absolute midpoint
        }
    }
    points
}

/// Split [eff_start, eff_end] into chunks no longer than `max_secs`, breaking at
/// the latest silence point before each cap when one exists (else a hard cut).
fn build_chunk_ranges(
    eff_start: f64,
    eff_end: f64,
    max_secs: f64,
    silence: &[f64],
) -> Vec<(f64, f64)> {
    let min_secs = (max_secs * 0.4).min(6.0);
    let mut chunks = Vec::new();
    let mut s = eff_start;
    while s < eff_end - 0.1 {
        let hard_end = (s + max_secs).min(eff_end);
        // Latest silence point strictly inside (s + min, hard_end).
        let cut = silence
            .iter()
            .copied()
            .filter(|&p| p > s + min_secs && p < hard_end - 0.05)
            .next_back();
        let end = cut.unwrap_or(hard_end);
        chunks.push((s, end));
        s = end;
    }
    if chunks.is_empty() {
        chunks.push((eff_start, eff_end));
    }
    chunks
}

/// Use ffmpeg's silencedetect to find where speech actually begins and ends in a
/// chunk clip. Untimed (translate-mode) subtitles are then placed only within
/// that spoken window, instead of being spread from the chunk's very start — so
/// a chunk that opens with silence no longer shows its first line too early.
/// Returns (speech_start, speech_end) relative to the clip; falls back to the
/// whole clip when detection is inconclusive.
fn detect_speech_window(ffmpeg: &Path, clip: &Path, chunk_dur: f64) -> (f64, f64) {
    let output = quiet_command(ffmpeg)
        .args(["-hide_banner", "-nostats", "-i"])
        .arg(clip)
        .args(["-af", "silencedetect=noise=-30dB:d=0.5", "-f", "null", "-"])
        .output();
    let Ok(out) = output else {
        return (0.0, chunk_dur);
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut sil_starts: Vec<f64> = Vec::new();
    let mut sil_ends: Vec<f64> = Vec::new();
    for line in stderr.lines() {
        if let Some(v) = parse_silence_seconds(line, "silence_start:") {
            sil_starts.push(v);
        }
        if let Some(v) = parse_silence_seconds(line, "silence_end:") {
            sil_ends.push(v);
        }
    }
    // Clip opens with silence → speech begins at that first silence's end.
    let mut speech_start = 0.0;
    if sil_starts.first().map_or(false, |&s| s <= 0.35) {
        if let Some(&e) = sil_ends.first() {
            speech_start = e.clamp(0.0, chunk_dur);
        }
    }
    // Clip ends in silence → an unmatched final silence_start (no following end).
    let mut speech_end = chunk_dur;
    if sil_starts.len() > sil_ends.len() {
        if let Some(&s) = sil_starts.last() {
            speech_end = s.clamp(0.0, chunk_dur);
        }
    }
    if speech_end - speech_start < 0.5 {
        return (0.0, chunk_dur);
    }
    (speech_start, speech_end)
}

fn provider_error(status: reqwest::StatusCode, text: &str) -> String {
    let parsed = serde_json::from_str::<Value>(text).ok();
    let detail = parsed
        .as_ref()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("message").or(Some(e)))
                .map(|m| m.to_string())
        })
        .unwrap_or_else(|| text.chars().take(200).collect());
    // Google quota errors name the exact quota (e.g. "...PerMinute...-FreeTier")
    // in error.details[].violations[].quotaId — the message alone doesn't say
    // which limit tripped, so surface it.
    let quotas: Vec<String> = parsed
        .as_ref()
        .and_then(|v| v.get("error")?.get("details")?.as_array())
        .map(|details| {
            details
                .iter()
                .filter_map(|d| d.get("violations")?.as_array())
                .flatten()
                .filter_map(|viol| viol.get("quotaId")?.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    if quotas.is_empty() {
        format!("({status}): {detail}")
    } else {
        format!("({status}): {detail} [quota: {}]", quotas.join(", "))
    }
}

/// Transcribe one audio file to timestamped segments.
fn transcribe_chunk(
    client: &reqwest::blocking::Client,
    url: &str,
    api_key: &str,
    model: &str,
    file_path: &Path,
    language: Option<&str>,
) -> Result<Vec<Segment>, String> {
    let bytes = std::fs::read(file_path).map_err(|e| e.to_string())?;
    let part = reqwest::blocking::multipart::Part::bytes(bytes)
        .file_name("audio.m4a")
        .mime_str("audio/mp4")
        .map_err(|e| e.to_string())?;
    let mut form = reqwest::blocking::multipart::Form::new()
        .part("file", part)
        .text("model", model.to_string())
        .text("response_format", "verbose_json")
        .text("temperature", "0");
    if let Some(lang) = language {
        let lang = lang.trim();
        if !lang.is_empty() && lang != "auto" {
            form = form.text("language", lang.to_string());
        }
    }

    let resp = client
        .post(url)
        .header("authorization", format!("Bearer {api_key}"))
        .multipart(form)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;
    let status = resp.status();
    // Rate limits carry a suggested wait; encode it so the caller can decide to
    // wait-and-retry (short per-minute limits) or stop-and-resume (hourly cap).
    if status.as_u16() == 429 {
        let retry = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<f64>().ok());
        let text = resp.text().unwrap_or_default();
        let secs = retry.or_else(|| parse_retry_seconds(&text)).unwrap_or(3600.0);
        return Err(format!("RATE_LIMIT|{secs}|{}", provider_error(status, &text)));
    }
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Transcription {}", provider_error(status, &text)));
    }

    let body: Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid response: {e}"))?;
    let mut segments = Vec::new();
    if let Some(list) = body.get("segments").and_then(Value::as_array) {
        for s in list {
            let start = s.get("start").and_then(Value::as_f64).unwrap_or(0.0);
            let end = s.get("end").and_then(Value::as_f64).unwrap_or(start);
            let text = s
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if !text.is_empty() {
                segments.push(Segment { start, end, text });
            }
        }
    }
    // Some endpoints return only `text` without segments.
    if segments.is_empty() {
        if let Some(t) = body.get("text").and_then(Value::as_str) {
            let t = t.trim();
            if !t.is_empty() {
                segments.push(Segment {
                    start: 0.0,
                    end: 0.0,
                    text: t.to_string(),
                });
            }
        }
    }
    Ok(segments)
}

/// Transcribe (and optionally translate) one chunk with Gemini's multimodal API.
/// Gemini returns segment-level SRT which we parse; when `translate_to` is set it
/// transcribes and translates in one call, so no separate chat step is needed.
fn transcribe_chunk_gemini(
    client: &reqwest::blocking::Client,
    base: &str,
    api_key: &str,
    model: &str,
    clip: &Path,
    source_language: Option<&str>,
    translate_to: Option<&str>,
    chunk_dur: f64,
) -> Result<Vec<Segment>, String> {
    use base64::Engine as _;
    let bytes = std::fs::read(clip).map_err(|e| e.to_string())?;
    let audio_b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    let src = source_language
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != "auto");
    let src_desc = match src {
        Some(code) => format!("The speech is in {}.", language_name(code)),
        None => "Detect the spoken language.".to_string(),
    };
    let task = match translate_to.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => format!(
            "Transcribe the speech, then translate it into {0}. The subtitle text must be written in {0}.",
            language_name(t)
        ),
        None => "Transcribe the speech verbatim.".to_string(),
    };
    let prompt = format!(
        "You are a subtitle generator. {src_desc} {task} Output ONLY a valid SRT \
subtitle document: sequential numbers, `HH:MM:SS,mmm --> HH:MM:SS,mmm` time codes \
measured from the START of THIS audio clip (the clip begins at 00:00:00,000), then \
the subtitle text. Keep each cue to one short, readable line, timed to when it is \
actually spoken. If there is no speech, output nothing. No markdown, no code \
fences, no commentary."
    );

    let url = format!("{}/models/{}:generateContent", base.trim_end_matches('/'), model);
    let payload = json!({
        "contents": [{
            "parts": [
                { "text": prompt },
                { "inline_data": { "mime_type": "audio/flac", "data": audio_b64 } }
            ]
        }],
        "generationConfig": { "temperature": 0.0 }
    });

    let resp = client
        .post(&url)
        .header("x-goog-api-key", api_key)
        .json(&payload)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;
    let status = resp.status();
    if status.as_u16() == 429 {
        let text = resp.text().unwrap_or_default();
        let secs = parse_retry_seconds(&text).unwrap_or(60.0);
        return Err(format!("RATE_LIMIT|{secs}|{}", provider_error(status, &text)));
    }
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Transcription {}", provider_error(status, &text)));
    }
    let body: Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid response: {e}"))?;
    let content = body
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(Value::as_str)
        .unwrap_or("");
    // Strip any stray code fences the model added.
    let cleaned = content
        .trim()
        .trim_start_matches("```srt")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let mut segments = parse_srt(cleaned);
    // Clamp to the clip so a hallucinated tail can't overrun into the next chunk.
    segments.retain(|s| s.start < chunk_dur + 0.5);
    for s in segments.iter_mut() {
        s.start = s.start.min(chunk_dur);
        s.end = s.end.min(chunk_dur).max(s.start);
    }
    if segments.is_empty() && !cleaned.is_empty() {
        // Reply had text but no parseable timings — spread the dialogue across the
        // clip, dropping SRT scaffolding (cue numbers, `-->` timing lines) so they
        // never leak into the on-screen text.
        let plain = cleaned
            .lines()
            .map(str::trim)
            .filter(|l| {
                !l.is_empty()
                    && !l.contains("-->")
                    && !l.chars().all(|c| c.is_ascii_digit())
            })
            .collect::<Vec<_>>()
            .join(" ");
        if !plain.is_empty() {
            return Ok(distribute_segments(&plain, 0.0, chunk_dur));
        }
    }
    Ok(segments)
}

/// Translate a batch of lines via the chat API. Returns the translated lines, or
/// the originals if the model's reply can't be parsed 1:1.
fn translate_batch(
    client: &reqwest::blocking::Client,
    url: &str,
    api_key: &str,
    model: &str,
    target_language: &str,
    lines: &[String],
) -> Vec<String> {
    let system = format!(
        "You are a subtitle translator. Translate each string in the user's JSON \
array into {target_language}. Return ONLY a JSON array of strings, the same \
length and order, each the natural, concise translation of the matching line. \
No commentary, no markdown."
    );
    let user = serde_json::to_string(lines).unwrap_or_default();
    let strict_openai = url.contains("api.openai.com");
    let mut payload = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    });
    if strict_openai {
        payload["max_completion_tokens"] = Value::from(4000);
    } else {
        payload["max_tokens"] = Value::from(4000);
    }

    let parsed = (|| -> Option<Vec<String>> {
        let resp = client
            .post(url)
            .header("authorization", format!("Bearer {api_key}"))
            .json(&payload)
            .send()
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body: Value = resp.json().ok()?;
        let content = body
            .get("choices")?
            .as_array()?
            .first()?
            .get("message")?
            .get("content")?
            .as_str()?;
        let start = content.find('[')?;
        let end = content.rfind(']')?;
        let arr: Vec<String> = serde_json::from_str(&content[start..=end]).ok()?;
        if arr.len() == lines.len() {
            Some(arr)
        } else {
            None
        }
    })();

    parsed.unwrap_or_else(|| lines.to_vec())
}

/// Translate one string with Sarvam's own /translate endpoint. `source_lang` is
/// an `xx-IN` code or "auto"; `target_lang` is an `xx-IN` code. Returns None on
/// any failure so the caller can fall back per-line.
fn sarvam_translate_once(
    client: &reqwest::blocking::Client,
    base: &str,
    api_key: &str,
    source_lang: &str,
    target_lang: &str,
    input: &str,
) -> Option<String> {
    if input.trim().is_empty() {
        return Some(String::new());
    }
    let url = format!("{}/translate", base.trim_end_matches('/'));
    // mayura:v1 supports source "auto" and all Indic↔English pairs (≤1000 chars).
    let body = json!({
        "input": input,
        "source_language_code": source_lang,
        "target_language_code": target_lang,
        "model": "mayura:v1",
    });
    let resp = client
        .post(&url)
        .header("api-subscription-key", api_key)
        .json(&body)
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: Value = resp.json().ok()?;
    v.get("translated_text")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
}

/// Translate a batch of subtitle lines via Sarvam. Tries a single newline-joined
/// request (cheap), and only trusts it when the line count is preserved;
/// otherwise falls back to one request per line so the 1:1 mapping is exact.
fn translate_batch_sarvam(
    client: &reqwest::blocking::Client,
    base: &str,
    api_key: &str,
    source_lang: &str,
    target_lang: &str,
    texts: &[String],
) -> Vec<String> {
    if texts.is_empty() {
        return vec![];
    }
    let joined = texts.join("\n");
    if let Some(out) = sarvam_translate_once(client, base, api_key, source_lang, target_lang, &joined)
    {
        let lines: Vec<String> = out.split('\n').map(|s| s.trim().to_string()).collect();
        if lines.len() == texts.len() {
            return lines;
        }
    }
    texts
        .iter()
        .map(|t| {
            sarvam_translate_once(client, base, api_key, source_lang, target_lang, t)
                .unwrap_or_else(|| t.clone())
        })
        .collect()
}

/// Parse "…try again in 2m13.6s" style messages into seconds.
fn parse_retry_seconds(text: &str) -> Option<f64> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find("try again in ")? + "try again in ".len();
    let rest = &lower[start..];
    let mut total = 0.0;
    let mut num = String::new();
    for ch in rest.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            num.push(ch);
        } else if ch == 'm' {
            total += num.parse::<f64>().unwrap_or(0.0) * 60.0;
            num.clear();
        } else if ch == 's' {
            total += num.parse::<f64>().unwrap_or(0.0);
            break;
        } else if num.is_empty() {
            continue;
        } else {
            break;
        }
    }
    if total > 0.0 {
        Some(total)
    } else {
        None
    }
}

/// Persistent per-chunk transcript cache dir, keyed by video + model + language
/// + range (so a partial test and the full run don't share a cache).
fn cache_dir_for(
    app: &tauri::AppHandle,
    video: &str,
    model: &str,
    language: Option<&str>,
    range_tag: &str,
) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    video.hash(&mut hasher);
    model.hash(&mut hasher);
    language.unwrap_or("auto").hash(&mut hasher);
    range_tag.hash(&mut hasher);
    let key = format!("{:016x}", hasher.finish());
    app.path()
        .app_cache_dir()
        .map(|d| d.join("ai_subtitles_cache").join(&key))
        .unwrap_or_else(|_| std::env::temp_dir().join(key))
}

/// Run a transcription attempt, waiting out short per-minute rate limits and
/// stopping (so a later re-run resumes from the cache) on the hourly audio cap.
fn retry_transcription<F>(
    app: &tauri::AppHandle,
    mut attempt_fn: F,
    chunk: usize,
    total: usize,
    pace_secs: &mut u64,
) -> Result<Vec<Segment>, String>
where
    F: FnMut() -> Result<Vec<Segment>, String>,
{
    // Pace requests once we've been rate-limited: a few seconds between chunks
    // stays under a tight per-minute cap, which beats eating the provider's
    // ~60s penalty wait every few chunks.
    for _ in 0..*pace_secs {
        if cancelled() {
            return Err("Cancelled".to_string());
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    let mut attempt = 0;
    loop {
        match attempt_fn() {
            Ok(segments) => {
                *pace_secs = pace_secs.saturating_sub(1);
                return Ok(segments);
            }
            Err(e) if e.starts_with("RATE_LIMIT|") => {
                let secs = e
                    .split('|')
                    .nth(1)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(3600.0);
                // The provider's own reason, e.g. "[quota: ...PerMinute...]".
                let quota = e
                    .find("[quota:")
                    .map(|i| e[i..].trim_end_matches(']').trim_start_matches("[quota:").trim().to_string())
                    .unwrap_or_default();
                if secs <= 180.0 && attempt < 6 {
                    *pace_secs = (*pace_secs + 5).min(20);
                    let wait = (secs.max(1.0) + 1.0).ceil() as u64;
                    for elapsed in 0..wait {
                        if cancelled() {
                            return Err("Cancelled".to_string());
                        }
                        let _ = app.emit(
                            "ai_subtitles_progress",
                            json!({
                                "stage": "rate_wait",
                                "done": chunk,
                                "total": total,
                                "wait": wait - elapsed,
                                "quota": quota,
                            }),
                        );
                        std::thread::sleep(Duration::from_secs(1));
                    }
                    emit_progress(app, "transcribe", chunk, total);
                    attempt += 1;
                    continue;
                }
                let mins = (secs / 60.0).ceil().max(1.0) as i64;
                // Quote the provider's own reason so the user can see WHICH
                // quota tripped (free vs paid tier, per-minute vs per-day).
                let detail = e.splitn(3, '|').nth(2).unwrap_or("").trim().to_string();
                let detail = if detail.is_empty() {
                    String::new()
                } else {
                    format!(" Provider says: {detail}")
                };
                return Err(format!(
                    "Rate limit reached — {chunk}/{total} chunks done. Run Generate \
again in about {mins} min to resume from here (finished parts are cached, so they \
won't re-count).{detail}"
                ));
            }
            Err(e) => return Err(e),
        }
    }
}

/// Map our short language code to Sarvam's `xx-IN` form (None = auto-detect).
fn sarvam_language_code(lang: Option<&str>) -> Option<String> {
    let code = lang?.trim().to_ascii_lowercase();
    if code.is_empty() || code == "auto" {
        return None;
    }
    let mapped = match code.as_str() {
        "hi" => "hi-IN",
        "bn" => "bn-IN",
        "kn" => "kn-IN",
        "ml" => "ml-IN",
        "mr" => "mr-IN",
        "or" | "od" => "od-IN",
        "pa" => "pa-IN",
        "ta" => "ta-IN",
        "te" => "te-IN",
        "gu" => "gu-IN",
        "en" => "en-IN",
        _ => return None, // unsupported by Sarvam → let it auto-detect
    };
    Some(mapped.to_string())
}

/// Break text into readable, sentence-ish subtitle lines (~one screen each).
fn split_into_lines(text: &str) -> Vec<String> {
    const MAX_LINE: usize = 84;
    let mut units: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if matches!(ch, '.' | '?' | '!' | '।' | '॥') {
            let t = cur.trim();
            if !t.is_empty() {
                units.push(t.to_string());
            }
            cur.clear();
        }
    }
    let tail = cur.trim();
    if !tail.is_empty() {
        units.push(tail.to_string());
    }

    // Wrap over-long sentences on word boundaries.
    let mut lines = Vec::new();
    for unit in units {
        if unit.chars().count() <= MAX_LINE {
            lines.push(unit);
            continue;
        }
        let mut line = String::new();
        for word in unit.split_whitespace() {
            if !line.is_empty() && line.chars().count() + word.chars().count() + 1 > MAX_LINE {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.trim().is_empty() {
            lines.push(line);
        }
    }
    lines
}

/// Lay `text` out as timed lines when the provider gives no word timestamps
/// (e.g. translate mode). Each line's on-screen time is estimated from its
/// length, lines play back-to-back from the chunk start, and each is capped —
/// so a caption disappears after it's "spoken" instead of lingering across the
/// silence until the next chunk.
/// Spread untimed transcript text across [window_start, window_end], estimating
/// each line's duration from its length. The window is the actual spoken region
/// of the chunk (see `detect_speech_window`), so lines don't start during
/// leading silence or linger through trailing silence.
fn distribute_segments(text: &str, window_start: f64, window_end: f64) -> Vec<Segment> {
    let lines = split_into_lines(text);
    if lines.is_empty() {
        return vec![];
    }
    let window = (window_end - window_start).max(0.5);
    const CHARS_PER_SEC: f64 = 14.0; // rough speech pace
    const MIN_LINE: f64 = 1.0;
    const MAX_LINE: f64 = 7.0;
    const GAP: f64 = 0.15;

    let durations: Vec<f64> = lines
        .iter()
        .map(|l| (l.chars().count() as f64 / CHARS_PER_SEC).clamp(MIN_LINE, MAX_LINE))
        .collect();
    let gaps = GAP * lines.len().saturating_sub(1) as f64;
    let total: f64 = durations.iter().sum::<f64>() + gaps;

    // If the estimate overflows the window, compress to fit (no overlap into the
    // next chunk). If it's shorter, the remainder stays empty — a real gap.
    let (scale, gap) = if total > window && total > 0.0 {
        (window / total, 0.0)
    } else {
        (1.0, GAP)
    };

    let mut segments = Vec::new();
    let mut t = window_start;
    for (i, line) in lines.into_iter().enumerate() {
        let dur = (durations[i] * scale).max(0.4);
        let end = (t + dur).min(window_end);
        segments.push(Segment {
            start: t,
            end,
            text: line,
        });
        t = end + gap;
        if t >= window_end {
            break;
        }
    }
    segments
}

/// Group Sarvam's word-level timestamps into readable subtitle lines.
fn words_to_segments(
    words: &[Value],
    starts: &[Value],
    ends: &[Value],
    fallback_text: &str,
    speech_window: (f64, f64),
) -> Vec<Segment> {
    let n = words.len().min(starts.len()).min(ends.len());
    if n == 0 {
        // No word timings (translate mode) — spread the transcript across the
        // detected spoken window so it doesn't start during leading silence.
        return distribute_segments(fallback_text, speech_window.0, speech_window.1);
    }

    const MAX_GAP: f64 = 0.8;
    const MAX_DUR: f64 = 6.0;
    const MAX_CHARS: usize = 84;
    let mut segments = Vec::new();
    let mut line = String::new();
    let mut line_start = 0.0;
    let mut last_end = 0.0;

    for i in 0..n {
        let word = words[i].as_str().unwrap_or("").trim();
        if word.is_empty() {
            continue;
        }
        let ws = starts[i].as_f64().unwrap_or(last_end);
        let we = ends[i].as_f64().unwrap_or(ws);
        if line.is_empty() {
            line_start = ws;
        } else {
            let gap = ws - last_end;
            let dur = we - line_start;
            if gap > MAX_GAP || dur > MAX_DUR || line.len() + word.len() + 1 > MAX_CHARS {
                segments.push(Segment {
                    start: line_start,
                    end: last_end,
                    text: line.trim().to_string(),
                });
                line.clear();
                line_start = ws;
            }
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
        last_end = we;
    }
    if !line.trim().is_empty() {
        segments.push(Segment {
            start: line_start,
            end: last_end,
            text: line.trim().to_string(),
        });
    }
    segments
}

/// Transcribe (or translate-to-English) one chunk via Sarvam's Indic ASR.
fn transcribe_chunk_sarvam(
    client: &reqwest::blocking::Client,
    base: &str,
    api_key: &str,
    model: &str,
    mode: &str,
    language_code: Option<&str>,
    file_path: &Path,
    speech_window: (f64, f64),
) -> Result<Vec<Segment>, String> {
    let url = format!("{}/speech-to-text", base.trim_end_matches('/'));
    let bytes = std::fs::read(file_path).map_err(|e| e.to_string())?;
    let part = reqwest::blocking::multipart::Part::bytes(bytes)
        .file_name("audio.m4a")
        .mime_str("audio/mp4")
        .map_err(|e| e.to_string())?;
    let mut form = reqwest::blocking::multipart::Form::new()
        .part("file", part)
        .text("model", model.to_string());
    // `mode` is only honoured by saaras models.
    if model.contains("saaras") {
        form = form.text("mode", mode.to_string());
    }
    if let Some(lang) = language_code {
        form = form.text("language_code", lang.to_string());
    }

    let resp = client
        .post(&url)
        .header("api-subscription-key", api_key)
        .multipart(form)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;
    let status = resp.status();
    if status.as_u16() == 429 {
        let retry = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<f64>().ok());
        let text = resp.text().unwrap_or_default();
        let secs = retry.or_else(|| parse_retry_seconds(&text)).unwrap_or(60.0);
        return Err(format!("RATE_LIMIT|{secs}|{}", provider_error(status, &text)));
    }
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Transcription {}", provider_error(status, &text)));
    }

    let body: Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid response: {e}"))?;

    // Prefer diarized entries — they carry real per-turn timestamps.
    if let Some(entries) = body
        .pointer("/diarized_transcript/entries")
        .and_then(Value::as_array)
    {
        let mut segments = Vec::new();
        for entry in entries {
            let line = entry
                .get("transcript")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if line.is_empty() {
                continue;
            }
            let start = entry
                .get("start_time_seconds")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let end = entry
                .get("end_time_seconds")
                .and_then(Value::as_f64)
                .unwrap_or(start);
            for mut seg in distribute_segments(line, 0.0, (end - start).max(0.5)) {
                seg.start += start;
                seg.end += start;
                segments.push(seg);
            }
        }
        if !segments.is_empty() {
            return Ok(segments);
        }
    }

    let empty: Vec<Value> = Vec::new();
    let words = body
        .pointer("/timestamps/words")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let starts = body
        .pointer("/timestamps/start_time_seconds")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let ends = body
        .pointer("/timestamps/end_time_seconds")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let fallback = body.get("transcript").and_then(Value::as_str).unwrap_or("");
    Ok(words_to_segments(words, starts, ends, fallback, speech_window))
}

/// Threads for whisper.cpp: most of the machine, leaving 1–2 cores for the UI.
fn whisper_thread_count() -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    cores.saturating_sub(2).clamp(4, 12)
}

/// Map our 2-letter UI code to a Whisper language code. Whisper uses ISO-639-1
/// codes directly, so most pass through; "auto" lets it detect.
fn whisper_language_arg(lang: Option<&str>) -> String {
    match lang.map(str::trim).filter(|l| !l.is_empty() && *l != "auto") {
        Some(code) => code.to_ascii_lowercase(),
        None => "auto".to_string(),
    }
}

/// Transcribe the whole range offline with a local whisper.cpp build. Extracts a
/// 16 kHz WAV, runs `whisper-cli -osrt`, and parses the SRT it writes. With
/// `translate_english` it uses whisper's built-in translate-to-English. Returns
/// absolutely-timed segments (offset by the range start).
fn run_whisper(
    app: &tauri::AppHandle,
    ffmpeg: &Path,
    exe: &str,
    model: &str,
    input: &str,
    eff_start: f64,
    eff_end: f64,
    language: Option<&str>,
    translate_english: bool,
    work: &Path,
) -> Result<Vec<Segment>, String> {
    if exe.trim().is_empty() || !Path::new(exe).exists() {
        return Err("Set the path to whisper-cli (the whisper.cpp program) first".to_string());
    }
    if model.trim().is_empty() || !Path::new(model).exists() {
        return Err("Set the path to a Whisper model file (.bin) first".to_string());
    }

    let wav = work.join("whisper_audio.wav");
    extract_chunk(ffmpeg, input, eff_start, eff_end - eff_start, &wav)?;

    emit_progress(app, "transcribe", 0, 1);
    let out_base = work.join("whisper_out");
    let mut cmd = quiet_command(Path::new(exe));
    cmd.arg("-m")
        .arg(model)
        .arg("-f")
        .arg(&wav)
        .arg("-osrt")
        .arg("-of")
        .arg(&out_base)
        .arg("-l")
        .arg(whisper_language_arg(language))
        // whisper.cpp defaults to only 4 threads — use most of the machine so a
        // multi-core CPU isn't left idle (leave 1–2 cores for the UI).
        .arg("-t")
        .arg(whisper_thread_count().to_string())
        // No console in a windowed app — discard whisper's chatter so a full pipe
        // buffer can't stall it.
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if translate_english {
        cmd.arg("-tr");
    }

    // Spawn and poll so a cancel can kill the (potentially long) run.
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Couldn't launch whisper-cli: {e}"))?;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(
                        "whisper-cli failed — check the program and model are compatible \
(and that the model matches your CPU/GPU build)."
                            .to_string(),
                    );
                }
                break;
            }
            Ok(None) => {
                if cancelled() {
                    let _ = child.kill();
                    return Err("Cancelled".to_string());
                }
                std::thread::sleep(Duration::from_millis(300));
            }
            Err(e) => return Err(format!("whisper-cli error: {e}")),
        }
    }

    let srt_path = work.join("whisper_out.srt");
    let content = std::fs::read_to_string(&srt_path)
        .map_err(|e| format!("whisper-cli produced no subtitles: {e}"))?;
    let mut segments = parse_srt(&content);
    for s in segments.iter_mut() {
        s.start += eff_start;
        s.end += eff_start;
    }
    emit_progress(app, "transcribe", 1, 1);
    Ok(segments)
}

fn output_srt_path(app: &tauri::AppHandle, video: &str, lang_suffix: &str) -> PathBuf {
    let stem = Path::new(video)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "subtitles".to_string());
    let name = format!("{stem}.{lang_suffix}.ai.srt");
    // Prefer next to the video (so it's picked up automatically next time).
    // Probe writability with a throwaway file so we never clobber an existing
    // subtitle file (the merge step reads it back in).
    if let Some(parent) = Path::new(video).parent() {
        let probe = parent.join(format!(".{stem}.ai.write-test"));
        if std::fs::write(&probe, "").is_ok() {
            let _ = std::fs::remove_file(&probe);
            return parent.join(&name);
        }
    }
    // Fallback: app cache dir.
    let dir = app
        .path()
        .app_cache_dir()
        .map(|d| d.join("ai_subtitles"))
        .unwrap_or_else(|_| std::env::temp_dir());
    let _ = std::fs::create_dir_all(&dir);
    dir.join(name)
}

/// Whether AI subtitle generation is available (needs a full ffmpeg for audio).
#[tauri::command]
pub(crate) async fn ai_subtitles_available(app: tauri::AppHandle) -> Result<bool, String> {
    let configured = crate::store::ui_state_store::load_setting_value(&app, FFMPEG_PATH_SETTING)
        .ok()
        .flatten();
    tauri::async_runtime::spawn_blocking(move || {
        crate::mpv::find_ffmpeg(configured.as_deref()).is_some()
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn cancel_ai_subtitles() {
    CANCEL.store(true, Ordering::SeqCst);
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn generate_ai_subtitles(
    app: tauri::AppHandle,
    path: String,
    transcribe_base: String,
    transcribe_key: String,
    transcribe_model: String,
    source_language: Option<String>,
    translate_to: Option<String>,
    chat_base: Option<String>,
    chat_key: Option<String>,
    chat_model: Option<String>,
    range_start: Option<f64>,
    range_end: Option<f64>,
    engine: Option<String>,
    accurate_timing: Option<bool>,
    whisper_exe: Option<String>,
    whisper_model: Option<String>,
) -> Result<SubtitleResult, String> {
    if path.trim().is_empty() || is_remote(&path) {
        return Err("AI subtitles need a local video file".to_string());
    }
    // Local Whisper needs no key — it uses a program + model path instead.
    if engine.as_deref() != Some("whisper") && transcribe_key.trim().is_empty() {
        return Err("Set a transcription API key in Settings → Subtitles (AI)".to_string());
    }
    CANCEL.store(false, Ordering::SeqCst);

    let configured = crate::store::ui_state_store::load_setting_value(&app, FFMPEG_PATH_SETTING)
        .ok()
        .flatten();
    let work = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("Failed to resolve cache dir: {e}"))?
        .join("ai_subtitles_work");

    let app_handle = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let ffmpeg = crate::mpv::find_ffmpeg(configured.as_deref()).ok_or_else(|| {
            "AI subtitles need ffmpeg. Install it, or set its path in Settings → Advanced."
                .to_string()
        })?;
        let duration = crate::mpv::probe_duration(&path).unwrap_or(0.0);
        if duration <= 0.0 {
            return Err("Could not read the video's duration".to_string());
        }

        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))?;
        let transcribe_url = format!("{}/audio/transcriptions", api_root(&transcribe_base));

        // Optional From→To window (for quick testing). Absolute timestamps are
        // kept, so the generated track lines up with the video.
        let mut eff_start = range_start.unwrap_or(0.0).clamp(0.0, duration);
        let mut eff_end = range_end.unwrap_or(duration).clamp(0.0, duration);
        if eff_end <= eff_start {
            eff_start = 0.0;
            eff_end = duration;
        }
        let is_range = eff_start > 0.0 || eff_end < duration - 0.5;
        let range_tag = if is_range {
            format!("{}-{}", eff_start as i64, eff_end as i64)
        } else {
            String::new()
        };

        // Sarvam (Indic-first) uses a different endpoint + 30s sync limit, and
        // saaras can translate Indic audio straight to English (`mode=translate`),
        // skipping the lossy transcribe-then-LLM-translate path.
        let is_sarvam = engine.as_deref() == Some("sarvam");
        let is_gemini = engine.as_deref() == Some("gemini");
        let is_whisper = engine.as_deref() == Some("whisper");
        // Gemini takes larger clips (it timestamps them itself); Sarvam has a 30s
        // sync cap; the OpenAI-shaped endpoints take big chunks.
        let chunk_secs = if is_sarvam {
            28.0
        } else if is_gemini {
            60.0
        } else {
            CHUNK_SECS
        };
        let target = translate_to
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let target_is_english = target
            .map(|t| t.eq_ignore_ascii_case("en") || t.eq_ignore_ascii_case("english"))
            .unwrap_or(false);
        // Direct Sarvam translate → English gives the best phrasing but carries
        // no timestamps. When the user wants accurate timing we instead transcribe
        // (which returns real word-level timestamps) and translate via the chat AI.
        let want_accurate = accurate_timing.unwrap_or(false);
        let sarvam_direct_english = is_sarvam && target_is_english && !want_accurate;
        // Gemini transcribes and translates in one multimodal call, so it produces
        // the target-language track directly — no separate translate step.
        let gemini_inline_translate = is_gemini && target.is_some();
        // Local whisper can translate to English itself (offline). Other targets
        // fall through to the shared translate step (which needs a provider).
        let whisper_english_inline = is_whisper && target_is_english;
        let sarvam_mode = if sarvam_direct_english {
            "translate"
        } else {
            "transcribe"
        };
        let sarvam_lang = sarvam_language_code(source_language.as_deref());

        // Persistent per-chunk transcript cache so a re-run after a rate limit
        // resumes instead of re-sending (and re-counting) finished audio.
        let cache_dir = cache_dir_for(
            &app_handle,
            &path,
            &transcribe_model,
            source_language.as_deref(),
            &format!("{range_tag}|{sarvam_mode}"),
        );
        let _ = std::fs::create_dir_all(&cache_dir);

        let mut segments: Vec<Segment> = Vec::new();

        if is_whisper {
            // Fully offline: one local whisper.cpp run over the whole range.
            segments = run_whisper(
                &app_handle,
                &ffmpeg,
                whisper_exe.as_deref().unwrap_or(""),
                whisper_model.as_deref().unwrap_or(""),
                &path,
                eff_start,
                eff_end,
                source_language.as_deref(),
                whisper_english_inline,
                &work,
            )?;
        } else {
        // Break chunks at natural pauses (silence) rather than on a fixed grid, so
        // a song's tail and the dialogue after it fall into separate chunks — the
        // dialogue chunk then starts at the real pause instead of ~20s too early.
        let silence_points = if is_sarvam || is_gemini {
            detect_silence_points(&ffmpeg, &path, eff_start, eff_end)
        } else {
            Vec::new()
        };
        let chunk_ranges = build_chunk_ranges(eff_start, eff_end, chunk_secs, &silence_points);
        let total_chunks = chunk_ranges.len().max(1);
        let mut pace_secs: u64 = 0;

        for (chunk, &(start, chunk_end)) in chunk_ranges.iter().enumerate() {
            if cancelled() {
                let _ = std::fs::remove_dir_all(&work);
                return Err("Cancelled".to_string());
            }
            emit_progress(&app_handle, "transcribe", chunk, total_chunks);
            let dur = chunk_end - start;
            if dur <= 0.1 {
                continue;
            }

            let chunk_cache = cache_dir.join(format!("chunk_{chunk:04}.json"));
            // Chunk-local (0-based) segments, from cache or a fresh transcription.
            let local: Vec<Segment> = if let Ok(txt) = std::fs::read_to_string(&chunk_cache)
            {
                serde_json::from_str(&txt).unwrap_or_default()
            } else {
                let clip_ext = if is_gemini { "flac" } else { "m4a" };
                let clip = work.join(format!("chunk_{chunk:04}.{clip_ext}"));
                extract_chunk(&ffmpeg, &path, start, dur, &clip)?;
                // Find the real spoken window so that any chunk which comes back
                // without word timestamps (songs, music-heavy passages) has its
                // distributed lines placed during speech, not leading silence.
                // Harmless when timestamps do arrive (words_to_segments ignores it).
                let speech_window = if is_sarvam {
                    detect_speech_window(&ffmpeg, &clip, dur)
                } else {
                    (0.0, dur)
                };
                let segs = retry_transcription(
                    &app_handle,
                    || {
                        if is_sarvam {
                            transcribe_chunk_sarvam(
                                &client,
                                &transcribe_base,
                                &transcribe_key,
                                &transcribe_model,
                                sarvam_mode,
                                sarvam_lang.as_deref(),
                                &clip,
                                speech_window,
                            )
                        } else if is_gemini {
                            transcribe_chunk_gemini(
                                &client,
                                &transcribe_base,
                                &transcribe_key,
                                &transcribe_model,
                                &clip,
                                source_language.as_deref(),
                                target,
                                dur,
                            )
                        } else {
                            transcribe_chunk(
                                &client,
                                &transcribe_url,
                                &transcribe_key,
                                &transcribe_model,
                                &clip,
                                source_language.as_deref(),
                            )
                        }
                    },
                    chunk,
                    total_chunks,
                    &mut pace_secs,
                )?;
                let _ = std::fs::remove_file(&clip);
                let _ =
                    std::fs::write(&chunk_cache, serde_json::to_string(&segs).unwrap_or_default());
                segs
            };
            for mut seg in local {
                seg.start += start;
                seg.end += start;
                segments.push(seg);
            }
        }
        }

        if segments.is_empty() {
            let _ = std::fs::remove_dir_all(&work);
            return Err("No speech was transcribed".to_string());
        }

        // Optional translation of the recognised lines.
        let mut lang_suffix = source_language
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "auto")
            .unwrap_or("orig")
            .to_string();

        if sarvam_direct_english {
            // Sarvam already produced English — no LLM translation step needed.
            lang_suffix = "en".to_string();
        } else if whisper_english_inline {
            // whisper --translate already produced English offline.
            lang_suffix = "en".to_string();
        } else if gemini_inline_translate {
            // Gemini already produced the target-language track in one call.
            lang_suffix = target.unwrap_or("en").to_string();
        } else if let Some(target) = translate_to
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            // Snapshot the lines first so we're not borrowing `segments` while
            // writing translations back into it.
            let texts: Vec<String> = segments.iter().map(|s| s.text.clone()).collect();
            // Sarvam can translate with the same key it transcribed with, so the
            // accurate-timing path needs no separate chat provider for Indic↔English.
            let sarvam_target = if is_sarvam {
                sarvam_language_code(Some(target))
            } else {
                None
            };
            if let Some(tgt) = sarvam_target {
                let src = sarvam_lang.clone().unwrap_or_else(|| "auto".to_string());
                // Keep each request under mayura's ~1000-char limit.
                let mut ranges: Vec<(usize, usize)> = Vec::new();
                let (mut start, mut chars) = (0usize, 0usize);
                for (i, t) in texts.iter().enumerate() {
                    let len = t.chars().count() + 1;
                    if i > start && (chars + len > 900 || i - start >= 12) {
                        ranges.push((start, i));
                        start = i;
                        chars = 0;
                    }
                    chars += len;
                }
                if start < texts.len() {
                    ranges.push((start, texts.len()));
                }
                let total = ranges.len();
                for (bi, (s, e)) in ranges.into_iter().enumerate() {
                    if cancelled() {
                        let _ = std::fs::remove_dir_all(&work);
                        return Err("Cancelled".to_string());
                    }
                    emit_progress(&app_handle, "translate", bi, total);
                    let translated = translate_batch_sarvam(
                        &client,
                        &transcribe_base,
                        &transcribe_key,
                        &src,
                        &tgt,
                        &texts[s..e],
                    );
                    for (i, line) in translated.into_iter().enumerate() {
                        if let Some(seg) = segments.get_mut(s + i) {
                            seg.text = line;
                        }
                    }
                }
            } else {
                let chat_url = format!(
                    "{}/chat/completions",
                    api_root(chat_base.as_deref().unwrap_or(""))
                );
                let key = chat_key.clone().unwrap_or_default();
                let model = chat_model.clone().unwrap_or_default();
                if chat_base.as_deref().unwrap_or("").is_empty() || key.is_empty() {
                    return Err(
                        "Translation needs the chat AI configured (Settings → AI Enhance)"
                            .to_string(),
                    );
                }
                let batches = texts.len().div_ceil(TRANSLATE_BATCH);
                for bi in 0..batches {
                    if cancelled() {
                        let _ = std::fs::remove_dir_all(&work);
                        return Err("Cancelled".to_string());
                    }
                    emit_progress(&app_handle, "translate", bi, batches);
                    let base = bi * TRANSLATE_BATCH;
                    let end = (base + TRANSLATE_BATCH).min(texts.len());
                    let translated = translate_batch(
                        &client,
                        &chat_url,
                        &key,
                        &model,
                        target,
                        &texts[base..end],
                    );
                    for (i, line) in translated.into_iter().enumerate() {
                        if let Some(seg) = segments.get_mut(base + i) {
                            seg.text = line;
                        }
                    }
                }
            }
            lang_suffix = target.to_string();
        }

        let _ = is_range;
        // One accumulating file per language, so generating another time-range
        // adds to the previous subtitles instead of replacing them. Drop any
        // existing lines that fall inside *this* run's range (so re-generating a
        // range refreshes it), keep the rest, then splice the new lines in.
        let out_path = output_srt_path(&app_handle, &path, &lang_suffix);
        let mut merged: Vec<Segment> = std::fs::read_to_string(&out_path)
            .ok()
            .map(|c| parse_srt(&c))
            .unwrap_or_default();
        merged.retain(|s| s.end <= eff_start + 0.05 || s.start >= eff_end - 0.05);
        merged.extend(segments.iter().cloned());
        merged.sort_by(|a, b| {
            a.start
                .partial_cmp(&b.start)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let srt = build_srt(&merged);
        std::fs::write(&out_path, srt)
            .map_err(|e| format!("Failed to write subtitles: {e}"))?;
        let _ = std::fs::remove_dir_all(&work);
        // Job finished — the transcript cache is no longer needed.
        let _ = std::fs::remove_dir_all(&cache_dir);
        emit_progress(&app_handle, "done", 1, 1);

        let file_name = out_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        Ok(SubtitleResult {
            srt_path: out_path.to_string_lossy().to_string(),
            file_name,
            line_count: merged.len(),
        })
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}

/// Translate an existing `.srt` file into another language, preserving the
/// original timings. Reuses Sarvam's own translator for Indic↔English targets,
/// otherwise a chat model (Groq / OpenAI / Gemini / AI Enhance).
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn translate_subtitle_file(
    app: tauri::AppHandle,
    srt_path: String,
    target_language: String,
    engine: Option<String>,
    transcribe_base: String,
    transcribe_key: String,
    source_language: Option<String>,
    chat_base: Option<String>,
    chat_key: Option<String>,
    chat_model: Option<String>,
) -> Result<SubtitleResult, String> {
    let target = target_language.trim().to_string();
    if target.is_empty() {
        return Err("Choose a language to translate into".to_string());
    }
    let app_handle = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let content = std::fs::read_to_string(&srt_path)
            .map_err(|e| format!("Couldn't read the subtitle file: {e}"))?;
        let mut segments = parse_srt(&content);
        if segments.is_empty() {
            return Err("No subtitles found in that file".to_string());
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

        let is_sarvam = engine.as_deref() == Some("sarvam");
        let sarvam_target = if is_sarvam {
            sarvam_language_code(Some(&target))
        } else {
            None
        };
        let texts: Vec<String> = segments.iter().map(|s| s.text.clone()).collect();

        if let Some(tgt) = sarvam_target {
            let src = sarvam_language_code(source_language.as_deref())
                .unwrap_or_else(|| "auto".to_string());
            let mut ranges: Vec<(usize, usize)> = Vec::new();
            let (mut start, mut chars) = (0usize, 0usize);
            for (i, t) in texts.iter().enumerate() {
                let len = t.chars().count() + 1;
                if i > start && (chars + len > 900 || i - start >= 12) {
                    ranges.push((start, i));
                    start = i;
                    chars = 0;
                }
                chars += len;
            }
            if start < texts.len() {
                ranges.push((start, texts.len()));
            }
            let total = ranges.len();
            for (bi, (s, e)) in ranges.into_iter().enumerate() {
                if cancelled() {
                    return Err("Cancelled".to_string());
                }
                emit_progress(&app_handle, "translate", bi, total);
                let translated = translate_batch_sarvam(
                    &client,
                    &transcribe_base,
                    &transcribe_key,
                    &src,
                    &tgt,
                    &texts[s..e],
                );
                for (i, line) in translated.into_iter().enumerate() {
                    if let Some(seg) = segments.get_mut(s + i) {
                        seg.text = line;
                    }
                }
            }
        } else {
            let base = chat_base.unwrap_or_default();
            let key = chat_key.unwrap_or_default();
            let model = chat_model.unwrap_or_default();
            if base.is_empty() || key.is_empty() {
                return Err(
                    "Translation needs a chat model (Sarvam/Groq/OpenAI/Gemini or AI Enhance)."
                        .to_string(),
                );
            }
            let chat_url = format!("{}/chat/completions", api_root(&base));
            let total = texts.len().div_ceil(TRANSLATE_BATCH);
            for bi in 0..total {
                if cancelled() {
                    return Err("Cancelled".to_string());
                }
                emit_progress(&app_handle, "translate", bi, total);
                let b = bi * TRANSLATE_BATCH;
                let e = (b + TRANSLATE_BATCH).min(texts.len());
                let translated =
                    translate_batch(&client, &chat_url, &key, &model, &target, &texts[b..e]);
                for (i, line) in translated.into_iter().enumerate() {
                    if let Some(seg) = segments.get_mut(b + i) {
                        seg.text = line;
                    }
                }
            }
        }

        emit_progress(&app_handle, "done", 1, 1);

        // Name the output next to the source, dropping a trailing ".ai" and any
        // trailing language token so suffixes don't pile up.
        let src_file = Path::new(&srt_path);
        let stem = src_file
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "subtitles".to_string());
        let mut base_stem = stem.as_str();
        if let Some(s) = base_stem.strip_suffix(".ai") {
            base_stem = s;
        }
        if let Some((head, tail)) = base_stem.rsplit_once('.') {
            if (2..=3).contains(&tail.len()) && tail.chars().all(|c| c.is_ascii_alphabetic()) {
                base_stem = head;
            }
        }
        let name = format!("{base_stem}.{target}.srt");
        let out_path = src_file
            .parent()
            .map(|p| p.join(&name))
            .unwrap_or_else(|| PathBuf::from(&name));
        let srt = build_srt(&segments);
        std::fs::write(&out_path, srt).map_err(|e| format!("Failed to write subtitles: {e}"))?;
        let file_name = out_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        Ok(SubtitleResult {
            srt_path: out_path.to_string_lossy().to_string(),
            file_name,
            line_count: segments.len(),
        })
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}

// ---------------------------------------------------------------------------
// Subtitle sync — re-time an existing .srt to this video.
// ---------------------------------------------------------------------------

/// Speech intervals (absolute seconds) across [start,end], as the complement of
/// silencedetect's silences. Quick sync aligns the subtitle's cue pattern to
/// this; smart sync uses it to find gaps to fill and orphans to drop.
fn detect_speech_intervals(ffmpeg: &Path, input: &str, start: f64, end: f64) -> Vec<(f64, f64)> {
    let dur = end - start;
    if dur <= 0.0 {
        return vec![];
    }
    let output = quiet_command(ffmpeg)
        .args(["-hide_banner", "-nostats", "-ss"])
        .arg(format!("{start}"))
        .arg("-t")
        .arg(format!("{dur}"))
        .arg("-i")
        .arg(input)
        .args(["-vn", "-af", "silencedetect=noise=-30dB:d=0.4", "-f", "null", "-"])
        .output();
    let Ok(out) = output else {
        return vec![(start, end)];
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut silences: Vec<(f64, f64)> = Vec::new();
    let mut pending: Option<f64> = None;
    for line in stderr.lines() {
        if let Some(v) = parse_silence_seconds(line, "silence_start:") {
            pending = Some(v + start);
        }
        if let Some(v) = parse_silence_seconds(line, "silence_end:") {
            let s = pending.take().unwrap_or(start);
            silences.push((s, v + start));
        }
    }
    if let Some(s) = pending {
        silences.push((s, end));
    }
    // Speech = the gaps between silences.
    let mut speech = Vec::new();
    let mut cursor = start;
    for (s, e) in silences {
        if s > cursor {
            speech.push((cursor, s));
        }
        cursor = cursor.max(e);
    }
    if cursor < end {
        speech.push((cursor, end));
    }
    speech
}

const SYNC_BIN: f64 = 0.25; // 250 ms resolution for the correlation
// Common film/PAL/NTSC frame-rate conversion ratios (plus identity).
const SYNC_SCALES: [f64; 7] = [
    1.0,
    24.0 / 25.0,
    25.0 / 24.0,
    (24000.0 / 1001.0) / 25.0,
    25.0 / (24000.0 / 1001.0),
    (24000.0 / 1001.0) / 24.0,
    24.0 / (24000.0 / 1001.0),
];

/// Rasterise time intervals (scaled) into a coarse on/off signal.
fn sync_signal(intervals: &[(f64, f64)], n: usize, scale: f64) -> Vec<u8> {
    let mut sig = vec![0u8; n];
    for &(s, e) in intervals {
        let a = ((s * scale) / SYNC_BIN).floor().max(0.0) as usize;
        let b = ((e * scale) / SYNC_BIN).ceil() as usize;
        for slot in sig.iter_mut().take(b.min(n)).skip(a) {
            *slot = 1;
        }
    }
    sig
}

/// Best (scale, offset_seconds, score) aligning the subtitle cue pattern to the
/// video's speech pattern, via a binned cross-correlation over candidate scales.
fn best_alignment(
    video_speech: &[(f64, f64)],
    sub_cues: &[(f64, f64)],
    duration: f64,
) -> (f64, f64, f64) {
    let n = ((duration / SYNC_BIN).ceil() as usize).max(1);
    let vid = sync_signal(video_speech, n, 1.0);
    let vid_sum: f64 = vid.iter().map(|&x| x as f64).sum();
    let max_off = (120.0 / SYNC_BIN) as isize; // search +/- 120 s
    let mut best = (1.0f64, 0.0f64, -1.0f64);
    for &scale in SYNC_SCALES.iter() {
        let sub = sync_signal(sub_cues, n, scale);
        let sub_sum: f64 = sub.iter().map(|&x| x as f64).sum();
        if sub_sum < 1.0 {
            continue;
        }
        for off in -max_off..=max_off {
            let (lo, hi) = if off >= 0 {
                (off as usize, n)
            } else {
                (0usize, (n as isize + off).max(0) as usize)
            };
            let mut inter = 0f64;
            for i in lo..hi {
                let j = (i as isize - off) as usize;
                inter += (vid[i] & sub[j]) as f64;
            }
            // Dice coefficient — rewards real overlap, not just large signals.
            let score = 2.0 * inter / (vid_sum + sub_sum);
            if score > best.2 {
                best = (scale, off as f64 * SYNC_BIN, score);
            }
        }
    }
    best
}

fn synced_output_path(video: &str, suffix: &str) -> PathBuf {
    let stem = Path::new(video)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "subtitles".to_string());
    let name = format!("{stem}.{suffix}.srt");
    Path::new(video)
        .parent()
        .map(|p| p.join(&name))
        .unwrap_or_else(|| PathBuf::from(name))
}

/// Quick sync: no AI. Find the global offset + frame-rate scale that best lines
/// the subtitle's cue pattern up with where the video actually has speech.
#[tauri::command]
pub(crate) async fn quick_sync_subtitle(
    app: tauri::AppHandle,
    srt_path: String,
    video_path: String,
) -> Result<SubtitleResult, String> {
    if is_remote(&video_path) {
        return Err("Subtitle sync needs a local video file".to_string());
    }
    let configured = crate::store::ui_state_store::load_setting_value(&app, FFMPEG_PATH_SETTING)
        .ok()
        .flatten();
    tauri::async_runtime::spawn_blocking(move || {
        let ffmpeg = crate::mpv::find_ffmpeg(configured.as_deref())
            .ok_or_else(|| "Subtitle sync needs ffmpeg (Settings → Advanced).".to_string())?;
        let duration = crate::mpv::probe_duration(&video_path).unwrap_or(0.0);
        if duration <= 0.0 {
            return Err("Could not read the video's duration".to_string());
        }
        let content = std::fs::read_to_string(&srt_path)
            .map_err(|e| format!("Couldn't read the subtitle file: {e}"))?;
        let mut segments = parse_srt(&content);
        if segments.is_empty() {
            return Err("No subtitles found in that file".to_string());
        }

        emit_progress(&app, "transcribe", 0, 1);
        let speech = detect_speech_intervals(&ffmpeg, &video_path, 0.0, duration);
        if speech.is_empty() {
            return Err("No speech detected in the video to sync against".to_string());
        }
        let cues: Vec<(f64, f64)> = segments.iter().map(|s| (s.start, s.end)).collect();
        let (scale, offset, score) = best_alignment(&speech, &cues, duration);
        if score < 0.15 {
            return Err(
                "Couldn't confidently line these subtitles up with the audio — the file may be for a very different cut. Try Smart sync.".to_string(),
            );
        }
        for seg in segments.iter_mut() {
            seg.start = (seg.start * scale + offset).max(0.0);
            seg.end = (seg.end * scale + offset).max(seg.start + 0.2);
        }
        segments.retain(|s| s.start < duration + 1.0);
        segments.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));

        let out_path = synced_output_path(&video_path, "synced");
        std::fs::write(&out_path, build_srt(&segments))
            .map_err(|e| format!("Failed to write subtitles: {e}"))?;
        emit_progress(&app, "done", 1, 1);
        let file_name = out_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        Ok(SubtitleResult {
            srt_path: out_path.to_string_lossy().to_string(),
            file_name,
            line_count: segments.len(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

// --- Smart sync: content-aware alignment --------------------------------------

/// Bundle of everything needed to transcribe (and translate) a region.
struct SyncEngine {
    engine: String,
    transcribe_base: String,
    transcribe_key: String,
    transcribe_model: String,
    source_language: Option<String>,
    sub_language: String,
    chat_base: Option<String>,
    chat_key: Option<String>,
    chat_model: Option<String>,
    whisper_exe: Option<String>,
    whisper_model: Option<String>,
}

/// Word set of a line, lower-cased and stripped to alphanumerics, for fuzzy
/// cross-translation matching.
fn word_set(text: &str) -> std::collections::HashSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(|w| w.to_string())
        .collect()
}

/// Translate lines to the subtitle language (Sarvam's own translator, else chat).
fn translate_texts(cfg: &SyncEngine, client: &reqwest::blocking::Client, texts: &[String]) -> Vec<String> {
    if texts.is_empty() {
        return vec![];
    }
    let sarvam_target = if cfg.engine == "sarvam" {
        sarvam_language_code(Some(&cfg.sub_language))
    } else {
        None
    };
    if let Some(tgt) = sarvam_target {
        let src = sarvam_language_code(cfg.source_language.as_deref())
            .unwrap_or_else(|| "auto".to_string());
        return translate_batch_sarvam(client, &cfg.transcribe_base, &cfg.transcribe_key, &src, &tgt, texts);
    }
    let base = cfg.chat_base.clone().unwrap_or_default();
    let key = cfg.chat_key.clone().unwrap_or_default();
    let model = cfg.chat_model.clone().unwrap_or_default();
    if base.is_empty() || key.is_empty() {
        return texts.to_vec();
    }
    let url = format!("{}/chat/completions", api_root(&base));
    translate_batch(client, &url, &key, &model, &cfg.sub_language, texts)
}

/// Transcribe [start, start+dur] of the video and return segments in the
/// subtitle language, absolutely timed.
fn transcribe_region(
    cfg: &SyncEngine,
    app: &tauri::AppHandle,
    client: &reqwest::blocking::Client,
    ffmpeg: &Path,
    video: &str,
    start: f64,
    dur: f64,
    work: &Path,
) -> Result<Vec<Segment>, String> {
    let is_gemini = cfg.engine == "gemini";
    let is_sarvam = cfg.engine == "sarvam";
    let is_whisper = cfg.engine == "whisper";
    let target_is_english = cfg.sub_language.eq_ignore_ascii_case("en");

    if is_whisper {
        let mut segs = run_whisper(
            app,
            ffmpeg,
            cfg.whisper_exe.as_deref().unwrap_or(""),
            cfg.whisper_model.as_deref().unwrap_or(""),
            video,
            start,
            start + dur,
            cfg.source_language.as_deref(),
            target_is_english,
            work,
        )?;
        if !target_is_english {
            let texts: Vec<String> = segs.iter().map(|s| s.text.clone()).collect();
            for (i, t) in translate_texts(cfg, client, &texts).into_iter().enumerate() {
                if let Some(s) = segs.get_mut(i) {
                    s.text = t;
                }
            }
        }
        return Ok(segs);
    }

    let ext = if is_gemini { "flac" } else { "m4a" };
    let clip = work.join(format!("sync_{}.{ext}", (start * 1000.0) as i64));
    extract_chunk(ffmpeg, video, start, dur, &clip)?;
    let mut local: Vec<Segment> = if is_gemini {
        transcribe_chunk_gemini(
            client,
            &cfg.transcribe_base,
            &cfg.transcribe_key,
            &cfg.transcribe_model,
            &clip,
            cfg.source_language.as_deref(),
            Some(&cfg.sub_language),
            dur,
        )?
    } else if is_sarvam {
        let window = detect_speech_window(ffmpeg, &clip, dur);
        let sarvam_lang = sarvam_language_code(cfg.source_language.as_deref());
        transcribe_chunk_sarvam(
            client,
            &cfg.transcribe_base,
            &cfg.transcribe_key,
            &cfg.transcribe_model,
            "transcribe",
            sarvam_lang.as_deref(),
            &clip,
            window,
        )?
    } else {
        let url = format!("{}/audio/transcriptions", api_root(&cfg.transcribe_base));
        transcribe_chunk(
            client,
            &url,
            &cfg.transcribe_key,
            &cfg.transcribe_model,
            &clip,
            cfg.source_language.as_deref(),
        )?
    };
    let _ = std::fs::remove_file(&clip);
    // Gemini already returned the target language; others may need translation.
    if !is_gemini {
        let src_is_target = cfg
            .source_language
            .as_deref()
            .map(|l| l.eq_ignore_ascii_case(&cfg.sub_language))
            .unwrap_or(false);
        if !src_is_target {
            let texts: Vec<String> = local.iter().map(|s| s.text.clone()).collect();
            for (i, t) in translate_texts(cfg, client, &texts).into_iter().enumerate() {
                if let Some(s) = local.get_mut(i) {
                    s.text = t;
                }
            }
        }
    }
    for s in local.iter_mut() {
        s.start += start;
        s.end += start;
    }
    Ok(local)
}

/// Transcribe the whole video into a dense, precisely-timed reference in the
/// subtitle language. This is the "ground-truth clock" the original subtitle is
/// aligned against.
fn build_reference(
    cfg: &SyncEngine,
    app: &tauri::AppHandle,
    client: &reqwest::blocking::Client,
    ffmpeg: &Path,
    video: &str,
    duration: f64,
    work: &Path,
) -> Vec<Segment> {
    let chunk_secs = match cfg.engine.as_str() {
        "sarvam" => 28.0,
        "gemini" => 60.0,
        "whisper" => duration.max(1.0), // one whisper run over the whole film
        _ => CHUNK_SECS,
    };
    let silence = if cfg.engine == "sarvam" || cfg.engine == "gemini" {
        detect_silence_points(ffmpeg, video, 0.0, duration)
    } else {
        Vec::new()
    };
    let ranges = build_chunk_ranges(0.0, duration, chunk_secs, &silence);
    let total = ranges.len().max(1);
    let mut reference: Vec<Segment> = Vec::new();
    for (i, &(a, b)) in ranges.iter().enumerate() {
        if cancelled() {
            break;
        }
        emit_progress(app, "transcribe", i, total);
        if let Ok(mut segs) = transcribe_region(cfg, app, client, ffmpeg, video, a, b - a, work) {
            reference.append(&mut segs);
        }
    }
    reference.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
    reference
}

/// Align the original subtitle lines to the reference by text (Needleman-Wunsch),
/// then rebuild: matched lines keep their (human) text at the reference's precise
/// time; short unmatched runs are interpolated (an ASR miss, not a cut); long
/// unmatched runs are dropped (a cut scene); reference lines with no match are
/// AI-filled (an added scene). De-duplicates near-identical fills.
fn align_subs(orig: &[Segment], reference: &[Segment]) -> Vec<Segment> {
    let n = orig.len();
    let m = reference.len();
    if m == 0 || n == 0 {
        return orig.to_vec();
    }
    let ow: Vec<std::collections::HashSet<String>> =
        orig.iter().map(|s| word_set(&s.text)).collect();
    let rw: Vec<std::collections::HashSet<String>> =
        reference.iter().map(|s| word_set(&s.text)).collect();
    let sim = |i: usize, j: usize| -> f64 {
        let (a, b) = (&ow[i], &rw[j]);
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }
        let inter = a.intersection(b).count() as f64;
        2.0 * inter / (a.len() as f64 + b.len() as f64)
    };
    const TH: f64 = 0.34; // min similarity to call it a match
    const GAP: f64 = 0.12; // penalty for skipping a line on either side
    const NEG: f64 = -1.0e6;

    let mut dp = vec![vec![0f64; m + 1]; n + 1];
    let mut bt = vec![vec![0u8; m + 1]; n + 1]; // 1=match, 2=skip orig, 3=skip ref
    for i in 1..=n {
        dp[i][0] = -(i as f64) * GAP;
        bt[i][0] = 2;
    }
    for j in 1..=m {
        dp[0][j] = -(j as f64) * GAP;
        bt[0][j] = 3;
    }
    for i in 1..=n {
        for j in 1..=m {
            let s = sim(i - 1, j - 1);
            let m_score = if s >= TH { dp[i - 1][j - 1] + s } else { NEG };
            let up = dp[i - 1][j] - GAP;
            let left = dp[i][j - 1] - GAP;
            if m_score >= up && m_score >= left {
                dp[i][j] = m_score;
                bt[i][j] = 1;
            } else if up >= left {
                dp[i][j] = up;
                bt[i][j] = 2;
            } else {
                dp[i][j] = left;
                bt[i][j] = 3;
            }
        }
    }

    // Backtrack into forward-ordered ops.
    enum Op {
        Match(usize, usize),
        DropO(usize),
        FillR(usize),
    }
    let mut ops: Vec<Op> = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        match bt[i][j] {
            1 => {
                ops.push(Op::Match(i - 1, j - 1));
                i -= 1;
                j -= 1;
            }
            2 => {
                ops.push(Op::DropO(i - 1));
                i -= 1;
            }
            _ => {
                ops.push(Op::FillR(j - 1));
                j -= 1;
            }
        }
    }
    ops.reverse();

    // Helper: map an original time onto the reference clock, given the last and
    // next matched (orig_time -> ref_time) pairs.
    let interp = |ot: f64, p: Option<(f64, f64)>, q: Option<(f64, f64)>| -> f64 {
        match (p, q) {
            (Some((o0, r0)), Some((o1, r1))) if (o1 - o0).abs() > 0.01 => {
                r0 + (ot - o0) / (o1 - o0) * (r1 - r0)
            }
            (Some((o0, r0)), _) => ot + (r0 - o0),
            (_, Some((o1, r1))) => ot + (r1 - o1),
            _ => ot,
        }
    };

    // Each output line is tagged with whether it's an AI fill (true) or your
    // original text (false).
    let mut out: Vec<(Segment, bool)> = Vec::new();
    let push_line = |out: &mut Vec<(Segment, bool)>, start: f64, end: f64, text: &str, fill: bool| {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        // Skip a line that just repeats its neighbour (different line splitting).
        if let Some((prev, _)) = out.last() {
            let a = word_set(&prev.text);
            let b = word_set(text);
            if !a.is_empty() && !b.is_empty() {
                let inter = a.intersection(&b).count() as f64;
                if 2.0 * inter / (a.len() as f64 + b.len() as f64) > 0.6 {
                    return;
                }
            }
        }
        out.push((
            Segment {
                start: start.max(0.0),
                end: end.max(start + 0.3),
                text: text.to_string(),
            },
            fill,
        ));
    };

    let mut pending_drop: Vec<usize> = Vec::new(); // buffered orig-only run
    let mut last_match: Option<(f64, f64)> = None; // (orig_time, ref_time)

    let flush_drops = |out: &mut Vec<(Segment, bool)>,
                       run: &mut Vec<usize>,
                       prev: Option<(f64, f64)>,
                       next: Option<(f64, f64)>| {
        if run.is_empty() {
            return;
        }
        // A short unmatched run between matches is likely an ASR miss → keep,
        // interpolated. A long run is a cut scene → drop.
        if run.len() < 4 {
            for &oi in run.iter() {
                let s = interp(orig[oi].start, prev, next);
                let e = interp(orig[oi].end, prev, next);
                push_line(out, s, e, &orig[oi].text, false);
            }
        }
        run.clear();
    };

    for op in ops.iter() {
        match *op {
            Op::Match(oi, rj) => {
                flush_drops(&mut out, &mut pending_drop, last_match, Some((orig[oi].start, reference[rj].start)));
                push_line(&mut out, reference[rj].start, reference[rj].end, &orig[oi].text, false);
                last_match = Some((orig[oi].start, reference[rj].start));
            }
            Op::DropO(oi) => pending_drop.push(oi),
            Op::FillR(rj) => {
                flush_drops(&mut out, &mut pending_drop, last_match, None);
                push_line(&mut out, reference[rj].start, reference[rj].end, &reference[rj].text, true);
            }
        }
    }
    flush_drops(&mut out, &mut pending_drop, last_match, None);

    out.sort_by(|a, b| a.0.start.partial_cmp(&b.0.start).unwrap_or(std::cmp::Ordering::Equal));

    // Never show two cues at once. When a human line and an AI fill collide on the
    // same moment (the same dialogue in two translations), keep the human one;
    // otherwise clip the earlier cue so they play back-to-back.
    let mut resolved: Vec<(Segment, bool)> = Vec::new();
    for (seg, fill) in out.into_iter() {
        if let Some((last, last_fill)) = resolved.last_mut() {
            if seg.start < last.end - 0.15 {
                let overlap = (last.end.min(seg.end) - seg.start).max(0.0);
                let shorter = (seg.end - seg.start).min(last.end - last.start).max(0.1);
                let strong = overlap / shorter > 0.4;
                if strong && fill != *last_fill {
                    if fill {
                        continue; // drop the AI fill, keep the human line
                    }
                    *last = seg; // replace an AI line with the human one
                    *last_fill = false;
                    continue;
                }
                last.end = (seg.start - 0.05).max(last.start + 0.3);
            }
        }
        resolved.push((seg, fill));
    }

    resolved.into_iter().map(|(s, _)| s).collect()
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn smart_sync_subtitle(
    app: tauri::AppHandle,
    srt_path: String,
    video_path: String,
    engine: Option<String>,
    transcribe_base: String,
    transcribe_key: String,
    transcribe_model: String,
    source_language: Option<String>,
    sub_language: Option<String>,
    chat_base: Option<String>,
    chat_key: Option<String>,
    chat_model: Option<String>,
    whisper_exe: Option<String>,
    whisper_model: Option<String>,
) -> Result<SubtitleResult, String> {
    if is_remote(&video_path) {
        return Err("Subtitle sync needs a local video file".to_string());
    }
    let configured = crate::store::ui_state_store::load_setting_value(&app, FFMPEG_PATH_SETTING)
        .ok()
        .flatten();
    CANCEL.store(false, Ordering::SeqCst);
    let app_handle = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let ffmpeg = crate::mpv::find_ffmpeg(configured.as_deref())
            .ok_or_else(|| "Subtitle sync needs ffmpeg (Settings → Advanced).".to_string())?;
        let duration = crate::mpv::probe_duration(&video_path).unwrap_or(0.0);
        if duration <= 0.0 {
            return Err("Could not read the video's duration".to_string());
        }
        let content = std::fs::read_to_string(&srt_path)
            .map_err(|e| format!("Couldn't read the subtitle file: {e}"))?;
        let mut subs = parse_srt(&content);
        if subs.is_empty() {
            return Err("No subtitles found in that file".to_string());
        }
        subs.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));

        let work = app_handle
            .path()
            .app_cache_dir()
            .map_err(|e| e.to_string())?
            .join("ai_subtitles_work");
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;

        let cfg = SyncEngine {
            engine: engine.clone().unwrap_or_else(|| "gemini".to_string()),
            transcribe_base,
            transcribe_key,
            transcribe_model,
            source_language,
            sub_language: sub_language
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "en".to_string()),
            chat_base,
            chat_key,
            chat_model,
            whisper_exe,
            whisper_model,
        };
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

        // Transcribe the whole video into a dense, precisely-timed reference…
        let reference = build_reference(&cfg, &app_handle, &client, &ffmpeg, &video_path, duration, &work);
        if cancelled() {
            let _ = std::fs::remove_dir_all(&work);
            return Err("Cancelled".to_string());
        }

        if reference.len() < 5 {
            // Reference too sparse to align against — fall back to a global fit.
            let speech = detect_speech_intervals(&ffmpeg, &video_path, 0.0, duration);
            let cues: Vec<(f64, f64)> = subs.iter().map(|s| (s.start, s.end)).collect();
            let (scale, offset, score) = best_alignment(&speech, &cues, duration);
            if score < 0.15 {
                let _ = std::fs::remove_dir_all(&work);
                return Err(
                    "Couldn't transcribe enough of the video to sync against — check the AI engine/key and languages.".to_string(),
                );
            }
            for s in subs.iter_mut() {
                s.start = (s.start * scale + offset).max(0.0);
                s.end = (s.end * scale + offset).max(s.start + 0.2);
            }
        } else {
            // …then align the original subtitle line-by-line onto it.
            subs = align_subs(&subs, &reference);
        }

        subs.retain(|s| !s.text.trim().is_empty() && s.start < duration + 1.0);
        subs.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
        if subs.is_empty() {
            let _ = std::fs::remove_dir_all(&work);
            return Err("Nothing left after syncing — the match was too poor".to_string());
        }

        let out_path = synced_output_path(&video_path, "synced");
        std::fs::write(&out_path, build_srt(&subs))
            .map_err(|e| format!("Failed to write subtitles: {e}"))?;
        let _ = std::fs::remove_dir_all(&work);
        emit_progress(&app_handle, "done", 1, 1);
        let file_name = out_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        Ok(SubtitleResult {
            srt_path: out_path.to_string_lossy().to_string(),
            file_name,
            line_count: subs.len(),
        })
    })
    .await
    .map_err(|e| e.to_string())?;

    result
}

fn is_remote(path: &str) -> bool {
    let l = path.to_ascii_lowercase();
    l.starts_with("http://")
        || l.starts_with("https://")
        || l.starts_with("rtsp://")
        || l.starts_with("rtmp://")
        || l.starts_with("smb://")
        || l.starts_with("webdav://")
}
