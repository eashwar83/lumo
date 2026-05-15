use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub(crate) const SOIA_WEBDAV_KEY_PREFIX: &str = "soia-webdav://";
pub(crate) const SOIA_DLNA_KEY_PREFIX: &str = "soia-dlna://";
pub(crate) const SOIA_SMB_KEY_PREFIX: &str = "soia-smb://";

const ENCODE_URI_COMPONENT_SET: &percent_encoding::AsciiSet =
    &percent_encoding::CONTROLS
        .add(b' ')
        .add(b'"')
        .add(b'#')
        .add(b'$')
        .add(b'%')
        .add(b'&')
        .add(b'+')
        .add(b',')
        .add(b'/')
        .add(b':')
        .add(b';')
        .add(b'<')
        .add(b'=')
        .add(b'>')
        .add(b'?')
        .add(b'@')
        .add(b'[')
        .add(b'\\')
        .add(b']')
        .add(b'^')
        .add(b'`')
        .add(b'{')
        .add(b'|')
        .add(b'}');

pub(crate) enum PlaybackSource {
    Local {
        path: String,
    },
    Webdav {
        connection_id: String,
        file_path: String,
    },
    Dlna {
        connection_id: String,
        resource_url: String,
        parent_path: Option<String>,
    },
    Smb {
        connection_id: Option<String>,
        file_path: Option<String>,
    },
    DirectSmbUrl,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AdjacentPlaybackSourcePayload {
    playback_key: String,
    direction: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AdjacentPlaybackSourceResult {
    playback_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SmbPlaybackSourceResult {
    connection_id: String,
    file_path: String,
    playback_key: String,
}

pub(crate) fn resolve_local_media_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("file://") {
        let parsed = url::Url::parse(trimmed).ok()?;
        if parsed.scheme() == "file" {
            return parsed.to_file_path().ok();
        }
        return None;
    }
    Some(PathBuf::from(trimmed))
}

pub(crate) fn decode_url_component(value: &str) -> String {
    percent_encoding::percent_decode_str(value)
        .decode_utf8_lossy()
        .to_string()
}

pub(crate) fn normalize_file_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    let with_leading = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    with_leading.trim_end_matches('/').to_string()
}

pub(crate) fn path_file_name(path: &str) -> String {
    path.split(['?', '#'])
        .next()
        .unwrap_or(path)
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_string()
}

pub(crate) fn path_parent(path: &str) -> Option<String> {
    let normalized = normalize_file_path(path);
    if normalized == "/" {
        return None;
    }
    let index = normalized.rfind('/')?;
    if index == 0 {
        Some("/".to_string())
    } else {
        Some(normalized[..index].to_string())
    }
}

pub(crate) fn path_extension(path: &str) -> String {
    let file_name = path_file_name(path);
    file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

pub(crate) fn path_stem(path: &str) -> String {
    let file_name = path_file_name(path);
    file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem.to_string())
        .unwrap_or(file_name)
}

fn runtime_connection_id(prefix: &str, value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.chars().all(|item| item.is_ascii_digit()) {
        format!("{prefix}-{trimmed}")
    } else {
        trimmed.to_string()
    }
}

fn key_connection_id(prefix: &str, value: &str) -> String {
    value
        .trim()
        .strip_prefix(&format!("{prefix}-"))
        .filter(|suffix| suffix.chars().all(|item| item.is_ascii_digit()))
        .unwrap_or_else(|| value.trim())
        .to_string()
}

fn parse_webdav_like_key(key: &str, prefix: &str) -> Option<(String, String)> {
    if !key.starts_with(prefix) {
        return None;
    }
    let value = &key[prefix.len()..];
    let slash_index = value.find('/')?;
    if slash_index == 0 {
        return None;
    }
    let connection_id = &value[..slash_index];
    let file_path = normalize_file_path(&value[slash_index..]);
    if connection_id.trim().is_empty() {
        return None;
    }
    Some((connection_id.to_string(), file_path))
}

fn parse_dlna_key(key: &str) -> Option<(String, String, Option<String>)> {
    if !key.starts_with(SOIA_DLNA_KEY_PREFIX) {
        return None;
    }
    let value = &key[SOIA_DLNA_KEY_PREFIX.len()..];
    let slash_index = value.find('/')?;
    if slash_index == 0 {
        return None;
    }
    let encoded_connection_id = &value[..slash_index];
    let encoded_parts = value[slash_index + 1..].split('/').collect::<Vec<_>>();
    let encoded_resource_url = encoded_parts.first().copied().unwrap_or_default();
    if encoded_connection_id.is_empty() || encoded_resource_url.is_empty() {
        return None;
    }
    let connection_id = decode_url_component(encoded_connection_id);
    let resource_url = decode_url_component(encoded_resource_url);
    let parent_path = encoded_parts
        .get(1)
        .map(|value| decode_url_component(value))
        .filter(|value| !value.trim().is_empty());
    Some((connection_id, resource_url, parent_path))
}

pub(crate) fn parse_playback_source(key: &str) -> PlaybackSource {
    if let Some((connection_id, file_path)) =
        parse_webdav_like_key(key, SOIA_WEBDAV_KEY_PREFIX)
    {
        return PlaybackSource::Webdav {
            connection_id: runtime_connection_id("webdav", &connection_id),
            file_path,
        };
    }
    if let Some((connection_id, resource_url, parent_path)) = parse_dlna_key(key) {
        return PlaybackSource::Dlna {
            connection_id,
            resource_url,
            parent_path,
        };
    }
    if let Some((connection_id, file_path)) =
        parse_webdav_like_key(key, SOIA_SMB_KEY_PREFIX)
    {
        return PlaybackSource::Smb {
            connection_id: Some(runtime_connection_id(
                "smb",
                &decode_url_component(&connection_id),
            )),
            file_path: Some(file_path),
        };
    }
    if key.trim().to_ascii_lowercase().starts_with("smb://") {
        return PlaybackSource::DirectSmbUrl;
    }
    PlaybackSource::Local {
        path: key.to_string(),
    }
}

pub(crate) fn create_webdav_playback_key(connection_id: &str, file_path: &str) -> String {
    format!(
        "{}{}{}",
        SOIA_WEBDAV_KEY_PREFIX,
        key_connection_id("webdav", connection_id),
        normalize_file_path(file_path)
    )
}

pub(crate) fn create_dlna_playback_key(
    connection_id: &str,
    resource_url: &str,
    parent_path: Option<&str>,
) -> String {
    let base = format!(
        "{}{}/{}",
        SOIA_DLNA_KEY_PREFIX,
        percent_encoding::utf8_percent_encode(connection_id.trim(), ENCODE_URI_COMPONENT_SET),
        percent_encoding::utf8_percent_encode(resource_url.trim(), ENCODE_URI_COMPONENT_SET)
    );
    let Some(parent_path) = parent_path.map(str::trim).filter(|value| !value.is_empty()) else {
        return base;
    };
    format!(
        "{}/{}",
        base,
        percent_encoding::utf8_percent_encode(parent_path, ENCODE_URI_COMPONENT_SET)
    )
}

pub(crate) fn create_smb_playback_key(connection_id: &str, file_path: &str) -> String {
    format!(
        "{}{}{}",
        SOIA_SMB_KEY_PREFIX,
        percent_encoding::utf8_percent_encode(
            &key_connection_id("smb", connection_id),
            ENCODE_URI_COMPONENT_SET,
        ),
        normalize_file_path(file_path)
    )
}

fn is_smb_protocol(protocol: &str) -> bool {
    let normalized = protocol.trim().to_ascii_lowercase();
    normalized == "smb" || normalized == "samba"
}

fn url_host_key(url: &url::Url) -> Option<String> {
    let host = url.host_str()?.to_ascii_lowercase();
    Some(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    })
}

