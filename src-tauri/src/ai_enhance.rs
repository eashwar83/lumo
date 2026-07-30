//! Cloud AI colour correction.
//!
//! Samples a handful of frames from the current video, sends them to a vision
//! model, and asks for per-channel tone curves plus a saturation / temperature /
//! tint nudge. The whole exchange runs in the backend so the user's API key
//! never touches the webview and there's no browser CORS to fight.
//!
//! Five of the six supported providers (OpenAI, Gemini, Kimi/Moonshot,
//! Qwen/DashScope, DeepSeek) speak the OpenAI-compatible chat API, so they
//! share one request path; only Anthropic (Claude) needs its own shape.
//! DeepSeek's public models are text-only and will report that they can't read
//! images — that's surfaced, not hidden.

use base64::Engine as _;
use image::ImageEncoder;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;
use tauri::Manager;
use std::time::Duration;

const FRAME_COUNT: u32 = 9;
const FRAME_WIDTH: u32 = 512;
const REQUEST_TIMEOUT_SECS: u64 = 120;

/// Reference images uploaded alongside the frames: how many, and the longest
/// edge they're downscaled to before upload / for the on-screen chip preview.
const MAX_REFERENCE_IMAGES: usize = 3;
const REFERENCE_UPLOAD_DIM: u32 = 512;
const REFERENCE_THUMB_DIM: u32 = 220;

/// Precedes the reference stills in the message so the model treats them as a
/// target look, not as more footage to correct.
const REFERENCE_LABEL: &str = "The following images are REFERENCE stills showing \
the look I want. Their subject matter is irrelevant — study only their colour \
palette, contrast, warmth/coolness, black level and overall mood, and grade the \
video toward that look while keeping it natural and free of clipping:";

const SYSTEM_PROMPT: &str = "You are a professional film colourist. You are given \
several frames sampled across one video. Judge the footage as a whole and design \
a gentle, tasteful colour correction that makes it look its best: fix exposure, \
contrast, colour casts and dull colour while keeping skin tones natural and \
avoiding clipping or over-saturation. Subtle corrections look best. Respond with \
ONLY a JSON object — no prose, no markdown fences.";

const DESCRIBE_SYSTEM: &str = "You are a film scene analyst. You are given a few \
frames sampled in order from one short clip of a film, and sometimes the dialogue \
spoken during it. Describe what happens in the clip: the setting, who is present, \
their actions and apparent emotions, and how the moment develops. Be concise \
(3–6 sentences), concrete and grounded — do not invent details you cannot see, \
and do not mention frames, images, or that you are an AI.";

const USER_PROMPT: &str = "Return a JSON object with exactly these keys:\n\
- \"rgb\", \"r\", \"g\", \"b\": each an array of 2 to 5 control points, where each \
point is [x, y] with x and y between 0 and 1, x strictly ascending, the first \
point's x = 0 and the last point's x = 1. \"rgb\" is the master tone curve; \
\"r\"/\"g\"/\"b\" are per-channel curves for white-balance/cast correction. The \
identity (no change) is [[0,0],[1,1]].\n\
- \"saturation\": integer from -100 to 100 (0 = unchanged).\n\
- \"temperature\": integer from -100 to 100 (negative = cooler/bluer, positive = \
warmer).\n\
- \"tint\": integer from -100 to 100 (negative = green, positive = magenta).\n\
- \"sharpen\": integer from 0 to 100 (0 = none). Add sharpening ONLY for genuinely \
soft or slightly out-of-focus footage; keep it subtle (usually 0-25). Leave 0 for \
already-crisp footage — over-sharpening looks harsh.\n\
- \"sharpenRadius\": integer from 1 to 30, meaningful only when sharpen > 0. Small \
values (2-4) sharpen fine edges; larger values add broad local-contrast glow. Use \
3 unless a wider radius is clearly warranted.\n\
- \"grain\": integer from 0 to 100 (0 = none). Add a little grain ONLY to mask \
visible banding or flat, overly-digital gradients; keep it low (usually 0-15). \
Leave 0 for footage that already has natural texture.\n\
- \"notes\": one short sentence describing the correction.\n\
Be conservative. Output JSON only.";

