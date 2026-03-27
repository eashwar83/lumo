use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{
    build_load_file_command_args, json_value_to_string, mpv_command_checked,
    mpv_set_option_string_checked, with_mpv, AppState, OpenFileState,
};

#[tauri::command]
pub(crate) fn mpv_run_command(
    state: tauri::State<'_, AppState>,
    args: Vec<serde_json::Value>,
) -> Result<(), String> {
    let args_owned: Vec<String> = args
        .iter()
        .cloned()
        .map(json_value_to_string)
        .collect::<Result<Vec<_>, _>>()?;
    let args_str: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();
    with_mpv(&state, |mpv_guard| {
        mpv_command_checked(mpv_guard, &args_str)
    })
}

#[tauri::command]
pub(crate) fn mpv_set_option_string(
    state: tauri::State<'_, AppState>,
    name: String,
    value: serde_json::Value,
) -> Result<(), String> {
    let value_str = json_value_to_string(value)?;
    with_mpv(&state, |mpv_guard| {
        mpv_set_option_string_checked(mpv_guard, &name, &value_str)
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoadFilePayload {
    url: String,
    resume_position: Option<f64>,
    auto_play: Option<bool>,
}

fn resume_playback(mpv_guard: &crate::mpv::MpvHandle) -> Result<(), String> {
    mpv_command_checked(mpv_guard, &["set", "pause", "no"])
}

fn restart_from_beginning_after_eof(mpv_guard: &crate::mpv::MpvHandle) -> Result<(), String> {
    mpv_command_checked(mpv_guard, &["seek", "0", "absolute", "exact"])?;
    resume_playback(mpv_guard)
}

#[tauri::command]
pub(crate) fn load_file(
    state: tauri::State<'_, AppState>,
    payload: LoadFilePayload,
) -> Result<(), String> {
    let resume_position = payload.resume_position.unwrap_or(0.0);
    let auto_play = payload.auto_play.unwrap_or(true);
    let command_args = build_load_file_command_args(&payload.url, resume_position);
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

#[tauri::command]
pub(crate) fn cycle_pause(state: tauri::State<'_, AppState>) -> Result<(), String> {
    with_mpv(&state, |mpv_guard| {
        if mpv_guard.eof_reached() {
            return restart_from_beginning_after_eof(mpv_guard);
        }

        mpv_command_checked(mpv_guard, &["cycle", "pause"])
    })?;
    Ok(())
}

#[tauri::command]
pub(crate) fn seek_video(state: tauri::State<'_, AppState>, position: f64) -> Result<(), String> {
    let position_str = position.to_string();
    with_mpv(&state, |mpv_guard| {
        mpv_command_checked(mpv_guard, &["seek", &position_str, "absolute"])?;

        if mpv_guard.eof_reached() {
            resume_playback(mpv_guard)?;
        }

        Ok(())
    })?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeVersions {
    soia_version: String,
    mpv_version: Option<String>,
    ffmpeg_version: Option<String>,
}

fn normalize_mpv_version(value: Option<String>) -> Option<String> {
    value.map(|raw| raw.trim().to_string()).and_then(|trimmed| {
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.strip_prefix("mpv ").unwrap_or(&trimmed).to_string())
        }
    })
}

fn normalize_generic_version(value: Option<String>) -> Option<String> {
    value.map(|raw| raw.trim().to_string()).and_then(|trimmed| {
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

#[tauri::command]
pub(crate) fn get_runtime_versions(
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeVersions, String> {
    let (mpv_version, ffmpeg_version) = with_mpv(&state, |mpv_guard| {
        Ok((
            mpv_guard.get_property_string("mpv-version").ok(),
            mpv_guard.get_property_string("ffmpeg-version").ok(),
        ))
    })?;

    Ok(RuntimeVersions {
        soia_version: env!("CARGO_PKG_VERSION").to_string(),
        mpv_version: normalize_mpv_version(mpv_version),
        ffmpeg_version: normalize_generic_version(ffmpeg_version),
    })
}

fn resolve_local_media_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(parsed) = url::Url::parse(trimmed) {
        if parsed.scheme() == "file" {
            return parsed.to_file_path().ok();
        }
        return None;
    }
    Some(PathBuf::from(trimmed))
}

#[tauri::command]
pub(crate) fn get_media_file_size(path: String) -> Result<Option<u64>, String> {
    let Some(local_path) = resolve_local_media_path(&path) else {
        return Ok(None);
    };
    if !local_path.is_file() {
        return Ok(None);
    }
    match std::fs::metadata(local_path) {
        Ok(metadata) => Ok(Some(metadata.len())),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub(crate) fn consume_pending_open_files(
    state: tauri::State<'_, OpenFileState>,
) -> Result<Vec<String>, String> {
    let mut pending = state.pending_paths.lock().map_err(|e| e.to_string())?;
    Ok(std::mem::take(&mut *pending))
}
