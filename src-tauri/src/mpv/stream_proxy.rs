use log::{debug, info, warn};
use percent_encoding::percent_decode_str;
use reqwest::header::{
    HeaderName, HeaderValue, ACCEPT_ENCODING, ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE,
    CONTENT_TYPE, RANGE, USER_AGENT,
};
use futures_util::future::BoxFuture;
use futures_util::stream::{FuturesUnordered, StreamExt};
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use std::collections::HashMap;
use std::net::TcpListener as StdTcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

const HTTP_USER_AGENT: &str = "Lavf/61.7.100";
const MAX_REQUEST_HEADER_BYTES: usize = 128 * 1024;
const FETCH_REMOTE_MAX_RETRIES: usize = 2;
const FETCH_REMOTE_RETRY_DELAY: Duration = Duration::from_millis(500);
const PARALLEL_RANGE_MIN_BYTES: u64 = 16 * 1024 * 1024;
const PARALLEL_RANGE_CHUNK_BYTES: u64 = 2 * 1024 * 1024;
const PARALLEL_RANGE_CONNECTIONS: usize = 3;
const PARALLEL_RANGE_SETTING_LABEL: &str = "NETWORK_PARALLEL_DOWNLOAD";
const SMB_STREAM_CHUNK_BYTES: u64 = 4 * 1024 * 1024;
const STREAM_BACKEND_IDLE_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const STREAM_BACKEND_CLEANUP_INTERVAL: Duration = Duration::from_secs(60);
const STREAM_BACKEND_MAX_ENTRIES: usize = 4096;
const STREAM_BACKEND_TARGET_ENTRIES: usize = 3072;

type BasicAuth = (String, String);
pub(crate) type ProxyHeaders = Vec<(String, String)>;

static STREAM_PROXY_BASE_URL: OnceLock<String> = OnceLock::new();
static STREAM_PROXY_BASIC_AUTH: OnceLock<Mutex<HashMap<String, BasicAuth>>> = OnceLock::new();
static STREAM_PROXY_HEADERS: OnceLock<Mutex<HashMap<String, ProxyHeaders>>> = OnceLock::new();
static STREAM_PROXY_CLIENT: OnceLock<Mutex<Option<CachedClient>>> = OnceLock::new();
static STREAM_PROXY_PARALLEL_RANGE_ENABLED: AtomicBool = AtomicBool::new(false);
static STREAM_PROXY_BACKENDS: OnceLock<Mutex<StreamBackendRegistry>> =
    OnceLock::new();

struct CachedClient {
    proxy_key: Option<String>,
    client: Client,
}

#[derive(Clone)]
struct ByteRange {
    start: u64,
    end: u64,
}

#[derive(Clone)]
struct ParallelRangePlan {
    response_start: u64,
    response_end: u64,
    total_size: u64,
    content_length: u64,
}

struct StreamBackendEntry {
    backend: Arc<dyn StreamBackend>,
    last_access: Instant,
}

struct StreamBackendRegistry {
    entries: HashMap<String, StreamBackendEntry>,
    last_cleanup: Instant,
}

impl StreamBackendRegistry {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            entries: HashMap::new(),
            last_cleanup: now,
        }
    }

    fn insert(&mut self, token: String, backend: Arc<dyn StreamBackend>) {
        let now = Instant::now();
        self.cleanup_if_due(now);
        self.entries.insert(
            token,
            StreamBackendEntry {
                backend,
                last_access: now,
            },
        );
        self.enforce_limit(now);
    }

    fn get(&mut self, token: &str) -> Option<Arc<dyn StreamBackend>> {
        let now = Instant::now();
        self.cleanup_if_due(now);
        let entry = self.entries.get_mut(token)?;
        entry.last_access = now;
        Some(entry.backend.clone())
    }

    fn cleanup_if_due(&mut self, now: Instant) {
        if now.duration_since(self.last_cleanup) < STREAM_BACKEND_CLEANUP_INTERVAL
            && self.entries.len() <= STREAM_BACKEND_MAX_ENTRIES
        {
            return;
        }
        self.cleanup_idle(now);
    }

    fn cleanup_idle(&mut self, now: Instant) {
        let before = self.entries.len();
        self.entries
            .retain(|_, entry| now.duration_since(entry.last_access) <= STREAM_BACKEND_IDLE_TIMEOUT);
        self.last_cleanup = now;
        let removed = before.saturating_sub(self.entries.len());
        if removed > 0 {
            debug!("stream proxy: cleaned up {removed} idle backend token(s)");
        }
    }

    fn enforce_limit(&mut self, now: Instant) {
        if self.entries.len() <= STREAM_BACKEND_MAX_ENTRIES {
            return;
        }
        let remove_count = self
            .entries
            .len()
            .saturating_sub(STREAM_BACKEND_TARGET_ENTRIES);
        let mut oldest = self
            .entries
            .iter()
            .map(|(token, entry)| (token.clone(), entry.last_access))
            .collect::<Vec<_>>();
        oldest.sort_by_key(|(_, last_access)| *last_access);
        for (token, _) in oldest.into_iter().take(remove_count) {
            self.entries.remove(&token);
        }
        self.last_cleanup = now;
        debug!(
            "stream proxy: evicted {remove_count} backend token(s) to enforce registry limit"
        );
    }
}

