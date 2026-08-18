use log::{debug, info, warn};
use percent_encoding::percent_decode_str;
use reqwest::header::{
    HeaderName, HeaderValue, ACCEPT_ENCODING, ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE,
    CONTENT_TYPE, RANGE, USER_AGENT,
};
use futures_util::future::BoxFuture;
use futures_util::stream::{FuturesUnordered, StreamExt};
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use std::collections::{HashMap, HashSet};
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
const SMB_PIPELINE_DEPTH: usize = 4;
const STREAM_BACKEND_IDLE_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const STREAM_BACKEND_CLEANUP_INTERVAL: Duration = Duration::from_secs(60);
// A long HLS video is thousands of segments, and each one holds a token
// for as long as it might still be played. The old 4096 cap was under
// that for a feature-length film, so eviction reached segments that had
// not been reached yet.
const STREAM_BACKEND_MAX_ENTRIES: usize = 24_576;
const STREAM_BACKEND_TARGET_ENTRIES: usize = 18_432;

type BasicAuth = (String, String);
pub(crate) type ProxyHeaders = Vec<(String, String)>;

static STREAM_PROXY_BASE_URL: OnceLock<String> = OnceLock::new();
static STREAM_PROXY_BASIC_AUTH: OnceLock<Mutex<HashMap<String, BasicAuth>>> = OnceLock::new();
static STREAM_PROXY_HEADERS: OnceLock<Mutex<HashMap<String, ProxyHeaders>>> = OnceLock::new();
/// Headers keyed by host, so a playlist's segments inherit them without an
/// entry per segment URL — those URLs are regenerated on every playlist
/// fetch, so keying by URL grows without bound and never matches twice.
static STREAM_PROXY_HOST_HEADERS: OnceLock<Mutex<HashMap<String, ProxyHeaders>>> = OnceLock::new();
static STREAM_PROXY_CLIENT: OnceLock<Mutex<Option<CachedClient>>> = OnceLock::new();
static STREAM_PROXY_PARALLEL_RANGE_ENABLED: AtomicBool = AtomicBool::new(false);
static STREAM_PROXY_BACKENDS: OnceLock<Mutex<StreamBackendRegistry>> =
    OnceLock::new();
/// Segment URLs per rewritten playlist, addressed by index.
static STREAM_PROXY_SEGMENT_SETS: OnceLock<Mutex<HashMap<String, SegmentEntry>>> = OnceLock::new();
/// How many playlists' segment lists to keep at once. One YouTube video
/// costs eight — two master playlists and the six renditions ffmpeg probes
/// before it settles — so anything near that has no room for the next
/// video's masters to arrive before the last one's are done with.
const SEGMENT_SET_MAX: usize = 16;

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
    /// Reverse index so re-serving a URL reuses its token instead of
    /// minting another. An HLS playlist is rewritten segment by segment
    /// and refreshed as it plays, so without this the registry fills with
    /// duplicates and evicts the segments about to be requested.
    tokens_by_origin: HashMap<String, String>,
    last_cleanup: Instant,
}

