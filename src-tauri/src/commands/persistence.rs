use crate::store::installation_store::{DailyActionResult, InstallationState};
use crate::store::play_history::PlayHistoryEntry;
use crate::store::ui_state_store::UiState;
use crate::{mpv_command_checked, mpv_set_option_string_checked, with_mpv, AppState};
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
use std::process::Command;
use tauri::Manager;

const LOG_LEVEL_SETTING_LABEL: &str = "SOIA_LOG_LEVEL";
const YTDL_PATH_SETTING_LABEL: &str = "SOIA_YTDL_PATH";
const DEFAULT_LOG_LEVEL: &str = "Info";
const DEFAULT_SOIA_BUNDLE_IDENTIFIER: &str = "com.soia.player";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StreamProxySettingsState {
    parallel_download_enabled: bool,
}

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
pub(crate) fn stage_play_history_entry(
    state: tauri::State<'_, AppState>,
    entry: PlayHistoryEntry,
) -> Result<(), String> {
    crate::stage_pending_play_history_entry(&state, entry)
}

#[tauri::command]
pub(crate) fn clear_staged_play_history_entry(
    state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<(), String> {
    crate::clear_pending_play_history_entry(&state, path)
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
pub(crate) fn factory_reset(app: tauri::AppHandle) -> Result<(), String> {
    crate::store::installation_store::factory_reset(&app)
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
        .filter(|value| is_existing_ytdl_path(value))
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

fn persisted_ytdl_cookies_from_browser(app: &tauri::AppHandle) -> Option<String> {
    crate::store::ui_state_store::load_ytdl_cookies_from_browser(app)
}

fn default_log_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path()
        .app_log_dir()
        .ok()
        .map(|dir| dir.join("soia.log"))
}

fn default_ytdl_path() -> Option<String> {
    let candidate = PathBuf::from("/opt/homebrew/bin/yt-dlp");
    is_usable_ytdl_file(&candidate).then(|| candidate.to_string_lossy().to_string())
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
        None => {
            if let Some(value) = persisted_ytdl_path(app) {
                return configured_ytdl_path(Some(value));
            }
            configured_ytdl_path(std::env::var("SOIA_YTDL_PATH").ok()).or_else(default_ytdl_path)
        }
    }
}

fn resolve_current_ytdl_cookies_from_browser(
    app: &tauri::AppHandle,
    configured: Option<String>,
) -> Option<String> {
    match configured {
        Some(value) => {
            let value = value.trim().to_string();
            if value.is_empty() || value == "Off" { None } else { Some(value) }
        }
        None => persisted_ytdl_cookies_from_browser(app),
    }
}

fn resolve_current_ytdlp_settings(
    app: &tauri::AppHandle,
    configured_path: Option<String>,
    configured_cookies_from_browser: Option<String>,
    configured_max_height: Option<u32>,
) -> crate::mpv::YtdlpSettings {
    let max_height = configured_max_height.unwrap_or_else(|| {
        crate::store::ui_state_store::load_ytdl_max_height(app)
    });
    crate::mpv::YtdlpSettings::new(
        resolve_current_ytdl_path(app, configured_path),
        resolve_current_ytdl_cookies_from_browser(app, configured_cookies_from_browser),
        crate::mpv::YtdlpFormatSettings { max_height },
    )
}

fn is_existing_ytdl_path(path: &str) -> bool {
    let path = PathBuf::from(path);
    is_usable_ytdl_file(&path)
}

fn is_usable_ytdl_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let Ok(metadata) = std::fs::metadata(path) else {
            return false;
        };
        return metadata.permissions().mode() & 0o111 != 0;
    }

    #[cfg(not(unix))]
    {
        true
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
    ytdl_cookies_from_browser: Option<String>,
    ytdl_max_height: Option<u32>,
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

    for extension in crate::media_extensions::media_association_extensions() {
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

// The `glsl-shaders` list has two independent owners: the manual shader picker
// in Settings and the one-click "AI Upscaling" control. We keep each layer's
// shaders here and always rebuild the combined list (upscale first, then the
// user's manual shaders) so neither owner clobbers the other.
struct GlslLayers {
    manual: Vec<String>,
    upscale: Vec<String>,
    grade: Vec<String>,
    sharpen: Vec<String>,
    grain: Vec<String>,
}

static GLSL_LAYERS: std::sync::Mutex<GlslLayers> = std::sync::Mutex::new(GlslLayers {
    manual: Vec::new(),
    upscale: Vec::new(),
    grade: Vec::new(),
    sharpen: Vec::new(),
    grain: Vec::new(),
});

fn rebuild_combined_glsl(mpv_guard: &crate::mpv::MpvHandle) -> Result<(), String> {
    let layers = GLSL_LAYERS.lock().map_err(|_| "glsl layer lock poisoned".to_string())?;
    // Order: upscale -> grade (colour) -> sharpen -> grain (final) -> manual.
    let mut combined = layers.upscale.clone();
    combined.extend(layers.grade.iter().cloned());
    combined.extend(layers.sharpen.iter().cloned());
    combined.extend(layers.grain.iter().cloned());
    combined.extend(layers.manual.iter().cloned());
    apply_runtime_glsl_shaders(mpv_guard, &combined)
}

// A parametric unsharp-mask shader (13-tap disk blur on MAIN/RGB) with the
// amount and radius baked in as constants. Radius is in source pixels and is
// unbounded — unlike the `unsharp` video filter (matrix capped at 13), so a
// large radius yields the broad local-contrast / "HDR" look. Runs on the GPU,
// so there is no hardware-decode copy-back penalty.
fn sharpen_shader_source(amount: f64, radius: f64) -> String {
    format!(
        "//!HOOK MAIN\n\
//!BIND HOOKED\n\
//!DESC Lumo USM sharpen (amount={amount:.3} radius={radius:.2})\n\
vec4 hook() {{\n\
    vec4 col = HOOKED_tex(HOOKED_pos);\n\
    vec2 r = HOOKED_pt * {radius:.3};\n\
    vec3 blur = col.rgb * 0.20;\n\
    blur += ( HOOKED_tex(HOOKED_pos + vec2( 0.500, 0.000) * r).rgb\n\
            + HOOKED_tex(HOOKED_pos + vec2( 0.250, 0.433) * r).rgb\n\
            + HOOKED_tex(HOOKED_pos + vec2(-0.250, 0.433) * r).rgb\n\
            + HOOKED_tex(HOOKED_pos + vec2(-0.500, 0.000) * r).rgb\n\
            + HOOKED_tex(HOOKED_pos + vec2(-0.250,-0.433) * r).rgb\n\
            + HOOKED_tex(HOOKED_pos + vec2( 0.250,-0.433) * r).rgb ) * 0.08333;\n\
    blur += ( HOOKED_tex(HOOKED_pos + vec2( 1.000, 0.000) * r).rgb\n\
            + HOOKED_tex(HOOKED_pos + vec2( 0.500, 0.866) * r).rgb\n\
            + HOOKED_tex(HOOKED_pos + vec2(-0.500, 0.866) * r).rgb\n\
            + HOOKED_tex(HOOKED_pos + vec2(-1.000, 0.000) * r).rgb\n\
            + HOOKED_tex(HOOKED_pos + vec2(-0.500,-0.866) * r).rgb\n\
            + HOOKED_tex(HOOKED_pos + vec2( 0.500,-0.866) * r).rgb ) * 0.05;\n\
    vec3 sharp = col.rgb + {amount:.3} * (col.rgb - blur);\n\
    return vec4(clamp(sharp, 0.0, 1.0), col.a);\n\
}}\n"
    )
}

// mpv caches user shaders by file path, so rewriting the same filename and
// re-adding it reuses the first compiled shader — later Amount/Radius changes
// would never take effect. Write a fresh, uniquely-named file each time (and
// sweep away the old ones) so every change is a genuinely new shader to mpv.
static SHARPEN_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn write_sharpen_shader(app: &tauri::AppHandle, amount: f64, radius: f64) -> Result<String, String> {
    let dir = app.path().app_cache_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;

    // Remove previously generated sharpen shaders so they don't accumulate.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("lumo_sharpen") && name.ends_with(".glsl") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    let seq = SHARPEN_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = dir.join(format!("lumo_sharpen_{seq}.glsl"));
    std::fs::write(&path, sharpen_shader_source(amount, radius)).map_err(|err| err.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

// A parametric colour-grade shader on MAIN/RGB. Inputs are -100..100; we bake
// derived constants: exposure (multiplicative stops), temperature (R/B balance),
// tint (green/magenta), and separate highlight/shadow tone lifts. Identity when
// all inputs are zero. GPU, so no decode copy-back.
fn color_grade_shader_source(
    exposure: f64,
    temperature: f64,
    tint: f64,
    highlights: f64,
    shadows: f64,
) -> String {
    let n = |v: f64| (v / 100.0).clamp(-1.0, 1.0);
    let expo = 2.0_f64.powf(n(exposure)); // 0.5x .. 2.0x
    let temp_r = 1.0 + n(temperature) * 0.25;
    let temp_b = 1.0 - n(temperature) * 0.25;
    let tint_g = 1.0 + n(tint) * 0.15;
    let hi = n(highlights) * 0.35;
    let sh = n(shadows) * 0.35;
    format!(
        "//!HOOK MAIN\n\
//!BIND HOOKED\n\
//!DESC Lumo colour grade\n\
vec4 hook() {{\n\
    vec4 col = HOOKED_tex(HOOKED_pos);\n\
    vec3 c = col.rgb;\n\
    c *= {expo:.5};\n\
    c.r *= {temp_r:.5};\n\
    c.b *= {temp_b:.5};\n\
    c.g *= {tint_g:.5};\n\
    float l = dot(c, vec3(0.299, 0.587, 0.114));\n\
    float sMask = 1.0 - smoothstep(0.0, 0.5, l);\n\
    float hMask = smoothstep(0.5, 1.0, l);\n\
    c += {sh:.5} * sMask;\n\
    c += {hi:.5} * hMask;\n\
    return vec4(clamp(c, 0.0, 1.0), col.a);\n\
}}\n"
    )
}

static GRADE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn write_color_grade_shader(
    app: &tauri::AppHandle,
    exposure: f64,
    temperature: f64,
    tint: f64,
    highlights: f64,
    shadows: f64,
) -> Result<String, String> {
    let dir = app.path().app_cache_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("lumo_grade") && name.ends_with(".glsl") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let seq = GRADE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = dir.join(format!("lumo_grade_{seq}.glsl"));
    std::fs::write(
        &path,
        color_grade_shader_source(exposure, temperature, tint, highlights, shadows),
    )
    .map_err(|err| err.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

// Animated, luminance-aware film grain on MAIN/RGB. The `random` uniform mpv
// provides changes every frame, so the grain shifts like real film; a midtone
// weight keeps it out of clipped highlights/shadows. amount 0..100 -> ~0..0.18.
fn grain_shader_source(amount: f64) -> String {
    let strength = (amount / 100.0).clamp(0.0, 1.0) * 0.18;
    format!(
        "//!HOOK MAIN\n\
//!BIND HOOKED\n\
//!DESC Lumo film grain (amount={amount:.1})\n\
float lumo_grain_hash(vec2 p) {{\n\
    p = fract(p * vec2(123.34, 456.21));\n\
    p += dot(p, p + 45.32);\n\
    return fract(p.x * p.y);\n\
}}\n\
vec4 hook() {{\n\
    vec4 col = HOOKED_tex(HOOKED_pos);\n\
    vec2 uv = HOOKED_pos * HOOKED_size + random * 1000.0;\n\
    float n = lumo_grain_hash(uv) - 0.5;\n\
    float l = dot(col.rgb, vec3(0.299, 0.587, 0.114));\n\
    float w = mix(0.35, 1.0, 1.0 - abs(l - 0.5) * 2.0);\n\
    col.rgb += n * {strength:.5} * w;\n\
    return vec4(clamp(col.rgb, 0.0, 1.0), col.a);\n\
}}\n"
    )
}

static GRAIN_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn write_grain_shader(app: &tauri::AppHandle, amount: f64) -> Result<String, String> {
    let dir = app.path().app_cache_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("lumo_grain") && name.ends_with(".glsl") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let seq = GRAIN_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = dir.join(format!("lumo_grain_{seq}.glsl"));
    std::fs::write(&path, grain_shader_source(amount)).map_err(|err| err.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

// Ordered bundled shader files (relative to the shaders dir) per mode.
fn upscale_shader_files(mode: &str) -> Vec<&'static str> {
    match mode {
        "anime" => vec![
            "anime/1_Clamp_Highlights.glsl",
            "anime/2_Restore_CNN_M.glsl",
            "anime/3_Upscale_CNN_x2_M.glsl",
        ],
        "live" => vec!["live/ravu-lite-ar-r3.glsl"],
        _ => Vec::new(),
    }
}

fn resolve_upscale_shaders(app: &tauri::AppHandle, mode: &str) -> Vec<String> {
    // Candidate roots under which the bundled shaders may live, most-likely
    // first. Tauri normally exposes them via the Resource base dir, but layout
    // differs across bundle types, so we also probe the executable's directory.
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        roots.push(res.clone());
        roots.push(res.join("resources"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
            roots.push(dir.join("resources"));
        }
    }

    let mut resolved = Vec::new();
    for tail in upscale_shader_files(mode) {
        for root in &roots {
            let path = root.join("shaders").join(tail);
            let path_str = path.to_string_lossy().into_owned();
            if is_existing_glsl_file(&path_str) {
                resolved.push(path_str);
                break;
            }
        }
    }
    resolved
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
    app: tauri::AppHandle,
    ytdl_path: Option<String>,
    ytdl_cookies_from_browser: Option<String>,
    ytdl_max_height: Option<u32>,
) -> Result<YtdlSettingsState, String> {
    let settings = resolve_current_ytdlp_settings(&app, ytdl_path, ytdl_cookies_from_browser, ytdl_max_height);

    crate::mpv::store_runtime_ytdlp_settings(settings.clone());

    Ok(YtdlSettingsState {
        ytdl_path: settings.binary.path,
        ytdl_cookies_from_browser: settings.cookies.browser,
        ytdl_max_height: Some(settings.format.max_height),
    })
}

#[tauri::command]
pub(crate) fn apply_proxy_settings(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    proxy_mode: Option<String>,
    proxy_address: Option<String>,
) -> Result<crate::network::proxy::ProxySettingsState, String> {
    let resolved = crate::network::proxy::resolve_settings(&app, proxy_mode, proxy_address)?;
    #[cfg(debug_assertions)]
    crate::network::proxy::store_runtime_settings(resolved.clone());
    with_mpv(&state, |mpv_guard| {
        crate::network::proxy::apply_to_mpv(mpv_guard, &resolved)?;
        Ok(())
    })?;
    Ok(resolved)
}

#[tauri::command]
pub(crate) fn apply_stream_proxy_settings(
    parallel_download_enabled: Option<bool>,
) -> Result<StreamProxySettingsState, String> {
    let enabled = parallel_download_enabled.unwrap_or(false);
    crate::mpv::set_parallel_range_enabled(enabled);
    Ok(StreamProxySettingsState {
        parallel_download_enabled: enabled,
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

    {
        let mut layers = GLSL_LAYERS
            .lock()
            .map_err(|_| "glsl layer lock poisoned".to_string())?;
        layers.manual = active.clone();
    }
    with_mpv(&state, |mpv_guard| rebuild_combined_glsl(mpv_guard))?;

    Ok(RenderingSettingsState {
        selected_shader_files: selected,
        active_shader_files: active,
    })
}

// One-click AI upscaling. Sets the bundled upscale-shader layer for the given
// mode ("anime" | "live" | "off"/anything else) and rebuilds the combined
// glsl-shaders list, preserving any manual shaders.
#[tauri::command]
pub(crate) fn apply_upscale_shaders(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mode: String,
) -> Result<usize, String> {
    let upscale = resolve_upscale_shaders(&app, &mode);
    let count = upscale.len();
    {
        let mut layers = GLSL_LAYERS
            .lock()
            .map_err(|_| "glsl layer lock poisoned".to_string())?;
        layers.upscale = upscale;
    }
    with_mpv(&state, |mpv_guard| rebuild_combined_glsl(mpv_guard))?;
    Ok(count)
}

// GPU sharpen. amount <= 0 clears it; otherwise a parametric unsharp shader is
// generated with the given amount/radius and loaded as the sharpen layer.
#[tauri::command]
pub(crate) fn apply_sharpen_shader(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    amount: f64,
    radius: f64,
) -> Result<(), String> {
    let sharpen = if amount > 0.0 {
        vec![write_sharpen_shader(&app, amount, radius)?]
    } else {
        Vec::new()
    };
    {
        let mut layers = GLSL_LAYERS
            .lock()
            .map_err(|_| "glsl layer lock poisoned".to_string())?;
        layers.sharpen = sharpen;
    }
    with_mpv(&state, |mpv_guard| rebuild_combined_glsl(mpv_guard))?;
    Ok(())
}

// GPU colour grade. All-zero inputs clear the grade; otherwise a parametric
// shader (exposure / temperature / tint / highlights / shadows) is generated and
// loaded as the grade layer.
#[tauri::command]
pub(crate) fn apply_color_grade_shader(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    exposure: f64,
    temperature: f64,
    tint: f64,
    highlights: f64,
    shadows: f64,
) -> Result<(), String> {
    let any = [exposure, temperature, tint, highlights, shadows]
        .iter()
        .any(|v| v.abs() > 0.0001);
    let grade = if any {
        vec![write_color_grade_shader(
            &app,
            exposure,
            temperature,
            tint,
            highlights,
            shadows,
        )?]
    } else {
        Vec::new()
    };
    {
        let mut layers = GLSL_LAYERS
            .lock()
            .map_err(|_| "glsl layer lock poisoned".to_string())?;
        layers.grade = grade;
    }
    with_mpv(&state, |mpv_guard| rebuild_combined_glsl(mpv_guard))?;
    Ok(())
}

// GPU film grain. amount <= 0 clears it.
#[tauri::command]
pub(crate) fn apply_grain_shader(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    amount: f64,
) -> Result<(), String> {
    let grain = if amount > 0.0 {
        vec![write_grain_shader(&app, amount)?]
    } else {
        Vec::new()
    };
    {
        let mut layers = GLSL_LAYERS
            .lock()
            .map_err(|_| "glsl layer lock poisoned".to_string())?;
        layers.grain = grain;
    }
    with_mpv(&state, |mpv_guard| rebuild_combined_glsl(mpv_guard))?;
    Ok(())
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

        for extension in crate::media_extensions::media_association_extensions() {
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
