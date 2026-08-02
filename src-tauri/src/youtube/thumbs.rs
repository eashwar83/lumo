//! Disk-cached YouTube thumbnails, served to the webview as data URLs
//! (repo convention — the webview performs no direct network I/O).

use base64::Engine as _;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const MAX_THUMB_BYTES: usize = 3 * 1024 * 1024;
const MAX_CACHED_THUMBS: usize = 800;
const ALLOWED_HOSTS: &[&str] = &[
    "i.ytimg.com",
    "yt3.ggpht.com",
    "yt3.googleusercontent.com",
];

#[tauri::command]
pub(crate) async fn youtube_thumbnail(
    app: AppHandle,
    url: String,
) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || fetch_thumbnail(&app, &url))
        .await
        .map_err(|error| format!("Thumbnail worker failed: {error}"))?
}

fn fetch_thumbnail(app: &AppHandle, url: &str) -> Result<Option<String>, String> {
    if !is_allowed_thumbnail_url(url) {
        return Ok(None);
    }

    let dir = thumbs_dir(app)?;
    let path = dir.join(format!("{}.img", hash_key(url)));
    if let Ok(bytes) = fs::read(&path) {
        return Ok(Some(to_data_url(&bytes)));
    }

    let client = super::innertube::blocking_client(app)?;
    // Cold-start connections occasionally fail on the first burst; one
    // quick retry keeps first-search rows from rendering iconless.
    let response = match client.get(url).send() {
        Ok(response) => response,
        Err(_) => {
            std::thread::sleep(std::time::Duration::from_millis(300));
            client
                .get(url)
                .send()
                .map_err(|error| format!("Thumbnail fetch failed: {error}"))?
        }
    };
    if !response.status().is_success() {
        return Ok(None);
    }
    let bytes = response
        .bytes()
        .map_err(|error| format!("Thumbnail read failed: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAX_THUMB_BYTES {
        return Ok(None);
    }

    prune_thumbs(&dir);
    let _ = fs::write(&path, &bytes);
    Ok(Some(to_data_url(&bytes)))
}

fn is_allowed_thumbnail_url(raw: &str) -> bool {
    let Ok(url) = url::Url::parse(raw) else {
        return false;
    };
    if url.scheme() != "https" {
        return false;
    }
    url.host_str()
        .is_some_and(|host| ALLOWED_HOSTS.iter().any(|allowed| host == *allowed))
}

fn thumbs_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("Cache dir unavailable: {error}"))?
        .join("yt_thumbs");
    fs::create_dir_all(&dir).map_err(|error| format!("Cache dir create failed: {error}"))?;
    Ok(dir)
}

fn hash_key(url: &str) -> String {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Drop the oldest cached files once the cache exceeds its cap.
fn prune_thumbs(dir: &PathBuf) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            meta.is_file()
                .then(|| (meta.modified().ok().unwrap_or(std::time::UNIX_EPOCH), entry.path()))
        })
        .collect();
    if files.len() < MAX_CACHED_THUMBS {
        return;
    }
    files.sort_by_key(|(modified, _)| *modified);
    let drop_count = files.len() + 1 - MAX_CACHED_THUMBS;
    for (_, path) in files.into_iter().take(drop_count) {
        let _ = fs::remove_file(path);
    }
}

fn to_data_url(bytes: &[u8]) -> String {
    let mime = if bytes.starts_with(&[0xff, 0xd8]) {
        "image/jpeg"
    } else if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/jpeg"
    };
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}
