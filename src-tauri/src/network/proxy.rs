use crate::mpv_set_option_string_checked;
#[cfg(debug_assertions)]
use crate::store::ui_state_store;
#[cfg(debug_assertions)]
use std::sync::{Mutex, OnceLock};

#[cfg(debug_assertions)]
const PROXY_MODE_SETTING_LABEL: &str = "SOIA_PROXY_MODE";
#[cfg(debug_assertions)]
const PROXY_ADDRESS_SETTING_LABEL: &str = "SOIA_PROXY_ADDRESS";

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProxySettingsState {
    pub proxy_mode: String,
    pub proxy_address: Option<String>,
    pub proxy_url: Option<String>,
}

#[cfg(debug_assertions)]
static RUNTIME_PROXY_SETTINGS: OnceLock<Mutex<Option<ProxySettingsState>>> = OnceLock::new();

#[cfg(debug_assertions)]
fn runtime_proxy_settings() -> &'static Mutex<Option<ProxySettingsState>> {
    RUNTIME_PROXY_SETTINGS.get_or_init(|| Mutex::new(None))
}

#[cfg(debug_assertions)]
fn load_runtime_settings() -> Option<ProxySettingsState> {
    runtime_proxy_settings()
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

#[cfg(debug_assertions)]
pub(crate) fn store_runtime_settings(settings: ProxySettingsState) {
    if let Ok(mut guard) = runtime_proxy_settings().lock() {
        *guard = Some(settings);
    }
}

#[cfg(debug_assertions)]
fn configured_proxy_mode(configured_mode: Option<String>) -> Option<String> {
    configured_mode
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .and_then(|value| normalize_proxy_mode(&value))
}

#[cfg(debug_assertions)]
fn configured_proxy_address(configured_address: Option<String>) -> Option<String> {
    configured_address
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(debug_assertions)]
fn normalize_proxy_mode(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "off" | "disable" | "disabled" | "no" | "none" => Some("Off".to_string()),
        "http" | "https" => Some("HTTP".to_string()),
        _ => None,
    }
}

#[cfg(debug_assertions)]
fn persisted_proxy_mode(app: &tauri::AppHandle) -> Option<String> {
    ui_state_store::load_setting_value(app, PROXY_MODE_SETTING_LABEL)
        .ok()
        .flatten()
}

#[cfg(debug_assertions)]
fn persisted_proxy_address(app: &tauri::AppHandle) -> Option<String> {
    ui_state_store::load_setting_value(app, PROXY_ADDRESS_SETTING_LABEL)
        .ok()
        .flatten()
}

#[cfg(debug_assertions)]
fn resolve_current_proxy_mode(app: &tauri::AppHandle, configured_mode: Option<String>) -> String {
    configured_proxy_mode(configured_mode)
        .or_else(|| persisted_proxy_mode(app).and_then(|value| configured_proxy_mode(Some(value))))
        .unwrap_or_else(|| "Off".to_string())
}

#[cfg(debug_assertions)]
fn resolve_current_proxy_address(
    app: &tauri::AppHandle,
    configured_address: Option<String>,
) -> Option<String> {
    match configured_address {
        Some(value) => configured_proxy_address(Some(value)),
        None => persisted_proxy_address(app).and_then(|value| configured_proxy_address(Some(value))),
    }
}

#[cfg(debug_assertions)]
fn normalize_proxy_url(mode: &str, address: Option<String>) -> Result<Option<String>, String> {
    if mode == "Off" {
        return Ok(None);
    }

    let address = address.ok_or_else(|| "Proxy server is required".to_string())?;
    let address = address.trim();
    if address.is_empty() {
        return Err("Proxy server is required".to_string());
    }

    let with_scheme = if address.contains("://") {
        address.to_string()
    } else {
        format!("http://{address}")
    };
    let parsed = url::Url::parse(&with_scheme)
        .map_err(|e| format!("Invalid proxy server {address}: {e}"))?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if scheme != "http" {
        return Err(format!("HTTP proxy does not support {scheme} URLs"));
    }
    if parsed.host_str().unwrap_or_default().trim().is_empty() {
        return Err("Proxy host is required".to_string());
    }

    Ok(Some(parsed.to_string()))
}

pub(crate) fn resolve_settings(
    app: &tauri::AppHandle,
    proxy_mode: Option<String>,
    proxy_address: Option<String>,
) -> Result<ProxySettingsState, String> {
    #[cfg(not(debug_assertions))]
    {
        let _ = (app, proxy_mode, proxy_address);
        return Ok(ProxySettingsState {
            proxy_mode: "Off".to_string(),
            proxy_address: None,
            proxy_url: None,
        });
    }

    #[cfg(debug_assertions)]
    {
        let resolved_mode = resolve_current_proxy_mode(app, proxy_mode);
        let resolved_address = resolve_current_proxy_address(app, proxy_address);
        let proxy_url = normalize_proxy_url(&resolved_mode, resolved_address.clone())?;
        Ok(ProxySettingsState {
            proxy_mode: resolved_mode,
            proxy_address: resolved_address,
            proxy_url,
        })
    }
}

pub(crate) fn apply_to_mpv(
    mpv_guard: &crate::mpv::MpvHandle,
    settings: &ProxySettingsState,
) -> Result<(), String> {
    let proxy = settings.proxy_url.as_deref().unwrap_or("no");
    mpv_set_option_string_checked(mpv_guard, "http-proxy", proxy)?;

    let ytdl_proxy = settings
        .proxy_url
        .as_deref()
        .map(|value| format!("proxy={value}"))
        .unwrap_or_default();
    mpv_set_option_string_checked(mpv_guard, "ytdl-raw-options", &ytdl_proxy)?;
    Ok(())
}

pub(crate) fn configure_client_builder(
    app: &tauri::AppHandle,
    builder: reqwest::ClientBuilder,
) -> Result<reqwest::ClientBuilder, String> {
    #[cfg(debug_assertions)]
    let settings = load_runtime_settings()
        .map(Ok)
        .unwrap_or_else(|| resolve_settings(app, None, None))?;
    #[cfg(not(debug_assertions))]
    let settings = resolve_settings(app, None, None)?;

    let Some(proxy_url) = settings.proxy_url else {
        return Ok(builder);
    };
    let proxy = reqwest::Proxy::all(&proxy_url).map_err(|e| e.to_string())?;
    Ok(builder.proxy(proxy))
}

pub(crate) fn configure_blocking_client_builder(
    app: &tauri::AppHandle,
    builder: reqwest::blocking::ClientBuilder,
) -> Result<reqwest::blocking::ClientBuilder, String> {
    #[cfg(debug_assertions)]
    let settings = load_runtime_settings()
        .map(Ok)
        .unwrap_or_else(|| resolve_settings(app, None, None))?;
    #[cfg(not(debug_assertions))]
    let settings = resolve_settings(app, None, None)?;

    let Some(proxy_url) = settings.proxy_url else {
        return Ok(builder);
    };
    let proxy = reqwest::Proxy::all(&proxy_url).map_err(|e| e.to_string())?;
    Ok(builder.proxy(proxy))
}
