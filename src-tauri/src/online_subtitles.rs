use crate::store::{storage_paths, ui_state_store};
use flate2::read::GzDecoder;
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{ACCEPT, CONTENT_TYPE, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Duration;

const OPENSUBTITLES_API_KEY_SETTING_LABEL: &str = "OPENSUBTITLES_API_KEY";
const OPENSUBTITLES_ENABLED_SETTING_LABEL: &str = "OPENSUBTITLES_ENABLED";
const OPENSUBTITLES_LANGUAGES_SETTING_LABEL: &str = "OPENSUBTITLES_LANGUAGES";
const OPENSUBTITLES_API_BASE: &str = "https://api.opensubtitles.com/api/v1";
const SOIA_OPENSUBTITLES_API_KEY: &str = "XicSbb1oRkv5A7ZIaHRacIYrSZwsuYUF";
const SOIA_USER_AGENT: &str = "Soia/0.2.6";
const BROWSER_DOWNLOAD_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36";
const OPENSUBTITLES_HASH_CHUNK_SIZE: usize = 65_536;
const OPENSUBTITLES_MIN_HASH_FILE_SIZE: u64 = 131_072;
const ONLINE_SUBTITLE_CACHE_LIMIT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnlineSubtitlePayload {
    playback_key: String,
    media_title: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadOnlineSubtitlePayload {
    #[serde(default = "default_opensubtitles_provider_id")]
    provider_id: String,
    download_id: String,
    #[serde(default)]
    file_id: Option<i64>,
    file_name: String,
    title: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnlineSubtitleSearchResult {
    id: String,
    provider_id: String,
    download_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_id: Option<i64>,
    title: String,
    file_name: String,
    language: String,
    downloads: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnlineSubtitleDownload {
    path: String,
    title: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OnlineSubtitleCacheClearResult {
    removed_files: usize,
    removed_bytes: u64,
}

#[derive(Deserialize)]
struct OpenSubtitlesSearchResponse {
    data: Vec<OpenSubtitlesSearchItem>,
}

#[derive(Deserialize)]
struct OpenSubtitlesSearchItem {
    attributes: OpenSubtitlesAttributes,
}

#[derive(Deserialize)]
struct OpenSubtitlesAttributes {
    #[serde(default)]
    release: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    download_count: Option<i64>,
    #[serde(default)]
    files: Vec<OpenSubtitlesFile>,
}

#[derive(Deserialize)]
struct OpenSubtitlesFile {
    file_id: i64,
    #[serde(default)]
    file_name: Option<String>,
}

#[derive(Deserialize)]
struct OpenSubtitlesDownloadResponse {
    link: String,
    #[serde(default)]
    file_name: Option<String>,
}

struct OpenSubtitlesConfig {
    api_key: String,
    use_default_api_key: bool,
    languages: String,
}

struct SubtitleSearchContext<'a> {
    app: &'a tauri::AppHandle,
    client: &'a Client,
}

struct SubtitleDownloadContext<'a> {
    app: &'a tauri::AppHandle,
    client: &'a Client,
}

trait SubtitleProvider {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    fn search(
        &self,
        context: &SubtitleSearchContext<'_>,
        payload: &OnlineSubtitlePayload,
    ) -> Result<Vec<OnlineSubtitleSearchResult>, String>;

    fn download(
        &self,
        context: &SubtitleDownloadContext<'_>,
        payload: &DownloadOnlineSubtitlePayload,
    ) -> Result<OnlineSubtitleDownload, String>;
}

struct OpenSubtitlesProvider;

fn default_opensubtitles_provider_id() -> String {
    "opensubtitles".to_string()
}

fn persisted_setting(app: &tauri::AppHandle, label: &str) -> Option<String> {
    ui_state_store::load_setting_value(app, label)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn query_from_payload(payload: &OnlineSubtitlePayload) -> String {
    payload
        .media_title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            let normalized = payload.playback_key.replace('\\', "/");
            normalized
                .rsplit('/')
                .next()
                .unwrap_or(payload.playback_key.as_str())
                .trim()
                .to_string()
        })
}

fn local_media_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
    {
        return Some(PathBuf::from(trimmed));
    }
    if trimmed.starts_with("file://") {
        let parsed = url::Url::parse(trimmed).ok()?;
        if parsed.scheme() == "file" {
            return parsed.to_file_path().ok();
        }
        return None;
    }
    if url::Url::parse(trimmed).is_ok() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

fn add_hash_chunk(sum: &mut u64, bytes: &[u8]) {
    for chunk in bytes.chunks_exact(8) {
        let mut value = [0u8; 8];
        value.copy_from_slice(chunk);
        *sum = sum.wrapping_add(u64::from_le_bytes(value));
    }
}

fn opensubtitles_movie_hash(path: &str) -> Option<String> {
    let path = local_media_path(path)?;
    let mut file = std::fs::File::open(&path).ok()?;
    let file_size = file.metadata().ok()?.len();
    if file_size < OPENSUBTITLES_MIN_HASH_FILE_SIZE {
        return None;
    }

    let mut sum = file_size;
    let mut buffer = vec![0u8; OPENSUBTITLES_HASH_CHUNK_SIZE];
    file.read_exact(&mut buffer).ok()?;
    add_hash_chunk(&mut sum, &buffer);
    file.seek(SeekFrom::End(-(OPENSUBTITLES_HASH_CHUNK_SIZE as i64)))
        .ok()?;
    file.read_exact(&mut buffer).ok()?;
    add_hash_chunk(&mut sum, &buffer);
    Some(format!("{sum:016x}"))
}

fn safe_file_part(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "subtitle".to_string()
    } else {
        trimmed.chars().take(120).collect()
    }
}

fn subtitle_cache_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = storage_paths::app_data_dir(app)?.join("online_subtitles");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir)
}

