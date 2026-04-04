use crate::store::installation_store::{DailyActionResult, InstallationState};
use crate::store::play_history::PlayHistoryEntry;
use crate::store::ui_state_store::UiState;
use crate::{mpv_command_checked, mpv_set_option_string_checked, with_mpv, AppState};
#[cfg(debug_assertions)]
use log::info;
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
use std::process::Command;
use tauri::Manager;

const LOG_LEVEL_SETTING_LABEL: &str = "SOIA_LOG_LEVEL";
const YTDL_PATH_SETTING_LABEL: &str = "SOIA_YTDL_PATH";
const DEFAULT_LOG_LEVEL: &str = "Info";
const DEFAULT_SOIA_BUNDLE_IDENTIFIER: &str = "com.soia.player";

#[tauri::command]
pub(crate) fn load_play_history(app: tauri::AppHandle) -> Result<Vec<PlayHistoryEntry>, String> {
    crate::store::play_history::load_play_history(&app)
}

#[tauri::command]
pub(crate) fn save_play_history(
    app: tauri::AppHandle,
    entries: Vec<PlayHistoryEntry>,
) -> Result<(), String> {
    crate::store::play_history::save_play_history(&app, entries)
}

#[tauri::command]
pub(crate) fn save_play_history_entry(
    app: tauri::AppHandle,
    entry: PlayHistoryEntry,
) -> Result<(), String> {
    crate::store::play_history::save_play_history_entry(&app, entry)
}

#[tauri::command]
pub(crate) fn get_installation_state(app: tauri::AppHandle) -> Result<InstallationState, String> {
    crate::store::installation_store::get_installation_state(&app)
}

#[tauri::command]
pub(crate) fn update_uuid_update_data(
    app: tauri::AppHandle,
    data: serde_json::Value,
) -> Result<(), String> {
    crate::store::installation_store::update_uuid_update_data(&app, data)
}

#[tauri::command]
pub(crate) fn mark_daily_signal(app: tauri::AppHandle) -> Result<DailyActionResult, String> {
    crate::store::installation_store::mark_daily_signal(&app)
}

#[tauri::command]
pub(crate) fn mark_daily_update_check(app: tauri::AppHandle) -> Result<DailyActionResult, String> {
    crate::store::installation_store::mark_daily_update_check(&app)
}

#[tauri::command]
pub(crate) fn load_ui_state(app: tauri::AppHandle) -> Result<UiState, String> {
    crate::store::ui_state_store::load_ui_state(&app)
}

#[tauri::command]
pub(crate) fn save_ui_state(app: tauri::AppHandle, state: UiState) -> Result<(), String> {
    crate::store::ui_state_store::save_ui_state(&app, state)
}

fn configured_log_level(configured_level: Option<String>) -> Option<String> {
    configured_level
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn configured_ytdl_path(configured_path: Option<String>) -> Option<String> {
    configured_path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

fn persisted_log_level(app: &tauri::AppHandle) -> Option<String> {
    crate::store::ui_state_store::load_setting_value(app, LOG_LEVEL_SETTING_LABEL)
        .ok()
        .flatten()
}

fn persisted_ytdl_path(app: &tauri::AppHandle) -> Option<String> {
    crate::store::ui_state_store::load_setting_value(app, YTDL_PATH_SETTING_LABEL)
        .ok()
        .flatten()
}

fn default_log_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path()
        .app_log_dir()
        .ok()
        .map(|dir| dir.join("soia.log"))
}

fn default_ytdl_path() -> Option<String> {
    let candidate = PathBuf::from("/opt/homebrew/bin/yt-dlp");
    candidate
        .exists()
        .then(|| candidate.to_string_lossy().to_string())
}

fn resolve_current_log_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    default_log_path(app)
}

fn resolve_current_log_level(app: &tauri::AppHandle, configured_level: Option<String>) -> String {
    configured_log_level(configured_level)
        .and_then(|value| normalize_log_level(&value))
        .or_else(|| persisted_log_level(app))
        .and_then(|value| normalize_log_level(&value))
        .unwrap_or_else(|| DEFAULT_LOG_LEVEL.to_string())
}

fn resolve_current_ytdl_path(
    app: &tauri::AppHandle,
    configured_path: Option<String>,
) -> Option<String> {
    match configured_path {
        Some(value) => configured_ytdl_path(Some(value)),
        None => persisted_ytdl_path(app)
            .or_else(|| configured_ytdl_path(std::env::var("SOIA_YTDL_PATH").ok()))
            .or_else(default_ytdl_path),
    }
}

