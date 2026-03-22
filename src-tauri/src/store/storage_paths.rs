use std::fs;
use std::path::PathBuf;
use tauri::Manager;

pub fn app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
    Ok(data_dir)
}

pub fn media_db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app_data_dir(app)?;
    Ok(data_dir.join("media.db"))
}

pub fn thumbnails_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app_data_dir(app)?;
    let thumbnails_dir = data_dir.join("thumbnails");
    fs::create_dir_all(&thumbnails_dir).map_err(|e| e.to_string())?;
    Ok(thumbnails_dir)
}
