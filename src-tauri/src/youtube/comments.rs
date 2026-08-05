//! Watch-page comments via Innertube, plus optional AI translation.
//!
//! Modern YouTube returns the comment bodies in a separate entity batch, so
//! the renderers only carry ordering and the payloads carry the text; both
//! halves are stitched back together here.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tauri::AppHandle;

use super::innertube;

const NEXT_URL: &str = "https://www.youtube.com/youtubei/v1/next?prettyPrint=false";
const CLIENT_VERSION: &str = "2.20250801.01.00";
/// Keeps one request's worth of translation work sane.
const TRANSLATE_BATCH: usize = 20;
/// Marks a continuation token that was minted under a signed-in session.
const AUTHED_CURSOR_PREFIX: &str = "auth\u{1}";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YoutubeComment {
    pub(crate) id: String,
    pub(crate) author: String,
    pub(crate) author_thumbnail: Option<String>,
    pub(crate) text: String,
    pub(crate) published_text: Option<String>,
    pub(crate) like_count_text: Option<String>,
    pub(crate) reply_count_text: Option<String>,
    pub(crate) is_pinned: bool,
    pub(crate) is_hearted: bool,
    /// Continuation token for this comment's replies, when it has any.
    pub(crate) reply_token: Option<String>,
}

/// One entry of YouTube's "Sort by" menu ("Top" / "Newest"). The token
/// reloads the whole comment list in that order.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentSortOption {
    pub(crate) title: String,
    pub(crate) token: String,
    pub(crate) selected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YoutubeCommentPage {
    pub(crate) comments: Vec<YoutubeComment>,
    pub(crate) next_cursor: Option<String>,
    pub(crate) total_text: Option<String>,
    /// Only populated on the first page; the UI keeps them afterwards.
    pub(crate) sort_options: Vec<CommentSortOption>,
    /// How many comments the video has, when YouTube says.
    pub(crate) total_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentsPayload {
    pub(crate) video_id: String,
    /// Continuation token from a previous page.
    #[serde(default)]
    pub(crate) cursor: Option<String>,
    /// A sort option's token: loads the list afresh in that order.
    #[serde(default)]
    pub(crate) sort_token: Option<String>,
}

fn client_context() -> Value {
    json!({ "client": { "clientName": "WEB", "clientVersion": CLIENT_VERSION, "hl": "en" } })
}

fn post_next(
    app: &AppHandle,
    body: Value,
    auth: Option<&super::auth::SessionAuth>,
) -> Result<Value, String> {
    let client = innertube::blocking_client(app)?;
    let request = super::auth::apply(client.post(NEXT_URL).json(&body), auth);
    let response = request
        .send()
        .map_err(|error| format!("Comments request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Comments: HTTP {}", response.status()));
    }
    response
        .json::<Value>()
        .map_err(|error| format!("Comments: invalid JSON: {error}"))
}

