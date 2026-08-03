//! Search orchestration: Innertube first (fast, filterable, cheap paging),
//! yt-dlp `ytsearch` as the automatic fallback. Pages are cached in memory
//! with a 15-minute TTL so tab switches and back-navigation are instant.

use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::AppHandle;

use super::{
    format_duration, format_view_count, innertube, watch_url, ytdlp, YoutubeItem,
    YoutubeSearchFilters, YoutubeSearchPage, YoutubeSearchPayload,
};

const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const CACHE_MAX_ENTRIES: usize = 60;
const YTDLP_PAGE_SIZE: usize = 25;

static SEARCH_CACHE: OnceLock<Mutex<HashMap<String, (Instant, YoutubeSearchPage)>>> =
    OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, (Instant, YoutubeSearchPage)>> {
    SEARCH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_get(key: &str) -> Option<YoutubeSearchPage> {
    let guard = cache().lock().ok()?;
    let (stored_at, page) = guard.get(key)?;
    (stored_at.elapsed() < CACHE_TTL).then(|| page.clone())
}

fn cache_put(key: String, page: YoutubeSearchPage) {
    let Ok(mut guard) = cache().lock() else {
        return;
    };
    guard.retain(|_, (stored_at, _)| stored_at.elapsed() < CACHE_TTL);
    if guard.len() >= CACHE_MAX_ENTRIES {
        if let Some(oldest) = guard
            .iter()
            .min_by_key(|(_, (stored_at, _))| *stored_at)
            .map(|(key, _)| key.clone())
        {
            guard.remove(&oldest);
        }
    }
    guard.insert(key, (Instant::now(), page));
}

/// Opaque pagination cursor, serialized to a JSON string for the frontend.
#[derive(Serialize, Deserialize)]
#[serde(tag = "t")]
enum Cursor {
    #[serde(rename = "innertube")]
    Innertube { token: String },
    #[serde(rename = "ytsearch")]
    Ytsearch { offset: usize },
}

/// Fired when the YouTube panel first becomes visible: warms the HTTP
/// client's connection and pays yt-dlp's first-spawn cost (AV scan +
/// emulation) in the background, so the first real search is fast.
#[tauri::command]
pub(crate) async fn youtube_warmup(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        super::pot::ensure_pot_server(&app);
        innertube::warm_connection(&app);
        let settings = crate::mpv::resolve_ytdlp_settings(&app);
        if let Some(path) = settings.binary.path {
            if let Err(error) = ytdlp::run_version(&path) {
                warn!("youtube: yt-dlp warm-up failed: {error}");
            }
        }
    })
    .await
    .map_err(|error| format!("YouTube warm-up worker failed: {error}"))
}

static PRERESOLVE_GENERATION: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Speculatively resolves the top search results in the background so the
/// click-to-play path is a cache hit. A newer call (fresh search) cancels
/// the remainder of an older one.
#[tauri::command]
pub(crate) async fn youtube_preresolve(
    app: AppHandle,
    urls: Vec<String>,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    let generation = PRERESOLVE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    tauri::async_runtime::spawn(async move {
        for url in urls.into_iter().take(4) {
            if PRERESOLVE_GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            let _ = crate::mpv::try_resolve_with_ytdlp(&app, &url, None).await;
        }
    });
    Ok(())
}

#[tauri::command]
pub(crate) async fn youtube_search(
    app: AppHandle,
    payload: YoutubeSearchPayload,
) -> Result<YoutubeSearchPage, String> {
    let query = payload.query.trim().to_string();
    if query.is_empty() {
        return Err("Enter a search query".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        run_search(&app, &query, &payload.filters, payload.cursor.as_deref())
    })
    .await
    .map_err(|error| format!("YouTube search worker failed: {error}"))?
}