impl StreamBackendRegistry {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            entries: HashMap::new(),
            tokens_by_origin: HashMap::new(),
            last_cleanup: now,
        }
    }

    fn insert(&mut self, token: String, backend: Arc<dyn StreamBackend>) {
        let now = Instant::now();
        self.cleanup_if_due(now);
        self.tokens_by_origin
            .insert(backend.origin().to_string(), token.clone());
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

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn find_token_by_origin(&mut self, origin: &str) -> Option<String> {
        let now = Instant::now();
        self.cleanup_if_due(now);
        let token = self.tokens_by_origin.get(origin)?.clone();
        // Touching it keeps a segment that is still being served away
        // from the eviction end of the list.
        let entry = self.entries.get_mut(&token)?;
        entry.last_access = now;
        Some(token)
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
        let mut dropped = Vec::new();
        self.entries.retain(|token, entry| {
            let keep = now.duration_since(entry.last_access) <= STREAM_BACKEND_IDLE_TIMEOUT;
            if !keep {
                dropped.push((token.clone(), entry.backend.origin().to_string()));
            }
            keep
        });
        self.forget_origins(dropped);
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
        let mut dropped = Vec::new();
        for (token, _) in oldest.into_iter().take(remove_count) {
            if let Some(entry) = self.entries.remove(&token) {
                dropped.push((token, entry.backend.origin().to_string()));
            }
        }
        self.forget_origins(dropped);
        self.last_cleanup = now;
        debug!("stream proxy: evicted {remove_count} backend token(s) to enforce registry limit");
    }

    /// Drops reverse-index entries for removed tokens, leaving alone any
    /// origin that has since been re-registered under a newer token.
    fn forget_origins(&mut self, dropped: Vec<(String, String)>) {
        for (token, origin) in dropped {
            if self.tokens_by_origin.get(&origin) == Some(&token) {
                self.tokens_by_origin.remove(&origin);
            }
        }
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
    file: Arc<Mutex<Option<crate::network::protocols::smb::SmbPlaybackFile>>>,
}

impl SmbStreamBackend {
    fn new(url: String, open_url: String) -> Self {
        Self {
            url,
            open_url,
            file: Arc::new(Mutex::new(None)),
        }
    }

    async fn ensure_open(&self) -> Result<(), String> {
        {
            let guard = self.file.lock().map_err(|error| error.to_string())?;
            if guard.is_some() {
                return Ok(());
            }
        }
        let opened =
            crate::network::protocols::smb::open_playback_url(self.open_url.clone()).await?;
        let mut guard = self.file.lock().map_err(|error| error.to_string())?;
        if guard.is_none() {
            *guard = Some(opened);
        }
        Ok(())
    }

    fn clear_playback_file(&self) {
        if let Ok(mut guard) = self.file.lock() {
            *guard = None;
        }
    }

    async fn file_size(&self) -> Result<Option<u64>, String> {
        self.ensure_open().await?;
        let guard = self.file.lock().map_err(|error| error.to_string())?;
        Ok(guard.as_ref().and_then(|f| f.file_size()))
    }

    fn smb_chunk_size(
        file: &Arc<Mutex<Option<crate::network::protocols::smb::SmbPlaybackFile>>>,
    ) -> u64 {
        let negotiated = file
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref()?.max_read_size())
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
        self.ensure_open().await?;
        let file = self.file.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let mut guard = file.lock().map_err(|error| error.to_string())?;
            let f = guard
                .as_mut()
                .ok_or_else(|| "SMB file is not open".to_string())?;
            f.read_range(offset, length)
        })
        .await
        .map_err(|error| format!("SMB read task failed: {error}"))?
    }

    async fn read_pipeline(
        &self,
        requests: &[(u64, u32)],
    ) -> Result<crate::network::protocols::smb::SmbReadRangeResult, String> {
        match self.read_pipeline_once(requests).await {
            Ok(result) => Ok(result),
            Err(first_error) => {
                warn!(
                    "stream proxy: SMB pipeline read failed, reconnecting url={} error={first_error}",
                    redact_url(&self.url),
                );
                self.clear_playback_file();
                self.read_pipeline_once(requests).await
            }
        }
    }

    async fn read_pipeline_once(
        &self,
        requests: &[(u64, u32)],
    ) -> Result<crate::network::protocols::smb::SmbReadRangeResult, String> {
        self.ensure_open().await?;
        let file = self.file.clone();
        let requests = requests.to_vec();
        tauri::async_runtime::spawn_blocking(move || {
            let mut guard = file.lock().map_err(|error| error.to_string())?;
            let f = guard
                .as_mut()
                .ok_or_else(|| "SMB file is not open".to_string())?;
            f.read_pipeline(&requests)
        })
        .await
        .map_err(|error| format!("SMB pipeline task failed: {error}"))?
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
    if !is_http_url(url) || is_own_proxy_url(url) {
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
    if !is_https_url(url) || is_own_proxy_url(url) {
        return None;
    }
    let proxied = proxy_url_for(url)?;
    info!("stream proxy: rewrote HTTPS stream url={}", redact_url(url));
    Some(proxied)
}

pub(crate) fn rewrite_http_stream_url(url: &str) -> Option<String> {
    if !is_http_url(url) || is_own_proxy_url(url) {
        return None;
    }
    let proxied = proxy_url_for(url)?;
    info!("stream proxy: rewrote HTTP stream url={}", redact_url(url));
    Some(proxied)
}

/// Whether this URL already points back at us. Proxying our own proxy
/// URL puts a second hop in front of every request, and because each hop
/// rewrites the playlist it serves, the segment URLs come back wrapped
/// twice — which is how playback ended up opening
/// `/stream?url=http://127.0.0.1:PORT/stream%3Furl%3Dhttps://…`.
fn is_own_proxy_url(url: &str) -> bool {
    STREAM_PROXY_BASE_URL
        .get()
        .is_some_and(|base| url.starts_with(base.as_str()))
}

pub(crate) fn rewrite_smb_stream_url(url: &str) -> Option<String> {
    if !super::USE_SMB_STREAM_PROXY {
        return None;
    }
    if !is_smb_url(url) {
        return None;
    }
    // Reuse an existing backend for the same origin URL if available
    if let Some(proxied) = proxy_url_for_existing_origin(url) {
        info!("stream proxy: reused SMB backend url={}", redact_url(url));
        return Some(proxied);
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

/// Locks the registry, taking it back if a panicking request poisoned it.
/// The entries are plain data and stay valid, so refusing to touch them
/// after an unrelated panic would strand every stream behind HTTP 400
/// until the app restarted.
fn lock_stream_backends() -> std::sync::MutexGuard<'static, StreamBackendRegistry> {
    match stream_backends().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!("stream proxy: backend registry lock was poisoned; recovering");
            poisoned.into_inner()
        }
    }
}

