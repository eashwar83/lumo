use super::key::normalize_file_path;

fn format_webdav_display_host(connection_id: &str) -> String {
    let trimmed = connection_id.trim();
    if trimmed.is_empty() {
        return "webdav".to_string();
    }
    trimmed
        .strip_prefix("webdav-")
        .filter(|value| !value.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

pub(super) fn display_path_for_webdav(connection_id: &str, file_path: &str) -> String {
    let host = format_webdav_display_host(connection_id);
    let normalized_path = normalize_file_path(file_path)
        .trim_start_matches('/')
        .to_string();
    if normalized_path.is_empty() {
        format!("webdav://{host}")
    } else {
        format!("webdav://{host}/{normalized_path}")
    }
}

pub(super) fn display_path_for_smb(connection_id: Option<&str>, file_path: Option<&str>) -> String {
    let host = connection_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.strip_prefix("smb-").unwrap_or(value))
        .unwrap_or("smb");
    let normalized_path = file_path
        .map(normalize_file_path)
        .unwrap_or_else(|| "/".to_string())
        .trim_start_matches('/')
        .to_string();
    if normalized_path.is_empty() {
        format!("smb://{host}")
    } else {
        format!("smb://{host}/{normalized_path}")
    }
}