struct ProviderCfg {
    /// API "root" (e.g. https://host/v1). Endpoint paths are appended below.
    base_url: String,
    default_model: String,
    anthropic: bool,
}

/// Compute the chat and models endpoints from a base that may be either an API
/// root (…/v1) or a full chat/messages URL a user pasted.
fn endpoints(base: &str, anthropic: bool) -> (String, String) {
    let trimmed = base.trim().trim_end_matches('/');
    let root = trimmed
        .strip_suffix("/chat/completions")
        .or_else(|| trimmed.strip_suffix("/messages"))
        .or_else(|| trimmed.strip_suffix("/models"))
        .unwrap_or(trimmed);
    if anthropic {
        (format!("{root}/messages"), format!("{root}/models"))
    } else {
        (format!("{root}/chat/completions"), format!("{root}/models"))
    }
}

fn provider_config(
    provider: &str,
    model_override: Option<String>,
    base_override: Option<String>,
) -> Result<ProviderCfg, String> {
    let trimmed = provider.trim();
    let (default_base, model, anthropic) = match trimmed {
        "Gemini" => (
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "gemini-2.0-flash",
            false,
        ),
        "Claude" => ("https://api.anthropic.com/v1", "claude-sonnet-5", true),
        "OpenAI" => ("https://api.openai.com/v1", "gpt-4o", false),
        "Kimi (Moonshot)" | "Kimi" | "Moonshot" => (
            "https://api.moonshot.ai/v1",
            "moonshot-v1-8k-vision-preview",
            false,
        ),
        "Qwen" => (
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "qwen-vl-plus",
            false,
        ),
        "DeepSeek" => ("https://api.deepseek.com/v1", "deepseek-chat", false),
        "Grok (xAI)" | "Grok" => ("https://api.x.ai/v1", "grok-2-vision-1212", false),
        "Custom (OpenAI-compatible)" | "Custom" => ("", "", false),
        other => return Err(format!("Unknown AI provider: {other}")),
    };

    // An explicit base URL overrides the default for ANY provider — needed for
    // regional / workspace endpoints (e.g. Qwen international) and self-hosted.
    let base = base_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(default_base)
        .to_string();
    if base.is_empty() {
        return Err(
            "Set the base URL for this provider in Settings → AI Enhance".to_string(),
        );
    }

    let default_model = model_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(model)
        .to_string();

    Ok(ProviderCfg {
        base_url: base,
        default_model,
        anthropic,
    })
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiCurveResult {
    rgb: Vec<[f64; 2]>,
    r: Vec<[f64; 2]>,
    g: Vec<[f64; 2]>,
    b: Vec<[f64; 2]>,
    saturation: f64,
    temperature: f64,
    tint: f64,
    sharpen: f64,
    sharpen_radius: f64,
    grain: f64,
    notes: String,
}

/// Read the sampled frames as base64 JPEG strings.
fn collect_frames(dir: &Path) -> Vec<String> {
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .and_then(|x| x.to_str())
                        .map(|x| x.eq_ignore_ascii_case("jpg"))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
        .into_iter()
        .take(FRAME_COUNT as usize)
        .filter_map(|p| std::fs::read(p).ok())
        .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes))
        .collect()
}

/// Load an image file, downscale so its longest edge is `max_dim` (aspect
/// preserved), and return a base64 JPEG. Used for both the upload and the chip
/// preview. PNG/WebP/BMP/GIF/JPEG inputs are all decodable (see Cargo features).
fn encode_image_jpeg(path: &str, max_dim: u32) -> Result<String, String> {
    let img = image::open(path).map_err(|e| format!("Could not read image: {e}"))?;
    // `thumbnail` fits within the box while preserving aspect ratio.
    let rgb = img.thumbnail(max_dim, max_dim).to_rgb8();
    let mut buf: Vec<u8> = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 82)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("Failed to encode image: {e}"))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(buf))
}

/// Encode up to MAX_REFERENCE_IMAGES reference stills for upload, skipping any
/// that fail to decode rather than aborting the whole correction.
fn collect_reference_images(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .take(MAX_REFERENCE_IMAGES)
        .filter_map(|p| encode_image_jpeg(p, REFERENCE_UPLOAD_DIM).ok())
        .collect()
}

