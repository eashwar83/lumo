use crate::network::types::{DiscoveredNetworkConnection, NetworkBrowseEntry, NetworkBrowseResult};
use crate::store::network_connection_store::NetworkConnectionRecord;
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BrowseProtocol {
    Webdav,
    Dlna,
    Smb,
}

fn ensure_protocol(connection: &NetworkConnectionRecord, expected: &[&str]) -> Result<(), String> {
    let protocol = connection.protocol.trim().to_ascii_lowercase();
    if expected
        .iter()
        .any(|item| protocol == item.to_ascii_lowercase())
    {
        return Ok(());
    }
    Err(format!(
        "Connection {} protocol {} does not match required protocol",
        connection.id, connection.protocol
    ))
}

pub(crate) fn protocol_from_str(value: &str) -> Option<BrowseProtocol> {
    match value.trim().to_ascii_lowercase().as_str() {
        "webdav" => Some(BrowseProtocol::Webdav),
        "http-dlna" | "dlna" => Some(BrowseProtocol::Dlna),
        "smb" | "samba" => Some(BrowseProtocol::Smb),
        _ => None,
    }
}

pub(crate) fn protocol_from_connection(
    connection: &NetworkConnectionRecord,
) -> Result<BrowseProtocol, String> {
    protocol_from_str(&connection.protocol).ok_or_else(|| {
        format!(
            "Unsupported protocol {} for connection {}",
            connection.protocol, connection.id
        )
    })
}

pub(crate) fn resolve_protocol_with_hint(
    connection: &NetworkConnectionRecord,
    protocol_hint: Option<&str>,
) -> Result<BrowseProtocol, String> {
    let connection_protocol = protocol_from_connection(connection)?;
    if let Some(value) = protocol_hint {
        let requested = protocol_from_str(value)
            .ok_or_else(|| format!("Unsupported protocol hint: {}", value))?;
        if requested != connection_protocol {
            return Err(format!(
                "Protocol mismatch: connection={} requested={}",
                connection.protocol, value
            ));
        }
    }
    Ok(connection_protocol)
}

pub(crate) fn validate_mode(mode: &str) -> Result<(), String> {
    // `connect` means "initial browse using connection default path".
    // `browse` means "navigate to caller-provided path (or protocol fallback)".
    match mode.trim().to_ascii_lowercase().as_str() {
        "connect" | "browse" => Ok(()),
        other => Err(format!("Unsupported browse mode: {}", other)),
    }
}

pub(crate) fn resolve_browse_path(
    connection: &NetworkConnectionRecord,
    protocol: BrowseProtocol,
    path: Option<String>,
    mode: &str,
) -> String {
    let is_connect = mode.trim().eq_ignore_ascii_case("connect");
    if is_connect {
        return path.unwrap_or_else(|| default_browse_path(connection, protocol));
    }
    normalize_input_path(path, protocol)
}

pub(crate) async fn browse_connection(
    app: &tauri::AppHandle,
    connection: &NetworkConnectionRecord,
    path: &str,
    protocol: BrowseProtocol,
) -> Result<NetworkBrowseResult, String> {
    match protocol {
        BrowseProtocol::Webdav => {
            ensure_protocol(connection, &["webdav"])?;
            let result =
                crate::network::protocols::webdav::list_directory(app, connection, path).await?;
            Ok(NetworkBrowseResult {
                path: result.path,
                entries: result
                    .entries
                    .into_iter()
                    .map(|entry| NetworkBrowseEntry {
                        name: entry.name,
                        path: entry.path,
                        entry_type: if entry.is_dir {
                            "dir".into()
                        } else {
                            "file".into()
                        },
                        size: entry.size,
                        modified_at: entry.modified_at,
                    })
                    .collect(),
            })
        }
        BrowseProtocol::Dlna => {
            ensure_protocol(connection, &["http-dlna", "dlna"])?;
            let result =
                crate::network::protocols::dlna::browse_directory(app, connection, path).await?;
            Ok(NetworkBrowseResult {
                path: result.path,
                entries: result
                    .entries
                    .into_iter()
                    .map(|entry| NetworkBrowseEntry {
                        name: entry.name,
                        path: entry.path,
                        entry_type: if entry.is_dir {
                            "dir".into()
                        } else {
                            "file".into()
                        },
                        size: entry.size,
                        modified_at: entry.modified_at,
                    })
                    .collect(),
            })
        }
        BrowseProtocol::Smb => {
            ensure_protocol(connection, &["smb", "samba"])?;
            let result = crate::network::protocols::smb::list_directory(connection, path).await?;
            Ok(NetworkBrowseResult {
                path: result.path,
                entries: result
                    .entries
                    .into_iter()
                    .map(|entry| NetworkBrowseEntry {
                        name: entry.name,
                        path: entry.path,
                        entry_type: if entry.is_dir {
                            "dir".into()
                        } else {
                            "file".into()
                        },
                        size: entry.size,
                        modified_at: entry.modified_at,
                    })
                    .collect(),
            })
        }
    }
}

