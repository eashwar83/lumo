use crate::{json_value_to_string, mpv_command_checked, AppState};
use axum::extract::rejection::JsonRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, options, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::sync::OnceLock;
use tauri::Manager;
use tokio::net::TcpListener;

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:17668";
const BIND_ENV_VAR: &str = "SOIA_REMOTE_CONTROL_ADDR";
const TOKEN_ENV_VAR: &str = "SOIA_REMOTE_CONTROL_TOKEN";
const MAX_REQUEST_BODY_BYTES: usize = 64 * 1024;

static REMOTE_CONTROL_ADDR: OnceLock<SocketAddr> = OnceLock::new();

#[derive(Clone)]
struct RemoteControlState {
    app_handle: tauri::AppHandle,
    token: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CommandRequest {
    Object { args: Vec<serde_json::Value> },
    Array(Vec<serde_json::Value>),
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WebSocketClientMessage {
    Command {
        id: Option<String>,
        args: Vec<serde_json::Value>,
    },
    Ping {
        id: Option<String>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    ok: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandResponse {
    ok: bool,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WebSocketServerMessage {
    Hello { protocol_version: u32 },
    Pong { id: Option<String> },
    CommandResult { id: Option<String>, ok: bool },
    Error { id: Option<String>, error: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    error: String,
}

struct RemoteError {
    status: StatusCode,
    message: String,
}

impl RemoteError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }
}

impl IntoResponse for RemoteError {
    fn into_response(self) -> Response {
        with_cors((
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        ))
    }
}

pub(crate) fn start(app_handle: tauri::AppHandle) -> Result<(), String> {
    if REMOTE_CONTROL_ADDR.get().is_some() {
        return Ok(());
    }

    let bind_addr = std::env::var(BIND_ENV_VAR).unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string());
    let listener = StdTcpListener::bind(&bind_addr).map_err(|error| error.to_string())?;
    let addr = listener.local_addr().map_err(|error| error.to_string())?;
    let token = resolve_auth_token();
    validate_bind_addr(addr, token.as_deref())?;
    listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;

    let app_handle_for_thread = app_handle.clone();
    std::thread::Builder::new()
        .name("soia-remote-control".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_io()
                .enable_time()
                .thread_name("soia-remote-control-worker")
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    warn!("remote control: failed to create async runtime: {error}");
                    return;
                }
            };
            runtime.block_on(async move {
                match TcpListener::from_std(listener) {
                    Ok(listener) => {
                        let router = build_router(app_handle_for_thread, token);
                        if let Err(error) = axum::serve(listener, router).await {
                            warn!("remote control: server failed: {error}");
                        }
                    }
                    Err(error) => warn!("remote control: failed to adopt listener: {error}"),
                }
            });
        })
        .map_err(|error| error.to_string())?;

    let _ = REMOTE_CONTROL_ADDR.set(addr);
    info!("remote control: listening on http://{addr}");
    Ok(())
}

fn build_router(app_handle: tauri::AppHandle, token: Option<String>) -> Router {
    let state = RemoteControlState { app_handle, token };
    Router::new()
        .route("/health", get(health))
        .route("/mpv/command", post(mpv_command).options(options_handler))
        .route("/ws", get(websocket))
        .route("/*path", options(options_handler))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .with_state(state)
}

async fn health() -> Response {
    with_cors(Json(HealthResponse { ok: true }))
}

async fn options_handler() -> Response {
    with_cors(StatusCode::NO_CONTENT)
}

async fn mpv_command(
    State(state): State<RemoteControlState>,
    headers: HeaderMap,
    payload: Result<Json<CommandRequest>, JsonRejection>,
) -> Result<Response, RemoteError> {
    authorize_headers(&state, &headers)?;
    let Json(command_request) =
        payload.map_err(|error| RemoteError::bad_request(format!("invalid JSON: {error}")))?;
    execute_mpv_command(&state.app_handle, command_request).map_err(RemoteError::bad_request)?;
    Ok(with_cors(Json(CommandResponse { ok: true })))
}

