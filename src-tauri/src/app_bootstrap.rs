use crate::mpv::MpvHandle;
use crate::store::ui_state_store;
use crate::AppState;
#[cfg(debug_assertions)]
use log::info;
use log::warn;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use std::error::Error;
use std::ffi::c_void;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tokio::time::{interval, Duration};

const LOG_LEVEL_SETTING_LABEL: &str = "SOIA_LOG_LEVEL";
const YTDL_PATH_SETTING_LABEL: &str = "SOIA_YTDL_PATH";
const DEFAULT_LOG_LEVEL: &str = "Info";
const UPDATE_CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;

pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn Error>> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Failed to get main window"))?;

    let app_handle = app.handle().clone();
    let auth_token = crate::check_update::check_update(app_handle.clone());
    tauri::async_runtime::spawn(async move {
        let mut interval = interval(Duration::from_secs(UPDATE_CHECK_INTERVAL_SECS));
        interval.tick().await;
        loop {
            interval.tick().await;
            let _ = crate::check_update::check_update(app_handle.clone());
        }
    });

    let initial_scale_factor = window.scale_factor()?;
    let (render_target, display) = resolve_render_target(&window)?;

    let mpv_player_handle = MpvHandle::new(
        render_target,
        display,
        app.handle().clone(),
        auth_token,
    )
    .map_err(|e| Box::new(io::Error::other(e)) as Box<dyn Error>)?;

    app.manage(build_app_state(mpv_player_handle));
    app.manage(crate::platform::new_platform_state());

    configure_mpv_startup(app)?;
    start_event_listener(app)?;
    install_window_event_handlers(window, initial_scale_factor);

    crate::platform::setup(app)?;
    Ok(())
}

fn build_app_state(mpv_player_handle: MpvHandle) -> AppState {
    AppState {
        mpv_player: Arc::new(Mutex::new(mpv_player_handle)),
        now_playing: Mutex::new(Default::default()),
    }
}

fn start_event_listener(app: &tauri::App) -> Result<(), Box<dyn Error>> {
    let state: tauri::State<'_, AppState> = app.state::<AppState>();
    let mpv_guard = state
        .mpv_player
        .lock()
        .map_err(|e| io::Error::other(e.to_string()))?;
    mpv_guard.start_event_listener();
    Ok(())
}

#[cfg(target_os = "macos")]
fn should_skip_resize_for_native_pip(window: &tauri::WebviewWindow) -> bool {
    crate::platform::is_native_pip_enabled(&window.app_handle())
}

#[cfg(not(target_os = "macos"))]
fn should_skip_resize_for_native_pip(_window: &tauri::WebviewWindow) -> bool {
    false
}

fn apply_mpv_resize(
    _window: &tauri::WebviewWindow,
    state: &tauri::State<'_, AppState>,
    physical_width: u32,
    physical_height: u32,
    scale_factor: f64,
) {
    #[cfg(target_os = "macos")]
    {
        // macOS render target uses physical pixels directly.
        let _ = scale_factor;
    }

    if let Ok(mut mpv_guard) = state.mpv_player.lock() {
        mpv_guard.render_target_resize(
            #[cfg(target_os = "macos")]
            {
                physical_width.max(1)
            },
            #[cfg(target_os = "macos")]
            {
                physical_height.max(1)
            },
            #[cfg(not(target_os = "macos"))]
            {
                logical_size(physical_width, scale_factor)
            },
            #[cfg(not(target_os = "macos"))]
            {
                logical_size(physical_height, scale_factor)
            },
        );
        #[cfg(target_os = "macos")]
        {
            crate::platform::sync_mpv_metal_layer_geometry(
                _window,
                mpv_guard.soia_utils_ptr() as usize,
            );
        }
    };
}

