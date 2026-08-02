//! Key-less Innertube (youtubei/v1) client, mimicking YouTube's own WEB
//! client context. Parsers are deliberately tolerant: unknown renderers are
//! skipped so YouTube-side layout changes degrade to fewer results instead
//! of hard failures (the caller falls back to yt-dlp on real errors).

use base64::Engine as _;
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::AppHandle;

use super::{watch_url, YoutubeItem, YoutubeSearchFilters};

const SEARCH_URL: &str = "https://www.youtube.com/youtubei/v1/search?prettyPrint=false";
const CLIENT_VERSION: &str = "2.20250801.01.00";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// One pooled client, rebuilt only when the proxy setting changes — keeps
/// TLS connections warm across searches and thumbnail fetches.
static SHARED_CLIENT: OnceLock<Mutex<Option<(String, reqwest::blocking::Client)>>> =
    OnceLock::new();

pub(super) fn blocking_client(app: &AppHandle) -> Result<reqwest::blocking::Client, String> {
    let proxy_key = crate::network::proxy::current_proxy_key(app)?.unwrap_or_default();
    let cache = SHARED_CLIENT.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((cached_key, client)) = guard.as_ref() {
            if *cached_key == proxy_key {
                return Ok(client.clone());
            }
        }
    }
    let builder = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(15));
    let client = crate::network::proxy::configure_blocking_client_builder(app, builder)?
        .build()
        .map_err(|error| format!("innertube: failed to build HTTP client: {error}"))?;
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((proxy_key, client.clone()));
    }
    Ok(client)
}

/// Warms DNS + TLS on the shared client so the first real search is fast.
pub(super) fn warm_connection(app: &AppHandle) {
    if let Ok(client) = blocking_client(app) {
        let _ = client.get("https://www.youtube.com/robots.txt").send();
    }
}

fn client_context() -> Value {
    json!({ "client": { "clientName": "WEB", "clientVersion": CLIENT_VERSION } })
}