fn ensure_log_parent_dir(log_path: &Path) -> Result<(), String> {
    if let Some(parent) = log_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!("Failed to create log directory {}: {}", parent.display(), e)
            })?;
        }
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn open_directory(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer");

    let status = command
        .arg(path)
        .status()
        .map_err(|e| format!("Failed to open directory {}: {}", path.display(), e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Failed to open directory {}: exit code {:?}",
            path.display(),
            status.code()
        ))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn open_directory(_path: &Path) -> Result<(), String> {
    Err("Opening directories is unsupported on this platform".to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoggingSettingsState {
    log_path: Option<String>,
    log_level: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct YtdlSettingsState {
    ytdl_path: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RenderingSettingsState {
    selected_shader_files: Vec<String>,
    active_shader_files: Vec<String>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaAssociationStatus {
    supported: bool,
    target_bundle_id: String,
    is_default_for_all: bool,
    missing_extensions: Vec<String>,
    checked_extensions: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaAssociationApplyResult {
    success: bool,
    failed_extensions: Vec<String>,
    status: MediaAssociationStatus,
}

#[cfg(target_os = "macos")]
fn extension_to_content_type(
    extension: &str,
) -> Option<objc2_core_foundation::CFRetained<objc2_core_foundation::CFString>> {
    use core::ptr::NonNull;
    use objc2_core_foundation::{CFRetained, CFString};

    #[link(name = "CoreServices", kind = "framework")]
    extern "C-unwind" {
        static kUTTagClassFilenameExtension: &'static CFString;
        fn UTTypeCreatePreferredIdentifierForTag(
            inTagClass: Option<&CFString>,
            inTag: Option<&CFString>,
            inConformingToUTI: Option<&CFString>,
        ) -> Option<NonNull<CFString>>;
    }

    let ext = CFString::from_str(extension);
    let tag_class = unsafe { kUTTagClassFilenameExtension };
    let content_type =
        unsafe { UTTypeCreatePreferredIdentifierForTag(Some(tag_class), Some(&ext), None) }?;
    Some(unsafe { CFRetained::from_raw(content_type) })
}

#[cfg(target_os = "macos")]
fn copy_default_role_handler_bundle_id(
    content_type: &objc2_core_foundation::CFString,
    role: u32,
) -> Option<String> {
    use core::ptr::NonNull;
    use objc2_core_foundation::{CFRetained, CFString};

    #[link(name = "CoreServices", kind = "framework")]
    extern "C-unwind" {
        fn LSCopyDefaultRoleHandlerForContentType(
            inContentType: Option<&CFString>,
            inRole: u32,
        ) -> Option<NonNull<CFString>>;
    }

    let handler_bundle_id =
        unsafe { LSCopyDefaultRoleHandlerForContentType(Some(content_type), role) }?;
    let retained = unsafe { CFRetained::from_raw(handler_bundle_id) };
    Some(retained.to_string())
}

#[cfg(target_os = "macos")]
fn set_default_role_handler_bundle_id(
    content_type: &objc2_core_foundation::CFString,
    bundle_id: &objc2_core_foundation::CFString,
    role: u32,
) -> bool {
    use objc2_core_foundation::CFString;

    #[link(name = "CoreServices", kind = "framework")]
    extern "C-unwind" {
        fn LSSetDefaultRoleHandlerForContentType(
            inContentType: Option<&CFString>,
            inRole: u32,
            inHandlerBundleID: Option<&CFString>,
        ) -> i32;
    }

    let status =
        unsafe { LSSetDefaultRoleHandlerForContentType(Some(content_type), role, Some(bundle_id)) };
    status == 0
}

#[cfg(target_os = "macos")]
fn build_media_association_status(target_bundle_id: &str) -> MediaAssociationStatus {
    const LS_ROLES_ALL: u32 = u32::MAX;
    const LS_ROLES_VIEWER: u32 = 0x00000002;

    let target_bundle_id = target_bundle_id.to_string();
    let target_normalized = target_bundle_id.to_ascii_lowercase();
    let mut missing_extensions = Vec::new();
    let mut checked_extensions = Vec::new();

    for extension in crate::media_extensions::all() {
        let extension = extension.as_str();
        let Some(content_type) = extension_to_content_type(extension) else {
            missing_extensions.push(extension.to_string());
            continue;
        };
        checked_extensions.push(extension.to_string());
        let current_handler = copy_default_role_handler_bundle_id(&content_type, LS_ROLES_ALL)
            .or_else(|| copy_default_role_handler_bundle_id(&content_type, LS_ROLES_VIEWER))
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if current_handler != target_normalized {
            missing_extensions.push(extension.to_string());
        }
    }

    MediaAssociationStatus {
        supported: true,
        target_bundle_id,
        is_default_for_all: !checked_extensions.is_empty() && missing_extensions.is_empty(),
        missing_extensions,
        checked_extensions,
    }
}

#[cfg(not(target_os = "macos"))]
fn build_media_association_status(target_bundle_id: &str) -> MediaAssociationStatus {
    MediaAssociationStatus {
        supported: false,
        target_bundle_id: target_bundle_id.to_string(),
        is_default_for_all: false,
        missing_extensions: Vec::new(),
        checked_extensions: Vec::new(),
    }
}

fn resolve_target_bundle_id(app: &tauri::AppHandle) -> String {
    let configured = app.config().identifier.trim();
    if configured.is_empty() {
        DEFAULT_SOIA_BUNDLE_IDENTIFIER.to_string()
    } else {
        configured.to_string()
    }
}

fn apply_ytdl_path(mpv_guard: &crate::mpv::MpvHandle, ytdl_path: &str) -> Result<(), String> {
    mpv_set_option_string_checked(mpv_guard, "ytdl", "yes")?;

    #[cfg(debug_assertions)]
    info!("Using yt-dlp search path(s): {}", ytdl_path);
    let script_opts = format!("ytdl_hook-ytdl_path={ytdl_path}");
    let script_opts_result = mpv_guard.set_option_string("script-opts", &script_opts);
    if script_opts_result >= 0 {
        return Ok(());
    }

    let append_result = mpv_guard.set_option_string("script-opts-append", &script_opts);
    if append_result >= 0 {
        return Ok(());
    }

    let legacy_result = mpv_guard.set_option_string("ytdl-path", ytdl_path);
    if legacy_result >= 0 {
        return Ok(());
    }

    Err(format!(
        "Failed to set ytdl path via script-opts ({script_opts_result}), \
         script-opts-append ({append_result}), and legacy ytdl-path ({legacy_result})"
    ))
}

fn clear_ytdl_path(mpv_guard: &crate::mpv::MpvHandle) -> Result<(), String> {
    mpv_set_option_string_checked(mpv_guard, "ytdl", "yes")?;
    let script_opts = mpv_guard.set_option_string("script-opts", "ytdl_hook-ytdl_path=");
    let clear_result = mpv_guard.set_option_string("ytdl-path", "");
    if clear_result >= 0 {
        return Ok(());
    }

    let fallback_result = mpv_guard.set_option_string("ytdl-path", "yt-dlp");
    if fallback_result >= 0 {
        return Ok(());
    }

    Err(format!(
        "Failed to clear ytdl path; script-opts result ({script_opts}), ytdl-path empty ({clear_result}), ytdl-path fallback ({fallback_result})"
    ))
}

fn normalized_shader_file_list(input: Vec<String>) -> Vec<String> {
    input
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .filter(|value| {
            value
                .rsplit('.')
                .next()
                .map(|suffix| suffix.eq_ignore_ascii_case("glsl"))
                .unwrap_or(false)
        })
        .fold(Vec::new(), |mut acc, path| {
            if !acc.contains(&path) {
                acc.push(path);
            }
            acc
        })
}

fn collect_glsl_files_from_dir(dir: &Path, output: &mut Vec<String>) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let is_glsl = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("glsl"))
                .unwrap_or(false);
            if !is_glsl {
                continue;
            }
            output.push(path.to_string_lossy().into_owned());
        }
    }
}

fn align_active_shaders(selected: &[String], active: Vec<String>) -> Vec<String> {
    active
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .filter(|value| selected.contains(value))
        .fold(Vec::new(), |mut acc, path| {
            if !acc.contains(&path) {
                acc.push(path);
            }
            acc
        })
}

fn is_existing_glsl_file(path: &str) -> bool {
    let file_path = PathBuf::from(path);
    if !file_path.is_file() {
        return false;
    }
    file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("glsl"))
        .unwrap_or(false)
}

fn filter_existing_shader_files(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| is_existing_glsl_file(path))
        .cloned()
        .collect()
}

fn apply_runtime_glsl_shaders(
    mpv_guard: &crate::mpv::MpvHandle,
    shaders: &[String],
) -> Result<(), String> {
    mpv_command_checked(mpv_guard, &["change-list", "glsl-shaders", "clr", ""])?;
    for shader in shaders {
        mpv_command_checked(
            mpv_guard,
            &["change-list", "glsl-shaders", "append", shader],
        )?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn open_log_directory(app: tauri::AppHandle) -> Result<(), String> {
    let log_path = resolve_current_log_path(&app)
        .ok_or_else(|| "Current log path is unavailable".to_string())?;
    let directory = log_path.parent().map(PathBuf::from).unwrap_or(log_path);
    open_directory(&directory)
}

#[tauri::command]
pub(crate) fn apply_logging_settings(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    log_level: Option<String>,
) -> Result<LoggingSettingsState, String> {
    let resolved_log_path = resolve_current_log_path(&app);
    let resolved_log_level = resolve_current_log_level(&app, log_level);
    let mpv_log_level = to_mpv_msg_level(&resolved_log_level);

    if let Some(path) = resolved_log_path.as_ref() {
        ensure_log_parent_dir(path)?;
    }

    with_mpv(&state, |mpv_guard| {
        if let Some(path) = resolved_log_path.as_ref() {
            mpv_set_option_string_checked(mpv_guard, "log-file", &path.to_string_lossy())?;
        }
        mpv_set_option_string_checked(mpv_guard, "msg-level", mpv_log_level)?;
        Ok(())
    })?;

    Ok(LoggingSettingsState {
        log_path: resolved_log_path.map(|path| path.to_string_lossy().into_owned()),
        log_level: resolved_log_level,
    })
}

#[tauri::command]
pub(crate) fn apply_ytdl_settings(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    ytdl_path: Option<String>,
) -> Result<YtdlSettingsState, String> {
    let resolved_ytdl_path = resolve_current_ytdl_path(&app, ytdl_path);

    with_mpv(&state, |mpv_guard| {
        if let Some(path) = resolved_ytdl_path.as_deref() {
            apply_ytdl_path(mpv_guard, path)?;
        } else {
            clear_ytdl_path(mpv_guard)?;
        }
        Ok(())
    })?;

    Ok(YtdlSettingsState {
        ytdl_path: resolved_ytdl_path,
    })
}

#[tauri::command]
pub(crate) fn apply_rendering_settings(
    state: tauri::State<'_, AppState>,
    selected_shader_files: Vec<String>,
    active_shader_files: Vec<String>,
) -> Result<RenderingSettingsState, String> {
    let selected = normalized_shader_file_list(selected_shader_files);
    let existing_selected = filter_existing_shader_files(&selected);
    let active = align_active_shaders(&existing_selected, active_shader_files);

    with_mpv(&state, |mpv_guard| {
        apply_runtime_glsl_shaders(mpv_guard, &active)?;
        Ok(())
    })?;

    Ok(RenderingSettingsState {
        selected_shader_files: selected,
        active_shader_files: active,
    })
}

#[tauri::command]
pub(crate) fn resolve_existing_shader_files(paths: Vec<String>) -> Result<Vec<String>, String> {
    let normalized = normalized_shader_file_list(paths);
    Ok(filter_existing_shader_files(&normalized))
}

#[tauri::command]
pub(crate) fn resolve_shader_candidates(paths: Vec<String>) -> Result<Vec<String>, String> {
    let mut resolved = Vec::new();
    for raw in paths {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let path = PathBuf::from(trimmed);
        if path.is_dir() {
            collect_glsl_files_from_dir(&path, &mut resolved);
            continue;
        }
        if path.is_file() {
            let is_glsl = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("glsl"))
                .unwrap_or(false);
            if is_glsl {
                resolved.push(path.to_string_lossy().into_owned());
            }
        }
    }
    resolved.sort();
    resolved.dedup();
    Ok(resolved)
}

#[tauri::command]
pub(crate) fn get_media_association_status(
    app: tauri::AppHandle,
) -> Result<MediaAssociationStatus, String> {
    let target_bundle_id = resolve_target_bundle_id(&app);
    Ok(build_media_association_status(&target_bundle_id))
}

#[tauri::command]
pub(crate) fn set_media_association_to_soia(
    app: tauri::AppHandle,
) -> Result<MediaAssociationApplyResult, String> {
    let target_bundle_id_string = resolve_target_bundle_id(&app);

    #[cfg(target_os = "macos")]
    {
        const LS_ROLES_ALL: u32 = u32::MAX;
        const LS_ROLES_VIEWER: u32 = 0x00000002;
        const LS_ROLES_EDITOR: u32 = 0x00000004;

        let target_bundle_id = objc2_core_foundation::CFString::from_str(&target_bundle_id_string);
        let mut failed_extensions = Vec::new();

        for extension in crate::media_extensions::all() {
            let extension = extension.as_str();
            let Some(content_type) = extension_to_content_type(extension) else {
                failed_extensions.push(extension.to_string());
                continue;
            };
            let set_all =
                set_default_role_handler_bundle_id(&content_type, &target_bundle_id, LS_ROLES_ALL);
            let set_viewer = set_default_role_handler_bundle_id(
                &content_type,
                &target_bundle_id,
                LS_ROLES_VIEWER,
            );
            // Some media apps register as editors as well; set it for compatibility.
            let _ = set_default_role_handler_bundle_id(
                &content_type,
                &target_bundle_id,
                LS_ROLES_EDITOR,
            );
            if !set_all || !set_viewer {
                failed_extensions.push(extension.to_string());
            }
        }

        let status = build_media_association_status(&target_bundle_id_string);
        return Ok(MediaAssociationApplyResult {
            success: failed_extensions.is_empty() && status.is_default_for_all,
            failed_extensions,
            status,
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        let status = build_media_association_status(&target_bundle_id_string);
        Ok(MediaAssociationApplyResult {
            success: false,
            failed_extensions: Vec::new(),
            status,
        })
    }
}
