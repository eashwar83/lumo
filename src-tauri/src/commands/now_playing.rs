use crate::store::storage_paths;
use crate::{build_artwork_file_name, with_mpv, with_now_playing_mut, AppState};

#[tauri::command]
pub(crate) fn set_now_playing_metadata(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    title: String,
    duration: Option<f64>,
    artwork_path: Option<String>,
) -> Result<(), String> {
    with_now_playing_mut(&state, |now_playing| {
        now_playing.title = Some(title);
        now_playing.duration = duration;
        now_playing.artwork_path = artwork_path;
    })?;
    crate::platform::apply_now_playing_info(&app, &state)
}

#[tauri::command]
pub(crate) fn set_now_playing_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    is_playing: bool,
    position: Option<f64>,
) -> Result<(), String> {
    with_now_playing_mut(&state, |now_playing| {
        now_playing.is_playing = is_playing;
        if let Some(value) = position {
            now_playing.position = value;
        }
    })?;
    crate::platform::apply_now_playing_status(&app, &state)
}

#[tauri::command]
pub(crate) fn clear_now_playing(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    with_now_playing_mut(&state, |now_playing| {
        *now_playing = Default::default();
    })?;
    crate::platform::clear_now_playing_cache(&app)?;
    crate::platform::clear_now_playing_info(&app)?;
    Ok(())
}

#[tauri::command]
pub(crate) fn capture_now_playing_artwork(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<Option<String>, String> {
    if url.is_empty() {
        return Ok(None);
    }

    let artwork_dir = storage_paths::thumbnails_dir(&app)?;
    let file_path = artwork_dir.join(build_artwork_file_name(&url));
    let file_path_str = file_path.to_string_lossy().to_string();

    let result = with_mpv(&state, |mpv_guard| {
        Ok(mpv_guard.command(&["screenshot-to-file", &file_path_str, "video"]))
    })?;
    if result != 0 {
        return Ok(None);
    }

    Ok(Some(file_path_str))
}
