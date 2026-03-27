use log::error;
use std::sync::OnceLock;

const MEDIA_EXTENSIONS_JSON: &str = include_str!("../../src/constants/mediaExtensions.json");

static MEDIA_EXTENSIONS: OnceLock<Vec<String>> = OnceLock::new();

fn parse_media_extensions() -> Vec<String> {
    match serde_json::from_str::<Vec<String>>(MEDIA_EXTENSIONS_JSON) {
        Ok(values) => values
            .into_iter()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect(),
        Err(error) => {
            error!("Failed to parse media extensions JSON: {error}");
            Vec::new()
        }
    }
}

pub(crate) fn all() -> &'static [String] {
    MEDIA_EXTENSIONS
        .get_or_init(parse_media_extensions)
        .as_slice()
}

pub(crate) fn contains_extension(extension: &str) -> bool {
    let normalized = extension.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    all().iter().any(|candidate| candidate == &normalized)
}