trait StreamBackend: Send + Sync {
    fn label(&self) -> &'static str;

    fn origin(&self) -> &str;

    fn handle<'a>(
        &'a self,
        app_handle: &'a AppHandle,
        stream: &'a mut TcpStream,
        method: &'a str,
        range: Option<&'a str>,
    ) -> BoxFuture<'a, Result<(), String>>;
}

struct HttpStreamBackend {
    url: String,
}

impl HttpStreamBackend {
    fn new(url: String) -> Self {
        Self { url }
    }
}

impl StreamBackend for HttpStreamBackend {
    fn label(&self) -> &'static str {
        "http"
    }

    fn origin(&self) -> &str {
        &self.url
    }

    fn handle<'a>(
        &'a self,
        app_handle: &'a AppHandle,
        stream: &'a mut TcpStream,
        method: &'a str,
        range: Option<&'a str>,
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            handle_http_stream_source(app_handle, stream, method, &self.url, range).await
        })
    }
}

struct SmbStreamBackend {
    url: String,
    open_url: String,
    file: Mutex<Option<Arc<Mutex<crate::network::protocols::smb::SmbPlaybackFile>>>>,
}

impl SmbStreamBackend {
    fn new(url: String, open_url: String) -> Self {
        Self {
            url,
            open_url,
            file: Mutex::new(None),
        }
    }

    async fn playback_file(
        &self,
    ) -> Result<Arc<Mutex<crate::network::protocols::smb::SmbPlaybackFile>>, String> {
        if let Some(file) = self.file.lock().map_err(|error| error.to_string())?.clone() {
            return Ok(file);
        }

        let opened = crate::network::protocols::smb::open_playback_url(self.open_url.clone()).await?;
        let opened = Arc::new(Mutex::new(opened));
        let mut slot = self.file.lock().map_err(|error| error.to_string())?;
        if let Some(file) = slot.as_ref() {
            return Ok(file.clone());
        }
        *slot = Some(opened.clone());
        Ok(opened)
    }

    fn clear_playback_file(&self) {
        if let Ok(mut slot) = self.file.lock() {
            *slot = None;
        }
    }

    async fn file_size(&self) -> Result<Option<u64>, String> {
        let file = self.playback_file().await?;
        let guard = file.lock().map_err(|error| error.to_string())?;
        Ok(guard.file_size())
    }

    fn smb_chunk_size(file: &Arc<Mutex<crate::network::protocols::smb::SmbPlaybackFile>>) -> u64 {
        let negotiated = file
            .lock()
            .ok()
            .and_then(|guard| guard.max_read_size())
            .map(u64::from)
            .unwrap_or(SMB_STREAM_CHUNK_BYTES);
        let chunk_size = negotiated.min(SMB_STREAM_CHUNK_BYTES).max(64 * 1024);
        debug!("stream proxy: SMB chunk size negotiated={negotiated} effective={chunk_size}");
        chunk_size
    }

    async fn read_range(
        &self,
        offset: u64,
        length: usize,
    ) -> Result<crate::network::protocols::smb::SmbReadRangeResult, String> {
        match self.read_range_once(offset, length).await {
            Ok(result) => Ok(result),
            Err(first_error) => {
                warn!(
                    "stream proxy: SMB persistent read failed, reconnecting url={} offset={} length={} error={first_error}",
                    redact_url(&self.url),
                    offset,
                    length
                );
                self.clear_playback_file();
                self.read_range_once(offset, length).await
            }
        }
    }

    async fn read_range_once(
        &self,
        offset: u64,
        length: usize,
    ) -> Result<crate::network::protocols::smb::SmbReadRangeResult, String> {
        let file = self.playback_file().await?;
        tauri::async_runtime::spawn_blocking(move || {
            let mut guard = file.lock().map_err(|error| error.to_string())?;
            guard.read_range(offset, length)
        })
        .await
        .map_err(|error| format!("SMB read task failed: {error}"))?
    }

    async fn handle_smb_stream_source(
        &self,
        stream: &mut TcpStream,
        method: &str,
        range: Option<&str>,
    ) -> Result<(), String> {
        handle_smb_stream_source(self, stream, method, &self.url, range).await
    }
}

impl StreamBackend for SmbStreamBackend {
    fn label(&self) -> &'static str {
        "smb"
    }

    fn origin(&self) -> &str {
        &self.url
    }

    fn handle<'a>(
        &'a self,
        _app_handle: &'a AppHandle,
        stream: &'a mut TcpStream,
        method: &'a str,
        range: Option<&'a str>,
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move { self.handle_smb_stream_source(stream, method, range).await })
    }
}

enum RequestHeaderRead {
    Empty,
    Complete(Vec<u8>),
    TooLarge,
    Incomplete,
}

pub(crate) fn set_parallel_range_enabled(enabled: bool) {
    STREAM_PROXY_PARALLEL_RANGE_ENABLED.store(enabled, Ordering::Release);
    info!(
        "stream proxy: parallel range download {}",
        if enabled { "enabled" } else { "disabled" }
    );
}

