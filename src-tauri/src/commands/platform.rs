use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[tauri::command]
pub(crate) fn is_native_pip_enabled(app: tauri::AppHandle) -> bool {
    crate::platform::is_native_pip_enabled(&app)
}

#[tauri::command]
pub(crate) fn set_native_pip_enabled(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    crate::platform::set_native_pip_enabled(&app, enabled)
}

pub(crate) fn is_supported_media_file(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    crate::media_extensions::contains_extension(extension)
}

fn expand_media_paths(paths: Vec<String>) -> Vec<String> {
    let mut stack: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let mut expanded = BTreeSet::new();

    while let Some(path) = stack.pop() {
        if path.is_file() {
            if is_supported_media_file(&path) {
                expanded.insert(path.to_string_lossy().into_owned());
            }
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let entry_path = entry.path();
            if file_type.is_dir() {
                stack.push(entry_path);
                continue;
            }
            if file_type.is_file() && is_supported_media_file(&entry_path) {
                expanded.insert(entry_path.to_string_lossy().into_owned());
            }
        }
    }

    expanded.into_iter().collect()
}

#[tauri::command]
pub(crate) fn pick_media_paths_native(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let selected = crate::platform::pick_media_paths_native(app)?;
    Ok(expand_media_paths(selected))
}