fn install_window_event_handlers(window: tauri::WebviewWindow, initial_scale_factor: f64) {
    let window_clone = window.clone();
    let skip_next_resize = Arc::new(AtomicBool::new(false));
    let skip_next_resize_cl = skip_next_resize.clone();

    window.on_window_event(move |event| {
        let state: tauri::State<AppState> = window_clone.state();
        match event {
            tauri::WindowEvent::Resized(size) => {
                if should_skip_resize_for_native_pip(&window_clone) {
                    return;
                }
                if crate::platform::enforce_native_pip_aspect(
                    &window_clone,
                    size.width,
                    size.height,
                ) {
                    return;
                }
                if skip_next_resize_cl.swap(false, Ordering::SeqCst) {
                    return;
                }
                let scale = window_clone.scale_factor().unwrap_or(initial_scale_factor);
                apply_mpv_resize(&window_clone, &state, size.width, size.height, scale);
            }
            tauri::WindowEvent::ScaleFactorChanged {
                scale_factor: new_scale,
                new_inner_size,
                ..
            } => {
                if should_skip_resize_for_native_pip(&window_clone) {
                    return;
                }
                if crate::platform::enforce_native_pip_aspect(
                    &window_clone,
                    new_inner_size.width,
                    new_inner_size.height,
                ) {
                    return;
                }
                skip_next_resize_cl.store(true, Ordering::SeqCst);
                apply_mpv_resize(
                    &window_clone,
                    &state,
                    new_inner_size.width,
                    new_inner_size.height,
                    *new_scale,
                );
            }
            tauri::WindowEvent::CloseRequested { .. } => {
                if let Ok(mpv_guard) = state.mpv_player.lock() {
                    mpv_guard.terminate();
                }
                crate::platform::cleanup_on_window_close(&window_clone.app_handle());
            }
            _ => {}
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn logical_size(physical: u32, scale_factor: f64) -> u32 {
    ((physical as f64 / scale_factor).max(1.0)) as u32
}

fn configure_mpv_startup(app: &tauri::App) -> Result<(), Box<dyn Error>> {
    let state: tauri::State<'_, AppState> = app.state::<AppState>();
    let mpv_guard = state
        .mpv_player
        .lock()
        .map_err(|e| io::Error::other(e.to_string()))?;

    if let Some(log_path) = resolve_log_file_path(app) {
        rotate_log_file(&log_path);
        if let Some(parent) = log_path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }
        mpv_guard.set_option_string("log-file", log_path.to_string_lossy().as_ref());
    }

    let log_level = resolve_log_level(app);
    mpv_guard.set_option_string("msg-level", to_mpv_msg_level(&log_level));

    mpv_guard.set_option_string("ytdl", "yes");
    let ytdl_path = resolve_ytdl_path(app);
    if let Some(ytdl_path) = ytdl_path {
        #[cfg(debug_assertions)]
        info!("Using yt-dlp search path(s): {}", ytdl_path);

        let script_opts = format!("ytdl_hook-ytdl_path={ytdl_path}");
        let script_opts_result = mpv_guard.set_option_string("script-opts", &script_opts);
        if script_opts_result < 0 {
            let append_result = mpv_guard.set_option_string("script-opts-append", &script_opts);
            if append_result < 0 {
                let legacy_result = mpv_guard.set_option_string("ytdl-path", &ytdl_path);
                if legacy_result < 0 {
                    warn!(
                        "Failed to set ytdl path via script-opts ({script_opts_result}), \
                         script-opts-append ({append_result}), and legacy ytdl-path ({legacy_result})",
                    );
                }
            }
        }
    }
    mpv_guard.set_option_string(
        "ytdl-format",
        // "bestvideo[height<=1080]+bestaudio/best[height<=1080]",
        // "bv+ba/b",
        "bv[height<=1080]+ba/b",
    );
    // mpv_guard.set_option_string(
    //     "ytdl-raw-options",
    //     // "format-sort=+codec:avc:m4a"
    //     "format-sort=vcodec:h265+acodec:opus/best",
    // );

    // mpv_guard.set_option_string(
    //     "ytdl-raw-options",
    //     "write-auto-subs",
    // );

    mpv_guard.set_option_string("cache", "auto");
    mpv_guard.set_option_string("cache-pause", "yes");
    mpv_guard.set_option_string("keep-open", "yes");
    mpv_guard.set_option_string("demuxer-max-bytes", "100MiB");
    mpv_guard.set_option_string("demuxer-max-back-bytes", "20MiB");

    mpv_guard.set_option_string("hwdec", "auto");

    if let Ok(data_dir) = app.path().app_local_data_dir() {
        let screenshot_dir = data_dir.join("screenshots");
        let _ = fs::create_dir_all(&screenshot_dir);
        mpv_guard.set_option_string("screenshot-dir", screenshot_dir.to_string_lossy().as_ref());
    }

    Ok(())
}

fn load_setting_value(app: &tauri::AppHandle, label: &str) -> Option<String> {
    ui_state_store::load_setting_value(app, label)
        .ok()
        .flatten()
}

fn resolve_log_file_path(app: &tauri::App) -> Option<PathBuf> {
    app.path()
        .app_log_dir()
        .ok()
        .map(|dir| dir.join("soia.log"))
}

fn rotate_log_file(log_path: &PathBuf) {
    let metadata = match fs::metadata(log_path) {
        Ok(metadata) => metadata,
        Err(_) => return,
    };
    if !metadata.is_file() || metadata.len() == 0 {
        return;
    }

    let backup_path = log_path.with_file_name("soia.prev.log");
    if backup_path.exists() {
        if let Err(err) = fs::remove_file(&backup_path) {
            warn!(
                "Failed to remove old log backup {}: {}",
                backup_path.display(),
                err
            );
            return;
        }
    }

    if let Err(err) = fs::rename(log_path, &backup_path) {
        warn!("Failed to rotate log file {}: {}", log_path.display(), err);
    }
}

fn resolve_log_level(app: &tauri::App) -> String {
    load_setting_value(&app.handle(), LOG_LEVEL_SETTING_LABEL)
        .and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .and_then(|value| normalize_log_level(&value))
        .unwrap_or_else(|| DEFAULT_LOG_LEVEL.to_string())
}

fn normalize_log_level(level: &str) -> Option<String> {
    let trimmed = level.trim();
    if trimmed.is_empty() {
        return None;
    }

    let raw = trimmed
        .split_once('=')
        .map(|(_, suffix)| suffix.trim())
        .unwrap_or(trimmed);
    let token = raw.to_ascii_lowercase();
    let normalized = match token.as_str() {
        "error" | "err" => "Error",
        "warn" | "warning" => "Warn",
        "info" | "v" | "verbose" => "Info",
        "debug" => "Debug",
        "trace" => "Trace",
        _ => return None,
    };
    Some(normalized.to_string())
}

fn to_mpv_msg_level(level: &str) -> &'static str {
    match level {
        "Error" => "all=error",
        "Warn" => "all=warn",
        "Info" => "all=info",
        "Debug" => "all=debug",
        "Trace" => "all=trace",
        _ => "all=info",
    }
}

fn resolve_ytdl_path(app: &tauri::App) -> Option<String> {
    load_setting_value(&app.handle(), YTDL_PATH_SETTING_LABEL)
        .or_else(|| std::env::var("SOIA_YTDL_PATH").ok())
        .or_else(platform_default_ytdl_path)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(target_os = "macos")]
fn platform_default_ytdl_path() -> Option<String> {
    let candidate = PathBuf::from("/opt/homebrew/bin/yt-dlp");
    candidate
        .exists()
        .then(|| candidate.to_string_lossy().to_string())
}

#[cfg(not(target_os = "macos"))]
fn platform_default_ytdl_path() -> Option<String> {
    None
}

fn resolve_render_target(
    window: &tauri::WebviewWindow,
) -> Result<(*const c_void, Option<*const c_void>), Box<dyn Error>> {
    // Wayland "embedding" needs both wl_surface and the wl_display that created it.
    // raw-window-handle provides wl_display via the *display* handle, not the window handle.
    let wayland_display: Option<*const c_void> =
        window
            .display_handle()
            .ok()
            .and_then(|handle| match handle.as_raw() {
                RawDisplayHandle::Wayland(raw) => Some(raw.display.as_ptr() as *const c_void),
                _ => None,
            });

    let handle = window
        .window_handle()
        .map_err(|e| io::Error::other(e.to_string()))?;
    let (ptr, display) = match handle.as_raw() {
        RawWindowHandle::Win32(raw) => (raw.hwnd.get() as usize as *const c_void, None),
        RawWindowHandle::WinRt(raw) => (raw.core_window.as_ptr() as *const c_void, None),
        RawWindowHandle::Wayland(raw) => {
            let surface = raw.surface.as_ptr() as *const c_void;
            (surface, wayland_display)
        }
        RawWindowHandle::Xlib(raw) => (raw.window as usize as *const c_void, None),
        RawWindowHandle::Xcb(raw) => (raw.window.get() as usize as *const c_void, None),
        RawWindowHandle::AppKit(raw) => (raw.ns_view.as_ptr() as *const c_void, None),
        RawWindowHandle::UiKit(raw) => (raw.ui_view.as_ptr() as *const c_void, None),
        RawWindowHandle::AndroidNdk(raw) => (raw.a_native_window.as_ptr() as *const c_void, None),
        unsupported => {
            return Err(Box::new(io::Error::other(format!(
                "Unsupported raw window handle for mpv render target: {unsupported:?}"
            ))));
        }
    };
    if ptr.is_null() {
        return Err(Box::new(io::Error::other(
            "Resolved null render target pointer for mpv initialization",
        )));
    }
    Ok((ptr, display))
}
