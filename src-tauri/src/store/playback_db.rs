use crate::store::play_history::PlayHistoryEntry;
use crate::store::storage_paths;
use crate::store::ui_state_store::PlaylistEntry;
use rusqlite::{params, params_from_iter, Connection, Transaction};

const MAX_PLAY_HISTORY: i64 = 100;
const PLAY_HISTORY_UPSERT_SQL: &str =
    "INSERT INTO play_history (path, title, last_position, duration, last_played_at, is_pinned, external_audio, external_sub)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
     ON CONFLICT(path) DO UPDATE SET
         title = excluded.title,
         last_position = excluded.last_position,
         duration = excluded.duration,
         last_played_at = excluded.last_played_at,
         is_pinned = excluded.is_pinned,
         external_audio = excluded.external_audio,
         external_sub = excluded.external_sub";

fn ensure_play_history_duration_column(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(play_history)")
        .map_err(|e| e.to_string())?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;
    let mut has_duration = false;
    for column in columns {
        if column.map_err(|e| e.to_string())? == "duration" {
            has_duration = true;
            break;
        }
    }
    if !has_duration {
        conn.execute(
            "ALTER TABLE play_history ADD COLUMN duration REAL NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_play_history_is_pinned_column(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(play_history)")
        .map_err(|e| e.to_string())?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;
    let mut has_is_pinned = false;
    for column in columns {
        if column.map_err(|e| e.to_string())? == "is_pinned" {
            has_is_pinned = true;
            break;
        }
    }
    if !has_is_pinned {
        conn.execute(
            "ALTER TABLE play_history ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_play_history_external_audio_column(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(play_history)")
        .map_err(|e| e.to_string())?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;
    let mut has_column = false;
    for column in columns {
        if column.map_err(|e| e.to_string())? == "external_audio" {
            has_column = true;
            break;
        }
    }
    if !has_column {
        conn.execute(
            "ALTER TABLE play_history ADD COLUMN external_audio TEXT NOT NULL DEFAULT '[]'",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_play_history_external_sub_column(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(play_history)")
        .map_err(|e| e.to_string())?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;
    let mut has_column = false;
    for column in columns {
        if column.map_err(|e| e.to_string())? == "external_sub" {
            has_column = true;
            break;
        }
    }
    if !has_column {
        conn.execute(
            "ALTER TABLE play_history ADD COLUMN external_sub TEXT NOT NULL DEFAULT '[]'",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_play_history_title_column(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(play_history)")
        .map_err(|e| e.to_string())?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;
    let mut has_column = false;
    for column in columns {
        if column.map_err(|e| e.to_string())? == "title" {
            has_column = true;
            break;
        }
    }
    if !has_column {
        conn.execute(
            "ALTER TABLE play_history ADD COLUMN title TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn serialize_external_tracks(tracks: &[String]) -> String {
    serde_json::to_string(tracks).unwrap_or_else(|_| "[]".into())
}

fn parse_external_tracks(value: String) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(&value).unwrap_or_default()
}

fn open_db(app: &tauri::AppHandle) -> Result<Connection, String> {
    let path = storage_paths::media_db_path(app)?;
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS playlist (
             path TEXT PRIMARY KEY,
             added_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS play_history (
             path TEXT PRIMARY KEY,
             title TEXT NOT NULL DEFAULT '',
             last_position REAL NOT NULL,
             duration REAL NOT NULL DEFAULT 0,
             last_played_at INTEGER NOT NULL,
             is_pinned INTEGER NOT NULL DEFAULT 0,
             external_audio TEXT NOT NULL DEFAULT '[]',
             external_sub TEXT NOT NULL DEFAULT '[]'
         );
         CREATE INDEX IF NOT EXISTS idx_play_history_last_played_at
         ON play_history(last_played_at);",
    )
    .map_err(|e| e.to_string())?;
    ensure_play_history_duration_column(&conn)?;
    ensure_play_history_is_pinned_column(&conn)?;
    ensure_play_history_external_audio_column(&conn)?;
    ensure_play_history_external_sub_column(&conn)?;
    ensure_play_history_title_column(&conn)?;
    Ok(conn)
}

pub fn media_db_exists(app: &tauri::AppHandle) -> Result<bool, String> {
    Ok(storage_paths::media_db_path(app)?.exists())
}

fn collect_rows<T>(rows: impl Iterator<Item = rusqlite::Result<T>>) -> Result<Vec<T>, String> {
    rows.map(|row| row.map_err(|e| e.to_string())).collect()
}

fn upsert_play_history_entries(
    tx: &Transaction<'_>,
    entries: &[PlayHistoryEntry],
) -> Result<(), String> {
    let mut stmt = tx
        .prepare(PLAY_HISTORY_UPSERT_SQL)
        .map_err(|e| e.to_string())?;
    for entry in entries {
        stmt.execute(params![
            entry.path,
            entry.title,
            entry.last_position,
            entry.duration,
            entry.last_played_at,
            entry.is_pinned,
            serialize_external_tracks(&entry.external_audio_tracks),
            serialize_external_tracks(&entry.external_sub_tracks)
        ])
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn delete_play_history_except(
    tx: &Transaction<'_>,
    entries: &[PlayHistoryEntry],
) -> Result<(), String> {
    if entries.is_empty() {
        tx.execute("DELETE FROM play_history", [])
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let placeholders = std::iter::repeat("?")
        .take(entries.len())
        .collect::<Vec<_>>()
        .join(", ");
    let delete_sql = format!("DELETE FROM play_history WHERE path NOT IN ({placeholders})");
    tx.execute(
        &delete_sql,
        params_from_iter(entries.iter().map(|entry| &entry.path)),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_playlist(app: &tauri::AppHandle) -> Result<Vec<PlaylistEntry>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare("SELECT path, added_at FROM playlist")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PlaylistEntry {
                path: row.get(0)?,
                added_at: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;
    collect_rows(rows)
}

pub fn save_playlist(app: &tauri::AppHandle, entries: Vec<PlaylistEntry>) -> Result<(), String> {
    let mut conn = open_db(app)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM playlist", [])
        .map_err(|e| e.to_string())?;
    {
        let mut stmt = tx
            .prepare("INSERT INTO playlist (path, added_at) VALUES (?1, ?2)")
            .map_err(|e| e.to_string())?;
        for entry in entries {
            stmt.execute(params![entry.path, entry.added_at])
                .map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_play_history(app: &tauri::AppHandle) -> Result<Vec<PlayHistoryEntry>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare(
            "SELECT path, title, last_position, duration, last_played_at, is_pinned, external_audio, external_sub
             FROM play_history
             ORDER BY is_pinned DESC, last_played_at DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([MAX_PLAY_HISTORY], |row| {
            Ok(PlayHistoryEntry {
                path: row.get(0)?,
                title: row.get(1)?,
                last_position: row.get(2)?,
                duration: row.get(3)?,
                last_played_at: row.get(4)?,
                is_pinned: row.get(5)?,
                external_audio_tracks: parse_external_tracks(row.get(6)?),
                external_sub_tracks: parse_external_tracks(row.get(7)?),
            })
        })
        .map_err(|e| e.to_string())?;
    collect_rows(rows)
}

pub fn save_play_history(
    app: &tauri::AppHandle,
    entries: Vec<PlayHistoryEntry>,
) -> Result<(), String> {
    let mut conn = open_db(app)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    upsert_play_history_entries(&tx, &entries)?;
    delete_play_history_except(&tx, &entries)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn save_play_history_entry(
    app: &tauri::AppHandle,
    entry: PlayHistoryEntry,
) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute(
        PLAY_HISTORY_UPSERT_SQL,
        params![
            entry.path,
            entry.title,
            entry.last_position,
            entry.duration,
            entry.last_played_at,
            entry.is_pinned,
            serialize_external_tracks(&entry.external_audio_tracks),
            serialize_external_tracks(&entry.external_sub_tracks)
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