struct SubtitleCacheEntry {
    path: PathBuf,
    len: u64,
    modified: std::time::SystemTime,
}

fn subtitle_cache_entries(app: &tauri::AppHandle) -> Result<Vec<SubtitleCacheEntry>, String> {
    let dir = subtitle_cache_dir(app)?;
    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(&dir).map_err(|error| error.to_string())?;
    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let metadata = match entry.metadata() {
            Ok(metadata) if metadata.is_file() => metadata,
            _ => continue,
        };
        entries.push(SubtitleCacheEntry {
            path: entry.path(),
            len: metadata.len(),
            modified: metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        });
    }
    Ok(entries)
}

fn prune_subtitle_cache(app: &tauri::AppHandle) -> Result<(), String> {
    let mut entries = subtitle_cache_entries(app)?;
    let mut total: u64 = entries.iter().map(|entry| entry.len).sum();
    if total <= ONLINE_SUBTITLE_CACHE_LIMIT_BYTES {
        return Ok(());
    }

    entries.sort_by_key(|entry| entry.modified);
    for entry in entries {
        if total <= ONLINE_SUBTITLE_CACHE_LIMIT_BYTES {
            break;
        }
        if std::fs::remove_file(&entry.path).is_ok() {
            total = total.saturating_sub(entry.len);
        }
    }
    Ok(())
}

fn clear_subtitle_cache_blocking(
    app: tauri::AppHandle,
) -> Result<OnlineSubtitleCacheClearResult, String> {
    let entries = subtitle_cache_entries(&app)?;
    let mut removed_files = 0usize;
    let mut removed_bytes = 0u64;
    for entry in entries {
        if std::fs::remove_file(&entry.path).is_ok() {
            removed_files += 1;
            removed_bytes += entry.len;
        }
    }
    Ok(OnlineSubtitleCacheClearResult {
        removed_files,
        removed_bytes,
    })
}

