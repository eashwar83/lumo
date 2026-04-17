// Tauri imports
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

mod app_bootstrap;
mod check_update;
mod commands;
mod media_extensions;
mod mpv;
mod network;
mod platform;
use mpv::MpvHandle;
use tauri::{Emitter, Manager};
mod store;

pub struct AppState {
    pub mpv_player: Mutex<MpvHandle>,
    now_playing: Mutex<NowPlayingState>,
}

#[derive(Default)]
pub struct OpenFileState {
    pub pending_paths: Mutex<Vec<String>>,
}

#[derive(Clone, Default)]
struct NowPlayingState {
    title: Option<String>,
    duration: Option<f64>,
    position: f64,
    is_playing: bool,
    artwork_path: Option<String>,
}

type AppResult<T> = Result<T, String>;

fn init_logging() {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().filter_or("RUST_LOG", "info"));
    builder
        .filter_module("reqwest", log::LevelFilter::Warn)
        .filter_module("hyper", log::LevelFilter::Warn)
        .filter_module("hyper_util", log::LevelFilter::Warn)
        .filter_module("tauri_plugin_updater", log::LevelFilter::Warn);
    builder.format(|buf, record| {
        use std::io::Write;

        let time = chrono::Utc::now()
            .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).expect("valid UTC+08:00"))
            .format("%H:%M:%S%.3f");

        let level = match record.level() {
            log::Level::Error => "e",
            log::Level::Warn => "w",
            log::Level::Info => "i",
            log::Level::Debug => "d",
            log::Level::Trace => "t",
        };

        writeln!(buf, "{} [{}] {}", time, level, record.args())
    });
    let _ = builder.try_init();
}

fn json_value_to_string(value: serde_json::Value) -> AppResult<String> {
    match value {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Bool(b) => Ok(if b { "yes".into() } else { "no".into() }),
        _ => Err("Unsupported value type".into()),
    }
}

fn lock_mutex<T>(mutex: &Mutex<T>) -> AppResult<std::sync::MutexGuard<'_, T>> {
    mutex.lock().map_err(|e| e.to_string())
}

fn with_mpv<R>(
    state: &tauri::State<'_, AppState>,
    f: impl FnOnce(&MpvHandle) -> AppResult<R>,
) -> AppResult<R> {
    let mpv_guard = lock_mutex(&state.mpv_player)?;
    f(&mpv_guard)
}

fn with_now_playing_mut<R>(
    state: &tauri::State<'_, AppState>,
    f: impl FnOnce(&mut NowPlayingState) -> R,
) -> AppResult<R> {
    let mut now_playing = lock_mutex(&state.now_playing)?;
    Ok(f(&mut now_playing))
}

fn mpv_command_checked(mpv: &MpvHandle, args: &[&str]) -> AppResult<()> {
    let result_code = mpv.command(args);
    if result_code == 0 {
        Ok(())
    } else {
        Err(format!(
            "MPV command {:?} failed with error code: {}",
            args, result_code
        ))
    }
}

fn mpv_set_option_string_checked(mpv: &MpvHandle, name: &str, value: &str) -> AppResult<()> {
    let result_code = mpv.set_option_string(name, value);
    if result_code >= 0 {
        Ok(())
    } else {
        Err(format!(
            "MPV set_option_string({}, {}) failed with error code: {}",
            name, value, result_code
        ))
    }
}

fn build_artwork_file_name(url: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = hasher.finish();
    let url_path = PathBuf::from(url);
    let base_name = url_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("media");
    let safe_name: String = base_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{safe_name}_{hash:016x}.jpg")
}

fn build_load_file_command_args(url: &str, resume_position: f64) -> Vec<String> {
    if resume_position > 0.0 {
        vec![
            "loadfile".to_string(),
            url.to_string(),
            "replace".to_string(),
            "0".to_string(),
            format!("start={resume_position}"),
        ]
    } else {
        vec!["loadfile".to_string(), url.to_string()]
    }
}

fn normalize_open_media_path(path: &Path) -> Option<String> {
    if path.as_os_str().is_empty() || path.is_dir() {
        return None;
    }
    Some(path.to_string_lossy().into_owned())
}

