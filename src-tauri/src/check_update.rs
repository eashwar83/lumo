use chrono::Utc;
use log::{info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const UPDATE_AVAILABLE_EVENT: &str = "soia-update-available";
static HAS_AVAILABLE_UPDATE: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub(crate) fn has_available_update() -> bool {
    HAS_AVAILABLE_UPDATE.load(Ordering::Relaxed)
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

fn run_daily_signal_ping(app_handle: tauri::AppHandle) {
    let daily = match crate::store::installation_store::mark_daily_signal(&app_handle) {
        Ok(result) => result,
        Err(err) => {
            warn!("mark_daily_signal failed, skip daily signal ping: {err}");
            return;
        }
    };
    if !daily.should_run {
        return;
    }

    let version_query = daily.version.unwrap_or_default();
    if version_query.is_empty() {
        return;
    }

    let base_url = option_env!("SOIA_API_URL").unwrap_or("").trim();
    if base_url.is_empty() {
        return;
    }

    let base_url = base_url.to_string();
    std::thread::spawn(move || {
        let os = soia_os_code();
        let separator = if base_url.contains('?') { '&' } else { '?' };
        let url = format!("{base_url}{separator}{version_query}&os={os}");

        let client = match reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(10))
            .build()
        {
            Ok(client) => client,
            Err(_) => {
                return;
            }
        };

        let response = match client.get(&url).send() {
            Ok(response) => response,
            Err(_) => {
                return;
            }
        };

        if !response.status().is_success() {
            #[cfg(debug_assertions)]
            warn!(
                "daily signal ping request returned non-success status: {}",
                response.status()
            );
            return;
        }

        if let Err(err) = crate::store::installation_store::mark_daily_signal_reported(&app_handle)
        {
            warn!("mark_daily_signal_reported failed after successful ping: {err}");
        }
    });
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

pub(crate) fn check_update(app_handle: tauri::AppHandle) {
    set_update_available_state(false);
    run_daily_signal_ping(app_handle.clone());
    run_daily_update_check(&app_handle);
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
    use tauri_plugin_updater::UpdaterExt;
    use tauri::Emitter;

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