#[tauri::command]
pub(crate) async fn youtube_comments(
    app: AppHandle,
    payload: CommentsPayload,
) -> Result<YoutubeCommentPage, String> {
    tauri::async_runtime::spawn_blocking(move || fetch_comments(&app, &payload))
        .await
        .map_err(|error| format!("Comments worker failed: {error}"))?
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RepliesPayload {
    /// A comment's reply token, or a cursor from a previous reply page.
    pub(crate) token: String,
}

/// Loads one page of replies to a single comment.
#[tauri::command]
pub(crate) async fn youtube_comment_replies(
    app: AppHandle,
    payload: RepliesPayload,
) -> Result<YoutubeCommentPage, String> {
    tauri::async_runtime::spawn_blocking(move || fetch_replies_page(&app, &payload.token))
        .await
        .map_err(|error| format!("Replies worker failed: {error}"))?
}

/// Blocking form of [`youtube_comment_replies`], for the crawler.
pub(super) fn fetch_replies_page(
    app: &AppHandle,
    token: &str,
) -> Result<YoutubeCommentPage, String> {
    let mut auth = None;
    let token = match token.strip_prefix(AUTHED_CURSOR_PREFIX) {
        Some(inner) => {
            auth = super::auth::session_auth(app);
            inner.to_string()
        }
        None => token.to_string(),
    };
    let value = post_next(
        app,
        json!({ "context": client_context(), "continuation": token }),
        auth.as_ref(),
    )?;
    let mut page = parse_comment_page(&value);
    if auth.is_some() {
        page.next_cursor = page
            .next_cursor
            .map(|cursor| format!("{AUTHED_CURSOR_PREFIX}{cursor}"));
        for comment in &mut page.comments {
            comment.reply_token = comment
                .reply_token
                .take()
                .map(|token| format!("{AUTHED_CURSOR_PREFIX}{token}"));
        }
    }
    Ok(page)
}

/// Blocking form of [`youtube_comments`], for the crawler.
pub(super) fn fetch_page(
    app: &AppHandle,
    video_id: &str,
    cursor: Option<String>,
    sort_token: Option<String>,
) -> Result<YoutubeCommentPage, String> {
    fetch_comments(
        app,
        &CommentsPayload {
            video_id: video_id.to_string(),
            cursor,
            sort_token,
        },
    )
}

/// What the watch response tells us about a video's comments section.
struct CommentsEntry {
    token: String,
    sort_options: Vec<CommentSortOption>,
    total_count: Option<u32>,
}

fn fetch_comments(app: &AppHandle, payload: &CommentsPayload) -> Result<YoutubeCommentPage, String> {
    let mut auth = None;
    // Strips the marker that says a token was minted under a session; a
    // token only works under the session that produced it, and this keeps
    // anonymous videos anonymous when paging or re-sorting.
    let take_token = |value: String, auth: &mut Option<super::auth::SessionAuth>| {
        match value.strip_prefix(AUTHED_CURSOR_PREFIX) {
            Some(inner) => {
                *auth = super::auth::session_auth(app);
                inner.to_string()
            }
            None => value,
        }
    };

    let mut entry = None;
    // A sort choice reloads the list; a cursor extends it. Either way the
    // token is already in hand and no watch call is needed.
    let token = match payload
        .sort_token
        .clone()
        .or_else(|| payload.cursor.clone())
    {
        Some(value) => take_token(value, &mut auth),
        None => {
            let body = json!({ "context": client_context(), "videoId": payload.video_id });
            let watch = post_next(app, body.clone(), None)?;
            let found = match comments_entry(&watch) {
                Some(found) => Some(found),
                None => {
                    // YouTube hides an age-restricted video's comments from
                    // signed-out clients and reports them as turned off.
                    // Retry signed before believing it.
                    auth = super::auth::session_auth(app);
                    auth.as_ref()
                        .map(|auth| post_next(app, body, Some(auth)))
                        .transpose()?
                        .as_ref()
                        .and_then(comments_entry)
                }
            };
            let Some(found) = found else {
                return Ok(YoutubeCommentPage {
                    comments: Vec::new(),
                    next_cursor: None,
                    total_text: Some(if auth.is_some() {
                        "Comments are turned off".to_string()
                    } else {
                        "Comments are hidden for signed-out viewers — set a cookies \
                         file in Settings → YouTube"
                            .to_string()
                    }),
                    sort_options: Vec::new(),
                    total_count: None,
                });
            };
            let token = found.token.clone();
            entry = Some(found);
            token
        }
    };

    let value = post_next(
        app,
        json!({ "context": client_context(), "continuation": token }),
        auth.as_ref(),
    )?;
    let mut page = parse_comment_page(&value);
    if let Some(entry) = entry {
        page.total_count = entry.total_count;
        page.sort_options = entry.sort_options;
        if let Some(count) = entry.total_count {
            page.total_text = Some(format!("{count} comments"));
        }
    }
    // Sort and reply tokens are as session-bound as continuations are.
    if auth.is_some() {
        page.next_cursor = page
            .next_cursor
            .map(|cursor| format!("{AUTHED_CURSOR_PREFIX}{cursor}"));
        for option in &mut page.sort_options {
            option.token = format!("{AUTHED_CURSOR_PREFIX}{}", option.token);
        }
        for comment in &mut page.comments {
            comment.reply_token = comment
                .reply_token
                .take()
                .map(|token| format!("{AUTHED_CURSOR_PREFIX}{token}"));
        }
    }
    Ok(page)
}

/// Reads the comments engagement panel: its continuation token, the
/// "Sort by" menu, and the comment count shown beside the title.
fn comments_entry(watch: &Value) -> Option<CommentsEntry> {
    watch
        .get("engagementPanels")?
        .as_array()?
        .iter()
        .find_map(|panel| {
            let renderer = panel.get("engagementPanelSectionListRenderer")?;
            let identifier = renderer.get("panelIdentifier").and_then(Value::as_str)?;
            if !identifier.contains("comments") {
                return None;
            }
            let token = renderer
                .pointer("/content/sectionListRenderer/contents/0/itemSectionRenderer/contents/0/continuationItemRenderer/continuationEndpoint/continuationCommand/token")
                .and_then(Value::as_str)?
                .to_string();

            let header = renderer.pointer("/header/engagementPanelTitleHeaderRenderer");
            let total_count = header
                .and_then(|header| header.pointer("/contextualInfo/runs/0/text"))
                .and_then(Value::as_str)
                .and_then(|text| text.replace(',', "").parse::<u32>().ok());
            let sort_options = header
                .and_then(|header| header.pointer("/menu/sortFilterSubMenuRenderer/subMenuItems"))
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(parse_sort_option).collect())
                .unwrap_or_default();

            Some(CommentsEntry {
                token,
                sort_options,
                total_count,
            })
        })
}

