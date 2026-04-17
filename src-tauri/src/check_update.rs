use chrono::Utc;
use log::{info, warn};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const UPDATE_AVAILABLE_EVENT: &str = "soia-update-available";
static HAS_AVAILABLE_UPDATE: AtomicBool = AtomicBool::new(false);
const HOST_AUTH_CACHE_KEY: &str = "host_auth_v1";
const HOST_AUTH_EXPIRY_SKEW_SECS: i64 = 30;
const HOST_AUTH_CONNECT_TIMEOUT_SECS: u64 = 3;
const HOST_AUTH_TIMEOUT_SECS: u64 = 7;
#[cfg(target_os = "windows")]
const WINDOWS_PORTABLE_MARKER_FILE: &str = "marker.dll";
#[cfg(target_os = "windows")]
const WINDOWS_UNINSTALLER_FILE: &str = "uninstall.exe";

#[derive(Clone, Debug)]
pub(crate) struct HostAuthToken {
    pub payload: String,
    pub signature_hex: String,
    pub expire_at: i64,
}

#[tauri::command]
pub(crate) fn has_available_update() -> bool {
    HAS_AVAILABLE_UPDATE.load(Ordering::Relaxed)
}

#[tauri::command]
pub(crate) fn should_use_embedded_update_install() -> bool {
    #[cfg(not(target_os = "windows"))]
    {
        true
    }

    #[cfg(target_os = "windows")]
    {
        is_windows_setup_install()
    }
}

#[cfg(target_os = "windows")]
fn is_windows_setup_install() -> bool {
    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.to_path_buf()))
    else {
        return true;
    };
    let marker_exists = exe_dir.join(WINDOWS_PORTABLE_MARKER_FILE).is_file();
    let uninstall_exists = exe_dir.join(WINDOWS_UNINSTALLER_FILE).is_file();
    let is_portable = marker_exists && !uninstall_exists;
    !is_portable
}

fn set_update_available_state(available: bool) {
    HAS_AVAILABLE_UPDATE.store(available, Ordering::Relaxed);
}

fn soia_os_code() -> String {
    cfg_if::cfg_if! {
        if #[cfg(all(not(target_os = "macos"), target_arch = "aarch64"))] {
            const _OFFSET: i32 = 200;
        } else {
            const _OFFSET: i32 = 100;
        }
    }

    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            let os = if cfg!(target_arch = "aarch64") { 100 } else { 200 };
        } else if #[cfg(target_os = "windows")] {
            let os = 1 + _OFFSET;
        } else if #[cfg(target_os = "linux")] {
            let os = 2 + _OFFSET;
        } else if #[cfg(target_os = "android")] {
            let os = 3 + _OFFSET;
        } else {
            let os = 99 + _OFFSET;
        }
    }

    let mmdd = Utc::now().format("%m%d").to_string();
    let bytes = mmdd.as_bytes();
    if bytes.len() != 4 {
        return format!("00{os:03}00");
    }

    format!(
        "{}{}{:03}{}{}",
        bytes[1] as char, bytes[2] as char, os, bytes[0] as char, bytes[3] as char
    )
}

fn parse_payload_expire_at(payload: &str) -> Option<i64> {
    payload.split(';').find_map(|part| {
        let raw = part.strip_prefix("exp=")?;
        raw.trim().parse::<i64>().ok()
    })
}

fn parse_host_auth_from_json(value: &serde_json::Value) -> Option<HostAuthToken> {
    let payload = value
        .get("auth_payload")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("payload").and_then(|v| v.as_str()))?
        .trim()
        .to_string();
    let signature_hex = value
        .get("auth_signature_hex")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("signature_hex").and_then(|v| v.as_str()))
        .or_else(|| value.get("signature").and_then(|v| v.as_str()))?
        .trim()
        .to_string();
    if payload.is_empty() || signature_hex.is_empty() {
        return None;
    }
    let expire_at = value
        .get("expire_at")
        .and_then(|v| v.as_i64())
        .or_else(|| parse_payload_expire_at(&payload))?;
    Some(HostAuthToken {
        payload,
        signature_hex,
        expire_at,
    })
}

fn read_cached_host_auth(app_handle: &tauri::AppHandle) -> Option<HostAuthToken> {
    let state = crate::store::installation_store::get_installation_state(app_handle).ok()?;
    let token = state
        .uuid_update_data
        .get(HOST_AUTH_CACHE_KEY)
        .and_then(parse_host_auth_from_json)?;
    let now = Utc::now().timestamp();
    (token.expire_at > now + HOST_AUTH_EXPIRY_SKEW_SECS).then_some(token)
}

