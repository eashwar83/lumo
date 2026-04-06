use crate::store::media_db;
use crate::store::storage_paths;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstallationState {
    pub install_id: String,
    pub device_id: String,
    #[serde(default)]
    pub active_account_id: Option<String>,
    pub install_id_updated_at: i64,
    #[serde(default)]
    pub uuid_update_data: serde_json::Value,
    #[serde(default)]
    pub last_dau_reported_date_utc: Option<String>,
    #[serde(default)]
    pub last_update_checked_date_utc: Option<String>,
    #[serde(default)]
    pub last_sync_started_at: Option<i64>,
    #[serde(default)]
    pub last_sync_finished_at: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyActionResult {
    pub date_utc: String,
    pub should_run: bool,
    #[serde(default)]
    pub version: Option<String>,
}

pub fn get_installation_state(app: &tauri::AppHandle) -> Result<InstallationState, String> {
    let conn = media_db::open_db(app)?;
    read_installation_state(&conn)
}

pub fn update_uuid_update_data(
    app: &tauri::AppHandle,
    data: serde_json::Value,
) -> Result<(), String> {
    let conn = media_db::open_db(app)?;
    let payload = serde_json::to_string(&data).map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE app_installation_state
         SET uuid_update_data = ?1,
             updated_at = ?2
         WHERE singleton = 1",
        params![payload, media_db::now_millis()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn mark_daily_signal(app: &tauri::AppHandle) -> Result<DailyActionResult, String> {
    let mut result = check_daily_field(
        app,
        "SELECT last_dau_reported_date_utc FROM app_installation_state WHERE singleton = 1",
    )?;
    if result.should_run {
        let conn = media_db::open_db(app)?;
        let state = read_installation_state(&conn)?;
        result.version = Some(format!(
            "id={}&v={}",
            state.install_id,
            env!("CARGO_PKG_VERSION")
        ));
    }
    Ok(result)
}

pub fn mark_daily_signal_reported(app: &tauri::AppHandle) -> Result<(), String> {
    write_daily_field(
        app,
        "UPDATE app_installation_state
         SET last_dau_reported_date_utc = ?1,
             updated_at = ?2
         WHERE singleton = 1",
    )
}

pub fn mark_daily_update_check(app: &tauri::AppHandle) -> Result<DailyActionResult, String> {
    check_daily_field(
        app,
        "SELECT last_update_checked_date_utc FROM app_installation_state WHERE singleton = 1",
    )
}

pub fn mark_daily_update_check_success(app: &tauri::AppHandle) -> Result<(), String> {
    write_daily_field(
        app,
        "UPDATE app_installation_state
         SET last_update_checked_date_utc = ?1,
             updated_at = ?2
         WHERE singleton = 1",
    )
}

pub fn factory_reset(app: &tauri::AppHandle) -> Result<(), String> {
    let mut conn = media_db::open_db(app)?;
    let now = media_db::now_millis();
    let (install_id, device_id, install_id_updated_at): (String, String, i64) = conn
        .query_row(
            "SELECT install_id, device_id, install_id_updated_at
             FROM app_installation_state
             WHERE singleton = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM sync_tombstones", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM sync_state", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM playlist_entries", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM play_history", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM sync_devices", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM sync_accounts", [])
        .map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE app_installation_state
         SET install_id = ?1,
             device_id = ?2,
             active_account_id = NULL,
             install_id_updated_at = ?3,
             last_dau_reported_date_utc = NULL,
             last_update_checked_date_utc = NULL,
             last_sync_started_at = NULL,
             last_sync_finished_at = NULL,
             updated_at = ?4
         WHERE singleton = 1",
        params![&install_id, &device_id, install_id_updated_at, now],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO sync_devices (
             id,
             installation_id,
             device_name,
             platform,
             app_version,
             created_at,
             updated_at,
             last_seen_at,
             is_current
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?6, 1)",
        params![
            &device_id,
            &install_id,
            std::env::var("COMPUTERNAME")
                .ok()
                .or_else(|| std::env::var("HOSTNAME").ok())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "local-device".to_string()),
            std::env::consts::OS,
            env!("CARGO_PKG_VERSION"),
            now
        ],
    )
    .map_err(|e| e.to_string())?;

    for scope in ["playlist_entries", "play_history", "tombstones"] {
        tx.execute(
            "INSERT INTO sync_state (scope, updated_at)
             VALUES (?1, ?2)",
            params![scope, now],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;

    crate::store::ui_state_store::reset_ui_state(app)?;
    crate::store::network_connection_store::clear_network_connections(app)?;

    let thumbnails_dir = storage_paths::thumbnails_dir(app)?;
    if thumbnails_dir.exists() {
        fs::remove_dir_all(&thumbnails_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&thumbnails_dir).map_err(|e| e.to_string())?;

    Ok(())
}

fn check_daily_field(app: &tauri::AppHandle, select_sql: &str) -> Result<DailyActionResult, String> {
    let today = media_db::today_utc_date();
    let conn = media_db::open_db(app)?;
    let current: Option<String> = conn
        .query_row(select_sql, [], |row| row.get(0))
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();

    let should_run = current.as_deref() != Some(today.as_str());
    Ok(DailyActionResult {
        date_utc: today,
        should_run,
        version: None,
    })
}

fn write_daily_field(app: &tauri::AppHandle, update_sql: &str) -> Result<(), String> {
    let today = media_db::today_utc_date();
    let now = media_db::now_millis();
    let conn = media_db::open_db(app)?;
    conn.execute(update_sql, params![today, now])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn read_installation_state(conn: &rusqlite::Connection) -> Result<InstallationState, String> {
    conn.query_row(
        "SELECT
             install_id,
             device_id,
             active_account_id,
             install_id_updated_at,
             uuid_update_data,
             last_dau_reported_date_utc,
             last_update_checked_date_utc,
             last_sync_started_at,
             last_sync_finished_at
         FROM app_installation_state
         WHERE singleton = 1",
        [],
        |row| {
            let raw_update_data: String = row.get(4)?;
            Ok(InstallationState {
                install_id: row.get(0)?,
                device_id: row.get(1)?,
                active_account_id: row.get(2)?,
                install_id_updated_at: row.get(3)?,
                uuid_update_data: serde_json::from_str(&raw_update_data)
                    .unwrap_or_else(|_| serde_json::json!({})),
                last_dau_reported_date_utc: row.get(5)?,
                last_update_checked_date_utc: row.get(6)?,
                last_sync_started_at: row.get(7)?,
                last_sync_finished_at: row.get(8)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}
