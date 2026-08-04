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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YoutubeCommentPage {
    pub(crate) comments: Vec<YoutubeComment>,
    pub(crate) next_cursor: Option<String>,
    pub(crate) total_text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentsPayload {
    pub(crate) video_id: String,
    /// Continuation token from a previous page.
    #[serde(default)]
    pub(crate) cursor: Option<String>,
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

fn fetch_comments(app: &AppHandle, payload: &CommentsPayload) -> Result<YoutubeCommentPage, String> {
    // Page one needs the watch response first: it holds the token that opens
    // the comments section.
    let mut auth = None;
    let token = match payload.cursor.clone() {
        // A cursor minted under a session only works under that session;
        // the marker keeps anonymous videos anonymous on "load more".
        Some(cursor) => match cursor.strip_prefix(AUTHED_CURSOR_PREFIX) {
            Some(inner) => {
                auth = super::auth::session_auth(app);
                inner.to_string()
            }
            None => cursor,
        },
        None => {
            let body = json!({ "context": client_context(), "videoId": payload.video_id });
            let watch = post_next(app, body.clone(), None)?;
            match comments_token(&watch) {
                Some(token) => token,
                None => {
                    // YouTube hides an age-restricted video's comments from
                    // signed-out clients and reports them as turned off.
                    // Retry signed before believing it.
                    auth = super::auth::session_auth(app);
                    let signed = auth
                        .as_ref()
                        .map(|auth| post_next(app, body, Some(auth)))
                        .transpose()?;
                    let Some(token) = signed.as_ref().and_then(comments_token) else {
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
                        });
                    };
                    token
                }
            }
        }
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
    }
    Ok(page)
}

fn comments_token(watch: &Value) -> Option<String> {
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
            renderer
                .pointer("/content/sectionListRenderer/contents/0/itemSectionRenderer/contents/0/continuationItemRenderer/continuationEndpoint/continuationCommand/token")
                .and_then(Value::as_str)
                .map(str::to_string)
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

    YoutubeCommentPage {
        comments,
        next_cursor,
        total_text,
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
