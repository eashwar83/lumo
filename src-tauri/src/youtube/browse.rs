//! Channel, playlist and trending surfaces.
//!
//! Channels go through Innertube browse (about a second) because yt-dlp's
//! tab extraction takes ~14s. Playlists go through yt-dlp, whose flat JSON
//! is stable and paginates cleanly with `--playlist-items`. "Trending" is
//! backed by YouTube's own chart playlists: the classic trending feed was
//! retired in 2025 and no longer resolves.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::AppHandle;

use super::{format_duration, innertube, watch, watch_url, ytdlp, YoutubeItem};

const BROWSE_URL: &str = "https://www.youtube.com/youtubei/v1/browse?prettyPrint=false";
const RESOLVE_URL: &str =
    "https://www.youtube.com/youtubei/v1/navigation/resolve_url?prettyPrint=false";
const CLIENT_VERSION: &str = "2.20250801.01.00";
const PLAYLIST_PAGE_SIZE: usize = 30;

/// Chart playlists that stand in for the retired trending feed.
const TRENDING_PLAYLISTS: &[(&str, &str)] = &[
    ("now", "PLbpi6ZahtOH6Blw3RGYpWkSByi_T7Rygb"),
    ("music", "PL4fGSI1pDJn5oibdgJt8Hy0-dr2B7kSs2"),
    ("top100", "PL4fGSI1pDJn40WjZ6utkIuj2rNg-7iGsq"),
];

