//! Watch-page context via Innertube `next`: related videos (Up next) and
//! chapter markers. Related items arrive as YouTube's new `lockupViewModel`
//! renderer; the parser is tolerant so layout drift degrades to fewer rows.

use serde::Serialize;
use serde_json::{json, Value};
use tauri::AppHandle;

use super::{innertube, watch_url, YoutubeItem};

const NEXT_URL: &str = "https://www.youtube.com/youtubei/v1/next?prettyPrint=false";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YoutubeChapter {
    pub(crate) title: String,
    pub(crate) start_seconds: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YoutubeVideoContext {
    pub(crate) related: Vec<YoutubeItem>,
    pub(crate) chapters: Vec<YoutubeChapter>,
}

#[tauri::command]
pub(crate) async fn youtube_video_context(
    app: AppHandle,
    video_id: String,
) -> Result<YoutubeVideoContext, String> {
    let trimmed = video_id.trim().to_string();
    if trimmed.is_empty() {
        return Err("Missing video id".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || fetch_context(&app, &trimmed))
        .await
        .map_err(|error| format!("YouTube context worker failed: {error}"))?
}

fn fetch_context(app: &AppHandle, video_id: &str) -> Result<YoutubeVideoContext, String> {
    let client = innertube::blocking_client(app)?;
    let body = json!({
        "context": { "client": { "clientName": "WEB", "clientVersion": "2.20250801.01.00" } },
        "videoId": video_id,
    });
    let response = client
        .post(NEXT_URL)
        .json(&body)
        .send()
        .map_err(|error| format!("YouTube context request failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("YouTube context: HTTP {status}"));
    }
    let value: Value = response
        .json()
        .map_err(|error| format!("YouTube context: invalid JSON: {error}"))?;

    Ok(YoutubeVideoContext {
        related: parse_related(&value),
        chapters: parse_chapters(&value),
    })
}

fn parse_related(value: &Value) -> Vec<YoutubeItem> {
    value
        .pointer("/contents/twoColumnWatchNextResults/secondaryResults/secondaryResults/results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|result| {
            // Items appear directly or nested inside itemSectionRenderer.
            result
                .pointer("/itemSectionRenderer/contents")
                .and_then(Value::as_array)
                .map(|contents| contents.iter().collect::<Vec<_>>())
                .unwrap_or_else(|| vec![result])
        })
        .filter_map(|item| parse_lockup(item.get("lockupViewModel")?))
        .collect()
}

fn parse_lockup(lockup: &Value) -> Option<YoutubeItem> {
    if lockup.get("contentType").and_then(Value::as_str) != Some("LOCKUP_CONTENT_TYPE_VIDEO") {
        return None;
    }
    let id = lockup.get("contentId").and_then(Value::as_str)?.to_string();
    let metadata = lockup.pointer("/metadata/lockupMetadataViewModel")?;
    let title = metadata
        .pointer("/title/content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())?
        .to_string();

    // Metadata rows carry channel / views / age as loose text parts.
    let mut texts: Vec<String> = Vec::new();
    if let Some(rows) = metadata
        .pointer("/metadata/contentMetadataViewModel/metadataRows")
        .and_then(Value::as_array)
    {
        for row in rows {
            for part in row
                .get("metadataParts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(text) = part.pointer("/text/content").and_then(Value::as_str) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        texts.push(trimmed.to_string());
                    }
                }
            }
        }
    }
    let view_count_text = texts
        .iter()
        .find(|text| text.contains("view") || text.contains("watching"))
        .cloned();
    let published_text = texts.iter().find(|text| text.contains("ago")).cloned();
    let channel = texts
        .iter()
        .find(|text| {
            !text.contains("view") && !text.contains("watching") && !text.contains("ago")
        })
        .cloned();

    let duration_text = lockup
        .pointer("/contentImage/thumbnailViewModel/overlays")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find_map(|overlay| {
            overlay
                .pointer("/thumbnailBottomOverlayViewModel/badges")
                .and_then(Value::as_array)?
                .iter()
                .find_map(|badge| {
                    badge
                        .pointer("/thumbnailBadgeViewModel/text")
                        .and_then(Value::as_str)
                        .filter(|text| text.contains(':'))
                        .map(str::to_string)
                })
        });
    let is_live = duration_text.is_none()
        && view_count_text
            .as_deref()
            .is_some_and(|text| text.contains("watching"));

    Some(YoutubeItem {
        kind: "video".to_string(),
        url: watch_url(&id),
        title,
        channel,
        channel_url: None,
        duration_seconds: duration_text.as_deref().and_then(parse_clock),
        duration_text,
        view_count_text,
        published_text,
        thumbnail_url: Some(format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")),
        video_count_text: None,
        badge: is_live.then(|| "LIVE".to_string()),
        id,
    })
}

fn parse_chapters(value: &Value) -> Vec<YoutubeChapter> {
    let Some(panels) = value.get("engagementPanels").and_then(Value::as_array) else {
        return Vec::new();
    };
    let Some(list) = panels.iter().find_map(|panel| {
        let renderer = panel.get("engagementPanelSectionListRenderer")?;
        let identifier = renderer.get("panelIdentifier").and_then(Value::as_str)?;
        if !identifier.contains("macro-markers-description-chapters") {
            return None;
        }
        renderer
            .pointer("/content/macroMarkersListRenderer/contents")
            .and_then(Value::as_array)
    }) else {
        return Vec::new();
    };

    let mut chapters: Vec<YoutubeChapter> = list
        .iter()
        .filter_map(|item| {
            let marker = item.get("macroMarkersListItemRenderer")?;
            let title = marker
                .pointer("/title/simpleText")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|title| !title.is_empty())?
                .to_string();
            let start_seconds = marker
                .pointer("/onTap/watchEndpoint/startTimeSeconds")
                .and_then(Value::as_f64)
                .or_else(|| {
                    marker
                        .pointer("/timeDescription/simpleText")
                        .and_then(Value::as_str)
                        .and_then(parse_clock)
                })?;
            Some(YoutubeChapter {
                title,
                start_seconds,
            })
        })
        .collect();
    chapters.sort_by(|a, b| a.start_seconds.total_cmp(&b.start_seconds));
    chapters.dedup_by(|a, b| a.start_seconds == b.start_seconds);
    chapters
}

/// "1:02:03" / "12:34" → seconds.
fn parse_clock(text: &str) -> Option<f64> {
    let mut seconds = 0u64;
    for part in text.split(':') {
        seconds = seconds * 60 + part.trim().parse::<u64>().ok()?;
    }
    Some(seconds as f64)
}
