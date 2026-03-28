use crate::store::media_db;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

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

pub fn mark_daily_active(app: &tauri::AppHandle) -> Result<DailyActionResult, String> {
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

pub fn mark_daily_active_reported(app: &tauri::AppHandle) -> Result<(), String> {
    write_daily_field(
        app,
        "UPDATE app_installation_state
         SET last_dau_reported_date_utc = ?1,
             updated_at = ?2
         WHERE singleton = 1",
    )
}

pub fn mark_daily_update_check(app: &tauri::AppHandle) -> Result<DailyActionResult, String> {
    mark_daily_field(
        app,
        "SELECT last_update_checked_date_utc FROM app_installation_state WHERE singleton = 1",
        "UPDATE app_installation_state
         SET last_update_checked_date_utc = ?1,
             updated_at = ?2
         WHERE singleton = 1",
    )
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

fn mark_daily_field(
    app: &tauri::AppHandle,
    select_sql: &str,
    update_sql: &str,
) -> Result<DailyActionResult, String> {
    let result = check_daily_field(app, select_sql)?;
    if result.should_run {
        write_daily_field(app, update_sql)?;
    }
    Ok(result)
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