fn download_file_name(
    fallback_title: &str,
    fallback_file_name: &str,
    download: &OpenSubtitlesDownloadResponse,
) -> String {
    download
        .file_name
        .as_deref()
        .or(Some(fallback_file_name))
        .or(Some(fallback_title))
        .map(safe_file_part)
        .filter(|value| value.contains('.'))
        .unwrap_or_else(|| format!("{}.srt", safe_file_part("opensubtitles")))
}

fn maybe_decompress_gzip(bytes: Vec<u8>, file_name: &str) -> Result<(Vec<u8>, String), String> {
    if !file_name.to_ascii_lowercase().ends_with(".gz") {
        return Ok((bytes, file_name.to_string()));
    }

    let mut decoder = GzDecoder::new(bytes.as_slice());
    let mut decoded = Vec::new();
    decoder
        .read_to_end(&mut decoded)
        .map_err(|error| format!("Failed to decompress subtitle: {error}"))?;
    let normalized_name = file_name
        .strip_suffix(".gz")
        .or_else(|| file_name.strip_suffix(".GZ"))
        .unwrap_or("opensubtitles.srt")
        .to_string();
    Ok((decoded, normalized_name))
}

fn resolve_opensubtitles_config(app: &tauri::AppHandle) -> Result<OpenSubtitlesConfig, String> {
    let enabled = persisted_setting(app, OPENSUBTITLES_ENABLED_SETTING_LABEL)
        .map(|value| value == "On")
        .unwrap_or(false);
    if !enabled {
        return Err("OpenSubtitles is disabled in Settings.".to_string());
    }

    let configured_api_key = persisted_setting(app, OPENSUBTITLES_API_KEY_SETTING_LABEL);
    let use_default_api_key = configured_api_key.is_none();
    let api_key = configured_api_key.unwrap_or_else(|| SOIA_OPENSUBTITLES_API_KEY.to_string());
    let languages = persisted_setting(app, OPENSUBTITLES_LANGUAGES_SETTING_LABEL)
        .unwrap_or_else(|| "en".to_string());
    Ok(OpenSubtitlesConfig {
        api_key,
        use_default_api_key,
        languages,
    })
}

fn opensubtitles_client(app: &tauri::AppHandle) -> Result<Client, String> {
    crate::network::proxy::configure_blocking_client_builder(
        app,
        Client::builder()
            .connect_timeout(Duration::from_secs(12))
            .timeout(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(30)),
    )?
    .build()
    .map_err(|error| error.to_string())
}

fn read_error_response(response: Response) -> String {
    let status = response.status();
    match response.text() {
        Ok(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                format!("HTTP {status}")
            } else {
                format!("HTTP {status}: {trimmed}")
            }
        }
        Err(error) => format!("HTTP {status}: {error}"),
    }
}

fn default_api_key_access_hint(error: &str) -> String {
    format!(
        "{error}\nThe shared Soia OpenSubtitles API key is unavailable or its free download quota has been used. Add your own OpenSubtitles API key in Settings and try again."
    )
}

fn should_show_api_key_access_hint(error: &str) -> bool {
    ["HTTP 401", "HTTP 403", "HTTP 406", "HTTP 429"]
        .iter()
        .any(|needle| error.contains(needle))
        || error.to_ascii_lowercase().contains("api key")
        || error.to_ascii_lowercase().contains("quota")
}

fn with_api_key(request: RequestBuilder, api_key: &str) -> RequestBuilder {
    request.header("Api-Key", api_key)
}

fn send_with_retry(request: RequestBuilder, context: &str) -> Result<Response, String> {
    let mut last_error = None;
    for attempt in 0..3 {
        let Some(next_request) = request.try_clone() else {
            break;
        };
        match next_request.send() {
            Ok(response) if response.status().is_success() => return Ok(response),
            Ok(response) => {
                return Err(format!("{context}: {}", read_error_response(response)));
            }
            Err(error) => {
                last_error = Some(error);
                if attempt < 2 {
                    std::thread::sleep(Duration::from_millis(350 * (attempt + 1) as u64));
                }
            }
        }
    }

    Err(match last_error {
        Some(error) => {
            let mut details = error.to_string();
            let mut source = std::error::Error::source(&error);
            while let Some(error) = source {
                details.push_str(": ");
                details.push_str(&error.to_string());
                source = error.source();
            }
            format!("{context}: {details}")
        }
        None => format!("{context}: request could not be retried"),
    })
}