fn persist_host_auth(app_handle: &tauri::AppHandle, token: &HostAuthToken) {
    let state = match crate::store::installation_store::get_installation_state(app_handle) {
        Ok(state) => state,
        Err(err) => {
            warn!("failed to read installation state for host auth cache: {err}");
            return;
        }
    };
    let mut obj = state
        .uuid_update_data
        .as_object()
        .cloned()
        .unwrap_or_default();
    obj.insert(
        HOST_AUTH_CACHE_KEY.to_string(),
        json!({
            "auth_payload": token.payload,
            "auth_signature_hex": token.signature_hex,
            "expire_at": token.expire_at
        }),
    );
    if let Err(err) = crate::store::installation_store::update_uuid_update_data(
        app_handle,
        serde_json::Value::Object(obj),
    ) {
        warn!("failed to persist host auth cache: {err}");
    }
}

fn request_host_auth_from_server(app_handle: &tauri::AppHandle) -> Option<HostAuthToken> {
    let state = crate::store::installation_store::get_installation_state(app_handle).ok()?;
    let base_url = option_env!("SOIA_API_URL").unwrap_or("").trim();
    if base_url.is_empty() {
        return None;
    }

    let version_query = format!("id={}&v={}", state.install_id, env!("CARGO_PKG_VERSION"));
    let os = soia_os_code();
    let separator = if base_url.contains('?') { '&' } else { '?' };
    let url = format!("{base_url}{separator}{version_query}&os={os}");

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(HOST_AUTH_CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(HOST_AUTH_TIMEOUT_SECS))
        .build()
        .ok()?;

    let response = client.get(&url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }

    let body = response.text().ok()?;
    let payload: serde_json::Value = serde_json::from_str(&body).ok()?;
    let token = parse_host_auth_from_json(&payload).or_else(|| {
        payload
            .get("data")
            .and_then(parse_host_auth_from_json)
            .or_else(|| payload.get("result").and_then(parse_host_auth_from_json))
    })?;

    persist_host_auth(app_handle, &token);
    if let Err(err) = crate::store::installation_store::mark_daily_signal_reported(app_handle) {
        warn!("mark_daily_signal_reported failed after successful host auth fetch: {err}");
    }
    Some(token)
}

pub(crate) fn run_daily_signal_ping(app_handle: &tauri::AppHandle) -> Option<HostAuthToken> {
    if let Some(token) = read_cached_host_auth(app_handle) {
        return Some(token);
    }

    let daily = match crate::store::installation_store::mark_daily_signal(app_handle) {
        Ok(result) => result,
        Err(err) => {
            warn!("mark_daily_signal failed, skip daily signal ping: {err}");
            return None;
        }
    };
    if !daily.should_run {
        return None;
    }

    request_host_auth_from_server(app_handle)
}

fn run_daily_update_check(app_handle: &tauri::AppHandle) {
    match crate::store::installation_store::mark_daily_update_check(app_handle) {
        Ok(result) if result.should_run => {
            #[cfg(desktop)]
            {
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    run_tauri_updater_check(app_handle).await;
                });
            }
        }
        Ok(_) => {}
        Err(err) => {
            warn!("mark_daily_update_check failed: {err}");
        }
    }
}

pub(crate) fn check_update(app_handle: tauri::AppHandle) -> Option<HostAuthToken> {
    set_update_available_state(false);
    let daily_signal_result = run_daily_signal_ping(&app_handle);
    #[cfg(target_os = "macos")]
    let host_auth = daily_signal_result;
    #[cfg(not(target_os = "macos"))]
    let host_auth = None;
    run_daily_update_check(&app_handle);
    host_auth
}

#[cfg(desktop)]
pub(crate) fn check_update_now(app_handle: tauri::AppHandle) {
    set_update_available_state(false);
    tauri::async_runtime::spawn(async move {
        run_tauri_updater_check(app_handle).await;
    });
}

#[cfg(desktop)]
async fn run_tauri_updater_check(app_handle: tauri::AppHandle) {
    use tauri::Emitter;
    use tauri_plugin_updater::UpdaterExt;

    let updater = match app_handle.updater_builder().build() {
        Ok(updater) => updater,
        Err(tauri_plugin_updater::Error::EmptyEndpoints) => {
            info!("tauri updater skipped: no endpoints configured");
            return;
        }
        Err(err) => {
            warn!("failed to build tauri updater: {err}");
            return;
        }
    };

    match updater.check().await {
        Ok(Some(_update)) => {
            set_update_available_state(true);
            let _ = app_handle.emit(UPDATE_AVAILABLE_EVENT, true);
            info!("tauri updater found an available update");
        }
        Ok(None) => {
            set_update_available_state(false);
            if let Err(err) =
                crate::store::installation_store::mark_daily_update_check_success(&app_handle)
            {
                warn!("failed to persist last_update_checked_date_utc: {err}");
            }
            let _ = app_handle.emit(UPDATE_AVAILABLE_EVENT, false);
            info!("tauri updater: no update available");
        }
        Err(err) => {
            warn!("tauri updater check failed: {err}");
        }
    }
}
