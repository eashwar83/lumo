//! SponsorBlock skip segments (sponsor.ajay.app). Community data, fetched
//! per video with a short cache. Everything fails silent: no segments is
//! the normal case (404) and playback never depends on this.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::AppHandle;

const API_URL: &str = "https://sponsor.ajay.app/api/skipSegments";
const DEFAULT_CATEGORIES: &str = r#"["sponsor","intro","selfpromo"]"#;
const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const CACHE_MAX: usize = 60;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SponsorSegment {
    pub(crate) category: String,
    pub(crate) start_seconds: f64,
    pub(crate) end_seconds: f64,
}

static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Vec<SponsorSegment>)>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, (Instant, Vec<SponsorSegment>)>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[tauri::command]
pub(crate) async fn youtube_sponsorblock(
    app: AppHandle,
    video_id: String,
    categories: Option<Vec<String>>,
) -> Result<Vec<SponsorSegment>, String> {
    let video_id = video_id.trim().to_string();
    if video_id.is_empty() {
        return Ok(Vec::new());
    }
    let categories = categories
        .filter(|list| !list.is_empty())
        .map(|list| {
            serde_json::to_string(&list).unwrap_or_else(|_| DEFAULT_CATEGORIES.to_string())
        })
        .unwrap_or_else(|| DEFAULT_CATEGORIES.to_string());
    tauri::async_runtime::spawn_blocking(move || {
        Ok(fetch_segments(&app, &video_id, &categories))
    })
    .await
    .map_err(|error| format!("SponsorBlock worker failed: {error}"))?
}

fn fetch_segments(app: &AppHandle, video_id: &str, categories: &str) -> Vec<SponsorSegment> {
    let cache_key = format!("{video_id}|{categories}");
    if let Ok(guard) = cache().lock() {
        if let Some((stored_at, segments)) = guard.get(&cache_key) {
            if stored_at.elapsed() < CACHE_TTL {
                return segments.clone();
            }
        }
    }

    let segments = request_segments(app, video_id, categories).unwrap_or_default();

    if let Ok(mut guard) = cache().lock() {
        guard.retain(|_, (stored_at, _)| stored_at.elapsed() < CACHE_TTL);
        if guard.len() >= CACHE_MAX {
            if let Some(oldest) = guard
                .iter()
                .min_by_key(|(_, (stored_at, _))| *stored_at)
                .map(|(key, _)| key.clone())
            {
                guard.remove(&oldest);
            }
        }
        guard.insert(cache_key, (Instant::now(), segments.clone()));
    }
    segments
}

fn request_segments(
    app: &AppHandle,
    video_id: &str,
    categories: &str,
) -> Option<Vec<SponsorSegment>> {
    let client = super::innertube::blocking_client(app).ok()?;
    let response = client
        .get(API_URL)
        .query(&[("videoID", video_id), ("categories", categories)])
        .send()
        .ok()?;
    if !response.status().is_success() {
        // 404 = no segments submitted for this video; anything else is
        // equally non-fatal.
        return None;
    }
    let value: Value = response.json().ok()?;
    let mut segments: Vec<SponsorSegment> = value
        .as_array()?
        .iter()
        .filter_map(|entry| {
            let category = entry.get("category")?.as_str()?.to_string();
            let range = entry.get("segment")?.as_array()?;
            let start_seconds = range.first()?.as_f64()?;
            let end_seconds = range.get(1)?.as_f64()?;
            (end_seconds > start_seconds).then_some(SponsorSegment {
                category,
                start_seconds,
                end_seconds,
            })
        })
        .collect();
    segments.sort_by(|a, b| a.start_seconds.total_cmp(&b.start_seconds));
    Some(segments)
}
