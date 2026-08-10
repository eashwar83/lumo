use log::error;
use std::sync::OnceLock;

const MEDIA_EXTENSIONS_JSON: &str = include_str!("../../src/constants/mediaExtensions.json");

static MEDIA_EXTENSIONS: OnceLock<Vec<String>> = OnceLock::new();
static MEDIA_ASSOCIATION_EXTENSIONS: OnceLock<Vec<String>> = OnceLock::new();

#[derive(serde::Deserialize)]
struct MediaExtensionEntry {
    ext: String,
    kind: String,
}

fn parse_media_extensions() -> Vec<String> {
    match serde_json::from_str::<Vec<MediaExtensionEntry>>(MEDIA_EXTENSIONS_JSON) {
        Ok(values) => values
            .into_iter()
            .map(|value| value.ext.trim().to_ascii_lowercase())
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

/// Video and audio only. Doubles as the file-association set: Lumo is a video
/// player, so it does not claim `.jpg` from the system image viewer.
pub(crate) fn non_image_extensions() -> &'static [String] {
    MEDIA_ASSOCIATION_EXTENSIONS
        .get_or_init(|| {
            match serde_json::from_str::<Vec<MediaExtensionEntry>>(MEDIA_EXTENSIONS_JSON) {
                Ok(values) => values
                    .into_iter()
                    .filter(|value| !value.kind.eq_ignore_ascii_case("image"))
                    .map(|value| value.ext.trim().to_ascii_lowercase())
                    .filter(|value| !value.is_empty())
                    .collect(),
                Err(error) => {
                    error!("Failed to parse media extensions JSON for media association: {error}");
                    Vec::new()
                }
            }
        })
        .as_slice()
}

/// `include_images` mirrors the Include Images in Playlist setting: with it
/// off, a folder's cover art is not something Previous/Next steps through.
pub(crate) fn contains_extension_of_kind(extension: &str, include_images: bool) -> bool {
    let normalized = extension.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    let candidates = if include_images {
        all()
    } else {
        non_image_extensions()
    };
    candidates.iter().any(|candidate| candidate == &normalized)
}

const PLAYLIST_INCLUDE_IMAGES_LABEL: &str = "PLAYLIST_INCLUDE_IMAGES";

/// Defaults to false, which is also what an unreadable settings file gives:
/// the video player's answer, not the file manager's.
pub(crate) fn include_images_in_playlist(app: &tauri::AppHandle) -> bool {
    crate::store::ui_state_store::load_setting_value(app, PLAYLIST_INCLUDE_IMAGES_LABEL)
        .ok()
        .flatten()
        .is_some_and(|value| value.eq_ignore_ascii_case("on"))
}
