mod event_loop;
mod ffi;
mod handle;
mod stream_https;
mod stream_proxy;

#[cfg(target_os = "macos")]
pub(crate) use ffi::SoiaUtils;
pub(crate) use handle::MpvHandle;
pub(crate) use stream_proxy::rewrite_https_stream_url;

pub(crate) fn register_https_basic_auth(playback_url: &str, username: &str, password: &str) {
    stream_proxy::register_basic_auth(playback_url, username, password);
    stream_https::register_basic_auth(playback_url, username, password);
}

pub(crate) fn rewrite_https_callback_url(url: &str) -> Option<String> {
    stream_https::rewrite_https_callback_url(url)
}