fn collect_open_media_paths_from_args() -> Vec<String> {
    std::env::args_os()
        .skip(1)
        .filter_map(|arg| {
            if let Some(raw) = arg.to_str() {
                // macOS launches GUI apps with a process serial number arg.
                if raw.starts_with("-psn_") {
                    return None;
                }
            }
            let path = PathBuf::from(arg);
            normalize_open_media_path(&path)
        })
        .collect()
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn collect_open_media_paths_from_urls(urls: Vec<url::Url>) -> Vec<String> {
    urls.into_iter()
        .filter_map(|url| {
            if let Ok(path) = url.to_file_path() {
                return normalize_open_media_path(&path);
            }
            if url.scheme() != "file" {
                return None;
            }
            let decoded = percent_encoding::percent_decode_str(url.path())
                .decode_utf8()
                .ok()?;
            let candidate = PathBuf::from(decoded.as_ref());
            normalize_open_media_path(&candidate)
        })
        .collect()
}

fn queue_open_media_paths(app: &tauri::AppHandle, paths: Vec<String>, emit_event: bool) {
    if paths.is_empty() {
        return;
    }

    let mut deduped = Vec::new();
    for path in paths {
        if deduped.contains(&path) {
            continue;
        }
        deduped.push(path);
    }

    let mut queued_any = false;
    if let Some(open_state) = app.try_state::<OpenFileState>() {
        if let Ok(mut pending) = open_state.pending_paths.lock() {
            for path in deduped {
                if pending.contains(&path) {
                    continue;
                }
                pending.push(path);
                queued_any = true;
            }
        }
    }

    // Notify frontend to drain queue. If frontend isn't ready yet, paths stay queued.
    if emit_event && queued_any {
        let _ = app.emit("app-open-files", ());
    }
}

fn handle_run_event(app_handle: &tauri::AppHandle, event: tauri::RunEvent) {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    if let tauri::RunEvent::Opened { urls } = event {
        let paths = collect_open_media_paths_from_urls(urls);
        queue_open_media_paths(app_handle, paths, true);
    }

    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    let _ = (app_handle, event);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();

    let app = tauri::Builder::default()
        .manage(OpenFileState::default())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            let startup_paths = collect_open_media_paths_from_args();
            queue_open_media_paths(&app.handle(), startup_paths, false);
            app_bootstrap::setup(app)?;
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            commands::playback::mpv_run_command,
            commands::playback::mpv_set_option_string,
            commands::playback::load_file,
            commands::platform::pick_media_paths_native,
            commands::platform::pick_paths_native,
            commands::playback::consume_pending_open_files,
            commands::playback::cycle_pause,
            commands::playback::seek_video,
            commands::window::set_window_controls_visible,
            commands::window::apply_window_appearance,
            commands::platform::is_native_pip_enabled,
            commands::platform::set_native_pip_enabled,
            commands::playback::get_runtime_versions,
            commands::playback::get_media_file_size,
            commands::network::list_network_connections,
            commands::network::save_network_connection,
            commands::network::delete_network_connection,
            commands::network::discover_network_connections,
            commands::network::browse_network_connection,
            commands::network::load_network_file,
            commands::now_playing::set_now_playing_metadata,
            commands::now_playing::set_now_playing_status,
            commands::now_playing::clear_now_playing,
            commands::now_playing::capture_now_playing_artwork,
            commands::persistence::load_play_history,
            commands::persistence::save_play_history,
            commands::persistence::save_play_history_entry,
            commands::persistence::get_installation_state,
            commands::persistence::update_uuid_update_data,
            commands::persistence::factory_reset,
            commands::persistence::mark_daily_signal,
            commands::persistence::mark_daily_update_check,
            commands::persistence::load_ui_state,
            commands::persistence::save_ui_state,
            commands::persistence::open_log_directory,
            commands::persistence::apply_logging_settings,
            commands::persistence::apply_ytdl_settings,
            commands::persistence::apply_rendering_settings,
            commands::persistence::resolve_shader_candidates,
            commands::persistence::resolve_existing_shader_files,
            commands::persistence::get_media_association_status,
            commands::persistence::set_media_association_to_soia,
            check_update::has_available_update,
            check_update::should_use_embedded_update_install
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(handle_run_event);
}
