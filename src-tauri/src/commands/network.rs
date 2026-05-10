use crate::network::types::{
    BrowseNetworkPayload, DiscoverNetworkPayload, DiscoveredNetworkConnection,
    LoadNetworkFilePayload, NetworkBrowseResult,
};
use crate::store::network_connection_store::NetworkConnectionRecord;
use crate::{build_load_file_command_args_with_options, mpv_command_checked, with_mpv, AppState};
use base64::{engine::general_purpose::STANDARD, Engine};

#[tauri::command]
pub(crate) fn list_network_connections(
    app: tauri::AppHandle,
) -> Result<Vec<NetworkConnectionRecord>, String> {
    crate::store::network_connection_store::list_network_connections(&app)
}

#[tauri::command]
pub(crate) async fn discover_network_connections(
    app: tauri::AppHandle,
    payload: Option<DiscoverNetworkPayload>,
) -> Result<Vec<DiscoveredNetworkConnection>, String> {
    crate::network::service::discover_connections(
        &app,
        payload.as_ref().and_then(|item| item.protocol.as_deref()),
        payload.as_ref().and_then(|item| item.timeout_secs),
    )
    .await
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
pub(crate) async fn browse_network_connection(
    app: tauri::AppHandle,
    payload: BrowseNetworkPayload,
) -> Result<NetworkBrowseResult, String> {
    crate::network::service::validate_mode(&payload.mode)?;

    let connection = crate::store::network_connection_store::find_network_connection(
        &app,
        &payload.connection_id,
    )?;
    let protocol = crate::network::service::resolve_protocol_with_hint(
        &connection,
        payload.protocol.as_deref(),
    )?;

    let path = crate::network::service::resolve_browse_path(
        &connection,
        protocol,
        payload.path,
        &payload.mode,
    );
    crate::network::service::browse_connection(&app, &connection, &path, protocol).await
}

#[tauri::command]
pub(crate) fn load_network_file(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    payload: LoadNetworkFilePayload,
) -> Result<(), String> {
    let connection = crate::store::network_connection_store::find_network_connection(
        &app,
        &payload.connection_id,
    )?;
    let playback_url = crate::network::service::resolve_network_playback_url(
        &connection,
        payload.protocol.as_deref(),
        &payload.file_path,
    )?;
    let mut load_options: Vec<String> = Vec::new();
    let protocol = payload
        .protocol
        .as_deref()
        .unwrap_or(&connection.protocol)
        .trim()
        .to_ascii_lowercase();
    let username = connection.username.trim();
    if protocol == "webdav" && !username.is_empty() {
        let auth_value = STANDARD.encode(format!("{}:{}", username, connection.password));
        load_options.push(format!(
            "http-header-fields=Authorization: Basic {}",
            auth_value
        ));
    }
    // Network URLs should not trigger yt-dlp probing on failure.
    load_options.push("ytdl=no".to_string());
    let resume_position = payload.resume_position.unwrap_or(0.0);
    let auto_play = payload.auto_play.unwrap_or(true);
    let command_args =
        build_load_file_command_args_with_options(&playback_url, resume_position, &load_options);
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