fn post_search(app: &AppHandle, body: Value) -> Result<Value, String> {
    let client = blocking_client(app)?;
    let response = client
        .post(SEARCH_URL)
        .json(&body)
        .send()
        .map_err(|error| format!("innertube: request failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("innertube: HTTP {status}"));
    }
    response
        .json::<Value>()
        .map_err(|error| format!("innertube: invalid JSON: {error}"))
}

pub(super) fn search(
    app: &AppHandle,
    query: &str,
    filters: &YoutubeSearchFilters,
) -> Result<(Vec<YoutubeItem>, Option<String>), String> {
    let mut body = json!({ "context": client_context(), "query": query });
    if let Some(params) = filter_params(filters) {
        body["params"] = json!(params);
    }
    let value = post_search(app, body)?;
    let sections = value
        .pointer(
            "/contents/twoColumnSearchResultsRenderer/primaryContents/sectionListRenderer/contents",
        )
        .and_then(Value::as_array)
        .ok_or_else(|| "innertube: unexpected search response shape".to_string())?;
    Ok(collect_sections(sections))
}

pub(super) fn continue_search(
    app: &AppHandle,
    token: &str,
) -> Result<(Vec<YoutubeItem>, Option<String>), String> {
    let body = json!({ "context": client_context(), "continuation": token });
    let value = post_search(app, body)?;
    let commands = value
        .get("onResponseReceivedCommands")
        .and_then(Value::as_array)
        .ok_or_else(|| "innertube: unexpected continuation response shape".to_string())?;
    let sections = commands
        .iter()
        .find_map(|command| {
            command
                .pointer("/appendContinuationItemsAction/continuationItems")
                .and_then(Value::as_array)
        })
        .ok_or_else(|| "innertube: continuation carried no items".to_string())?;
    Ok(collect_sections(sections))
}

fn collect_sections(sections: &[Value]) -> (Vec<YoutubeItem>, Option<String>) {
    let mut items = Vec::new();
    let mut continuation = None;
    for section in sections {
        if let Some(contents) = section
            .pointer("/itemSectionRenderer/contents")
            .and_then(Value::as_array)
        {
            items.extend(contents.iter().filter_map(parse_item));
        }
        if let Some(token) = section
            .pointer("/continuationItemRenderer/continuationEndpoint/continuationCommand/token")
            .and_then(Value::as_str)
        {
            continuation = Some(token.to_string());
        }
    }
    (items, continuation)
}

fn parse_item(item: &Value) -> Option<YoutubeItem> {
    if let Some(video) = item.get("videoRenderer") {
        return parse_video(video);
    }
    if let Some(channel) = item.get("channelRenderer") {
        return parse_channel(channel);
    }
    if let Some(playlist) = item.get("playlistRenderer") {
        return parse_playlist(playlist);
    }
    None
}

fn parse_video(video: &Value) -> Option<YoutubeItem> {
    let id = video.get("videoId").and_then(Value::as_str)?.to_string();
    let title = text_of(video.get("title"))?;
    let duration_text = text_of(video.get("lengthText"));
    let duration_seconds = duration_text.as_deref().and_then(parse_clock);
    let is_live = video
        .pointer("/badges")
        .and_then(Value::as_array)
        .map(|badges| {
            badges.iter().any(|badge| {
                badge
                    .pointer("/metadataBadgeRenderer/style")
                    .and_then(Value::as_str)
                    .is_some_and(|style| style.contains("LIVE"))
            })
        })
        .unwrap_or(false);
    Some(YoutubeItem {
        kind: "video".to_string(),
        url: watch_url(&id),
        title,
        channel: text_of(video.get("ownerText")).or_else(|| text_of(video.get("longBylineText"))),
        channel_url: channel_url_of(video.get("ownerText")),
        duration_seconds,
        duration_text,
        view_count_text: text_of(video.get("shortViewCountText"))
            .or_else(|| text_of(video.get("viewCountText"))),
        published_text: text_of(video.get("publishedTimeText")),
        // Prefer the URL YouTube itself serves (always valid for this video);
        // the bytes get disk-cached so token expiry doesn't matter.
        thumbnail_url: last_thumbnail(video.get("thumbnail"))
            .or_else(|| Some(format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"))),
        video_count_text: None,
        badge: is_live.then(|| "LIVE".to_string()),
        id,
    })
}

fn parse_channel(channel: &Value) -> Option<YoutubeItem> {
    let id = channel.get("channelId").and_then(Value::as_str)?.to_string();
    let title = text_of(channel.get("title"))?;
    let canonical = channel
        .pointer("/navigationEndpoint/browseEndpoint/canonicalBaseUrl")
        .and_then(Value::as_str)
        .map(absolute_youtube_url);
    Some(YoutubeItem {
        kind: "channel".to_string(),
        url: canonical
            .clone()
            .unwrap_or_else(|| format!("https://www.youtube.com/channel/{id}")),
        title,
        channel: None,
        channel_url: canonical,
        duration_seconds: None,
        duration_text: None,
        view_count_text: text_of(channel.get("subscriberCountText"))
            .or_else(|| text_of(channel.get("videoCountText"))),
        published_text: None,
        thumbnail_url: last_thumbnail(channel.get("thumbnail")),
        video_count_text: None,
        badge: None,
        id,
    })
}

fn parse_playlist(playlist: &Value) -> Option<YoutubeItem> {
    let id = playlist.get("playlistId").and_then(Value::as_str)?.to_string();
    let title = text_of(playlist.get("title"))?;
    let video_count = playlist
        .get("videoCount")
        .and_then(Value::as_str)
        .map(|count| format!("{count} videos"))
        .or_else(|| text_of(playlist.get("videoCountText")));
    let thumbnail = playlist
        .pointer("/thumbnails/0")
        .and_then(|thumb| last_thumbnail(Some(thumb)))
        .or_else(|| last_thumbnail(playlist.get("thumbnail")));
    Some(YoutubeItem {
        kind: "playlist".to_string(),
        url: format!("https://www.youtube.com/playlist?list={id}"),
        title,
        channel: text_of(playlist.get("shortBylineText")),
        channel_url: channel_url_of(playlist.get("shortBylineText")),
        duration_seconds: None,
        duration_text: None,
        view_count_text: None,
        published_text: None,
        thumbnail_url: thumbnail,
        video_count_text: video_count,
        badge: None,
        id,
    })
}

/// Reads Innertube's two text shapes: `{simpleText}` or `{runs: [{text}]}`.
fn text_of(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(simple) = value.get("simpleText").and_then(Value::as_str) {
        let trimmed = simple.trim();
        return (!trimmed.is_empty()).then(|| trimmed.to_string());
    }
    let runs = value.get("runs")?.as_array()?;
    let joined: String = runs
        .iter()
        .filter_map(|run| run.get("text").and_then(Value::as_str))
        .collect();
    let trimmed = joined.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn channel_url_of(value: Option<&Value>) -> Option<String> {
    value?
        .pointer("/runs/0/navigationEndpoint/browseEndpoint/canonicalBaseUrl")
        .and_then(Value::as_str)
        .map(absolute_youtube_url)
}

fn absolute_youtube_url(path: &str) -> String {
    if path.starts_with("http") {
        path.to_string()
    } else {
        format!("https://www.youtube.com{path}")
    }
}

fn last_thumbnail(value: Option<&Value>) -> Option<String> {
    let url = value?
        .pointer("/thumbnails")
        .and_then(Value::as_array)?
        .last()?
        .get("url")
        .and_then(Value::as_str)?;
    Some(if url.starts_with("//") {
        format!("https:{url}")
    } else {
        url.to_string()
    })
}

/// "1:02:03" / "12:34" → seconds.
fn parse_clock(text: &str) -> Option<f64> {
    let mut seconds = 0u64;
    for part in text.split(':') {
        seconds = seconds * 60 + part.trim().parse::<u64>().ok()?;
    }
    Some(seconds as f64)
}

// --- Search filter params (protobuf) ---------------------------------------
//
// The `params` field is a base64url-encoded protobuf:
//   field 1 (varint) = sort            (1 rating, 2 date, 3 views)
//   field 2 (message) = filters:
//     field 1 = upload date  (1 hour, 2 today, 3 week, 4 month, 5 year)
//     field 2 = type         (1 video, 2 channel, 3 playlist, 4 movie)
//     field 3 = duration     (1 short, 2 long, 3 medium)
//     field 4 = HD           (bool)
// Verified against live Innertube (e.g. type=video+HD → "EgQQASAB").

fn push_varint(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
}

fn push_varint_field(field: u32, value: u64, out: &mut Vec<u8>) {
    push_varint(u64::from(field << 3), out);
    push_varint(value, out);
}

fn filter_params(filters: &YoutubeSearchFilters) -> Option<String> {
    if filters.is_empty() {
        return None;
    }

    let sort = match filters.sort.as_deref() {
        Some("rating") => Some(1u64),
        Some("date") => Some(2),
        Some("views") => Some(3),
        _ => None,
    };
    let upload_date = match filters.upload_date.as_deref() {
        Some("hour") => Some(1u64),
        Some("today") => Some(2),
        Some("week") => Some(3),
        Some("month") => Some(4),
        Some("year") => Some(5),
        _ => None,
    };
    let kind = match filters.kind.as_deref() {
        Some("video") => Some(1u64),
        Some("channel") => Some(2),
        Some("playlist") => Some(3),
        Some("movie") => Some(4),
        _ => None,
    };
    let duration = match filters.duration.as_deref() {
        Some("short") => Some(1u64),
        Some("long") => Some(2),
        Some("medium") => Some(3),
        _ => None,
    };

    let mut sub = Vec::new();
    if let Some(value) = upload_date {
        push_varint_field(1, value, &mut sub);
    }
    if let Some(value) = kind {
        push_varint_field(2, value, &mut sub);
    }
    if let Some(value) = duration {
        push_varint_field(3, value, &mut sub);
    }
    if filters.hd.unwrap_or(false) {
        push_varint_field(4, 1, &mut sub);
    }

    let mut proto = Vec::new();
    if let Some(value) = sort {
        push_varint_field(1, value, &mut proto);
    }
    if !sub.is_empty() {
        push_varint((2 << 3 | 2) as u64, &mut proto);
        push_varint(sub.len() as u64, &mut proto);
        proto.extend_from_slice(&sub);
    }
    if proto.is_empty() {
        return None;
    }

    let encoded = base64::engine::general_purpose::URL_SAFE.encode(&proto);
    // The web client percent-encodes the padding inside the JSON value too.
    Some(encoded.replace('=', "%3D"))
}