pub(crate) async fn discover_connections(
    app: &tauri::AppHandle,
    protocol: Option<&str>,
    timeout_secs: Option<u64>,
) -> Result<Vec<DiscoveredNetworkConnection>, String> {
    let protocol = protocol
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "all".to_string());
    let timeout_secs = timeout_secs.unwrap_or(3);

    let mut result: Vec<DiscoveredNetworkConnection> = Vec::new();
    if protocol == "all" || protocol == "http-dlna" || protocol == "dlna" {
        let devices = crate::network::protocols::dlna::discover_devices(app, timeout_secs).await?;
        result.extend(devices.into_iter().map(|item| DiscoveredNetworkConnection {
            protocol: "http-dlna".to_string(),
            usn: Some(item.usn),
            location: item.location,
            friendly_name: item.friendly_name,
            server: item.server,
            st: Some(item.st),
        }));
    }
    if protocol == "all" || protocol == "smb" || protocol == "samba" {
        let devices = crate::network::protocols::smb::discover_devices(timeout_secs).await?;
        result.extend(devices.into_iter().map(|item| DiscoveredNetworkConnection {
            protocol: "smb".to_string(),
            usn: Some(item.instance_name),
            location: item.location,
            friendly_name: item.friendly_name,
            server: item.server,
            st: Some(item.service_type),
        }));
    }

    Ok(result)
}

pub(crate) fn resolve_network_playback_url(
    connection: &NetworkConnectionRecord,
    protocol_hint: Option<&str>,
    file_path: &str,
) -> Result<String, String> {
    let protocol = resolve_protocol_with_hint(connection, protocol_hint)?;

    match protocol {
        BrowseProtocol::Webdav => {
            ensure_protocol(connection, &["webdav"])?;
            crate::network::protocols::webdav::build_playback_url(connection, file_path)
        }
        BrowseProtocol::Smb => {
            ensure_protocol(connection, &["smb", "samba"])?;
            crate::network::protocols::smb::build_playback_url(connection, file_path)
        }
        BrowseProtocol::Dlna => Err(format!(
            "load_network_file does not support DLNA playback URL resolution for {}",
            connection.protocol
        )),
    }
}

fn default_browse_path(connection: &NetworkConnectionRecord, protocol: BrowseProtocol) -> String {
    let value = connection.default_path.trim();
    if !value.is_empty() {
        return value.to_string();
    }
    match protocol {
        BrowseProtocol::Webdav => "/".to_string(),
        BrowseProtocol::Dlna => "0".to_string(),
        BrowseProtocol::Smb => "/".to_string(),
    }
}

fn normalize_input_path(path: Option<String>, protocol: BrowseProtocol) -> String {
    let fallback: Cow<'static, str> = match protocol {
        BrowseProtocol::Webdav => Cow::Borrowed("/"),
        BrowseProtocol::Dlna => Cow::Borrowed("0"),
        BrowseProtocol::Smb => Cow::Borrowed("/"),
    };

    let value = path.unwrap_or_else(|| fallback.to_string());
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}