fn parallel_range_enabled() -> bool {
    STREAM_PROXY_PARALLEL_RANGE_ENABLED.load(Ordering::Acquire)
}

fn initialize_parallel_range_setting(app_handle: &AppHandle) {
    let enabled = crate::store::ui_state_store::load_setting_value(
        app_handle,
        PARALLEL_RANGE_SETTING_LABEL,
    )
    .ok()
    .flatten()
    .map(|value| !value.eq_ignore_ascii_case("off"))
    .unwrap_or(false);
    set_parallel_range_enabled(enabled);
}

pub(crate) fn register_basic_auth(playback_url: &str, username: &str, password: &str) {
    let username = username.trim();
    if username.is_empty() {
        return;
    }
    if let Ok(mut auth_map) = STREAM_PROXY_BASIC_AUTH
        .get_or_init(|| Mutex::new(std::collections::HashMap::new()))
        .lock()
    {
        auth_map.insert(
            playback_url.to_string(),
            (username.to_string(), password.to_string()),
        );
    }
}

pub(crate) fn rewrite_stream_url_with_headers(
    url: &str,
    headers: &[(String, String)],
) -> Option<String> {
    if !is_http_url(url) {
        return None;
    }
    register_headers(url, headers);
    let proxied = proxy_url_for(url)?;
    info!("stream proxy: rewrote yt-dlp stream url={}", redact_url(url));
    Some(proxied)
}

pub(crate) fn register_headers(playback_url: &str, headers: &[(String, String)]) {
    let normalized = normalize_headers(headers);
    if normalized.is_empty() {
        return;
    }
    if let Ok(mut headers_map) = STREAM_PROXY_HEADERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        headers_map.insert(playback_url.to_string(), normalized);
    }
}

pub(crate) fn start(app_handle: AppHandle) -> Result<(), String> {
    initialize_parallel_range_setting(&app_handle);

    if STREAM_PROXY_BASE_URL.get().is_some() {
        return Ok(());
    }

    let listener = StdTcpListener::bind(("127.0.0.1", 0)).map_err(|error| error.to_string())?;
    let addr = listener.local_addr().map_err(|error| error.to_string())?;
    listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;

    let base_url = format!("http://{addr}");
    let _ = STREAM_PROXY_BASE_URL.set(base_url);

    std::thread::Builder::new()
        .name("soia-stream-proxy".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_io()
                .enable_time()
                .thread_name("soia-stream-proxy-worker")
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    warn!("stream proxy: failed to create async runtime: {error}");
                    return;
                }
            };
            runtime.block_on(async move {
                match TcpListener::from_std(listener) {
                    Ok(listener) => serve(listener, app_handle).await,
                    Err(error) => warn!("stream proxy: failed to adopt listener: {error}"),
                }
            });
        })
        .map_err(|error| error.to_string())?;

    info!("stream proxy: listening on http://{addr}");
    Ok(())
}

pub(crate) fn rewrite_https_stream_url(url: &str) -> Option<String> {
    if !is_https_url(url) {
        return None;
    }
    let proxied = proxy_url_for(url)?;
    info!("stream proxy: rewrote HTTPS stream url={}", redact_url(url));
    Some(proxied)
}

pub(crate) fn rewrite_http_stream_url(url: &str) -> Option<String> {
    if !is_http_url(url) {
        return None;
    }
    let proxied = proxy_url_for(url)?;
    info!("stream proxy: rewrote HTTP stream url={}", redact_url(url));
    Some(proxied)
}

pub(crate) fn rewrite_smb_stream_url(url: &str) -> Option<String> {
    if !super::USE_SMB_STREAM_PROXY {
        return None;
    }
    if !is_smb_url(url) {
        return None;
    }
    let open_url = lookup_basic_auth(url)
        .and_then(|(username, password)| {
            crate::network::protocols::smb::playback_url_with_credentials(
                url,
                &username,
                &password,
            )
            .ok()
        })
        .unwrap_or_else(|| url.to_string());
    let proxied = proxy_url_for_backend(Arc::new(SmbStreamBackend::new(
        url.to_string(),
        open_url,
    )))?;
    info!("stream proxy: rewrote SMB stream url={}", redact_url(url));
    Some(proxied)
}

fn is_http_url(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
}

fn is_https_url(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    url.scheme() == "https"
}

fn is_smb_url(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    url.scheme().eq_ignore_ascii_case("smb")
}

fn stream_backends() -> &'static Mutex<StreamBackendRegistry> {
    STREAM_PROXY_BACKENDS.get_or_init(|| Mutex::new(StreamBackendRegistry::new()))
}

fn proxy_url_for(raw: &str) -> Option<String> {
    proxy_url_for_backend(Arc::new(HttpStreamBackend::new(raw.to_string())))
}

fn proxy_url_for_backend(backend: Arc<dyn StreamBackend>) -> Option<String> {
    let base = STREAM_PROXY_BASE_URL.get()?;
    let token = uuid::Uuid::now_v7().to_string();
    stream_backends().lock().ok()?.insert(token.clone(), backend);
    Some(format!("{base}/stream/{token}"))
}