impl SubtitleProvider for OpenSubtitlesProvider {
    fn id(&self) -> &'static str {
        "opensubtitles"
    }

    fn display_name(&self) -> &'static str {
        "OpenSubtitles"
    }

    fn search(
        &self,
        context: &SubtitleSearchContext<'_>,
        payload: &OnlineSubtitlePayload,
    ) -> Result<Vec<OnlineSubtitleSearchResult>, String> {
        let config = resolve_opensubtitles_config(context.app)?;
        let query = query_from_payload(payload);
        if query.is_empty() {
            return Err("No media title or path is available for subtitle search.".to_string());
        }
        let movie_hash = opensubtitles_movie_hash(&payload.playback_key);

        let mut params = vec![
            ("query", query.as_str()),
            ("languages", config.languages.as_str()),
            ("order_by", "download_count"),
            ("order_direction", "desc"),
            ("per_page", "50"),
        ];
        if let Some(movie_hash) = movie_hash.as_deref() {
            params.push(("moviehash", movie_hash));
        }
        let request = with_api_key(
            context
                .client
                .get(format!("{OPENSUBTITLES_API_BASE}/subtitles"))
                .header(USER_AGENT, SOIA_USER_AGENT)
                .query(&params),
            &config.api_key,
        );
        let search: OpenSubtitlesSearchResponse =
            send_with_retry(request, "OpenSubtitles search failed")
                .map_err(|error| {
                    if config.use_default_api_key && should_show_api_key_access_hint(&error) {
                        default_api_key_access_hint(&error)
                    } else {
                        error
                    }
                })?
                .json()
                .map_err(|error| format!("OpenSubtitles search response is invalid: {error}"))?;

        Ok(search
            .data
            .iter()
            .flat_map(|item| {
                item.attributes.files.iter().map(|file| {
                    let file_name = file
                        .file_name
                        .as_deref()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| "opensubtitles.srt".to_string());
                    let title = item
                        .attributes
                        .release
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| file_name.clone());
                    let language = item
                        .attributes
                        .language
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| "unknown".to_string());
                    OnlineSubtitleSearchResult {
                        id: format!("{}-{}", self.id(), file.file_id),
                        provider_id: self.id().to_string(),
                        download_id: file.file_id.to_string(),
                        file_id: Some(file.file_id),
                        title,
                        file_name,
                        language,
                        downloads: item.attributes.download_count,
                    }
                })
            })
            .take(50)
            .collect())
    }

    fn download(
        &self,
        context: &SubtitleDownloadContext<'_>,
        payload: &DownloadOnlineSubtitlePayload,
    ) -> Result<OnlineSubtitleDownload, String> {
        let config = resolve_opensubtitles_config(context.app)?;
        let file_id = payload
            .file_id
            .or_else(|| payload.download_id.parse::<i64>().ok())
            .ok_or_else(|| "OpenSubtitles download id is invalid.".to_string())?;

        let download_request = with_api_key(
            context
                .client
                .post(format!("{OPENSUBTITLES_API_BASE}/download"))
                .header(USER_AGENT, SOIA_USER_AGENT)
                .header(ACCEPT, "application/json")
                .header(CONTENT_TYPE, "application/json")
                .body(json!({ "file_id": file_id }).to_string()),
            &config.api_key,
        );
        let download: OpenSubtitlesDownloadResponse =
            send_with_retry(download_request, "OpenSubtitles download request failed")
                .map_err(|error| {
                    if config.use_default_api_key && should_show_api_key_access_hint(&error) {
                        default_api_key_access_hint(&error)
                    } else {
                        error
                    }
                })?
                .json()
                .map_err(|error| {
                    format!("OpenSubtitles download response is invalid: {error}")
                })?;

        let file_name = download_file_name(&payload.title, &payload.file_name, &download);
        let subtitle_request = with_api_key(
            context
                .client
                .get(&download.link)
                .header(USER_AGENT, BROWSER_DOWNLOAD_USER_AGENT)
                .header(ACCEPT, "text/plain, text/vtt, application/x-subrip, */*")
                .header(CONTENT_TYPE, "application/json")
                .header(REFERER, "https://www.opensubtitles.com/"),
            &config.api_key,
        );
        let bytes = send_with_retry(subtitle_request, "Subtitle download failed")
            .map_err(|error| {
                if config.use_default_api_key && should_show_api_key_access_hint(&error) {
                    default_api_key_access_hint(&error)
                } else {
                    error
                }
            })?
            .bytes()
            .map_err(|error| format!("Subtitle body is invalid: {error}"))?
            .to_vec();
        let (bytes, file_name) = maybe_decompress_gzip(bytes, &file_name)?;

        let cache_name = format!(
            "{}_{}",
            safe_file_part(&payload.title),
            safe_file_part(&file_name),
        );
        let path = subtitle_cache_dir(context.app)?.join(cache_name);
        std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
        prune_subtitle_cache(context.app)?;

        Ok(OnlineSubtitleDownload {
            path: path.to_string_lossy().to_string(),
            title: format!("{}: {file_name}", self.display_name()),
        })
    }
}