fn run_search(
    app: &AppHandle,
    query: &str,
    filters: &YoutubeSearchFilters,
    cursor: Option<&str>,
) -> Result<YoutubeSearchPage, String> {
    let cache_key = serde_json::json!({
        "query": query,
        "filters": filters,
        "cursor": cursor,
    })
    .to_string();
    if let Some(page) = cache_get(&cache_key) {
        return Ok(page);
    }

    let parsed_cursor = cursor
        .map(|raw| {
            serde_json::from_str::<Cursor>(raw)
                .map_err(|error| format!("Invalid pagination cursor: {error}"))
        })
        .transpose()?;

    let page = match parsed_cursor {
        Some(Cursor::Innertube { token }) => match innertube::continue_search(app, &token) {
            Ok((items, next)) => innertube_page(items, next),
            Err(error) => {
                warn!("youtube: innertube continuation failed: {error}");
                return Err(
                    "YouTube stopped serving more results for this search — try again".to_string(),
                );
            }
        },
        Some(Cursor::Ytsearch { offset }) => ytdlp_search(app, query, filters, offset)?,
        None => {
            // Patient retries before the yt-dlp fallback — the fallback only
            // knows videos (no channels/playlists, no upload dates), so a
            // transient cold-start failure shouldn't degrade the results.
            let mut attempt = innertube::search(app, query, filters);
            for backoff_ms in [500u64, 1500] {
                if attempt.is_ok() {
                    break;
                }
                warn!(
                    "youtube: innertube search failed, retrying in {backoff_ms}ms: {}",
                    attempt.as_ref().err().map(String::as_str).unwrap_or("?")
                );
                std::thread::sleep(Duration::from_millis(backoff_ms));
                attempt = innertube::search(app, query, filters);
            }
            match attempt {
                Ok((items, next)) => innertube_page(items, next),
                Err(error) => {
                    warn!("youtube: innertube search failed, falling back to yt-dlp: {error}");
                    ytdlp_search(app, query, filters, 0)?
                }
            }
        }
    };

    cache_put(cache_key, page.clone());
    Ok(page)
}

fn innertube_page(items: Vec<YoutubeItem>, next: Option<String>) -> YoutubeSearchPage {
    let next_cursor = next.and_then(|token| {
        serde_json::to_string(&Cursor::Innertube { token }).ok()
    });
    YoutubeSearchPage {
        items,
        next_cursor,
        source: "innertube".to_string(),
    }
}

/// yt-dlp fallback: `ytsearchN:` only returns videos and knows no filter
/// params, so filters degrade to post-filtering (duration/type) and the
/// date-sort variant (`ytsearchdateN:`). Pagination re-runs the search with
/// a larger N and skips what was already delivered.
fn ytdlp_search(
    app: &AppHandle,
    query: &str,
    filters: &YoutubeSearchFilters,
    offset: usize,
) -> Result<YoutubeSearchPage, String> {
    let want_total = offset + YTDLP_PAGE_SIZE;
    let prefix = if filters.sort.as_deref() == Some("date") {
        "ytsearchdate"
    } else {
        "ytsearch"
    };
    let value = ytdlp::run_flat_json(app, &format!("{prefix}{want_total}:{query}"))?;
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let total_returned = entries.len();
    let items: Vec<YoutubeItem> = entries
        .iter()
        .skip(offset)
        .filter_map(flat_entry_to_item)
        .filter(|item| passes_post_filters(item, filters))
        .collect();
    // Fewer entries than requested means the result set is exhausted.
    let next_cursor = (total_returned >= want_total)
        .then(|| serde_json::to_string(&Cursor::Ytsearch { offset: want_total }).ok())
        .flatten();
    Ok(YoutubeSearchPage {
        items,
        next_cursor,
        source: "yt-dlp".to_string(),
    })
}

fn flat_entry_to_item(entry: &Value) -> Option<YoutubeItem> {
    let id = entry.get("id").and_then(Value::as_str)?.to_string();
    let title = entry
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())?
        .to_string();
    let duration_seconds = entry.get("duration").and_then(Value::as_f64);
    let view_count_text = entry
        .get("view_count")
        .and_then(Value::as_u64)
        .map(format_view_count);
    let is_live = entry
        .get("live_status")
        .and_then(Value::as_str)
        .is_some_and(|status| status == "is_live");
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
        view_count_text,
        published_text: None,
        thumbnail_url: Some(format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")),
        video_count_text: None,
        badge: is_live.then(|| "LIVE".to_string()),
        id,
    })
}

fn passes_post_filters(item: &YoutubeItem, filters: &YoutubeSearchFilters) -> bool {
    if let Some(kind) = filters.kind.as_deref() {
        if kind != "movie" && item.kind != kind {
            return false;
        }
    }
    if let Some(duration) = filters.duration.as_deref() {
        let Some(seconds) = item.duration_seconds else {
            return item.badge.as_deref() == Some("LIVE");
        };
        let matches = match duration {
            "short" => seconds < 4.0 * 60.0,
            "medium" => (4.0 * 60.0..=20.0 * 60.0).contains(&seconds),
            "long" => seconds > 20.0 * 60.0,
            _ => true,
        };
        if !matches {
            return false;
        }
    }
    // Upload-date and HD cannot be derived from flat entries — accept rather
    // than silently hiding everything.
    true
}