fn lookup_stream_backend(target: &str) -> Option<Arc<dyn StreamBackend>> {
    if let Some(remote_url) = parse_remote_url(target) {
        return Some(Arc::new(HttpStreamBackend::new(remote_url)));
    }
    let path = target.split_once('?').map(|(path, _)| path).unwrap_or(target);
    let token = path.strip_prefix("/stream/")?.trim();
    if token.is_empty() {
        return None;
    }
    stream_backends().lock().ok()?.get(token)
}

fn redact_url(raw: &str) -> String {
    let Ok(mut url) = Url::parse(raw) else {
        return raw.to_string();
    };
    if !url.username().is_empty() {
        let _ = url.set_username("<user>");
        let _ = url.set_password(Some("<redacted>"));
    }
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

fn is_client_disconnect_error(error: &str) -> bool {
    error.contains("Broken pipe")
        || error.contains("Connection reset by peer")
        || error.contains("connection reset by peer")
}

async fn serve(listener: TcpListener, app_handle: AppHandle) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let app_handle = app_handle.clone();
                tokio::spawn(async move {
                    if let Err(error) = handle_connection(stream, &app_handle).await {
                        if is_client_disconnect_error(&error) {
                            debug!("stream proxy: client disconnected: {error}");
                        } else {
                            warn!("stream proxy: request failed: {error}");
                        }
                    }
                });
            }
            Err(error) => warn!("stream proxy: accept failed: {error}"),
        }
    }
}

async fn handle_connection(mut stream: TcpStream, app_handle: &AppHandle) -> Result<(), String> {
    let request_bytes = match read_request_header(&mut stream).await? {
        RequestHeaderRead::Empty => return Ok(()),
        RequestHeaderRead::Complete(bytes) => bytes,
        RequestHeaderRead::TooLarge => {
            write_status(
                &mut stream,
                431,
                "Request Header Fields Too Large",
                b"request header too large",
            )
            .await?;
            return Ok(());
        }
        RequestHeaderRead::Incomplete => {
            write_status(&mut stream, 400, "Bad Request", b"incomplete request header").await?;
            return Ok(());
        }
    };

    let request = String::from_utf8_lossy(&request_bytes);
    let (method, target, range) = parse_request(&request)?;
    if method != "GET" && method != "HEAD" {
        write_status(&mut stream, 405, "Method Not Allowed", b"method not allowed").await?;
        return Ok(());
    }

    let Some(backend) = lookup_stream_backend(&target) else {
        write_status(&mut stream, 400, "Bad Request", b"missing stream source").await?;
        return Ok(());
    };

    debug!(
        "stream proxy: dispatch backend={} origin={}",
        backend.label(),
        redact_url(backend.origin())
    );
    backend
        .handle(app_handle, &mut stream, &method, range.as_deref())
        .await
}

async fn handle_http_stream_source(
    app_handle: &AppHandle,
    stream: &mut TcpStream,
    method: &str,
    remote_url: &str,
    range: Option<&str>,
) -> Result<(), String> {
    debug!("stream proxy: fetch {}", redact_url(remote_url));

    let response = match fetch_remote(app_handle, remote_url, range).await {
        Ok(response) => response,
        Err(error) => {
            warn!(
                "stream proxy: upstream fetch failed url={} error={error}",
                redact_url(remote_url)
            );
            write_status(stream, 502, "Bad Gateway", error.as_bytes()).await?;
            return Ok(());
        }
    };
    let status = response.status();
    if !status.is_success() {
        let code = status.as_u16();
        let reason = status.canonical_reason().unwrap_or("Upstream Error").to_string();
        let body = response.bytes().await.map_err(|error| error.to_string())?;
        write_status(stream, code, &reason, &body).await?;
        return Ok(());
    }

    if should_rewrite_playlist(remote_url, &response) {
        let content_type = content_type(&response);
        let reason = status.canonical_reason().unwrap_or("OK").to_string();
        let bytes = response.bytes().await.map_err(|error| error.to_string())?;
        let text = String::from_utf8_lossy(&bytes);
        let inherited_headers = lookup_headers(remote_url);
        let body = rewrite_playlist(remote_url, &text, inherited_headers.as_deref()).into_bytes();
        write_response(
            stream,
            status.as_u16(),
            &reason,
            &content_type,
            Some(body.len() as u64),
            None,
            None,
        )
        .await?;
        if method != "HEAD" {
            stream
                .write_all(&body)
                .await
                .map_err(|error| error.to_string())?;
        }
        return Ok(());
    }

    stream_response(
        app_handle,
        stream,
        method,
        remote_url,
        range,
        response,
    )
    .await
}

