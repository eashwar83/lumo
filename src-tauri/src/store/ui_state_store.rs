use crate::store::{json_io, playback_store, storage_paths};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const STATE_FILE_NAME: &str = "state.json";
const LEGACY_STATE_FILE_NAME: &str = "ui_state.json";

fn ui_state_cache() -> &'static Mutex<Option<UiState>> {
    static UI_STATE_CACHE: OnceLock<Mutex<Option<UiState>>> = OnceLock::new();
    UI_STATE_CACHE.get_or_init(|| Mutex::new(None))
}

fn cached_ui_state() -> Option<UiState> {
    ui_state_cache().lock().ok().and_then(|cache| cache.clone())
}

fn store_ui_state_cache(state: &UiState) {
    if let Ok(mut cache) = ui_state_cache().lock() {
        *cache = Some(strip_playlist(state));
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    #[serde(default)]
    pub active_panel: Option<String>,
    #[serde(default)]
    pub network: Option<NetworkState>,
    #[serde(default)]
    pub settings: Option<SettingsState>,
    #[serde(default)]
    pub playlist: Option<Vec<PlaylistEntry>>,
    #[serde(default)]
    pub playlists: Option<Vec<Playlist>>,
    #[serde(default)]
    pub active_playlist_id: Option<String>,
    #[serde(default)]
    pub playlist_loop_mode: Option<String>,
    #[serde(default)]
    pub playlist_sort_mode: Option<String>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            active_panel: Some("home".into()),
            network: None,
            settings: None,
            playlist: None,
            playlists: None,
            active_playlist_id: None,
            playlist_loop_mode: None,
            playlist_sort_mode: None,
        }
    }
}

impl UiState {
    fn merge(self, incoming: UiState) -> UiState {
        UiState {
            active_panel: incoming.active_panel.or(self.active_panel),
            network: incoming.network.or(self.network),
            settings: incoming.settings.or(self.settings),
            playlist: incoming.playlist.or(self.playlist),
            playlists: incoming.playlists.or(self.playlists),
            active_playlist_id: incoming.active_playlist_id.or(self.active_playlist_id),
            playlist_loop_mode: incoming.playlist_loop_mode.or(self.playlist_loop_mode),
            playlist_sort_mode: incoming.playlist_sort_mode.or(self.playlist_sort_mode),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub entries: Vec<PlaylistEntry>,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntry {
    pub path: String,
    pub added_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct NetworkState {
    #[serde(default)]
    pub selected_connection: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettingsState {
    #[serde(default)]
    pub groups: Option<Vec<SettingsGroup>>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettingsGroup {
    pub title: String,
    pub items: Vec<SettingsItem>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettingsItem {
    pub label: String,
    pub value: String,
}

fn ui_state_file_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = storage_paths::app_data_dir(app)?;
    Ok(data_dir.join(STATE_FILE_NAME))
}

fn legacy_ui_state_file_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = storage_paths::app_data_dir(app)?;
    Ok(data_dir.join(LEGACY_STATE_FILE_NAME))
}

fn strip_playlist(state: &UiState) -> UiState {
    let mut stripped = state.clone();
    stripped.playlist = None;
    stripped
}

fn load_state_from_disk(path: &Path, legacy_path: &Path) -> Result<UiState, String> {
    if let Some(state) = cached_ui_state() {
        return Ok(state);
    }

    if path.exists() {
        info!("Loading UI state from {}", path.display());
        let state: UiState = json_io::read_json_or_default(path)?;
        store_ui_state_cache(&state);
        return Ok(state);
    }

    if legacy_path.exists() {
        info!("Loading UI state from {}", legacy_path.display());
        let legacy_state: UiState = json_io::read_json_or_default(legacy_path)?;
        json_io::write_json(path, &strip_playlist(&legacy_state))?;
        store_ui_state_cache(&legacy_state);
        return Ok(legacy_state);
    }

    let default_state = UiState::default();
    store_ui_state_cache(&default_state);
    Ok(default_state)
}

pub fn load_ui_state(app: &tauri::AppHandle) -> Result<UiState, String> {
    let path = ui_state_file_path(app)?;
    let legacy_path = legacy_ui_state_file_path(app)?;
    let mut state = load_state_from_disk(&path, &legacy_path)?;

    let playlist = playback_store::load_playlist(app)?;
    let legacy_playlist = state.playlist.take();
    if legacy_playlist.is_some() && path.exists() {
        json_io::write_json(&path, &state)?;
    }
    store_ui_state_cache(&state);
    state.playlist = Some(playlist);
    Ok(state)
}

pub fn load_setting_value(app: &tauri::AppHandle, label: &str) -> Result<Option<String>, String> {
    let path = ui_state_file_path(app)?;
    let legacy_path = legacy_ui_state_file_path(app)?;
    let state = load_state_from_disk(&path, &legacy_path)?;

    let value = state
        .settings
        .and_then(|settings| settings.groups)
        .and_then(|groups| {
            groups
                .into_iter()
                .flat_map(|group| group.items.into_iter())
                .find(|item| item.label == label)
                .map(|item| item.value)
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Ok(value)
}

pub fn save_ui_state(app: &tauri::AppHandle, state: UiState) -> Result<(), String> {
    let path = ui_state_file_path(app)?;
    let legacy_path = legacy_ui_state_file_path(app)?;
    let mut state = state;
    if let Some(playlist) = state.playlist.take() {
        playback_store::save_playlist(app, playlist)?;
    }
    let existing = load_state_from_disk(&path, &legacy_path)?;
    let merged = existing.merge(state);
    json_io::write_json(&path, &merged)?;
    store_ui_state_cache(&merged);
    Ok(())
}
