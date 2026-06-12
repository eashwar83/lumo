#[tauri::command]
pub(crate) fn set_window_controls_visible(
    window: tauri::Window,
    visible: bool,
) -> Result<(), String> {
    crate::platform::set_window_controls_visible(window, visible)
}

#[tauri::command]
pub(crate) fn apply_window_appearance(
    window: tauri::Window,
    compact_mode: bool,
    corner_radius: Option<f64>,
) -> Result<(), String> {
    crate::platform::apply_window_appearance(window, compact_mode, corner_radius)
}

#[tauri::command]
pub(crate) fn set_window_vibrancy_visible(
    window: tauri::Window,
    visible: bool,
) -> Result<(), String> {
    crate::platform::set_window_vibrancy_visible(window, visible)
}

#[tauri::command]
pub(crate) fn sync_mpv_render_target(
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    crate::app_bootstrap::sync_mpv_render_target_to_window(&window)
}