fn build_openai_request(
    model: &str,
    user_prompt: &str,
    frames: &[String],
    references: &[String],
    strict_openai: bool,
) -> Value {
    let mut content: Vec<Value> = vec![json!({ "type": "text", "text": user_prompt })];
    for frame in frames {
        content.push(json!({
            "type": "image_url",
            "image_url": { "url": format!("data:image/jpeg;base64,{frame}") }
        }));
    }
    if !references.is_empty() {
        content.push(json!({ "type": "text", "text": REFERENCE_LABEL }));
        for reference in references {
            content.push(json!({
                "type": "image_url",
                "image_url": { "url": format!("data:image/jpeg;base64,{reference}") }
            }));
        }
    }
    let mut request = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": content }
        ]
    });
    // `temperature` is deliberately omitted: providers disagree on the valid
    // range (OpenAI's o-series forbids non-default, Moonshot rejects some
    // values), and the default is fine for a structured JSON reply. Only the
    // token-limit parameter name differs — newer OpenAI needs the new name.
    if strict_openai {
        request["max_completion_tokens"] = Value::from(1400);
    } else {
        request["max_tokens"] = Value::from(1400);
    }
    request
}

fn build_anthropic_request(
    model: &str,
    user_prompt: &str,
    frames: &[String],
    references: &[String],
) -> Value {
    let mut content: Vec<Value> = vec![json!({ "type": "text", "text": user_prompt })];
    for frame in frames {
        content.push(json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": "image/jpeg",
                "data": frame
            }
        }));
    }
    if !references.is_empty() {
        content.push(json!({ "type": "text", "text": REFERENCE_LABEL }));
        for reference in references {
            content.push(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": "image/jpeg",
                    "data": reference
                }
            }));
        }
    }
    json!({
        "model": model,
        "max_tokens": 1400,
        "system": SYSTEM_PROMPT,
        "messages": [ { "role": "user", "content": content } ]
    })
}

/// Pull the assistant's text out of either response shape.
fn extract_text(anthropic: bool, body: &Value) -> Option<String> {
    if anthropic {
        body.get("content")?
            .as_array()?
            .iter()
            .find_map(|block| block.get("text").and_then(Value::as_str))
            .map(|s| s.to_string())
    } else {
        body.get("choices")?
            .as_array()?
            .first()?
            .get("message")?
            .get("content")?
            .as_str()
            .map(|s| s.to_string())
    }
}

/// Find the JSON object inside a possibly fenced / chatty reply.
fn parse_json_object(text: &str) -> Result<Value, String> {
    let start = text.find('{');
    let end = text.rfind('}');
    if let (Some(s), Some(e)) = (start, end) {
        if e > s {
            if let Ok(value) = serde_json::from_str::<Value>(&text[s..=e]) {
                return Ok(value);
            }
        }
    }
    Err("The model did not return valid JSON".to_string())
}

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}

/// Validate one channel's points; fall back to identity if unusable.
fn parse_channel(value: Option<&Value>) -> Vec<[f64; 2]> {
    let identity = vec![[0.0, 0.0], [1.0, 1.0]];
    let Some(arr) = value.and_then(Value::as_array) else {
        return identity;
    };
    let mut points: Vec<[f64; 2]> = arr
        .iter()
        .filter_map(|p| {
            let pair = p.as_array()?;
            let x = pair.first()?.as_f64()?;
            let y = pair.get(1)?.as_f64()?;
            Some([clamp(x, 0.0, 1.0), clamp(y, 0.0, 1.0)])
        })
        .collect();
    if points.len() < 2 {
        return identity;
    }
    points.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));
    // Drop duplicate-x points, which break the spline.
    points.dedup_by(|a, b| (a[0] - b[0]).abs() < 0.001);
    if points.len() < 2 {
        return identity;
    }
    // Anchor the ends so the curve spans the full range.
    points[0][0] = 0.0;
    let last = points.len() - 1;
    points[last][0] = 1.0;
    points
}

fn int_field(value: &Value, key: &str) -> f64 {
    clamp(value.get(key).and_then(Value::as_f64).unwrap_or(0.0), -100.0, 100.0)
        .round()
}

/// Like `int_field` but with an explicit range and default (for the sharpen /
/// radius / grain knobs, which aren't symmetric around zero).
fn ranged_field(value: &Value, key: &str, lo: f64, hi: f64, default: f64) -> f64 {
    clamp(value.get(key).and_then(Value::as_f64).unwrap_or(default), lo, hi).round()
}

