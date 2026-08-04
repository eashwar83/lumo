//! YouTube browser module: key-less Innertube search with yt-dlp fallback,
//! result caching, and disk-cached thumbnails served as data URLs.
//!
//! All network I/O happens here in Rust (the webview never fetches directly),
//! matching the rest of the app. Subprocess spawns go through quiet commands
//! so no console window flashes over the player.

mod browse;
mod captions;
mod comments;
mod data;
mod downloads;
mod innertube;
mod pot;
mod sponsorblock;
mod thumbs;
mod watch;
mod ytdlp;

pub(crate) use browse::{
    __cmd__youtube_channel, __cmd__youtube_playlist, __cmd__youtube_trending,
    youtube_channel, youtube_playlist, youtube_trending,
};
pub(crate) use captions::{
    __cmd__youtube_caption_file, __cmd__youtube_caption_tracks, youtube_caption_file,
    youtube_caption_tracks,
};
pub(crate) use comments::{
    __cmd__youtube_comments, __cmd__youtube_translate_comments, youtube_comments,
    youtube_translate_comments,
};
pub(crate) use downloads::{
    __cmd__youtube_download_add, __cmd__youtube_download_cancel,
    __cmd__youtube_download_clear_done, __cmd__youtube_download_list,
    __cmd__youtube_download_open_folder, __cmd__youtube_download_pause,
    __cmd__youtube_download_remove, __cmd__youtube_download_resume,
    youtube_download_add, youtube_download_cancel, youtube_download_clear_done,
    youtube_download_list, youtube_download_open_folder, youtube_download_pause,
    youtube_download_remove, youtube_download_resume,
};
pub(crate) use pot::{ensure_pot_server, shutdown_pot_server};
pub(crate) use sponsorblock::{__cmd__youtube_sponsorblock, youtube_sponsorblock};
pub(crate) use ytdlp::{
    __cmd__youtube_ytdlp_status, __cmd__youtube_ytdlp_update, youtube_ytdlp_status,
    youtube_ytdlp_update,
};

use serde::{Deserialize, Serialize};

pub(crate) use data::{
    __cmd__youtube_preresolve, __cmd__youtube_search, __cmd__youtube_warmup,
    youtube_preresolve, youtube_search, youtube_warmup,
};
pub(crate) use thumbs::{__cmd__youtube_thumbnail, youtube_thumbnail};
pub(crate) use watch::{__cmd__youtube_video_context, youtube_video_context};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YoutubeItem {
    /// "video" | "channel" | "playlist"
    pub(crate) kind: String,
    pub(crate) id: String,
    pub(crate) url: String,
    pub(crate) title: String,
    pub(crate) channel: Option<String>,
    pub(crate) channel_url: Option<String>,
    pub(crate) duration_seconds: Option<f64>,
    pub(crate) duration_text: Option<String>,
    pub(crate) view_count_text: Option<String>,
    pub(crate) published_text: Option<String>,
    pub(crate) thumbnail_url: Option<String>,
    pub(crate) video_count_text: Option<String>,
    /// Set to "LIVE" for live streams.
    pub(crate) badge: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct YoutubeSearchFilters {
    /// "relevance" (default) | "date" | "views" | "rating"
    pub(crate) sort: Option<String>,
    /// "hour" | "today" | "week" | "month" | "year"
    pub(crate) upload_date: Option<String>,
    /// "short" | "medium" | "long"
    pub(crate) duration: Option<String>,
    /// "video" | "channel" | "playlist" | "movie"
    pub(crate) kind: Option<String>,
    pub(crate) hd: Option<bool>,
}

impl YoutubeSearchFilters {
    pub(crate) fn is_empty(&self) -> bool {
        self.sort.as_deref().unwrap_or("relevance") == "relevance"
            && self.upload_date.is_none()
            && self.duration.is_none()
            && self.kind.is_none()
            && !self.hd.unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YoutubeSearchPage {
    pub(crate) items: Vec<YoutubeItem>,
    /// Opaque cursor to fetch the next page; None when exhausted.
    pub(crate) next_cursor: Option<String>,
    /// "innertube" | "yt-dlp" — which backend produced this page.
    pub(crate) source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YoutubeSearchPayload {
    pub(crate) query: String,
    #[serde(default)]
    pub(crate) filters: YoutubeSearchFilters,
    #[serde(default)]
    pub(crate) cursor: Option<String>,
}

pub(crate) fn watch_url(id: &str) -> String {
    format!("https://www.youtube.com/watch?v={id}")
}

/// 2_200_000_000 -> "2.2B views" (YouTube's own shorthand).
pub(crate) fn format_view_count(count: u64) -> String {
    let formatted = if count >= 1_000_000_000 {
        format!("{:.1}B", count as f64 / 1_000_000_000.0)
    } else if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        return format!("{count} views");
    };
    format!("{} views", formatted.replace(".0", ""))
}

pub(crate) fn format_duration(seconds: f64) -> String {
    let total = seconds.round().max(0.0) as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}
