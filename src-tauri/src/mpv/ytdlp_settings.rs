use std::sync::{Mutex, OnceLock};
use tauri::AppHandle;

const DEFAULT_YTDLP_MAX_HEIGHT: u32 = 1080;

#[derive(Clone)]
pub(crate) struct YtdlpSettings {
    pub(crate) binary: YtdlpBinarySettings,
    pub(crate) cookies: YtdlpCookieSettings,
    pub(crate) format: YtdlpFormatSettings,
}

#[derive(Clone)]
pub(crate) struct YtdlpBinarySettings {
    pub(crate) path: Option<String>,
}

#[derive(Clone)]
pub(crate) struct YtdlpCookieSettings {
    pub(crate) browser: Option<String>,
}

#[derive(Clone)]
pub(crate) struct YtdlpFormatSettings {
    pub(crate) max_height: u32,
}

impl YtdlpSettings {
    pub(crate) fn new(
        binary_path: Option<String>,
        cookies_from_browser: Option<String>,
        format: YtdlpFormatSettings,
    ) -> Self {
        Self {
            binary: YtdlpBinarySettings { path: binary_path },
            cookies: YtdlpCookieSettings {
                browser: cookies_from_browser,
            },
            format,
        }
    }
}

impl Default for YtdlpFormatSettings {
    fn default() -> Self {
        Self {
            max_height: DEFAULT_YTDLP_MAX_HEIGHT,
        }
    }
}

impl YtdlpFormatSettings {
    pub(crate) fn selector(&self) -> String {
        let max_height = self.max_height;
        format!(
            "bv*[height<={max_height}][vcodec^=avc1]+ba/\
             bv*[height<={max_height}][vcodec^=h264]+ba/\
             bv*[height<={max_height}][vcodec^=vp9.2]+ba/\
             bv*[height<={max_height}][vcodec^=vp9]+ba/\
             bv*[height<={max_height}][vcodec^=av01]+ba/\
             bv*[height<={max_height}][vcodec^=hev1]+ba/\
             bv*[height<={max_height}][vcodec^=hvc1]+ba/\
             b[height<={max_height}]/\
             bv*[height<={max_height}]+ba/\
             bv*+ba/b"
        )
    }
}

static RUNTIME_YTDLP_SETTINGS: OnceLock<Mutex<Option<YtdlpSettings>>> = OnceLock::new();

fn runtime_ytdlp_settings() -> &'static Mutex<Option<YtdlpSettings>> {
    RUNTIME_YTDLP_SETTINGS.get_or_init(|| Mutex::new(None))
}

fn load_runtime_settings() -> Option<YtdlpSettings> {
    runtime_ytdlp_settings()
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

pub(crate) fn store_runtime_settings(settings: YtdlpSettings) {
    if let Ok(mut guard) = runtime_ytdlp_settings().lock() {
        *guard = Some(settings);
    }
}

pub(crate) fn resolve(app: &AppHandle) -> YtdlpSettings {
    load_runtime_settings().unwrap_or_else(|| {
        let max_height = crate::store::ui_state_store::load_ytdl_max_height(app);
        YtdlpSettings::new(
            crate::app_bootstrap::resolve_ytdl_path(app),
            crate::store::ui_state_store::load_ytdl_cookies_from_browser(app),
            YtdlpFormatSettings { max_height },
        )
    })
}