async fn handle_smb_stream_source(
    backend: &SmbStreamBackend,
    stream: &mut TcpStream,
    method: &str,
    remote_url: &str,
    range: Option<&str>,
) -> Result<(), String> {
    debug!("stream proxy: fetch {}", redact_url(remote_url));
    let total_size = match backend.file_size().await {
        Ok(Some(size)) => size,
        Ok(None) => {
            write_status(stream, 502, "Bad Gateway", b"SMB file size unavailable").await?;
            return Ok(());
        }
        Err(error) => {
            warn!(
                "stream proxy: SMB metadata failed url={} error={error}",
                redact_url(remote_url)
            );
            return Err(error);
        }
    };

    let (status, response_start, response_end, content_range) = if let Some(range) = range {
        let parsed_range = parse_open_ended_range(Some(range))
            .and_then(|start| {
                (start < total_size).then(|| {
                    let end = total_size.saturating_sub(1);
                    (start, end)
                })
            })
            .or_else(|| parse_single_byte_range(range, total_size));
        let Some((start, end)) = parsed_range else {
            write_response(
                stream,
                StatusCode::RANGE_NOT_SATISFIABLE.as_u16(),
                StatusCode::RANGE_NOT_SATISFIABLE
                    .canonical_reason()
                    .unwrap_or("Range Not Satisfiable"),
                "text/plain; charset=utf-8",
                Some(0),
                Some(&format!("bytes */{total_size}")),
                Some("bytes"),
            )
            .await?;
            return Ok(());
        };
        (
            StatusCode::PARTIAL_CONTENT,
            start,
            end,
            Some(format!("bytes {start}-{end}/{total_size}")),
        )
    } else {
        let end = total_size.saturating_sub(1);
        (StatusCode::OK, 0, end, None)
    };

    let content_length = if total_size == 0 {
        0
    } else {
        response_end.saturating_sub(response_start).saturating_add(1)
    };
    write_response(
        stream,
        status.as_u16(),
        status.canonical_reason().unwrap_or("OK"),
        "application/octet-stream",
        Some(content_length),
        content_range.as_deref(),
        Some("bytes"),
    )
    .await?;

    if method == "HEAD" || content_length == 0 {
        return Ok(());
    }

    let chunk_size = {
        let file = backend.playback_file().await?;
        SmbStreamBackend::smb_chunk_size(&file)
    };
    let mut next = response_start;
    while next <= response_end {
        let length = response_end
            .saturating_sub(next)
            .saturating_add(1)
            .min(chunk_size) as usize;
        let chunk = match backend.read_range(next, length).await {
            Ok(chunk) => chunk,
            Err(error) => {
                warn!(
                    "stream proxy: SMB read failed url={} offset={} length={} error={error}",
                    redact_url(remote_url),
                    next,
                    length
                );
                return Err(error);
            }
        };
        if chunk.data.is_empty() {
            break;
        }
        stream
            .write_all(&chunk.data)
            .await
            .map_err(|error| error.to_string())?;
        next = next.saturating_add(chunk.data.len() as u64);
    }

    Ok(())
}

async fn read_request_header(stream: &mut TcpStream) -> Result<RequestHeaderRead, String> {
    let mut bytes = Vec::with_capacity(16 * 1024);
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            return if bytes.is_empty() {
                Ok(RequestHeaderRead::Empty)
            } else {
                Ok(RequestHeaderRead::Incomplete)
            };
        }
        bytes.extend_from_slice(&buffer[..read]);
        if request_header_end(&bytes).is_some() {
            return Ok(RequestHeaderRead::Complete(bytes));
        }
        if bytes.len() > MAX_REQUEST_HEADER_BYTES {
            return Ok(RequestHeaderRead::TooLarge);
        }
    }
}

fn request_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .or_else(|| {
            bytes.windows(2)
                .position(|window| window == b"\n\n")
                .map(|index| index + 2)
        })
}

