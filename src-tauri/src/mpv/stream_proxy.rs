use log::{debug, info, warn};
use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};
use reqwest::header::{
    HeaderName, HeaderValue, ACCEPT_ENCODING, ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE,
    CONTENT_TYPE, RANGE, USER_AGENT,
};
use futures_util::stream::{FuturesUnordered, StreamExt};
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use std::collections::HashMap;
use std::net::TcpListener as StdTcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::AppHandle;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

const HTTP_USER_AGENT: &str = "Lavf/61.7.100";
const MAX_REQUEST_HEADER_BYTES: usize = 128 * 1024;
const PARALLEL_RANGE_MIN_BYTES: u64 = 16 * 1024 * 1024;
const PARALLEL_RANGE_CHUNK_BYTES: u64 = 2 * 1024 * 1024;
const PARALLEL_RANGE_CONNECTIONS: usize = 3;
const PARALLEL_RANGE_SETTING_LABEL: &str = "NETWORK_PARALLEL_DOWNLOAD";

type BasicAuth = (String, String);
pub(crate) type ProxyHeaders = Vec<(String, String)>;

static STREAM_PROXY_BASE_URL: OnceLock<String> = OnceLock::new();
static STREAM_PROXY_BASIC_AUTH: OnceLock<Mutex<HashMap<String, BasicAuth>>> = OnceLock::new();
static STREAM_PROXY_HEADERS: OnceLock<Mutex<HashMap<String, ProxyHeaders>>> = OnceLock::new();
static STREAM_PROXY_CLIENT: OnceLock<Mutex<Option<CachedClient>>> = OnceLock::new();
static STREAM_PROXY_PARALLEL_RANGE_ENABLED: AtomicBool = AtomicBool::new(false);

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

    let base_url = format!("http://{addr}/stream?url=");
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

fn proxy_url_for(raw: &str) -> Option<String> {
    let base = STREAM_PROXY_BASE_URL.get()?;
    Some(format!(
        "{}{}",
        base,
        utf8_percent_encode(raw, NON_ALPHANUMERIC)
    ))
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

    let Some(remote_url) = parse_remote_url(&target) else {
        write_status(&mut stream, 400, "Bad Request", b"missing url").await?;
        return Ok(());
    };
    info!("stream proxy: fetch {}", redact_url(&remote_url));

    let response = match fetch_remote(app_handle, &remote_url, range.as_deref()).await {
        Ok(response) => response,
        Err(error) => {
            warn!(
                "stream proxy: upstream fetch failed url={} error={error}",
                redact_url(&remote_url)
            );
            write_status(&mut stream, 502, "Bad Gateway", error.as_bytes()).await?;
            return Ok(());
        }
    };
    let status = response.status();
    if !status.is_success() {
        let code = status.as_u16();
        let reason = status.canonical_reason().unwrap_or("Upstream Error").to_string();
        let body = response.bytes().await.map_err(|error| error.to_string())?;
        write_status(&mut stream, code, &reason, &body).await?;
        return Ok(());
    }

    if should_rewrite_playlist(&remote_url, &response) {
        let content_type = content_type(&response);
        let reason = status.canonical_reason().unwrap_or("OK").to_string();
        let bytes = response.bytes().await.map_err(|error| error.to_string())?;
        let text = String::from_utf8_lossy(&bytes);
        let inherited_headers = lookup_headers(&remote_url);
        let body = rewrite_playlist(&remote_url, &text, inherited_headers.as_deref()).into_bytes();
        write_response(
            &mut stream,
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
        &mut stream,
        &method,
        &remote_url,
        range.as_deref(),
        response,
    )
    .await
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
    request.send().await.map_err(|error| error.to_string())
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
        "https" => proxy_url_for(resolved.as_str()),
        "http" if inherited_headers.is_some() => proxy_url_for(resolved.as_str()),
        "http" => Some(resolved.to_string()),
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
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let path = url.path().to_ascii_lowercase();
    path.ends_with(".m3u8")
        || host.contains("googlevideo.com")
        || host.contains("youtube.com")
        || host.contains("youtu.be")
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