/// Run the full pipeline: extract frames, call the model, validate the answer.
/// Blocking — call inside spawn_blocking.
pub(crate) fn analyze(
    video_path: &str,
    duration: f64,
    work_dir: &Path,
    provider: &str,
    api_key: &str,
    model_override: Option<String>,
    base_override: Option<String>,
    instruction: Option<String>,
    reference_images: Vec<String>,
) -> Result<AiCurveResult, String> {
    if api_key.trim().is_empty() {
        return Err("Set your AI API key in Settings → AI Enhance".to_string());
    }
    if duration <= 0.0 {
        return Err("Unknown video duration".to_string());
    }

    let cfg = provider_config(provider, model_override, base_override)?;
    if cfg.default_model.is_empty() {
        return Err("Set a model name in Settings → AI Enhance".to_string());
    }

    let _ = std::fs::remove_dir_all(work_dir);
    crate::mpv::generate_frames(video_path, work_dir, duration, FRAME_COUNT, FRAME_WIDTH)?;
    let frames = collect_frames(work_dir);
    let _ = std::fs::remove_dir_all(work_dir);
    if frames.is_empty() {
        return Err("Could not sample any frames from this video".to_string());
    }

    let references = collect_reference_images(&reference_images);

    // Fold the user's own instruction into the prompt when supplied.
    let mut user_prompt = match instruction
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(instr) => format!(
            "{USER_PROMPT}\n\nThe viewer specifically requests: \"{instr}\". \
Honour this instruction where reasonable while keeping the result tasteful and \
avoiding clipping."
        ),
        None => USER_PROMPT.to_string(),
    };
    if !references.is_empty() {
        user_prompt.push_str(
            "\n\nReference images of the desired look are attached after the video \
frames. Match the video's palette, contrast, warmth and mood toward those \
references (their content is irrelevant — only their grade matters).",
        );
    }

    let payload = if cfg.anthropic {
        build_anthropic_request(&cfg.default_model, &user_prompt, &frames, &references)
    } else {
        // OpenAI proper (and a Custom endpoint pointed at it) needs the newer
        // parameter names; the other OpenAI-compatible providers use max_tokens.
        let strict_openai = cfg.base_url.contains("api.openai.com");
        build_openai_request(
            &cfg.default_model,
            &user_prompt,
            &frames,
            &references,
            strict_openai,
        )
    };

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let (chat_url, _) = endpoints(&cfg.base_url, cfg.anthropic);
    let mut request = client.post(&chat_url).json(&payload);
    request = if cfg.anthropic {
        request
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
    } else {
        request.header("authorization", format!("Bearer {api_key}"))
    };

    let response = request
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;
    let status = response.status();
    let text = response.text().unwrap_or_default();
    if !status.is_success() {
        // Surface the provider's own error (trimmed) — this is where "model has
        // no vision" / "bad key" / "quota" show up.
        let detail = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| {
                v.get("error")
                    .and_then(|e| e.get("message").or(Some(e)))
                    .map(|m| m.to_string())
            })
            .unwrap_or_else(|| text.chars().take(300).collect());
        // A 400 on a vision request is almost always a model that can't read
        // images — point the user at a vision model.
        let hint = if status.as_u16() == 400 {
            "  Tip: the selected model may not accept images — choose a vision \
model (e.g. one with 'vl' or 'vision' in its name)."
        } else {
            ""
        };
        return Err(format!("{provider} error ({status}): {detail}{hint}"));
    }

    let body: Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid response: {e}"))?;
    let content = extract_text(cfg.anthropic, &body)
        .ok_or_else(|| "The model returned an empty answer".to_string())?;
    let parsed = parse_json_object(&content)?;

    Ok(AiCurveResult {
        rgb: parse_channel(parsed.get("rgb")),
        r: parse_channel(parsed.get("r")),
        g: parse_channel(parsed.get("g")),
        b: parse_channel(parsed.get("b")),
        saturation: int_field(&parsed, "saturation"),
        temperature: int_field(&parsed, "temperature"),
        tint: int_field(&parsed, "tint"),
        sharpen: ranged_field(&parsed, "sharpen", 0.0, 100.0, 0.0),
        sharpen_radius: ranged_field(&parsed, "sharpenRadius", 1.0, 30.0, 3.0),
        grain: ranged_field(&parsed, "grain", 0.0, 100.0, 0.0),
        notes: parsed
            .get("notes")
            .and_then(Value::as_str)
            .unwrap_or("")
            .chars()
            .take(200)
            .collect(),
    })
}