fn subtitle_providers() -> Vec<Box<dyn SubtitleProvider>> {
    vec![Box::new(OpenSubtitlesProvider)]
}

fn search_online_subtitles_blocking(
    app: tauri::AppHandle,
    payload: OnlineSubtitlePayload,
) -> Result<Vec<OnlineSubtitleSearchResult>, String> {
    let client = opensubtitles_client(&app)?;
    let context = SubtitleSearchContext {
        app: &app,
        client: &client,
    };
    let providers = subtitle_providers();
    let mut results = Vec::new();
    let mut errors = Vec::new();
    for provider in providers {
        match provider.search(&context, &payload) {
            Ok(mut provider_results) => results.append(&mut provider_results),
            Err(error) => errors.push(format!("{}: {error}", provider.display_name())),
        }
    }
    if results.is_empty() && !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    Ok(results)
}

fn download_online_subtitle_blocking(
    app: tauri::AppHandle,
    payload: DownloadOnlineSubtitlePayload,
) -> Result<OnlineSubtitleDownload, String> {
    let client = opensubtitles_client(&app)?;
    let context = SubtitleDownloadContext {
        app: &app,
        client: &client,
    };
    let providers = subtitle_providers();
    let provider = providers
        .iter()
        .find(|provider| provider.id() == payload.provider_id)
        .ok_or_else(|| format!("Unsupported subtitle provider: {}", payload.provider_id))?;
    provider.download(&context, &payload)
}

#[tauri::command]
pub(crate) async fn search_online_subtitles(
    app: tauri::AppHandle,
    payload: OnlineSubtitlePayload,
) -> Result<Vec<OnlineSubtitleSearchResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        search_online_subtitles_blocking(app, payload)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn download_online_subtitle(
    app: tauri::AppHandle,
    payload: DownloadOnlineSubtitlePayload,
) -> Result<OnlineSubtitleDownload, String> {
    tauri::async_runtime::spawn_blocking(move || {
        download_online_subtitle_blocking(app, payload)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn clear_online_subtitle_cache(
    app: tauri::AppHandle,
) -> Result<OnlineSubtitleCacheClearResult, String> {
    tauri::async_runtime::spawn_blocking(move || clear_subtitle_cache_blocking(app))
        .await
        .map_err(|error| error.to_string())?
}
