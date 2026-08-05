//! Fetches a video's whole comment thread so search can reach comments
//! that were never scrolled into view.
//!
//! YouTube has no comment-search endpoint, so "search everything" means
//! walking every continuation page locally. That is cheap on a small
//! video and expensive on a large one, so the crawl streams results as
//! they arrive, reports progress, and can be stopped at any point.

use serde::Deserialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::{AppHandle, Emitter};

use super::comments::{self, YoutubeComment};

/// New comments, emitted per page so matches appear while the rest loads.
pub(crate) const BATCH_EVENT: &str = "youtube://comments-batch";
/// How far the crawl has got.
pub(crate) const PROGRESS_EVENT: &str = "youtube://comments-progress";

/// Politeness gap between requests. A full crawl of a busy video is
/// hundreds of calls; pacing them keeps YouTube from rate-limiting us.
const REQUEST_GAP_MS: u64 = 120;

static CANCEL: AtomicBool = AtomicBool::new(false);
/// Identifies the crawl a batch belongs to, so a stale one that is still
/// unwinding cannot pour results into a video the user has moved on to.
static RUN_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrawlPayload {
    pub(crate) video_id: String,
    /// Continue from where the UI has already read, when it has.
    #[serde(default)]
    pub(crate) cursor: Option<String>,
    /// Also open every thread's replies.
    #[serde(default)]
    pub(crate) include_replies: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CrawlBatch {
    run_id: u64,
    comments: Vec<YoutubeComment>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CrawlProgress {
    run_id: u64,
    loaded: usize,
    done: bool,
    stopped: bool,
    error: Option<String>,
}

/// Asks the running crawl to stop at the next page boundary.
#[tauri::command]
pub(crate) fn youtube_comments_stop() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// Walks every remaining comment page (and optionally every reply
/// thread), emitting each batch as it lands. Returns the run id so the
/// caller can ignore events from a crawl it has since abandoned.
#[tauri::command]
pub(crate) async fn youtube_comments_fetch_all(
    app: AppHandle,
    payload: CrawlPayload,
) -> Result<u64, String> {
    CANCEL.store(true, Ordering::SeqCst);
    let run_id = RUN_ID.fetch_add(1, Ordering::SeqCst) + 1;

    tauri::async_runtime::spawn_blocking(move || {
        // Let any in-flight page from the previous crawl notice the stop
        // before this one clears the flag.
        std::thread::sleep(std::time::Duration::from_millis(REQUEST_GAP_MS));
        CANCEL.store(false, Ordering::SeqCst);
        crawl(&app, run_id, payload);
    });

    Ok(run_id)
}

fn cancelled(run_id: u64) -> bool {
    CANCEL.load(Ordering::SeqCst) || RUN_ID.load(Ordering::SeqCst) != run_id
}

fn crawl(app: &AppHandle, run_id: u64, payload: CrawlPayload) {
    let mut loaded = 0usize;
    let mut cursor = payload.cursor.clone();
    let mut error = None;

    loop {
        if cancelled(run_id) {
            break;
        }
        // Without a cursor this is the first page, which also needs the
        // watch call to find the comments section.
        let page = match comments::fetch_page(app, &payload.video_id, cursor.clone(), None) {
            Ok(page) => page,
            Err(message) => {
                error = Some(message);
                break;
            }
        };

        let mut batch = page.comments.clone();
        if payload.include_replies {
            for comment in &page.comments {
                if cancelled(run_id) {
                    break;
                }
                let Some(token) = comment.reply_token.clone() else {
                    continue;
                };
                batch.extend(fetch_thread(app, run_id, token));
            }
        }

        loaded += batch.len();
        emit_batch(app, run_id, batch);
        emit_progress(app, run_id, loaded, false, false, None);

        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(REQUEST_GAP_MS));
    }

    let stopped = cancelled(run_id);
    emit_progress(app, run_id, loaded, true, stopped, error);
}

/// Every reply to one comment, following the thread's own paging.
fn fetch_thread(app: &AppHandle, run_id: u64, first_token: String) -> Vec<YoutubeComment> {
    let mut replies = Vec::new();
    let mut token = Some(first_token);
    while let Some(current) = token {
        if cancelled(run_id) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(REQUEST_GAP_MS));
        match comments::fetch_replies_page(app, &current) {
            Ok(page) => {
                replies.extend(page.comments);
                token = page.next_cursor;
            }
            // One unreachable thread should not abort the whole crawl.
            Err(_) => break,
        }
    }
    replies
}

fn emit_batch(app: &AppHandle, run_id: u64, comments: Vec<YoutubeComment>) {
    if comments.is_empty() {
        return;
    }
    let _ = app.emit(BATCH_EVENT, CrawlBatch { run_id, comments });
}

fn emit_progress(
    app: &AppHandle,
    run_id: u64,
    loaded: usize,
    done: bool,
    stopped: bool,
    error: Option<String>,
) {
    let _ = app.emit(
        PROGRESS_EVENT,
        CrawlProgress {
            run_id,
            loaded,
            done,
            stopped,
            error,
        },
    );
}
