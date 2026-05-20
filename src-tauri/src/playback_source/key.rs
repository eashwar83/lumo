use std::path::PathBuf;

const SOIA_WEBDAV_KEY_PREFIX: &str = "soia-webdav://";
const SOIA_DLNA_KEY_PREFIX: &str = "soia-dlna://";
const SOIA_SMB_KEY_PREFIX: &str = "soia-smb://";

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webdav_numeric_connection_ids_round_trip() {
        let key = create_webdav_playback_key("webdav-1", "/Movies/a b.mkv");
        assert_eq!(key, "soia-webdav://1/Movies/a b.mkv");

        match parse_playback_source(&key) {
            PlaybackSource::Webdav {
                connection_id,
                file_path,
            } => {
                assert_eq!(connection_id, "webdav-1");
                assert_eq!(file_path, "/Movies/a b.mkv");
            }
            _ => panic!("expected WebDAV playback source"),
        }
    }

    #[test]
    fn smb_numeric_connection_ids_round_trip() {
        let key = create_smb_playback_key("smb-1", "/Movies/a b.mkv");
        assert_eq!(key, "soia-smb://1/Movies/a b.mkv");

        match parse_playback_source(&key) {
            PlaybackSource::Smb {
                connection_id,
                file_path,
            } => {
                assert_eq!(connection_id.as_deref(), Some("smb-1"));
                assert_eq!(file_path.as_deref(), Some("/Movies/a b.mkv"));
            }
            _ => panic!("expected SMB playback source"),
        }
    }

    #[test]
    fn dlna_resource_url_is_percent_encoded() {
        let key = create_dlna_playback_key(
            "dlna-main",
            "http://host:8200/media/a b/[01].mkv?x=1&y=2",
            Some("0/children"),
        );
        assert_eq!(
            key,
            "soia-dlna://dlna-main/http%3A%2F%2Fhost%3A8200%2Fmedia%2Fa%20b%2F%5B01%5D.mkv%3Fx%3D1%26y%3D2/0%2Fchildren"
        );

        match parse_playback_source(&key) {
            PlaybackSource::Dlna {
                connection_id,
                resource_url,
                parent_path,
            } => {
                assert_eq!(connection_id, "dlna-main");
                assert_eq!(
                    resource_url,
                    "http://host:8200/media/a b/[01].mkv?x=1&y=2"
                );
                assert_eq!(parent_path.as_deref(), Some("0/children"));
            }
            _ => panic!("expected DLNA playback source"),
        }
    }

    #[test]
    fn reserved_display_safe_path_characters_keep_key_shape() {
        let path = "/影视/Season 1/[01] wow!~*'().mkv";
        assert_eq!(
            create_webdav_playback_key("webdav-2", path),
            "soia-webdav://2/影视/Season 1/[01] wow!~*'().mkv"
        );
        assert_eq!(
            create_smb_playback_key("smb-2", path),
            "soia-smb://2/影视/Season 1/[01] wow!~*'().mkv"
        );
    }

    #[test]
    fn direct_smb_url_is_detected() {
        match parse_playback_source("smb://host/share/a%20b.mkv") {
            PlaybackSource::DirectSmbUrl => {}
            _ => panic!("expected direct SMB URL"),
        }
    }
}
