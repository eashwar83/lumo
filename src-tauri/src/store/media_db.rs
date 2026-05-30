use crate::store::storage_paths;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

pub const MAX_PLAY_HISTORY: i64 = 100;
const SCHEMA_VERSION: i32 = 3;

pub fn now_millis() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn today_utc_date() -> String {
    Utc::now().date_naive().format("%Y-%m-%d").to_string()
}

pub fn new_uuid() -> String {
    Uuid::now_v7().to_string()
}

pub fn normalize_uuid_or_new(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return new_uuid();
    }
    Uuid::parse_str(trimmed)
        .map(|uuid| uuid.to_string())
        .unwrap_or_else(|_| new_uuid())
}

pub fn open_db(app: &tauri::AppHandle) -> Result<Connection, String> {
    let path = storage_paths::media_db_path(app)?;
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA synchronous = NORMAL;",
    )
    .map_err(|e| e.to_string())?;
    ensure_schema(&conn)?;
    Ok(conn)
}

pub fn local_device_id(conn: &Connection) -> Result<String, String> {
    conn.query_row(
        "SELECT device_id FROM app_installation_state WHERE singleton = 1",
        [],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

fn ensure_schema(conn: &Connection) -> Result<(), String> {
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    if version == 2 {
        migrate_schema_v2_to_v3(conn)?;
        conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))
            .map_err(|e| e.to_string())?;
    } else if version != SCHEMA_VERSION {
        reset_schema(conn)?;
        conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))
            .map_err(|e| e.to_string())?;
    }

    ensure_installation_state(conn)?;
    ensure_local_device_record(conn)?;
    ensure_sync_state_rows(conn)?;
    Ok(())
}