fn proxy_url_for(raw: &str) -> Option<String> {
    proxy_url_for_backend(Arc::new(HttpStreamBackend::new(raw.to_string())))
}

fn proxy_url_for_backend(backend: Arc<dyn StreamBackend>) -> Option<String> {
    let base = STREAM_PROXY_BASE_URL.get()?;
    let token = uuid::Uuid::now_v7().to_string();
    let registered = {
        let mut registry = lock_stream_backends();
        registry.insert(token.clone(), backend);
        registry.len()
    };
    debug!("stream proxy: registered token {token} ({registered} live)");
    Some(format!("{base}/stream/{token}"))
}

fn proxy_url_for_existing_origin(origin: &str) -> Option<String> {
    let base = STREAM_PROXY_BASE_URL.get()?;
    let token = lock_stream_backends().find_token_by_origin(origin)?;
    Some(format!("{base}/stream/{token}"))
}

fn lookup_stream_backend(target: &str) -> Option<Arc<dyn StreamBackend>> {
    if let Some(remote_url) = parse_remote_url(target) {
        return Some(Arc::new(HttpStreamBackend::new(remote_url)));
    }
    // A request line may carry the whole URL rather than just the path
    // (absolute-form, RFC 9112 §3.2.2 — clients send it when they think
    // they are talking to a proxy, which is exactly what we look like).
    let target = strip_request_authority(target);
    let path = target.split_once('?').map(|(path, _)| path).unwrap_or(target);
    if let Some(url) = segment_set_url(path) {
        return Some(Arc::new(HttpStreamBackend::new(url)));
    }
    if path.starts_with("/seg/") {
        warn_segment_miss_once(path);
        return None;
    }
    let token = path.strip_prefix("/stream/")?.trim();
    if token.is_empty() {
        return None;
    }
    let mut registry = lock_stream_backends();
    let backend = registry.get(token);
    if backend.is_none() {
        // Every playback failure funnels through here, so say enough to
        // tell an unknown token from an emptied registry.
        warn!(
            "stream proxy: no backend for token {token} ({} live)",
            registry.len()
        );
    }
    backend
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

/// The URL first, then the same URL on each mirror edge it names.
///
/// A googlevideo host reads `rr3---sn-h557sn6l.googlevideo.com`, and the
/// URL carries `mn=sn-h5576nsr,sn-h557sn6l` — the edges serving this
/// content. The signature stays valid across them (verified live: one
/// answered 206 at the moment the other said 403), so a refusal on the
/// assigned edge does not have to be the last word. Mirrors inherit the
/// original host's registered headers, since the header registry is
/// keyed by host.
fn with_mirror_hosts(remote_url: &str) -> Vec<String> {
    let mut candidates = vec![remote_url.to_string()];
    let Ok(url) = Url::parse(remote_url) else {
        return candidates;
    };
    let Some(host) = url.host_str() else {
        return candidates;
    };
    if !host.ends_with(".googlevideo.com") {
        return candidates;
    }
    let Some((prefix, rest)) = host.split_once("---") else {
        return candidates;
    };
    let Some((current_sn, domain)) = rest.split_once('.') else {
        return candidates;
    };

    // `mn` is a query parameter on DASH URLs and a path segment on the
    // HLS-style ones (`…/mn/sn-a,sn-b/…`).
    let mirrors = url
        .query_pairs()
        .find(|(key, _)| key == "mn")
        .map(|(_, value)| value.into_owned())
        .or_else(|| {
            let path = url.path();
            let start = path.find("/mn/")? + "/mn/".len();
            Some(path[start..].split('/').next()?.to_string())
        })
        .unwrap_or_default();

    let inherited = lookup_headers(remote_url);
    for mirror in mirrors.split(',').map(str::trim) {
        if mirror.is_empty() || mirror == current_sn {
            continue;
        }
        let mirror_host = format!("{prefix}---{mirror}.{domain}");
        let swapped = remote_url.replacen(host, &mirror_host, 1);
        if let Some(headers) = inherited.as_deref() {
            register_host_headers(&swapped, headers);
        }
        candidates.push(swapped);
    }
    candidates
}

async fn handle_http_stream_source(
    app_handle: &AppHandle,
    stream: &mut TcpStream,
    method: &str,
    remote_url: &str,
    range: Option<&str>,
) -> Result<(), String> {
    debug!("stream proxy: fetch {}", redact_url(remote_url));

    // YouTube withholds a freshly issued stream URL for a while — 403 until
    // the CDN edge is ready. Its `available_at` field is supposed to say how
    // long, but it has claimed zero while an edge went on refusing for
    // minutes, and a re-resolve only mints another URL on the same edge.
    // The URL itself names the way out: `mn=` lists mirror hosts carrying
    // the same content, the signature is honoured on any of them, and one
    // mirror has answered 206 at the very moment the other refused. So a
    // 403 first rotates through the mirrors, and only then waits — mpv sits
    // in its loading state through the waits, which turns this failure
    // class into a slower start instead of a dead video.
    const NOT_READY_BACKOFF_SECS: [u64; 3] = [2, 4, 8];

    let candidates = with_mirror_hosts(remote_url);
    let mut cycle = 0usize;
    let (active_url, response) = 'found: loop {
        for candidate in &candidates {
            let response = match fetch_remote(app_handle, candidate, range).await {
                Ok(response) => response,
                Err(error) => {
                    warn!(
                        "stream proxy: upstream fetch failed url={} error={error}",
                        redact_url(candidate)
                    );
                    write_status(stream, 502, "Bad Gateway", error.as_bytes()).await?;
                    return Ok(());
                }
            };
            if response.status().as_u16() != 403 {
                if candidate != remote_url {
                    info!(
                        "stream proxy: mirror host answered where {} refused",
                        redact_url(remote_url)
                    );
                }
                break 'found (candidate.clone(), response);
            }
            if cycle == NOT_READY_BACKOFF_SECS.len() {
                // Out of patience: relay the last refusal as-is.
                break 'found (candidate.clone(), response);
            }
        }
        let wait = NOT_READY_BACKOFF_SECS[cycle];
        cycle += 1;
        info!(
            "stream proxy: all {} host(s) answered 403 — retrying in {wait}s ({cycle}/{}) url={}",
            candidates.len(),
            NOT_READY_BACKOFF_SECS.len(),
            redact_url(remote_url)
        );
        tokio::time::sleep(Duration::from_secs(wait)).await;
    };
    let remote_url = active_url.as_str();
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
        let bytes = response.bytes().await.map_err(|error| error.to_string())?;
        if !body_is_playlist(&bytes) {
            // Guessed wrong. Pass it through untouched rather than mangle
            // media into text — the failure that costs is the silent one.
            debug!(
                "stream proxy: not a playlist after all, passing {} bytes through from {}",
                bytes.len(),
                redact_url(remote_url)
            );
            write_response(
                stream,
                200,
                "OK",
                &content_type,
                Some(bytes.len() as u64),
                None,
                None,
            )
            .await?;
            if method != "HEAD" {
                stream
                    .write_all(&bytes)
                    .await
                    .map_err(|error| error.to_string())?;
            }
            return Ok(());
        }
        let text = String::from_utf8_lossy(&bytes);
        let inherited_headers = lookup_headers(remote_url);
        let body = rewrite_playlist(remote_url, &text, inherited_headers.as_deref()).into_bytes();
        // Always 200. The body is rewritten, so its length no longer
        // matches the range that was asked for — answering 206 with a
        // mismatched length and no Content-Range is malformed, and a
        // player is entitled to make nothing of it.
        write_response(
            stream,
            200,
            "OK",
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
        backend.ensure_open().await?;
        SmbStreamBackend::smb_chunk_size(&backend.file)
    };
    let mut next = response_start;
    while next <= response_end {
        // Build a batch of pipeline requests (up to SMB_PIPELINE_DEPTH chunks)
        let mut requests: Vec<(u64, u32)> = Vec::with_capacity(SMB_PIPELINE_DEPTH);
        let mut batch_next = next;
        for _ in 0..SMB_PIPELINE_DEPTH {
            if batch_next > response_end {
                break;
            }
            let length = response_end
                .saturating_sub(batch_next)
                .saturating_add(1)
                .min(chunk_size) as u32;
            requests.push((batch_next, length));
            batch_next = batch_next.saturating_add(length as u64);
        }

        if requests.len() <= 1 {
            // Single chunk: use the simpler read_range path
            let length = requests.first().map(|r| r.1 as usize).unwrap_or(0);
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
        } else {
            // Multiple chunks: use pipeline read for better throughput
            let batch = match backend.read_pipeline(&requests).await {
                Ok(batch) => batch,
                Err(error) => {
                    warn!(
                        "stream proxy: SMB pipeline read failed url={} offset={} count={} error={error}",
                        redact_url(remote_url),
                        next,
                        requests.len()
                    );
                    return Err(error);
                }
            };
            if batch.data.is_empty() {
                break;
            }
            stream
                .write_all(&batch.data)
                .await
                .map_err(|error| error.to_string())?;
            next = next.saturating_add(batch.data.len() as u64);
        }
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

/// Reduces `http://host:port/path` to `/path`, leaving origin-form alone.
fn strip_request_authority(target: &str) -> &str {
    let rest = target
        .strip_prefix("http://")
        .or_else(|| target.strip_prefix("https://"));
    match rest {
        // Everything from the first slash after the authority.
        Some(rest) => rest.find('/').map(|index| &rest[index..]).unwrap_or("/"),
        None => target,
    }
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
            .header(ACCEPT_ENCODING, "identity");
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
        "user-agent"
            | "referer"
            | "cookie"
            | "origin"
            | "accept"
            | "accept-language"
            | "sec-fetch-mode"
            | "sec-fetch-site"
            | "sec-fetch-dest"
    )
}

fn apply_headers(mut request: RequestBuilder, remote_url: &str) -> RequestBuilder {
    let headers = lookup_headers(remote_url);
    let has_registered_ua = headers
        .as_ref()
        .map(|h| h.iter().any(|(n, _)| n.eq_ignore_ascii_case("user-agent")))
        .unwrap_or(false);
    if !has_registered_ua {
        request = request.header(USER_AGENT, HTTP_USER_AGENT);
    }
    let Some(headers) = headers else {
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
    let exact = STREAM_PROXY_HEADERS
        .get()
        .and_then(|headers_map| headers_map.lock().ok())
        .and_then(|headers_map| headers_map.get(url).cloned());
    if exact.is_some() {
        return exact;
    }
    let host = Url::parse(url).ok()?.host_str()?.to_string();
    STREAM_PROXY_HOST_HEADERS
        .get()
        .and_then(|headers_map| headers_map.lock().ok())
        .and_then(|headers_map| headers_map.get(&host).cloned())
}

/// Remembers a stream's headers for every later request to the same host.
fn register_host_headers(url: &str, headers: &[(String, String)]) {
    let normalized = normalize_headers(headers);
    if normalized.is_empty() {
        return;
    }
    let Some(host) = Url::parse(url).ok().and_then(|url| url.host_str().map(str::to_string))
    else {
        return;
    };
    if let Ok(mut headers_map) = STREAM_PROXY_HOST_HEADERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        headers_map.insert(host, normalized);
    }
}

/// Does the URL name a playlist?
///
/// Only the last path component may answer. YouTube's HLS *segment* URLs
/// carry the playlist's name as an interior component and end in the real
/// one — `…/playlist/index.m3u8/govp/…/gosq/0/file/seg.ts` is a quarter
/// megabyte of MPEG-TS, not a playlist. A substring test called every one
/// of them a playlist and rewrote the video as if it were text.
fn looks_like_playlist_url(remote_url: &str) -> bool {
    let path = remote_url
        .split(['?', '#'])
        .next()
        .unwrap_or(remote_url)
        .trim_end_matches('/');
    path.rsplit('/')
        .next()
        .is_some_and(|last| last.to_ascii_lowercase().ends_with(".m3u8"))
}

fn should_rewrite_playlist(remote_url: &str, response: &Response) -> bool {
    content_type(response).to_ascii_lowercase().contains("mpegurl")
        || looks_like_playlist_url(remote_url)
}

/// Confirms a body really is a playlist before rewriting it as text. The
/// URL and the content type are both guesses; `#EXTM3U` is the format's own
/// answer, and mistaking media for text corrupts it beyond recognition.
fn body_is_playlist(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(64)];
    let text = String::from_utf8_lossy(head);
    text.trim_start_matches('\u{feff}').trim_start().starts_with("#EXTM3U")
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
    // One set per rewrite: segments become short references into it
    // rather than carrying their own URL. A googlevideo segment URL is
    // ~1.4 KB, so inlining them turned a 90 KB playlist into 2.3 MB and
    // made mpv re-read megabytes of text on every refresh.
    let set = SegmentSet::new();
    // Which playlist produced which set: a master and its audio rendition
    // are rewritten separately, and a miss needs to name the one it lost.
    info!(
        "stream proxy: rewriting playlist into set {} from {}",
        set.token,
        redact_url(base_url)
    );
    let body = text
        .lines()
        .map(|line| rewrite_playlist_line(base.as_ref(), line, inherited_headers, &set))
        .collect::<Vec<_>>()
        .join("\n");
    set.publish();
    body
}

