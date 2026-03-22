use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::path::Path;

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

pub fn read_json_or_default<T: DeserializeOwned + Default>(path: &Path) -> Result<T, String> {
    if !path.exists() {
        return Ok(T::default());
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    match serde_json::from_str(&content) {
        Ok(value) => Ok(value),
        Err(err) => {
            eprintln!("Failed to parse {}: {}", path.display(), err);
            Ok(T::default())
        }
    }
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let content = serde_json::to_string(value).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())?;
    Ok(())
}