fn reset_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "BEGIN;
         DROP TABLE IF EXISTS sync_tombstones;
         DROP TABLE IF EXISTS sync_state;
         DROP TABLE IF EXISTS playlist_entries;
         DROP TABLE IF EXISTS play_history;
         DROP TABLE IF EXISTS app_installation_state;
         DROP TABLE IF EXISTS sync_devices;
         DROP TABLE IF EXISTS sync_accounts;

         CREATE TABLE sync_accounts (
             id TEXT PRIMARY KEY,
             provider TEXT NOT NULL DEFAULT 'soia',
             remote_user_id TEXT UNIQUE,
             email TEXT,
             display_name TEXT,
             auth_data TEXT NOT NULL DEFAULT '{}',
             created_at INTEGER NOT NULL,
             updated_at INTEGER NOT NULL,
             last_login_at INTEGER,
             status INTEGER NOT NULL DEFAULT 0 CHECK (status IN (0, 1, 2))
         ) STRICT;

         CREATE TABLE sync_devices (
             id TEXT PRIMARY KEY,
             account_id TEXT,
             installation_id TEXT NOT NULL UNIQUE,
             device_name TEXT NOT NULL DEFAULT '',
             platform TEXT NOT NULL DEFAULT '',
             app_version TEXT NOT NULL DEFAULT '',
             created_at INTEGER NOT NULL,
             updated_at INTEGER NOT NULL,
             last_seen_at INTEGER,
             is_current INTEGER NOT NULL DEFAULT 0 CHECK (is_current IN (0, 1)),
             FOREIGN KEY(account_id) REFERENCES sync_accounts(id) ON DELETE SET NULL
         ) STRICT;

         CREATE INDEX idx_sync_devices_account
         ON sync_devices(account_id);

         CREATE TABLE playlist_entries (
             id TEXT PRIMARY KEY,
             playlist_id TEXT NOT NULL DEFAULT 'default',
             path TEXT NOT NULL,
             order_index INTEGER NOT NULL,
             added_at INTEGER NOT NULL,
             created_at INTEGER NOT NULL,
             updated_at INTEGER NOT NULL,
             record_version INTEGER NOT NULL DEFAULT 1 CHECK (record_version > 0),
             last_modified_by_device_id TEXT NOT NULL,
             sync_status INTEGER NOT NULL DEFAULT 0 CHECK (sync_status IN (0, 1, 2)),
             remote_record_id TEXT,
             remote_updated_at INTEGER,
             UNIQUE(playlist_id, path),
             UNIQUE(playlist_id, order_index)
         ) STRICT;

         CREATE INDEX idx_playlist_entries_order
         ON playlist_entries(playlist_id, order_index ASC);

         CREATE INDEX idx_playlist_entries_sync
         ON playlist_entries(sync_status, updated_at ASC);

         CREATE TABLE play_history (
             id TEXT PRIMARY KEY,
             path TEXT NOT NULL UNIQUE,
             title TEXT NOT NULL DEFAULT '',
             last_position REAL NOT NULL DEFAULT 0,
             duration REAL NOT NULL DEFAULT 0,
             last_played_at INTEGER NOT NULL,
             is_pinned INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0, 1)),
             is_live_playback INTEGER NOT NULL DEFAULT 0 CHECK (is_live_playback IN (0, 1)),
             external_audio TEXT NOT NULL DEFAULT '[]',
             external_sub TEXT NOT NULL DEFAULT '[]',
             created_at INTEGER NOT NULL,
             updated_at INTEGER NOT NULL,
             record_version INTEGER NOT NULL DEFAULT 1 CHECK (record_version > 0),
             last_modified_by_device_id TEXT NOT NULL,
             sync_status INTEGER NOT NULL DEFAULT 0 CHECK (sync_status IN (0, 1, 2)),
             remote_record_id TEXT,
             remote_updated_at INTEGER
         ) STRICT;

         CREATE INDEX idx_play_history_sort
         ON play_history(is_pinned DESC, last_played_at DESC);

         CREATE INDEX idx_play_history_sync
         ON play_history(sync_status, updated_at ASC);

         CREATE TABLE sync_tombstones (
             id TEXT PRIMARY KEY,
             entity_type TEXT NOT NULL CHECK (entity_type IN ('playlist_entries', 'play_history')),
             entity_id TEXT NOT NULL,
             payload TEXT NOT NULL DEFAULT '{}',
             deleted_at INTEGER NOT NULL,
             record_version INTEGER NOT NULL DEFAULT 1 CHECK (record_version > 0),
             last_modified_by_device_id TEXT NOT NULL,
             sync_status INTEGER NOT NULL DEFAULT 2 CHECK (sync_status IN (0, 2)),
             UNIQUE(entity_type, entity_id)
         ) STRICT;

         CREATE INDEX idx_sync_tombstones_sync
         ON sync_tombstones(sync_status, deleted_at ASC);

         CREATE TABLE sync_state (
             scope TEXT PRIMARY KEY CHECK (scope IN ('playlist_entries', 'play_history', 'tombstones')),
             last_sync_token TEXT,
             last_synced_at INTEGER,
             last_full_sync_at INTEGER,
             last_error TEXT,
             updated_at INTEGER NOT NULL
         ) STRICT;

         CREATE TABLE app_installation_state (
             singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
             install_id TEXT NOT NULL UNIQUE,
             device_id TEXT NOT NULL UNIQUE,
             active_account_id TEXT,
             install_id_updated_at INTEGER NOT NULL,
             uuid_update_data TEXT NOT NULL DEFAULT '{}',
             last_dau_reported_date_utc TEXT,
             last_update_checked_date_utc TEXT,
             last_sync_started_at INTEGER,
             last_sync_finished_at INTEGER,
             updated_at INTEGER NOT NULL,
             FOREIGN KEY(active_account_id) REFERENCES sync_accounts(id) ON DELETE SET NULL
         ) STRICT;
         COMMIT;",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn migrate_schema_v2_to_v3(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "BEGIN;
         ALTER TABLE play_history
         ADD COLUMN is_live_playback INTEGER NOT NULL DEFAULT 0 CHECK (is_live_playback IN (0, 1));
         COMMIT;",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn ensure_installation_state(conn: &Connection) -> Result<(), String> {
    let now = now_millis();
    conn.execute(
        "INSERT INTO app_installation_state (
             singleton,
             install_id,
             device_id,
             install_id_updated_at,
             uuid_update_data,
             updated_at
         )
         VALUES (1, ?1, ?2, ?3, '{}', ?3)
         ON CONFLICT(singleton) DO NOTHING",
        params![new_uuid(), new_uuid(), now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn ensure_local_device_record(conn: &Connection) -> Result<(), String> {
    let now = now_millis();
    let (install_id, device_id): (String, String) = conn
        .query_row(
            "SELECT install_id, device_id FROM app_installation_state WHERE singleton = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE sync_devices SET is_current = 0 WHERE is_current = 1 AND id != ?1",
        params![&device_id],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
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
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?6, 1)
         ON CONFLICT(id) DO UPDATE SET
             installation_id = excluded.installation_id,
             platform = excluded.platform,
             app_version = excluded.app_version,
             updated_at = excluded.updated_at,
             last_seen_at = excluded.last_seen_at,
             is_current = 1",
        params![
            &device_id,
            &install_id,
            local_device_name(),
            std::env::consts::OS,
            env!("CARGO_PKG_VERSION"),
            now
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn ensure_sync_state_rows(conn: &Connection) -> Result<(), String> {
    let now = now_millis();
    for scope in ["playlist_entries", "play_history", "tombstones"] {
        conn.execute(
            "INSERT INTO sync_state (scope, updated_at)
             VALUES (?1, ?2)
             ON CONFLICT(scope) DO NOTHING",
            params![scope, now],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn local_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "local-device".to_string())
}