/// A sort option YouTube itself offers for this surface (Latest / Popular /
/// Oldest on channel videos, "Date added" / "Last video added" on the
/// playlists tab). `token` is fed back as the request cursor.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SortOption {
    pub(crate) label: String,
    pub(crate) token: String,
    pub(crate) selected: bool,
    /// "continuation" (chip tokens) or "params" (browse params).
    pub(crate) kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowsePage {
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) avatar_url: Option<String>,
    pub(crate) items: Vec<YoutubeItem>,
    pub(crate) next_cursor: Option<String>,
    pub(crate) sort_options: Vec<SortOption>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelPayload {
    /// Channel URL, @handle or raw channel id.
    pub(crate) target: String,
    /// "videos" (default) | "playlists"
    #[serde(default)]
    pub(crate) tab: Option<String>,
    #[serde(default)]
    pub(crate) cursor: Option<String>,
    /// Sort token from a previous page's `sortOptions`.
    #[serde(default)]
    pub(crate) sort_token: Option<String>,
    /// "continuation" | "params"
    #[serde(default)]
    pub(crate) sort_kind: Option<String>,
    /// Searches within the channel when non-empty.
    #[serde(default)]
    pub(crate) query: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistPayload {
    /// Playlist URL or raw list id.
    pub(crate) target: String,
    #[serde(default)]
    pub(crate) cursor: Option<String>,
}

/// Unwraps an item section (grids, shelves and bare items) into its entries.
fn flatten_section(section: &Value) -> Vec<Value> {
    let Some(contents) = section
        .pointer("/itemSectionRenderer/contents")
        .and_then(Value::as_array)
    else {
        return vec![section.clone()];
    };
    contents
        .iter()
        .flat_map(|content| {
            content
                .pointer("/gridRenderer/items")
                .or_else(|| content.pointer("/shelfRenderer/content/horizontalListRenderer/items"))
                .or_else(|| {
                    content.pointer("/shelfRenderer/content/expandedShelfContentsRenderer/items")
                })
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_else(|| vec![content.clone()])
        })
        .collect()
}

fn client_context() -> Value {
    json!({ "client": { "clientName": "WEB", "clientVersion": CLIENT_VERSION, "hl": "en" } })
}

fn post(app: &AppHandle, url: &str, body: Value) -> Result<Value, String> {
    let client = innertube::blocking_client(app)?;
    let response = client
        .post(url)
        .json(&body)
        .send()
        .map_err(|error| format!("YouTube request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("YouTube: HTTP {}", response.status()));
    }
    response
        .json::<Value>()
        .map_err(|error| format!("YouTube: invalid JSON: {error}"))
}

// --- channel ----------------------------------------------------------------

static CHANNEL_IDS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn resolve_channel_id(app: &AppHandle, target: &str) -> Result<String, String> {
    let trimmed = target.trim();
    if trimmed.starts_with("UC") && trimmed.len() > 20 && !trimmed.contains('/') {
        return Ok(trimmed.to_string());
    }
    let cache = CHANNEL_IDS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some(id) = guard.get(trimmed) {
            return Ok(id.clone());
        }
    }
    let url = if trimmed.starts_with("http") {
        trimmed.to_string()
    } else if let Some(handle) = trimmed.strip_prefix('@') {
        format!("https://www.youtube.com/@{handle}")
    } else {
        format!("https://www.youtube.com/{trimmed}")
    };
    let value = post(
        app,
        RESOLVE_URL,
        json!({ "context": client_context(), "url": url }),
    )?;
    let browse_id = value
        .pointer("/endpoint/browseEndpoint/browseId")
        .and_then(Value::as_str)
        .ok_or_else(|| "Could not resolve that channel".to_string())?
        .to_string();
    if let Ok(mut guard) = cache.lock() {
        guard.insert(trimmed.to_string(), browse_id.clone());
    }
    Ok(browse_id)
}

#[tauri::command]
pub(crate) async fn youtube_channel(
    app: AppHandle,
    payload: ChannelPayload,
) -> Result<BrowsePage, String> {
    tauri::async_runtime::spawn_blocking(move || fetch_channel(&app, &payload))
        .await
        .map_err(|error| format!("YouTube channel worker failed: {error}"))?
}

fn fetch_channel(app: &AppHandle, payload: &ChannelPayload) -> Result<BrowsePage, String> {
    let body = match (
        payload.cursor.as_deref(),
        payload.sort_token.as_deref(),
        payload.sort_kind.as_deref(),
    ) {
        // Paging within the current view.
        (Some(token), _, _) => json!({ "context": client_context(), "continuation": token }),
        // Chip sorts (channel videos) re-request via a continuation token.
        (None, Some(token), Some("continuation")) => {
            json!({ "context": client_context(), "continuation": token })
        }
        // Menu sorts (playlists tab) re-browse with their own params.
        (None, Some(params), _) => {
            let browse_id = resolve_channel_id(app, &payload.target)?;
            json!({ "context": client_context(), "browseId": browse_id, "params": params })
        }
        (None, None, _) => {
            let browse_id = resolve_channel_id(app, &payload.target)?;
            let query = payload
                .query
                .as_deref()
                .map(str::trim)
                .filter(|query| !query.is_empty());
            match query {
                // The channel's own Search tab, same as the web page's
                // magnifier: results are scoped to this channel.
                Some(query) => json!({
                    "context": client_context(),
                    "browseId": browse_id,
                    "params": "EgZzZWFyY2jyBgQKAloA",
                    "query": query,
                }),
                None => {
                    let params = match payload.tab.as_deref() {
                        Some("playlists") => "EglwbGF5bGlzdHPyBgQKAkIA",
                        _ => "EgZ2aWRlb3PyBgQKAjoA",
                    };
                    json!({ "context": client_context(), "browseId": browse_id, "params": params })
                }
            }
        }
    };
    let value = post(app, BROWSE_URL, body)?;

    // Continuations answer with append actions instead of the full page.
    let grid: Vec<Value> = value
        .pointer("/contents/twoColumnBrowseResultsRenderer/tabs")
        .and_then(Value::as_array)
        .and_then(|tabs| {
            tabs.iter().find_map(|tab| {
                // The channel Search tab arrives as an expandable tab.
                let renderer = tab
                    .get("tabRenderer")
                    .or_else(|| tab.get("expandableTabRenderer"))?;
                renderer
                    .get("selected")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    .then(|| {
                        // Videos come back as a rich grid; the Playlists tab
                        // uses a section list of item sections instead.
                        renderer
                            .pointer("/content/richGridRenderer/contents")
                            .and_then(Value::as_array)
                            .cloned()
                            .or_else(|| {
                                let sections = renderer
                                    .pointer("/content/sectionListRenderer/contents")?
                                    .as_array()?;
                                Some(
                                    sections
                                        .iter()
                                        .flat_map(flatten_section)
                                        .collect::<Vec<Value>>(),
                                )
                            })
                            .unwrap_or_default()
                    })
            })
        })
        .or_else(|| {
            // Continuations append; chip sorts reload the whole grid.
            value
                .get("onResponseReceivedActions")
                .and_then(Value::as_array)
                .map(|actions| {
                    actions
                        .iter()
                        .filter_map(|action| {
                            action
                                .pointer("/appendContinuationItemsAction/continuationItems")
                                .or_else(|| {
                                    action.pointer(
                                        "/reloadContinuationItemsCommand/continuationItems",
                                    )
                                })
                                .and_then(Value::as_array)
                                .cloned()
                        })
                        .flatten()
                        .collect::<Vec<Value>>()
                })
                .filter(|items| !items.is_empty())
        })
        .unwrap_or_default();

    let mut items = Vec::new();
    let mut next_cursor = None;
    for entry in &grid {
        if let Some(token) = entry
            .pointer("/continuationItemRenderer/continuationEndpoint/continuationCommand/token")
            .and_then(Value::as_str)
        {
            next_cursor = Some(token.to_string());
            continue;
        }
        // Items arrive wrapped in a richItemRenderer or bare, as modern
        // lockups (grids) or classic renderers (channel search results).
        let content = entry
            .pointer("/richItemRenderer/content")
            .unwrap_or(entry);
        let parsed = content
            .get("lockupViewModel")
            .and_then(watch::parse_lockup)
            .or_else(|| innertube::parse_item(content));
        if let Some(parsed) = parsed {
            items.push(parsed);
        }
    }

    let header = value.pointer("/header/pageHeaderRenderer");
    let title = header
        .and_then(|item| item.get("pageTitle"))
        .and_then(Value::as_str)
        .unwrap_or("Channel")
        .to_string();
    let subtitle_parts: Vec<String> = header
        .and_then(|item| {
            item.pointer("/content/pageHeaderViewModel/metadata/contentMetadataViewModel/metadataRows")
        })
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .flat_map(|row| {
                    row.get("metadataParts")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default()
                })
                .filter_map(|part| {
                    part.pointer("/text/content")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    let avatar_url = header
        .and_then(|item| {
            item.pointer("/content/pageHeaderViewModel/image/decoratedAvatarViewModel/avatar/avatarViewModel/image/sources")
        })
        .and_then(Value::as_array)
        .and_then(|sources| sources.last())
        .and_then(|source| source.get("url"))
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(BrowsePage {
        title,
        subtitle: subtitle_parts.join(" · "),
        avatar_url,
        items,
        next_cursor,
        sort_options: extract_sort_options(&value),
    })
}

/// Reads whichever sort control the surface ships: the videos tab uses a
/// chip bar carrying continuation tokens, the playlists tab a sort menu
/// carrying browse params. Empty when the response has neither (e.g. a
/// continuation page).
fn extract_sort_options(value: &Value) -> Vec<SortOption> {
    let tabs = value
        .pointer("/contents/twoColumnBrowseResultsRenderer/tabs")
        .and_then(Value::as_array);
    let Some(selected) = tabs.and_then(|tabs| {
        tabs.iter().find_map(|tab| {
            let renderer = tab.get("tabRenderer")?;
            renderer
                .get("selected")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                .then_some(renderer)
        })
    }) else {
        return Vec::new();
    };

    if let Some(chips) = selected
        .pointer("/content/richGridRenderer/header/chipBarViewModel/chips")
        .and_then(Value::as_array)
    {
        return chips
            .iter()
            .filter_map(|chip| {
                let chip = chip.get("chipViewModel")?;
                Some(SortOption {
                    label: chip.get("text").and_then(Value::as_str)?.to_string(),
                    token: chip
                        .pointer("/tapCommand/innertubeCommand/continuationCommand/token")
                        .and_then(Value::as_str)?
                        .to_string(),
                    selected: chip
                        .get("selected")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    kind: "continuation".to_string(),
                })
            })
            .collect();
    }

    selected
        .pointer("/content/sectionListRenderer/subMenu/channelSubMenuRenderer/sortSetting/sortFilterSubMenuRenderer/subMenuItems")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    Some(SortOption {
                        label: entry.get("title").and_then(Value::as_str)?.to_string(),
                        token: entry
                            .pointer("/navigationEndpoint/browseEndpoint/params")
                            .and_then(Value::as_str)?
                            .to_string(),
                        selected: entry
                            .get("selected")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        kind: "params".to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

// --- playlist / trending ------------------------------------------------------

#[tauri::command]
pub(crate) async fn youtube_playlist(
    app: AppHandle,
    payload: PlaylistPayload,
) -> Result<BrowsePage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let start = payload
            .cursor
            .as_deref()
            .and_then(|cursor| cursor.parse::<usize>().ok())
            .unwrap_or(1);
        fetch_playlist(&app, &payload.target, start)
    })
    .await
    .map_err(|error| format!("YouTube playlist worker failed: {error}"))?
}

#[tauri::command]
pub(crate) async fn youtube_trending(
    app: AppHandle,
    category: String,
) -> Result<BrowsePage, String> {
    let playlist_id = TRENDING_PLAYLISTS
        .iter()
        .find(|(key, _)| *key == category)
        .map(|(_, id)| (*id).to_string())
        .unwrap_or_else(|| TRENDING_PLAYLISTS[0].1.to_string());
    tauri::async_runtime::spawn_blocking(move || fetch_playlist(&app, &playlist_id, 1))
        .await
        .map_err(|error| format!("YouTube trending worker failed: {error}"))?
}

fn playlist_url(target: &str) -> String {
    let trimmed = target.trim();
    if trimmed.starts_with("http") {
        trimmed.to_string()
    } else {
        format!("https://www.youtube.com/playlist?list={trimmed}")
    }
}

fn fetch_playlist(app: &AppHandle, target: &str, start: usize) -> Result<BrowsePage, String> {
    let end = start + PLAYLIST_PAGE_SIZE - 1;
    let value = ytdlp::run_flat_json_with(
        app,
        &playlist_url(target),
        &["--playlist-items".to_string(), format!("{start}:{end}")],
    )?;

    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let returned = entries.len();
    let items: Vec<YoutubeItem> = entries.iter().filter_map(playlist_entry_to_item).collect();

    let total = value
        .get("playlist_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let next_cursor = (returned >= PLAYLIST_PAGE_SIZE).then(|| (end + 1).to_string());
    let subtitle = match (
        value.get("uploader").and_then(Value::as_str),
        total,
    ) {
        (Some(uploader), 0) => uploader.to_string(),
        (Some(uploader), count) => format!("{uploader} · {count} videos"),
        (None, 0) => String::new(),
        (None, count) => format!("{count} videos"),
    };

    Ok(BrowsePage {
        title: value
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Playlist")
            .to_string(),
        subtitle,
        avatar_url: None,
        items,
        next_cursor,
        // YouTube offers no sort for a playlist's contents; the UI provides
        // its own client-side ordering instead.
        sort_options: Vec::new(),
    })
}

fn playlist_entry_to_item(entry: &Value) -> Option<YoutubeItem> {
    let id = entry.get("id").and_then(Value::as_str)?.to_string();
    let title = entry
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())?
        .to_string();
    let duration_seconds = entry.get("duration").and_then(Value::as_f64);
    Some(YoutubeItem {
        kind: "video".to_string(),
        url: entry
            .get("url")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| watch_url(&id)),
        title,
        channel: entry
            .get("channel")
            .or_else(|| entry.get("uploader"))
            .and_then(Value::as_str)
            .map(str::to_string),
        channel_url: entry
            .get("channel_url")
            .or_else(|| entry.get("uploader_url"))
            .and_then(Value::as_str)
            .map(str::to_string),
        duration_seconds,
        duration_text: duration_seconds.map(format_duration),
        view_count_text: entry
            .get("view_count")
            .and_then(Value::as_u64)
            .map(super::format_view_count),
        published_text: None,
        thumbnail_url: Some(format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")),
        video_count_text: None,
        badge: None,
        id,
    })
}
