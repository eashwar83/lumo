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