/// Collects the segment URLs of one playlist so they can be addressed by
/// index instead of by value.
struct SegmentSet {
    token: String,
    urls: Mutex<Vec<String>>,
}

impl SegmentSet {
    fn new() -> Self {
        Self {
            token: uuid::Uuid::now_v7().to_string(),
            urls: Mutex::new(Vec::new()),
        }
    }

    /// Adds a URL and returns the proxy reference that stands for it.
    fn reference(&self, url: &str) -> Option<String> {
        let base = STREAM_PROXY_BASE_URL.get()?;
        let mut urls = self.urls.lock().ok()?;
        urls.push(url.to_string());
        Some(format!("{base}/seg/{}/{}", self.token, urls.len() - 1))
    }

    fn publish(self) {
        let Ok(urls) = self.urls.into_inner() else {
            return;
        };
        if urls.is_empty() {
            return;
        }
        let count = urls.len();
        let mut evicted: Vec<String> = Vec::new();
        let mut live = 0usize;
        if let Ok(mut sets) = segment_sets().lock() {
            sets.insert(self.token.clone(), SegmentEntry::new(urls));
            // Drop the least recently *read*, not the oldest created. One
            // YouTube video rewrites the master plus every rendition ffmpeg
            // probes — eight sets for one film — and the set being played is
            // always the oldest of them, so evicting by age threw away the
            // one in use and 400'd every segment after it. A live stream's
            // superseded passes stop being read, so they still go first.
            while sets.len() > SEGMENT_SET_MAX {
                let Some(token) = sets
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_used)
                    .map(|(token, _)| token.clone())
                else {
                    break;
                };
                sets.remove(&token);
                evicted.push(token);
            }
            live = sets.len();
        } else {
            warn!(
                "stream proxy: segment set {} lost — the registry lock is poisoned",
                self.token
            );
        }
        info!(
            "stream proxy: playlist set {} holds {count} segments ({live} live)",
            self.token
        );
        if !evicted.is_empty() {
            info!("stream proxy: evicted segment sets {}", evicted.join(", "));
        }
    }
}

