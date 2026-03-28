use log::{info, warn};
use std::time::Duration;

#[cfg(target_os = "macos")]
fn soia_ping_os_code() -> i32 {
    0
}

#[cfg(target_os = "windows")]
fn soia_ping_os_code() -> i32 {
    1
}

#[cfg(target_os = "linux")]
fn soia_ping_os_code() -> i32 {
    2
}

#[cfg(target_os = "android")]
fn soia_ping_os_code() -> i32 {
    3
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    target_os = "linux",
    target_os = "android"
)))]
fn soia_ping_os_code() -> i32 {
    10
}

fn spawn_daily_active_ping(app_handle: tauri::AppHandle) {
    let daily = match crate::store::installation_store::mark_daily_active(&app_handle) {
        Ok(result) => result,
        Err(err) => {
            warn!("mark_daily_active failed, skip daily active ping: {err}");
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
    let os = soia_ping_os_code();
    std::thread::spawn(move || {
        let separator = if base_url.contains('?') { '&' } else { '?' };
        let url = format!("{base_url}{separator}{version_query}&os={os}");

        let client = match reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(1))
            .timeout(Duration::from_secs(2))
            .build()
        {
            Ok(client) => client,
            Err(err) => {
                warn!("daily active ping client init failed: {err}");
                return;
            }
        };

        let response = match client.get(&url).send() {
            Ok(response) => response,
            Err(err) => {
                warn!("daily active ping request failed: {err}");
                return;
            }
        };

        if !response.status().is_success() {
            warn!(
                "daily active ping request returned non-success status: {}",
                response.status()
            );
            return;
        }

        if let Err(err) = crate::store::installation_store::mark_daily_active_reported(&app_handle)
        {
            warn!("mark_daily_active_reported failed after successful ping: {err}");
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
    spawn_daily_active_ping(app_handle.clone());
    run_daily_update_check(&app_handle);
}

#[cfg(desktop)]
async fn run_tauri_updater_check(app_handle: tauri::AppHandle) {
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
            info!("tauri updater found an available update");
        }
        Ok(None) => {
            info!("tauri updater: no update available");
        }
        Err(err) => {
            warn!("tauri updater check failed: {err}");
        }
    }
}