async fn websocket(
    State(state): State<RemoteControlState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    match authorize_headers(&state, &headers) {
        Ok(()) => upgrade
            .on_upgrade(move |socket| handle_websocket(socket, state))
            .into_response(),
        Err(error) => error.into_response(),
    }
}

async fn handle_websocket(socket: WebSocket, state: RemoteControlState) {
    let (mut sender, mut receiver) = socket.split();
    let hello = WebSocketServerMessage::Hello {
        protocol_version: 1,
    };
    if send_ws_json(&mut sender, &hello).await.is_err() {
        return;
    }

    while let Some(message) = receiver.next().await {
        let message = match message {
            Ok(message) => message,
            Err(error) => {
                warn!("remote control: websocket receive failed: {error}");
                return;
            }
        };

        match message {
            Message::Text(text) => {
                let response = handle_websocket_text(&state, &text);
                if send_ws_json(&mut sender, &response).await.is_err() {
                    return;
                }
            }
            Message::Close(_) => return,
            Message::Ping(payload) => {
                if sender.send(Message::Pong(payload)).await.is_err() {
                    return;
                }
            }
            _ => {}
        }
    }
}

fn handle_websocket_text(
    state: &RemoteControlState,
    text: &str,
) -> WebSocketServerMessage {
    match serde_json::from_str::<WebSocketClientMessage>(text) {
        Ok(WebSocketClientMessage::Ping { id }) => WebSocketServerMessage::Pong { id },
        Ok(WebSocketClientMessage::Command { id, args }) => {
            match execute_mpv_command(&state.app_handle, CommandRequest::Object { args }) {
                Ok(()) => WebSocketServerMessage::CommandResult { id, ok: true },
                Err(error) => WebSocketServerMessage::Error {
                    id,
                    error,
                },
            }
        }
        Err(error) => WebSocketServerMessage::Error {
            id: None,
            error: format!("invalid websocket message: {error}"),
        },
    }
}

async fn send_ws_json(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: &WebSocketServerMessage,
) -> Result<(), String> {
    let text = serde_json::to_string(message).map_err(|error| error.to_string())?;
    sender
        .send(Message::Text(text))
        .await
        .map_err(|error| error.to_string())
}

fn execute_mpv_command(
    app_handle: &tauri::AppHandle,
    command_request: CommandRequest,
) -> Result<(), String> {
    let args = match command_request {
        CommandRequest::Object { args } => args,
        CommandRequest::Array(args) => args,
    };
    if args.is_empty() {
        return Err("mpv command args cannot be empty".to_string());
    }

    let args_owned: Vec<String> = args
        .into_iter()
        .map(json_value_to_string)
        .collect::<Result<Vec<_>, _>>()?;
    let args_str: Vec<&str> = args_owned.iter().map(String::as_str).collect();
    let state: tauri::State<'_, AppState> = app_handle.state();
    let mpv_guard = state.mpv_player.lock().map_err(|error| error.to_string())?;
    mpv_command_checked(&mpv_guard, &args_str)
}

fn resolve_auth_token() -> Option<String> {
    std::env::var(TOKEN_ENV_VAR)
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

fn validate_bind_addr(addr: SocketAddr, token: Option<&str>) -> Result<(), String> {
    if addr.ip().is_loopback() || token.is_some() {
        return Ok(());
    }
    Err(format!(
        "refusing to bind remote control to {addr} without {TOKEN_ENV_VAR}"
    ))
}

fn authorize_headers(state: &RemoteControlState, headers: &HeaderMap) -> Result<(), RemoteError> {
    let Some(expected_token) = state.token.as_deref() else {
        return Ok(());
    };

    let provided_token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .or_else(|| {
            headers
                .get("x-soia-remote-token")
                .and_then(|value| value.to_str().ok())
        });

    if provided_token == Some(expected_token) {
        Ok(())
    } else {
        Err(RemoteError::unauthorized("missing or invalid remote control token"))
    }
}

fn with_cors(response: impl IntoResponse) -> Response {
    let mut response = response.into_response();
    let headers = response.headers_mut();
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Authorization, Content-Type, X-Soia-Remote-Token"),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("86400"),
    );
    response
}