fn parse_request(request: &str) -> Result<(String, String, Option<String>), String> {
    let mut lines = request.lines();
    let request_line = lines.next().ok_or_else(|| "missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default().to_string();
    let target = request_parts.next().unwrap_or_default().to_string();
    let range = lines.find_map(|line| {
        line.split_once(':').and_then(|(name, value)| {
            name.eq_ignore_ascii_case("range")
                .then(|| value.trim().to_string())
        })
    });
    Ok((method, target, range))
}

fn parse_remote_url(target: &str) -> Option<String> {
    let query = target.split_once('?')?.1;
    query.split('&').find_map(|part| {
        let (name, value) = part.split_once('=')?;
        if name == "url" {
            Some(percent_decode_str(value).decode_utf8_lossy().to_string())
        } else {
            None
        }
    })
}

async fn fetch_remote(
    app_handle: &AppHandle,
    remote_url: &str,
    range: Option<&str>,
) -> Result<Response, String> {
    let mut last_error = String::new();
    for attempt in 0..=FETCH_REMOTE_MAX_RETRIES {
        if attempt > 0 {
            debug!(
                "stream proxy: retrying fetch attempt={} url={}",
                attempt,
                redact_url(remote_url)
            );
            tokio::time::sleep(FETCH_REMOTE_RETRY_DELAY).await;
        }
        let client = build_client(app_handle)?;
        let mut request = client
            .get(remote_url)
            .header(ACCEPT_ENCODING, "identity")
            .header(USER_AGENT, HTTP_USER_AGENT);
        if let Some(range) = range {
            request = request.header(RANGE, range);
        }
        request = apply_basic_auth(request, remote_url);
        request = apply_headers(request, remote_url);
        match request.send().await {
            Ok(response) => return Ok(response),
            Err(error) => {
                last_error = error.to_string();
                // Only retry on connection-level errors, not on HTTP-level errors.
                if !error.is_connect() && !error.is_request() {
                    break;
                }
            }
        }
    }
    Err(last_error)
}

fn build_client(app_handle: &AppHandle) -> Result<Client, String> {
    let proxy_key = crate::network::proxy::current_proxy_key(app_handle)?;
    let client_cache = STREAM_PROXY_CLIENT.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = client_cache.lock() {
        if let Some(cached) = guard.as_ref() {
            if cached.proxy_key == proxy_key {
                return Ok(cached.client.clone());
            }
        }
    }

    let builder = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .pool_idle_timeout(Duration::from_secs(30))
        .no_gzip()
        .no_brotli()
        .no_zstd()
        .no_deflate();
    let client = configure_client_builder_with_proxy_key(builder, proxy_key.as_deref())?
        .build()
        .map_err(|error| error.to_string())?;

    if let Ok(mut guard) = client_cache.lock() {
        *guard = Some(CachedClient {
            proxy_key,
            client: client.clone(),
        });
    }
    Ok(client)
}

fn configure_client_builder_with_proxy_key(
    builder: reqwest::ClientBuilder,
    proxy_key: Option<&str>,
) -> Result<reqwest::ClientBuilder, String> {
    let Some(proxy_url) = proxy_key else {
        return Ok(builder);
    };
    let proxy = reqwest::Proxy::all(proxy_url).map_err(|error| error.to_string())?;
    Ok(builder.proxy(proxy))
}

fn apply_basic_auth(request: RequestBuilder, remote_url: &str) -> RequestBuilder {
    match lookup_basic_auth(remote_url) {
        Some((username, password)) => request.basic_auth(username, Some(password)),
        None => request,
    }
}

fn lookup_basic_auth(url: &str) -> Option<BasicAuth> {
    STREAM_PROXY_BASIC_AUTH
        .get()
        .and_then(|auth_map| auth_map.lock().ok())
        .and_then(|auth_map| auth_map.get(url).cloned())
}

fn normalize_headers(headers: &[(String, String)]) -> ProxyHeaders {
    headers
        .iter()
        .filter_map(|(name, value)| {
            let name = name.trim();
            let value = value.trim();
            if name.is_empty() || value.is_empty() || !should_forward_registered_header(name) {
                return None;
            }
            Some((name.to_string(), value.to_string()))
        })
        .collect()
}

fn should_forward_registered_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "user-agent" | "referer" | "cookie" | "origin" | "accept-language"
    )
}

fn apply_headers(mut request: RequestBuilder, remote_url: &str) -> RequestBuilder {
    let Some(headers) = lookup_headers(remote_url) else {
        return request;
    };
    for (name, value) in headers {
        let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        let Ok(header_value) = HeaderValue::from_str(&value) else {
            continue;
        };
        request = request.header(header_name, header_value);
    }
    request
}

fn lookup_headers(url: &str) -> Option<ProxyHeaders> {
    STREAM_PROXY_HEADERS
        .get()
        .and_then(|headers_map| headers_map.lock().ok())
        .and_then(|headers_map| headers_map.get(url).cloned())
}

fn should_rewrite_playlist(remote_url: &str, response: &Response) -> bool {
    remote_url.to_ascii_lowercase().contains(".m3u8")
        || content_type(response).to_ascii_lowercase().contains("mpegurl")
}

fn content_type(response: &Response) -> String {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string()
}

fn rewrite_playlist(
    base_url: &str,
    text: &str,
    inherited_headers: Option<&[(String, String)]>,
) -> String {
    let base = Url::parse(base_url).ok();
    text.lines()
        .map(|line| rewrite_playlist_line(base.as_ref(), line, inherited_headers))
        .collect::<Vec<_>>()
        .join("\n")
}

fn rewrite_playlist_line(
    base: Option<&Url>,
    line: &str,
    inherited_headers: Option<&[(String, String)]>,
) -> String {
    if line.trim().is_empty() {
        return line.to_string();
    }
    if line.starts_with('#') {
        return rewrite_uri_attributes(base, line, inherited_headers);
    }
    rewrite_playlist_url(base, line, inherited_headers).unwrap_or_else(|| line.to_string())
}

fn rewrite_uri_attributes(
    base: Option<&Url>,
    line: &str,
    inherited_headers: Option<&[(String, String)]>,
) -> String {
    let mut rewritten = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(index) = rest.find("URI=\"") {
        let (before, after_prefix) = rest.split_at(index);
        rewritten.push_str(before);
        rewritten.push_str("URI=\"");
        let uri_start = &after_prefix[5..];
        let Some(end) = uri_start.find('"') else {
            rewritten.push_str(uri_start);
            return rewritten;
        };
        let uri = &uri_start[..end];
        rewritten.push_str(
            &rewrite_playlist_url(base, uri, inherited_headers).unwrap_or_else(|| uri.to_string()),
        );
        rest = &uri_start[end..];
    }
    rewritten.push_str(rest);
    rewritten
}

fn rewrite_playlist_url(
    base: Option<&Url>,
    value: &str,
    inherited_headers: Option<&[(String, String)]>,
) -> Option<String> {
    let resolved = if let Ok(url) = Url::parse(value) {
        url
    } else {
        base?.join(value).ok()?
    };
    if let Some(headers) = inherited_headers {
        register_headers(resolved.as_str(), headers);
    }
    match resolved.scheme() {
        "http" | "https" => proxy_url_for(resolved.as_str()),
        _ => None,
    }
}