/// A rewritten playlist's segment URLs, with the last time one was served.
/// The timestamp is what keeps the playing set alive: it is read on every
/// segment, while a rendition ffmpeg only probed is never read again.
struct SegmentEntry {
    urls: Vec<String>,
    last_used: Instant,
}

impl SegmentEntry {
    fn new(urls: Vec<String>) -> Self {
        Self {
            urls,
            last_used: Instant::now(),
        }
    }
}

fn segment_sets() -> &'static Mutex<HashMap<String, SegmentEntry>> {
    STREAM_PROXY_SEGMENT_SETS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Resolves `/seg/<playlist>/<index>` back to the segment's real URL, and
/// marks the set as in use so it outlives the renditions nobody is reading.
fn segment_set_url(path: &str) -> Option<String> {
    let (token, index) = path.strip_prefix("/seg/")?.split_once('/')?;
    let index: usize = index.trim().parse().ok()?;
    let mut sets = segment_sets().lock().ok()?;
    let entry = sets.get_mut(token)?;
    let url = entry.urls.get(index).cloned()?;
    entry.last_used = Instant::now();
    Some(url)
}

/// Says why a `/seg/` lookup missed, so a 400 storm names its own cause.
/// Kept separate from the hot path: only a failed lookup pays for it.
fn segment_set_miss_reason(path: &str) -> String {
    let Some(rest) = path.strip_prefix("/seg/") else {
        return "not a segment path".to_string();
    };
    let Some((token, index)) = rest.split_once('/') else {
        return format!("malformed segment path {rest}");
    };
    let Ok(sets) = segment_sets().lock() else {
        return "segment registry lock is poisoned".to_string();
    };
    let known: Vec<&String> = sets.keys().collect();
    match sets.get(token) {
        None => format!(
            "unknown set {token}; {} live: {}",
            known.len(),
            known
                .iter()
                .map(|value| value.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Some(entry) => format!(
            "set {token} holds {} segments, index {index} is out of range",
            entry.urls.len()
        ),
    }
}

/// One warning per set, not per segment: a failed playlist misses on every
/// one of its ~1400 entries, and that much noise buries the reason.
fn warn_segment_miss_once(path: &str) {
    static WARNED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let token = path
        .strip_prefix("/seg/")
        .and_then(|rest| rest.split_once('/'))
        .map(|(token, _)| token.to_string())
        .unwrap_or_else(|| path.to_string());
    let warned = WARNED.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut seen) = warned.lock() {
        if !seen.insert(token) {
            return;
        }
    }
    warn!("stream proxy: segment miss — {}", segment_set_miss_reason(path));
}

fn rewrite_playlist_line(
    base: Option<&Url>,
    line: &str,
    inherited_headers: Option<&[(String, String)]>,
    set: &SegmentSet,
) -> String {
    if line.trim().is_empty() {
        return line.to_string();
    }
    if line.starts_with('#') {
        return rewrite_uri_attributes(base, line, inherited_headers, set);
    }
    rewrite_playlist_url(base, line, inherited_headers, set).unwrap_or_else(|| line.to_string())
}

fn rewrite_uri_attributes(
    base: Option<&Url>,
    line: &str,
    inherited_headers: Option<&[(String, String)]>,
    set: &SegmentSet,
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
            &rewrite_playlist_url(base, uri, inherited_headers, set)
                .unwrap_or_else(|| uri.to_string()),
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
    set: &SegmentSet,
) -> Option<String> {
    let resolved = if let Ok(url) = Url::parse(value) {
        url
    } else {
        base?.join(value).ok()?
    };
    // Already ours — a playlist served through two hops would otherwise
    // have its lines wrapped a second time.
    if is_own_proxy_url(resolved.as_str()) {
        return None;
    }
    if let Some(headers) = inherited_headers {
        // By host, not by URL: these URLs are single-use.
        register_host_headers(resolved.as_str(), headers);
    }
    match resolved.scheme() {
        // Segments carry their target in the URL rather than taking a
        // registry token. YouTube regenerates every segment URL on each
        // playlist fetch, so a token per segment meant thousands of dead
        // entries per pass, evicting the segments still to be played —
        // mpv would ask for the next one and get HTTP 400.
        "http" | "https" => set.reference(resolved.as_str()),
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

#[cfg(test)]
mod tests {
    use super::strip_request_authority;

    #[test]
    fn keeps_origin_form_targets() {
        assert_eq!(strip_request_authority("/stream/abc"), "/stream/abc");
        assert_eq!(strip_request_authority("/stream/abc?x=1"), "/stream/abc?x=1");
    }

    #[test]
    fn reduces_absolute_form_to_the_path() {
        assert_eq!(
            strip_request_authority("http://127.0.0.1:6543/stream/abc"),
            "/stream/abc"
        );
        assert_eq!(
            strip_request_authority("https://localhost/stream/abc?x=1"),
            "/stream/abc?x=1"
        );
    }

    #[test]
    fn handles_an_authority_with_no_path() {
        assert_eq!(strip_request_authority("http://127.0.0.1:6543"), "/");
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    fn backend(url: &str) -> Arc<dyn StreamBackend> {
        Arc::new(HttpStreamBackend::new(url.to_string()))
    }

    #[test]
    fn reuses_the_token_for_a_url_already_registered() {
        let mut registry = StreamBackendRegistry::new();
        registry.insert("token-a".to_string(), backend("https://host/seg1.ts"));
        assert_eq!(
            registry.find_token_by_origin("https://host/seg1.ts"),
            Some("token-a".to_string())
        );
        assert_eq!(registry.find_token_by_origin("https://host/seg2.ts"), None);
    }

    #[test]
    fn rewriting_a_playlist_twice_does_not_grow_the_registry() {
        let mut registry = StreamBackendRegistry::new();
        let segments: Vec<String> = (0..500)
            .map(|index| format!("https://host/seg{index}.ts"))
            .collect();
        for pass in 0..2 {
            for (index, url) in segments.iter().enumerate() {
                if registry.find_token_by_origin(url).is_none() {
                    registry.insert(format!("t{pass}-{index}"), backend(url));
                }
            }
        }
        assert_eq!(registry.len(), segments.len());
    }

    #[test]
    fn eviction_keeps_the_reverse_index_consistent() {
        let mut registry = StreamBackendRegistry::new();
        registry.insert("token-a".to_string(), backend("https://host/seg1.ts"));
        registry.entries.remove("token-a");
        registry.forget_origins(vec![(
            "token-a".to_string(),
            "https://host/seg1.ts".to_string(),
        )]);
        // The index must not hand out a token whose entry is gone.
        assert_eq!(registry.find_token_by_origin("https://host/seg1.ts"), None);
    }
}

#[cfg(test)]
mod segment_set_tests {
    use super::*;

    #[test]
    fn resolves_a_reference_back_to_its_segment() {
        let _ = STREAM_PROXY_BASE_URL.set("http://127.0.0.1:3243".to_string());
        let set = SegmentSet::new();
        let token = set.token.clone();
        let first = set.reference("https://host/seg0.ts").unwrap();
        let second = set.reference("https://host/seg1.ts").unwrap();
        set.publish();

        // References stay short: the whole point is that a 1.4 KB
        // googlevideo URL no longer goes inline into the playlist.
        assert!(first.len() < 80, "reference too long: {first}");
        assert_eq!(first, format!("http://127.0.0.1:3243/seg/{token}/0"));
        assert_eq!(second, format!("http://127.0.0.1:3243/seg/{token}/1"));

        assert_eq!(
            segment_set_url(&format!("/seg/{token}/1")).as_deref(),
            Some("https://host/seg1.ts")
        );
        assert_eq!(segment_set_url(&format!("/seg/{token}/9")), None);
        assert_eq!(segment_set_url("/seg/unknown/0"), None);
    }

    /// Real URLs from a YouTube HLS stream. The segment carries the
    /// playlist's name mid-path and ends in the one that counts.
    #[test]
    fn tells_a_playlist_url_from_a_segment_that_mentions_one() {
        assert!(looks_like_playlist_url(
            "https://manifest.googlevideo.com/api/manifest/hls_playlist/expire/1786/playlist/index.m3u8"
        ));
        assert!(looks_like_playlist_url("https://host/path/index.m3u8?token=abc"));

        assert!(!looks_like_playlist_url(
            "https://rr1---sn-gwpa.googlevideo.com/videoplayback/id/2c4d/itag/94/playlist/index.m3u8/govp/slices%3D0-454798/begin/0/len/5200/gosq/0/file/seg.ts"
        ));
        assert!(!looks_like_playlist_url("https://host/video.mp4"));
    }

    #[test]
    fn derives_mirror_hosts_from_both_url_shapes() {
        // DASH: mn is a query parameter.
        let dash = "https://rr4---sn-h5576nsr.googlevideo.com/videoplayback?id=abc&mn=sn-h5576nsr%2Csn-h557sn6l&mvi=4";
        let candidates = with_mirror_hosts(dash);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0], dash);
        assert!(candidates[1].starts_with("https://rr4---sn-h557sn6l.googlevideo.com/"));

        // HLS: mn is a path segment.
        let hls = "https://rr1---sn-npoldn7e.googlevideo.com/videoplayback/id/x/mn/sn-npoldn7e,sn-npoe7nz7/mm/31,29/file/seg.ts";
        let candidates = with_mirror_hosts(hls);
        assert_eq!(candidates.len(), 2);
        assert!(candidates[1].starts_with("https://rr1---sn-npoe7nz7.googlevideo.com/"));

        // Anything else passes through untouched.
        assert_eq!(with_mirror_hosts("https://example.com/video.mp4").len(), 1);
        assert_eq!(
            with_mirror_hosts("https://rr1---sn-abc.googlevideo.com/videoplayback?id=x").len(),
            1
        );
    }

    #[test]
    fn only_rewrites_a_body_that_says_it_is_a_playlist() {
        assert!(body_is_playlist(b"#EXTM3U\n#EXT-X-VERSION:3\n"));
        assert!(body_is_playlist("\u{feff}#EXTM3U\n".as_bytes()));
        // An MPEG-TS packet opens with the 0x47 sync byte, not text.
        assert!(!body_is_playlist(&[0x47, 0x40, 0x00, 0x30, 0xff, 0xff]));
        assert!(!body_is_playlist(b""));
    }

    /// The bug this guards: one YouTube video publishes the master playlist
    /// and then every rendition ffmpeg probes. Evicting by age dropped the
    /// master — the only one being read — and mpv got HTTP 400 for all
    /// fourteen hundred of its segments.
    #[test]
    fn keeps_the_playlist_being_read_and_drops_the_idle_ones() {
        let _ = STREAM_PROXY_BASE_URL.set("http://127.0.0.1:3243".to_string());
        if let Ok(mut sets) = segment_sets().lock() {
            sets.clear();
        }

        let playing = SegmentSet::new();
        let playing_token = playing.token.clone();
        playing.reference("https://host/playing.ts").unwrap();
        playing.publish();

        // Enough idle renditions to overflow the cap twice over, with the
        // playing set read between each — as mpv does while it streams.
        for index in 0..(SEGMENT_SET_MAX * 2) {
            let idle = SegmentSet::new();
            idle.reference(&format!("https://host/idle{index}.ts")).unwrap();
            idle.publish();
            assert_eq!(
                segment_set_url(&format!("/seg/{playing_token}/0")).as_deref(),
                Some("https://host/playing.ts"),
                "the set being read was evicted after {} idle playlists",
                index + 1
            );
        }

        let live = segment_sets().lock().map(|sets| sets.len()).unwrap_or(0);
        assert!(live <= SEGMENT_SET_MAX, "cap not enforced: {live} live");
    }
}

#[cfg(test)]
mod self_proxy_tests {
    use super::*;

    #[test]
    fn declines_to_proxy_its_own_urls() {
        let _ = STREAM_PROXY_BASE_URL.set("http://127.0.0.1:3243".to_string());

        assert!(is_own_proxy_url("http://127.0.0.1:3243/stream/abc"));
        assert!(is_own_proxy_url("http://127.0.0.1:3243/seg/abc/0"));
        assert!(!is_own_proxy_url("https://rr4---sn-x.googlevideo.com/videoplayback"));

        // An already-proxied URL must pass through untouched rather than
        // being wrapped a second time.
        assert_eq!(rewrite_http_stream_url("http://127.0.0.1:3243/stream/abc"), None);
        assert_eq!(
            rewrite_stream_url_with_headers("http://127.0.0.1:3243/stream/abc", &[]),
            None
        );
        assert!(rewrite_https_stream_url("https://host/video.m3u8").is_some());
    }
}