fn decoded_url_path_segments(url: &url::Url) -> Vec<String> {
    url.path()
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(decode_url_component)
        .collect()
}

fn normalized_path_from_segments(segments: &[String]) -> String {
    if segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", segments.join("/"))
    }
}

fn is_media_path(path: &str) -> bool {
    crate::media_extensions::contains_extension(&path_extension(path))
}

fn sorted_file_entries(
    entries: Vec<crate::network::types::NetworkBrowseEntry>,
    protocol: crate::network::service::BrowseProtocol,
) -> Vec<crate::network::types::NetworkBrowseEntry> {
    let mut files = entries
        .into_iter()
        .filter(|entry| {
            entry.entry_type == "file"
                && (protocol == crate::network::service::BrowseProtocol::Dlna
                    || is_media_path(&entry.path))
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    files
}

fn adjacent_from_index<T>(items: &[T], current_index: usize, direction: i32) -> Option<&T> {
    let next_index = current_index as i32 + direction;
    if next_index < 0 || next_index >= items.len() as i32 {
        return None;
    }
    items.get(next_index as usize)
}

fn normalize_dlna_object_id(value: &str) -> String {
    value.trim().trim_start_matches('/').to_string()
}

#[tauri::command]
pub(crate) async fn resolve_smb_playback_source_from_url(
    app: tauri::AppHandle,
    url: String,
) -> Result<Option<SmbPlaybackSourceResult>, String> {
    let parsed = match url::Url::parse(url.trim()) {
        Ok(value) if value.scheme().eq_ignore_ascii_case("smb") => value,
        _ => return Ok(None),
    };
    let Some(url_host) = url_host_key(&parsed) else {
        return Ok(None);
    };
    let url_segments = decoded_url_path_segments(&parsed);

    let connections = crate::store::network_connection_store::list_network_connections(&app)?;
    let mut best_match: Option<(String, String, usize)> = None;
    for connection in connections {
        if !is_smb_protocol(&connection.protocol) {
            continue;
        }
        let Ok(base_url) = url::Url::parse(connection.base_url.trim()) else {
            continue;
        };
        if !base_url.scheme().eq_ignore_ascii_case("smb") {
            continue;
        }
        if url_host_key(&base_url).as_deref() != Some(url_host.as_str()) {
            continue;
        }

        let base_segments = decoded_url_path_segments(&base_url);
        let is_prefix = base_segments
            .iter()
            .enumerate()
            .all(|(index, segment)| url_segments.get(index) == Some(segment));
        if !is_prefix {
            continue;
        }

        let score = base_segments.len();
        if best_match
            .as_ref()
            .map(|(_, _, best_score)| score > *best_score)
            .unwrap_or(true)
        {
            let file_path = normalized_path_from_segments(&url_segments[base_segments.len()..]);
            best_match = Some((connection.id, file_path, score));
        }
    }

    Ok(best_match.map(|(connection_id, file_path, _)| {
        let playback_key = create_smb_playback_key(&connection_id, &file_path);
        SmbPlaybackSourceResult {
            connection_id,
            file_path,
            playback_key,
        }
    }))
}

fn resolve_adjacent_local(path: &str, direction: i32) -> Option<String> {
    let local_path = resolve_local_media_path(path)?;
    if !local_path.is_file() || !is_media_path(&local_path.to_string_lossy()) {
        return None;
    }
    let parent = local_path.parent()?;
    let mut files = std::fs::read_dir(parent)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|entry_path| entry_path.is_file() && is_media_path(&entry_path.to_string_lossy()))
        .collect::<Vec<_>>();
    files.sort_by(|left, right| {
        let left_name = left
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_lowercase();
        let right_name = right
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_lowercase();
        left_name.cmp(&right_name)
    });
    let current_index = files.iter().position(|entry| entry == &local_path)?;
    adjacent_from_index(&files, current_index, direction)
        .map(|path| path.to_string_lossy().into_owned())
}

async fn resolve_adjacent_network(
    app: &tauri::AppHandle,
    connection_id: &str,
    protocol: crate::network::service::BrowseProtocol,
    current_path: &str,
    parent_path: &str,
    direction: i32,
) -> Result<Option<String>, String> {
    let connection =
        crate::store::network_connection_store::find_network_connection(app, connection_id)?;
    let result =
        crate::network::service::browse_connection(app, &connection, parent_path, protocol).await?;
    let files = sorted_file_entries(result.entries, protocol);
    let current_index = files
        .iter()
        .position(|entry| normalize_file_path(&entry.path) == normalize_file_path(current_path));
    let Some(current_index) = current_index else {
        return Ok(None);
    };
    let Some(next_entry) = adjacent_from_index(&files, current_index, direction) else {
        return Ok(None);
    };
    let playback_key = match protocol {
        crate::network::service::BrowseProtocol::Webdav => {
            create_webdav_playback_key(connection_id, &next_entry.path)
        }
        crate::network::service::BrowseProtocol::Dlna => {
            create_dlna_playback_key(connection_id, &next_entry.path, Some(parent_path))
        }
        crate::network::service::BrowseProtocol::Smb => {
            create_smb_playback_key(connection_id, &next_entry.path)
        }
    };
    Ok(Some(playback_key))
}

#[tauri::command]
pub(crate) async fn resolve_adjacent_playback_source(
    app: tauri::AppHandle,
    payload: AdjacentPlaybackSourcePayload,
) -> Result<Option<AdjacentPlaybackSourceResult>, String> {
    let direction = match payload.direction {
        value if value < 0 => -1,
        value if value > 0 => 1,
        _ => return Ok(None),
    };

    let playback_key = match parse_playback_source(&payload.playback_key) {
        PlaybackSource::Local { path } => resolve_adjacent_local(&path, direction),
        PlaybackSource::Webdav {
            connection_id,
            file_path,
        } => {
            let Some(parent_path) = path_parent(&file_path) else {
                return Ok(None);
            };
            resolve_adjacent_network(
                &app,
                &connection_id,
                crate::network::service::BrowseProtocol::Webdav,
                &file_path,
                &parent_path,
                direction,
            )
            .await?
        }
        PlaybackSource::Dlna {
            connection_id,
            resource_url,
            parent_path,
        } => {
            let Some(parent_path) = parent_path else {
                return Ok(None);
            };
            let parent_path = normalize_dlna_object_id(&parent_path);
            resolve_adjacent_network(
                &app,
                &connection_id,
                crate::network::service::BrowseProtocol::Dlna,
                &resource_url,
                &parent_path,
                direction,
            )
            .await?
        }
        PlaybackSource::Smb {
            connection_id: Some(connection_id),
            file_path: Some(file_path),
            ..
        } => {
            let Some(parent_path) = path_parent(&file_path) else {
                return Ok(None);
            };
            resolve_adjacent_network(
                &app,
                &connection_id,
                crate::network::service::BrowseProtocol::Smb,
                &file_path,
                &parent_path,
                direction,
            )
            .await?
        }
        PlaybackSource::Smb { .. } | PlaybackSource::DirectSmbUrl => None,
    };

    Ok(playback_key.map(|playback_key| AdjacentPlaybackSourceResult {
        playback_key,
    }))
}