fn parse_open_ended_range(value: Option<&str>) -> Option<u64> {
    let value = value?;
    let range = value.trim();
    let bytes = range.strip_prefix("bytes=")?;
    let (start, end) = bytes.split_once('-')?;
    if !end.trim().is_empty() {
        return None;
    }
    start.trim().parse::<u64>().ok()
}

fn parse_single_byte_range(value: &str, total_size: u64) -> Option<(u64, u64)> {
    let range = value.trim();
    let bytes = range.strip_prefix("bytes=")?;
    if bytes.contains(',') {
        return None;
    }
    let (start, end) = bytes.split_once('-')?;
    let start = start.trim();
    let end = end.trim();
    if total_size == 0 {
        return None;
    }
    if start.is_empty() {
        let suffix_len = end.parse::<u64>().ok()?;
        if suffix_len == 0 {
            return None;
        }
        let range_start = total_size.saturating_sub(suffix_len);
        return Some((range_start, total_size - 1));
    }

    let range_start = start.parse::<u64>().ok()?;
    if range_start >= total_size {
        return None;
    }
    let range_end = if end.is_empty() {
        total_size - 1
    } else {
        end.parse::<u64>().ok()?.min(total_size - 1)
    };
    if range_end < range_start {
        return None;
    }
    Some((range_start, range_end))
}

fn parse_content_range(value: &str) -> Option<(u64, u64, u64)> {
    let value = value.trim();
    let range = value.strip_prefix("bytes ")?;
    let (range, total) = range.split_once('/')?;
    if total == "*" {
        return None;
    }
    let (start, end) = range.split_once('-')?;
    Some((
        start.trim().parse::<u64>().ok()?,
        end.trim().parse::<u64>().ok()?,
        total.trim().parse::<u64>().ok()?,
    ))
}

fn is_parallel_range_excluded_url(remote_url: &str) -> bool {
    let Ok(url) = Url::parse(remote_url) else {
        return true;
    };
    let path = url.path().to_ascii_lowercase();
    path.ends_with(".m3u8")
}

fn parallel_range_plan(
    remote_url: &str,
    request_range: Option<&str>,
    status: StatusCode,
    content_length: Option<u64>,
    content_range: Option<&str>,
    accept_ranges: &str,
) -> Option<ParallelRangePlan> {
    if !accept_ranges
        .split(',')
        .any(|value| value.trim().eq_ignore_ascii_case("bytes"))
    {
        return None;
    }
    if is_parallel_range_excluded_url(remote_url) {
        return None;
    }

    let plan = if status == StatusCode::OK && request_range.is_none() {
        let content_length = content_length?;
        ParallelRangePlan {
            response_start: 0,
            response_end: content_length.checked_sub(1)?,
            total_size: content_length,
            content_length,
        }
    } else if status == StatusCode::PARTIAL_CONTENT {
        let requested_start = parse_open_ended_range(request_range)?;
        let (response_start, response_end, total_size) = parse_content_range(content_range?)?;
        if response_start != requested_start {
            return None;
        }
        ParallelRangePlan {
            response_start,
            response_end,
            total_size,
            content_length: response_end.checked_sub(response_start)?.saturating_add(1),
        }
    } else {
        return None;
    };

    if plan.content_length < PARALLEL_RANGE_MIN_BYTES {
        return None;
    }
    Some(plan)
}

fn split_byte_ranges(start: u64, end: u64) -> Vec<ByteRange> {
    let mut ranges = Vec::new();
    let mut next = start;
    while next <= end {
        let chunk_end = next.saturating_add(PARALLEL_RANGE_CHUNK_BYTES - 1).min(end);
        ranges.push(ByteRange {
            start: next,
            end: chunk_end,
        });
        if chunk_end == u64::MAX {
            break;
        }
        next = chunk_end + 1;
    }
    ranges
}

async fn fetch_range_bytes(
    app_handle: &AppHandle,
    remote_url: &str,
    range: ByteRange,
) -> Result<(u64, Vec<u8>), String> {
    let client = build_client(app_handle)?;
    let mut request = client
        .get(remote_url)
        .header(ACCEPT_ENCODING, "identity")
        .header(USER_AGENT, HTTP_USER_AGENT)
        .header(RANGE, format!("bytes={}-{}", range.start, range.end));
    request = apply_basic_auth(request, remote_url);
    request = apply_headers(request, remote_url);
    let response = request.send().await.map_err(|error| error.to_string())?;
    if response.status() != StatusCode::PARTIAL_CONTENT {
        return Err(format!(
            "parallel range request failed: status={} range={}-{}",
            response.status(),
            range.start,
            range.end
        ));
    }
    let expected_len = range.end.saturating_sub(range.start).saturating_add(1) as usize;
    let bytes = response.bytes().await.map_err(|error| error.to_string())?;
    if bytes.len() != expected_len {
        return Err(format!(
            "parallel range length mismatch: expected={} actual={} range={}-{}",
            expected_len,
            bytes.len(),
            range.start,
            range.end
        ));
    }
    Ok((range.start, bytes.to_vec()))
}

