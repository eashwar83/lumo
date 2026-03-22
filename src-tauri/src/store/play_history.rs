use crate::store::{json_io, playback_db, storage_paths};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayHistoryEntry {
    pub path: String,
    #[serde(default)]
    pub title: String,
    pub last_position: f64,
    #[serde(default)]
    pub duration: f64,
    pub last_played_at: i64,
    #[serde(default)]
    pub is_pinned: bool,
    #[serde(default)]
    pub external_audio_tracks: Vec<String>,
    #[serde(default)]
    pub external_sub_tracks: Vec<String>,
}

fn legacy_history_file_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = storage_paths::app_data_dir(app)?;
    Ok(data_dir.join("play_history.json"))
}

pub fn load_play_history(app: &tauri::AppHandle) -> Result<Vec<PlayHistoryEntry>, String> {
    let db_exists = playback_db::media_db_exists(app)?;
    if !db_exists {
        let path = legacy_history_file_path(app)?;
        if path.exists() {
            if let Ok(entries) = json_io::read_json::<Vec<PlayHistoryEntry>>(&path) {
                if !entries.is_empty() {
                    let _ = playback_db::save_play_history(app, entries);
                }
            }
        }
    }
    playback_db::load_play_history(app)
}

pub fn save_play_history(
    app: &tauri::AppHandle,
    entries: Vec<PlayHistoryEntry>,
) -> Result<(), String> {
    playback_db::save_play_history(app, entries)
}

pub fn save_play_history_entry(
    app: &tauri::AppHandle,
    entry: PlayHistoryEntry,
) -> Result<(), String> {
    playback_db::save_play_history_entry(app, entry)
}