fn parse_sort_option(item: &Value) -> Option<CommentSortOption> {
    Some(CommentSortOption {
        title: item.get("title").and_then(Value::as_str)?.to_string(),
        token: item
            .pointer("/serviceEndpoint/continuationCommand/token")
            .and_then(Value::as_str)?
            .to_string(),
        selected: item
            .get("selected")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn parse_comment_page(value: &Value) -> YoutubeCommentPage {
    // Bodies live in the entity batch, keyed by comment id.
    let mut comments: Vec<YoutubeComment> = value
        .pointer("/frameworkUpdates/entityBatchUpdate/mutations")
        .and_then(Value::as_array)
        .map(|mutations| {
            mutations
                .iter()
                .filter_map(|mutation| {
                    parse_comment_entity(mutation.pointer("/payload/commentEntityPayload")?)
                })
                .collect()
        })
        .unwrap_or_default();
    comments.retain(|comment| !comment.text.trim().is_empty());

    let items: Vec<Value> = value
        .get("onResponseReceivedEndpoints")
        .and_then(Value::as_array)
        .map(|endpoints| {
            endpoints
                .iter()
                .filter_map(|endpoint| {
                    endpoint
                        .pointer("/reloadContinuationItemsCommand/continuationItems")
                        .or_else(|| {
                            endpoint.pointer("/appendContinuationItemsAction/continuationItems")
                        })
                        .and_then(Value::as_array)
                        .cloned()
                })
                .flatten()
                .collect()
        })
        .unwrap_or_default();

    let next_cursor = items.iter().find_map(|item| {
        item.pointer("/continuationItemRenderer/continuationEndpoint/continuationCommand/token")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let total_text = items.iter().find_map(|item| {
        item.pointer("/commentsHeaderRenderer/countText/runs/0/text")
            .and_then(Value::as_str)
            .map(|count| format!("{count} comments"))
    });

    // A thread that has replies carries its own continuation token. The
    // bodies arrive in the same entity batch as everything else, so the
    // token is the only thing the renderers add — match it to its comment
    // and hand it to the UI as the "view replies" handle.
    for item in &items {
        let Some(thread) = item.get("commentThreadRenderer") else {
            continue;
        };
        let Some(id) = thread
            .pointer("/commentViewModel/commentViewModel/commentId")
            .and_then(Value::as_str)
        else {
            continue;
        };
        let token = thread
            .pointer("/replies/commentRepliesRenderer/contents/0/continuationItemRenderer/continuationEndpoint/continuationCommand/token")
            .and_then(Value::as_str);
        if let (Some(token), Some(comment)) = (
            token,
            comments.iter_mut().find(|comment| comment.id == id),
        ) {
            comment.reply_token = Some(token.to_string());
        }
    }

    YoutubeCommentPage {
        comments,
        next_cursor,
        total_text,
        sort_options: Vec::new(),
        total_count: None,
    }
}

fn parse_comment_entity(entity: &Value) -> Option<YoutubeComment> {
    let properties = entity.get("properties")?;
    let text = properties
        .pointer("/content/content")
        .and_then(Value::as_str)?
        .to_string();
    let author = entity
        .pointer("/author/displayName")
        .and_then(Value::as_str)
        .unwrap_or("Someone")
        .to_string();
    let toolbar = entity.get("toolbar");
    Some(YoutubeComment {
        id: properties
            .get("commentId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        author,
        author_thumbnail: entity
            .pointer("/author/avatarThumbnailUrl")
            .and_then(Value::as_str)
            .map(str::to_string),
        text,
        published_text: properties
            .get("publishedTime")
            .and_then(Value::as_str)
            .map(str::to_string),
        like_count_text: toolbar
            .and_then(|bar| bar.get("likeCountNotliked"))
            .and_then(Value::as_str)
            .filter(|count| !count.trim().is_empty())
            .map(str::to_string),
        reply_count_text: toolbar
            .and_then(|bar| bar.get("replyCount"))
            .and_then(Value::as_str)
            .filter(|count| !count.trim().is_empty())
            .map(str::to_string),
        is_pinned: entity
            .pointer("/author/isCreator")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        is_hearted: toolbar
            .and_then(|bar| bar.get("heartState"))
            .and_then(Value::as_str)
            .is_some_and(|state| state.contains("HEARTED")),
        // Filled in from the thread renderer, which owns the token.
        reply_token: None,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslatePayload {
    pub(crate) texts: Vec<String>,
    pub(crate) target_language: String,
    pub(crate) chat_base: String,
    pub(crate) chat_key: String,
    pub(crate) chat_model: String,
}

/// Translates comment bodies with the configured AI provider, reusing the
/// subtitle translator's batching so the model answers 1:1.
#[tauri::command]
pub(crate) async fn youtube_translate_comments(
    payload: TranslatePayload,
) -> Result<Vec<String>, String> {
    if payload.texts.is_empty() {
        return Ok(Vec::new());
    }
    if payload.chat_key.trim().is_empty() {
        return Err(
            "Add an AI key in Settings → Advanced → AI to translate comments".to_string(),
        );
    }
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|error| format!("Failed to build HTTP client: {error}"))?;
        let url = format!(
            "{}/chat/completions",
            payload.chat_base.trim_end_matches('/')
        );
        let mut translated = Vec::with_capacity(payload.texts.len());
        for chunk in payload.texts.chunks(TRANSLATE_BATCH) {
            translated.extend(crate::ai_subtitles::translate_batch(
                &client,
                &url,
                &payload.chat_key,
                &payload.chat_model,
                &payload.target_language,
                chunk,
            ));
        }
        Ok(translated)
    })
    .await
    .map_err(|error| format!("Translation worker failed: {error}"))?
}