/// Describe a clip [start, end] of the video: sample frames across the range,
/// send them (plus any dialogue) to the vision model, and return prose.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn describe_clip(
    app: tauri::AppHandle,
    path: String,
    start: f64,
    end: f64,
    provider: String,
    api_key: String,
    model: Option<String>,
    base_url: Option<String>,
    dialogue: Option<String>,
) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("Set your AI API key in Settings → AI Enhance".to_string());
    }
    if !(end > start) {
        return Err("Set an A–B range first (press K to mark start and end)".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = provider_config(&provider, model, base_url)?;
        if cfg.default_model.is_empty() {
            return Err("Set a model name in Settings → AI Enhance".to_string());
        }

        let work = app
            .path()
            .app_cache_dir()
            .map_err(|e| e.to_string())?
            .join("ai_describe_work");
        let _ = std::fs::remove_dir_all(&work);
        let span = (end - start).max(0.5);
        // Aim for ~10 frames across the range (fps is an integer ≥ 1).
        let fps = ((10.0 / span).round() as u32).clamp(1, 10);
        crate::mpv::generate_range_frames(&path, &work, start, end, fps, FRAME_WIDTH)?;
        let mut frames = collect_frames(&work);
        let _ = std::fs::remove_dir_all(&work);
        if frames.is_empty() {
            return Err("Could not sample any frames from that range".to_string());
        }
        // Cap to a dozen evenly-spaced frames to keep the request light.
        if frames.len() > 12 {
            let stepf = frames.len() as f64 / 12.0;
            frames = (0..12)
                .map(|i| frames[((i as f64) * stepf) as usize].clone())
                .collect();
        }

        let user_prompt = match dialogue.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(d) => format!(
                "Dialogue spoken during this clip:\n{d}\n\nDescribe what happens in this clip."
            ),
            None => "Describe what happens in this clip.".to_string(),
        };

        let no_refs: Vec<String> = Vec::new();
        let mut payload = if cfg.anthropic {
            build_anthropic_request(&cfg.default_model, &user_prompt, &frames, &no_refs)
        } else {
            let strict_openai = cfg.base_url.contains("api.openai.com");
            build_openai_request(&cfg.default_model, &user_prompt, &frames, &no_refs, strict_openai)
        };
        // Swap the colour-grading system prompt for the description one. Anthropic
        // carries it top-level; OpenAI-shaped requests use the first message.
        if cfg.anthropic {
            payload["system"] = Value::from(DESCRIBE_SYSTEM);
        } else if let Some(msgs) = payload.get_mut("messages").and_then(|m| m.as_array_mut()) {
            if let Some(first) = msgs.first_mut() {
                first["content"] = Value::from(DESCRIBE_SYSTEM);
            }
        }

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {e}"))?;
        let (chat_url, _) = endpoints(&cfg.base_url, cfg.anthropic);
        let mut req = client.post(&chat_url).json(&payload);
        req = if cfg.anthropic {
            req.header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
        } else {
            req.header("authorization", format!("Bearer {api_key}"))
        };
        let response = req.send().map_err(|e| format!("Request failed: {e}"))?;
        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() {
            let detail = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| {
                    v.get("error")
                        .and_then(|e| e.get("message").or(Some(e)))
                        .map(|m| m.to_string())
                })
                .unwrap_or_else(|| text.chars().take(300).collect());
            let hint = if status.as_u16() == 400 {
                "  Tip: pick a vision model (name contains 'vl' or 'vision')."
            } else {
                ""
            };
            return Err(format!("{provider} error ({status}): {detail}{hint}"));
        }
        let body: Value =
            serde_json::from_str(&text).map_err(|e| format!("Invalid response: {e}"))?;
        extract_text(cfg.anthropic, &body)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "The model returned an empty description".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Names that are clearly not chat/vision models — filtered out of the list.
