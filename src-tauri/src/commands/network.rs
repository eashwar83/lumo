use serde::{Deserialize, Serialize};

use crate::store::network_connection_store::NetworkConnectionRecord;
use crate::{build_load_file_command_args, mpv_command_checked, with_mpv, AppState};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebdavBrowseEntry {
    name: String,
    path: String,
    entry_type: String,
    size: Option<u64>,
    modified_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebdavBrowseResult {
    path: String,
    entries: Vec<WebdavBrowseEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebdavBrowsePayload {
    connection_id: String,
    path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoadWebdavFilePayload {
    connection_id: String,
    file_path: String,
    resume_position: Option<f64>,
    auto_play: Option<bool>,
}

fn to_browse_result(
    result: crate::network::webdav_client::WebdavBrowseResult,
) -> WebdavBrowseResult {
    let entries = result
        .entries
        .into_iter()
        .map(|entry| WebdavBrowseEntry {
            name: entry.name,
            path: entry.path,
            entry_type: if entry.is_dir {
                "dir".into()
            } else {
                "file".into()
            },
            size: entry.size,
            modified_at: entry.modified_at,
        })
        .collect();
    WebdavBrowseResult {
        path: result.path,
        entries,
    }
}

#[tauri::command]
pub(crate) fn list_network_connections(
    app: tauri::AppHandle,
) -> Result<Vec<NetworkConnectionRecord>, String> {
    crate::store::network_connection_store::list_network_connections(&app)
}

#[tauri::command]
pub(crate) fn save_network_connection(
    app: tauri::AppHandle,
    connection: NetworkConnectionRecord,
) -> Result<Vec<NetworkConnectionRecord>, String> {
    crate::store::network_connection_store::save_network_connection(&app, connection)
}

#[tauri::command]
pub(crate) fn delete_network_connection(
    app: tauri::AppHandle,
    connection_id: String,
) -> Result<Vec<NetworkConnectionRecord>, String> {
    crate::store::network_connection_store::delete_network_connection(&app, &connection_id)
}

#[tauri::command]
pub(crate) async fn connect_webdav(
    app: tauri::AppHandle,
    payload: WebdavBrowsePayload,
) -> Result<WebdavBrowseResult, String> {
    let connection_id = payload.connection_id;
    let requested_path = payload.path;

    tauri::async_runtime::spawn_blocking(move || {
        let connection =
            crate::store::network_connection_store::find_network_connection(&app, &connection_id)?;
        let path = requested_path.unwrap_or_else(|| connection.default_path.clone());
        let result = crate::network::webdav_client::list_directory(&connection, &path)?;
        Ok(to_browse_result(result))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn browse_webdav(
    app: tauri::AppHandle,
    payload: WebdavBrowsePayload,
) -> Result<WebdavBrowseResult, String> {
    let connection_id = payload.connection_id;
    let requested_path = payload.path.unwrap_or_else(|| "/".to_string());

    tauri::async_runtime::spawn_blocking(move || {
        let connection =
            crate::store::network_connection_store::find_network_connection(&app, &connection_id)?;
        let result = crate::network::webdav_client::list_directory(&connection, &requested_path)?;
        Ok(to_browse_result(result))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) fn load_webdav_file(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    payload: LoadWebdavFilePayload,
) -> Result<(), String> {
    let connection = crate::store::network_connection_store::find_network_connection(
        &app,
        &payload.connection_id,
    )?;
    let playback_url =
        crate::network::webdav_client::build_playback_url(&connection, &payload.file_path)?;
    let resume_position = payload.resume_position.unwrap_or(0.0);
    let auto_play = payload.auto_play.unwrap_or(true);
    let command_args = build_load_file_command_args(&playback_url, resume_position);
    let command_refs: Vec<&str> = command_args.iter().map(String::as_str).collect();

    with_mpv(&state, |mpv_guard| {
        mpv_command_checked(mpv_guard, &command_refs)?;
        mpv_command_checked(
            mpv_guard,
            &["set", "pause", if auto_play { "no" } else { "yes" }],
        )?;
        Ok(())
    })?;

    Ok(())
}