async fn stream_parallel_range_response(
    app_handle: &AppHandle,
    stream: &mut TcpStream,
    remote_url: &str,
    plan: ParallelRangePlan,
    first_chunk: Vec<u8>,
) -> Result<(), String> {
    info!(
        "stream proxy: parallel range enabled url={} start={} end={} total={} chunk={} connections={}",
        redact_url(remote_url),
        plan.response_start,
        plan.response_end,
        plan.total_size,
        PARALLEL_RANGE_CHUNK_BYTES,
        PARALLEL_RANGE_CONNECTIONS
    );

    let ranges = split_byte_ranges(plan.response_start, plan.response_end);
    let mut next_range_index = 1;
    let mut next_write_start = plan.response_start;
    let mut pending = FuturesUnordered::new();
    let mut completed: HashMap<u64, Vec<u8>> = HashMap::new();
    completed.insert(plan.response_start, first_chunk);

    loop {
        while pending.len() < PARALLEL_RANGE_CONNECTIONS && next_range_index < ranges.len() {
            let range = ranges[next_range_index].clone();
            next_range_index += 1;
            pending.push(fetch_range_bytes(app_handle, remote_url, range));
        }

        if let Some(bytes) = completed.remove(&next_write_start) {
            stream
                .write_all(&bytes)
                .await
                .map_err(|error| error.to_string())?;
            next_write_start = next_write_start.saturating_add(bytes.len() as u64);
            if next_write_start > plan.response_end {
                return Ok(());
            }
            continue;
        }

        let Some(result) = pending.next().await else {
            return Ok(());
        };
        let (start, bytes) = result?;
        if start == next_write_start {
            stream
                .write_all(&bytes)
                .await
                .map_err(|error| error.to_string())?;
            next_write_start = next_write_start.saturating_add(bytes.len() as u64);
            if next_write_start > plan.response_end {
                return Ok(());
            }
        } else {
            completed.insert(start, bytes);
        }
    }
}

async fn stream_response(
    app_handle: &AppHandle,
    stream: &mut TcpStream,
    method: &str,
    remote_url: &str,
    request_range: Option<&str>,
    mut response: Response,
) -> Result<(), String> {
    let status = response.status();
    let content_type = content_type(&response);
    let content_length = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let content_range = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let accept_ranges = response
        .headers()
        .get(ACCEPT_RANGES)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "bytes".to_string());
    let parallel_plan = if method == "GET" && parallel_range_enabled() {
        parallel_range_plan(
            remote_url,
            request_range,
            status,
            content_length,
            content_range.as_deref(),
            &accept_ranges,
        )
    } else {
        None
    };
    let parallel_first_chunk = if let Some(plan) = parallel_plan.as_ref() {
        let first_range = ByteRange {
            start: plan.response_start,
            end: plan
                .response_start
                .saturating_add(PARALLEL_RANGE_CHUNK_BYTES - 1)
                .min(plan.response_end),
        };
        match fetch_range_bytes(app_handle, remote_url, first_range).await {
            Ok((start, bytes)) if start == plan.response_start => Some(bytes),
            Ok((start, _)) => {
                warn!(
                    "stream proxy: parallel range preflight returned unexpected start={} expected={} url={}",
                    start,
                    plan.response_start,
                    redact_url(remote_url)
                );
                None
            }
            Err(error) => {
                debug!(
                    "stream proxy: parallel range disabled after preflight url={} error={}",
                    redact_url(remote_url),
                    error
                );
                None
            }
        }
    } else {
        None
    };

    write_response(
        stream,
        status.as_u16(),
        status.canonical_reason().unwrap_or("OK"),
        &content_type,
        content_length,
        content_range.as_deref(),
        Some(&accept_ranges),
    )
    .await?;

    if method == "HEAD" {
        return Ok(());
    }

    if let (Some(plan), Some(first_chunk)) = (parallel_plan, parallel_first_chunk) {
        drop(response);
        return stream_parallel_range_response(app_handle, stream, remote_url, plan, first_chunk)
            .await;
    }

    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        stream
            .write_all(&chunk)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

async fn write_status(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    body: &[u8],
) -> Result<(), String> {
    write_response(
        stream,
        code,
        reason,
        "text/plain; charset=utf-8",
        Some(body.len() as u64),
        None,
        None,
    )
    .await?;
    stream
        .write_all(body)
        .await
        .map_err(|error| error.to_string())
}

async fn write_response(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    content_type: &str,
    content_length: Option<u64>,
    content_range: Option<&str>,
    accept_ranges: Option<&str>,
) -> Result<(), String> {
    let mut header = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {content_type}\r\nConnection: close\r\n"
    );
    if let Some(length) = content_length {
        header.push_str(&format!("Content-Length: {length}\r\n"));
    }
    if let Some(range) = content_range {
        header.push_str(&format!("Content-Range: {range}\r\n"));
    }
    if let Some(accept_ranges) = accept_ranges {
        header.push_str(&format!("Accept-Ranges: {accept_ranges}\r\n"));
    }
    header.push_str("\r\n");
    stream
        .write_all(header.as_bytes())
        .await
        .map_err(|error| error.to_string())
}