fn looks_non_chat(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    [
        "embedding", "embed", "whisper", "tts", "audio", "moderation", "rerank",
        "image", "dall-e", "dalle", "sora", "text-", "search", "similarity",
        "davinci", "babbage", "ada", "curie", "guard", "bge", "code-",
    ]
    .iter()
    .any(|bad| l.contains(bad))
}

/// Fetch the model IDs a provider actually offers for the given key, filtered to
/// plausible chat/vision models and sorted newest-first when the API reports a
/// creation time. Blocking.
fn list_models(
    provider: &str,
    api_key: &str,
    base_override: Option<String>,
) -> Result<Vec<String>, String> {
    if api_key.trim().is_empty() {
        return Err("Set your AI API key first".to_string());
    }
    let cfg = provider_config(provider, None, base_override)?;
    let (_, url) = endpoints(&cfg.base_url, cfg.anthropic);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let mut request = client.get(&url);
    request = if cfg.anthropic {
        request
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
    } else {
        request.header("authorization", format!("Bearer {api_key}"))
    };

    let response = request
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;
    let status = response.status();
    let text = response.text().unwrap_or_default();
    if !status.is_success() {
        let detail = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| {
                v.get("error")
                    .and_then(|e| e.get("message").or(Some(e)))
                    .map(|m| m.to_string())
            })
            .unwrap_or_else(|| text.chars().take(200).collect());
        return Err(format!("{provider} ({status}): {detail}"));
    }

    let body: Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid response: {e}"))?;
    // Both OpenAI-compatible and Anthropic return { "data": [ { "id", ... } ] };
    // Gemini's OpenAI-compat also uses "data". Fall back to "models" for the
    // native Gemini shape just in case.
    let items = body
        .get("data")
        .or_else(|| body.get("models"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut rows: Vec<(i64, String)> = items
        .into_iter()
        .filter_map(|item| {
            let id = item
                .get("id")
                .or_else(|| item.get("name"))
                .and_then(Value::as_str)?
                // Gemini native prefixes names with "models/".
                .trim_start_matches("models/")
                .to_string();
            if id.is_empty() || looks_non_chat(&id) {
                return None;
            }
            let created = item
                .get("created")
                .or_else(|| item.get("created_at"))
                .and_then(Value::as_i64)
                .unwrap_or(0);
            Some((created, id))
        })
        .collect();

    // Newest first when timestamps exist; otherwise keep provider order.
    if rows.iter().any(|(c, _)| *c > 0) {
        rows.sort_by(|a, b| b.0.cmp(&a.0));
    }
    let mut seen = std::collections::HashSet::new();
    let ids: Vec<String> = rows
        .into_iter()
        .map(|(_, id)| id)
        .filter(|id| seen.insert(id.clone()))
        .take(40)
        .collect();
    Ok(ids)
}

#[tauri::command]
pub(crate) async fn ai_list_models(
    provider: String,
    api_key: String,
    base_url: Option<String>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        list_models(&provider, &api_key, base_url)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn ai_curve_correction(
    app: tauri::AppHandle,
    path: String,
    duration: f64,
    provider: String,
    api_key: String,
    model: Option<String>,
    base_url: Option<String>,
    instruction: Option<String>,
    reference_images: Option<Vec<String>>,
) -> Result<AiCurveResult, String> {
    use tauri::Manager;

    if path.trim().is_empty() || regex_like_remote(&path) {
        return Err("AI correction needs a local video file".to_string());
    }

    let work = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("Failed to resolve cache dir: {e}"))?
        .join("ai_frames");

    let references = reference_images.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        analyze(
            &path, duration, &work, &provider, &api_key, model, base_url, instruction,
            references,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Downscale a reference image to a small base64 data URL for the chip preview
/// in the AI Correction window. Keeps file bytes out of the webview and works
/// without the asset protocol.
#[tauri::command]
pub(crate) async fn ai_reference_thumbnail(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let encoded = encode_image_jpeg(&path, REFERENCE_THUMB_DIM)?;
        Ok(format!("data:image/jpeg;base64,{encoded}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn regex_like_remote(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("rtsp://")
        || lower.starts_with("rtmp://")
        || lower.starts_with("smb://")
        || lower.starts_with("webdav://")
}
