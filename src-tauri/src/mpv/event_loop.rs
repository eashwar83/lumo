use super::ffi::{
    mpv_destroy, mpv_event_id, mpv_format, mpv_free, mpv_get_property_string, mpv_node,
    mpv_observe_property, mpv_wait_event, MpvEventEndFile, MpvEventProperty,
};
use crate::AppState;
use log::{debug, error, info, trace, warn};
use serde::Serialize;
use std::ffi::{c_void, CStr, CString};
use std::os::raw::c_int;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize, Clone)]
struct ProgressPayload {
    time_pos: f64,
    duration: f64,
    buffered_pos: f64,
    is_playing: bool,
    video_bitrate: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EndFilePayload {
    reason: String,
}

#[derive(Serialize, Clone)]
struct MediaTrack {
    id: i64,
    track_type: String,
    title: String,
    lang: String,
    selected: bool,
    codec: Option<String>,
    codec_desc: Option<String>,
    decoder_desc: Option<String>,
    demux_w: Option<i64>,
    demux_h: Option<i64>,
    demux_fps: Option<f64>,
    demux_bitrate: Option<i64>,
    demux_samplerate: Option<i64>,
    demux_channels: Option<String>,
    fps: Option<f64>,
    w: Option<i64>,
    h: Option<i64>,
    is_default: Option<bool>,
    forced: Option<bool>,
    external: Option<bool>,
}

#[derive(Serialize, Clone)]
struct TracksPayload {
    tracks: Vec<MediaTrack>,
}

fn emit_event<T: Serialize + Clone>(app_handle: &AppHandle, name: &str, payload: T) -> bool {
    if let Err(e) = app_handle.emit(name, payload) {
        error!("MPV Event Loop: Failed to emit {}: {:?}", name, e);
        false
    } else {
        true
    }
}

fn emit_progress(
    app_handle: &AppHandle,
    time_pos: f64,
    duration: f64,
    buffered_pos: f64,
    is_playing: bool,
    video_bitrate: f64,
) {
    emit_event(
        app_handle,
        "mpv-progress-update",
        ProgressPayload {
            time_pos,
            duration,
            buffered_pos,
            is_playing,
            video_bitrate,
        },
    );
}

fn end_file_reason_label(reason: c_int) -> &'static str {
    match reason {
        0 => "eof",
        2 => "stop",
        3 => "quit",
        4 => "error",
        5 => "redirect",
        _ => "unknown",
    }
}

fn compute_buffered_pos(time_pos: f64, duration: f64, cache_ahead: f64) -> f64 {
    let safe_time_pos = if time_pos.is_finite() {
        time_pos.max(0.0)
    } else {
        0.0
    };
    let safe_cache_ahead = if cache_ahead.is_finite() {
        cache_ahead.max(0.0)
    } else {
        0.0
    };
    let mut buffered_pos = safe_time_pos + safe_cache_ahead;
    if duration.is_finite() && duration > 0.0 {
        buffered_pos = buffered_pos.min(duration);
    }
    buffered_pos.max(safe_time_pos)
}

fn emit_end_file_and_progress(
    app_handle: &AppHandle,
    reason: c_int,
    last_time_pos: &mut f64,
    last_duration: f64,
    last_buffered_pos: &mut f64,
    last_video_bitrate: &mut f64,
) {
    let reason_label = end_file_reason_label(reason);
    let ended_time_pos = if reason == 0 {
        if last_duration.is_finite() && last_duration > 0.0 {
            // MPV may not emit the final `time-pos` at EOF; force UI to full duration.
            last_duration
        } else {
            *last_time_pos
        }
    } else {
        0.0
    };
    #[cfg(debug_assertions)]
    info!(
        "MPV Event Loop: End of file reached. reason={} ({})",
        reason, reason_label
    );
    emit_event(
        app_handle,
        "mpv-end-file",
        EndFilePayload {
            reason: reason_label.to_string(),
        },
    );
    *last_time_pos = ended_time_pos;
    *last_buffered_pos = ended_time_pos;
    *last_video_bitrate = 0.0;
    trace!("MPV time-pos updated: {}", ended_time_pos);
    emit_progress(
        app_handle,
        ended_time_pos,
        last_duration,
        ended_time_pos,
        false,
        0.0,
    );
}

fn emit_resize_if_changed(
    app_handle: &AppHandle,
    width: i64,
    height: i64,
    last_emit_width: &mut i64,
    last_emit_height: &mut i64,
) {
    if crate::platform::is_native_pip_enabled(app_handle) {
        return;
    }
    #[cfg(target_os = "linux")]
    if width > 0 && height > 0 && (width != *last_emit_width || height != *last_emit_height) {
        *last_emit_width = width;
        *last_emit_height = height;
    }

    #[cfg(not(target_os = "linux"))]
    if width > 0 && height > 0 && (width != *last_emit_width || height != *last_emit_height) {
        if emit_event(app_handle, "resize_window", (width, height)) {
            *last_emit_width = width;
            *last_emit_height = height;
        }
    }
}

fn observe_property(client: *mut c_void, id: u64, name: &str, format: mpv_format) {
    let c_name = CString::new(name).expect("Property name contains null byte");
    let result = unsafe { mpv_observe_property(client, id, c_name.as_ptr(), format) };
    if result < 0 {
        warn!("MPV: observe_property {} failed with {}", name, result);
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn emit_pip_state_on_pause_change(
    app_handle: &AppHandle,
    is_paused: bool,
    width: i64,
    height: i64,
    last_pip_paused: &mut bool,
) {
    if is_paused == *last_pip_paused {
        return;
    }
    crate::platform::update_native_pip_state(app_handle, is_paused, width, height);
    *last_pip_paused = is_paused;
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn emit_pip_state_on_video_size_change(
    app_handle: &AppHandle,
    is_paused: bool,
    width: i64,
    height: i64,
    last_pip_aspect_width: &mut i64,
    last_pip_aspect_height: &mut i64,
    last_pip_paused: &mut bool,
) {
    if width <= 1 || height <= 1 {
        return;
    }
    if width == *last_pip_aspect_width && height == *last_pip_aspect_height {
        return;
    }
    crate::platform::update_native_pip_state(app_handle, is_paused, width, height);
    *last_pip_aspect_width = width;
    *last_pip_aspect_height = height;
    *last_pip_paused = is_paused;
}

unsafe fn parse_node(node: *mut mpv_node) -> serde_json::Value {
    let node = &*node;
    match node.format {
        mpv_format::MPV_FORMAT_NONE => serde_json::Value::Null,
        mpv_format::MPV_FORMAT_STRING => {
            let c_str = CStr::from_ptr(node.u.string);
            serde_json::Value::String(c_str.to_string_lossy().into_owned())
        }
        mpv_format::MPV_FORMAT_FLAG => serde_json::Value::Bool(node.u.flag != 0),
        mpv_format::MPV_FORMAT_INT64 => serde_json::Value::Number(node.u.int64.into()),
        mpv_format::MPV_FORMAT_DOUBLE => serde_json::json!(node.u.double),
        mpv_format::MPV_FORMAT_NODE_ARRAY | mpv_format::MPV_FORMAT_NODE_MAP => {
            let list = &*node.u.list;
            if node.format == mpv_format::MPV_FORMAT_NODE_ARRAY {
                let mut arr = Vec::new();
                for i in 0..list.num {
                    arr.push(parse_node(list.values.offset(i as isize)));
                }
                serde_json::Value::Array(arr)
            } else {
                let mut map = serde_json::Map::new();
                for i in 0..list.num {
                    let key = CStr::from_ptr(*list.keys.offset(i as isize));
                    let value = parse_node(list.values.offset(i as isize));
                    map.insert(key.to_string_lossy().into_owned(), value);
                }
                serde_json::Value::Object(map)
            }
        }
        _ => serde_json::Value::Null,
    }
}

pub(super) fn mpv_event_loop(
    app_handle: AppHandle,
    stop_flag: Arc<AtomicBool>,
    is_playing: Arc<AtomicBool>,
    eof_reached: Arc<AtomicBool>,
) {
    eof_reached.store(false, Ordering::SeqCst);
    let event_client: *mut c_void;
    {
        let app_state: tauri::State<'_, AppState> = app_handle.state::<AppState>();
        let mpv_player_guard = match app_state.mpv_player.lock() {
            Ok(guard) => guard,
            Err(err) => {
                error!("Failed to lock MPV player mutex: {}", err);
                return;
            }
        };
        event_client = match mpv_player_guard.create_client("event_loop_client") {
            Ok(ptr) => ptr,
            Err(e) => {
                error!("Failed to create MPV event client: {}", e);
                return;
            }
        };
    }

    const TIME_POS_ID: u64 = 1;
    const DURATION_ID: u64 = 2;
    const PAUSE_ID: u64 = 3;
    const WIDTH_ID: u64 = 4;
    const HEIGHT_ID: u64 = 5;
    const TRACK_ID: u64 = 6;
    const VIDEO_BITRATE_ID: u64 = 7;
    const MEDIA_TITLE_ID: u64 = 8;
    const EOF_REACHED_ID: u64 = 9;
    const DEMUXER_CACHE_TIME_ID: u64 = 10;

    let mut last_time_pos: f64 = 0.0;
    let mut last_duration: f64 = 0.0;
    let mut last_is_paused: bool = false;
    let mut last_video_bitrate: f64 = 0.0;
    let mut last_demuxer_cache_time: f64 = 0.0;
    let mut last_buffered_pos: f64 = 0.0;
    let mut notify_start: bool = false;
    let mut width: i64 = 0;
    let mut height: i64 = 0;
    let mut last_emit_width: i64 = 0;
    let mut last_emit_height: i64 = 0;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let mut last_pip_aspect_width: i64 = 0;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let mut last_pip_aspect_height: i64 = 0;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let mut last_pip_paused: bool = false;
    let mut last_media_title: Option<String> = None;
    let mut end_file_emitted_for_current_item: bool = false;
    let media_title_name = CString::new("media-title").expect("Property name contains null byte");

    unsafe {
        observe_property(
            event_client,
            TIME_POS_ID,
            "time-pos",
            mpv_format::MPV_FORMAT_DOUBLE,
        );
        observe_property(
            event_client,
            DURATION_ID,
            "duration",
            mpv_format::MPV_FORMAT_DOUBLE,
        );
        observe_property(event_client, PAUSE_ID, "pause", mpv_format::MPV_FORMAT_FLAG);
        observe_property(
            event_client,
            WIDTH_ID,
            "width",
            mpv_format::MPV_FORMAT_INT64,
        );
        observe_property(
            event_client,
            HEIGHT_ID,
            "height",
            mpv_format::MPV_FORMAT_INT64,
        );
        observe_property(
            event_client,
            TRACK_ID,
            "track-list",
            mpv_format::MPV_FORMAT_NODE,
        );
        observe_property(
            event_client,
            VIDEO_BITRATE_ID,
            "video-bitrate",
            mpv_format::MPV_FORMAT_DOUBLE,
        );
        observe_property(
            event_client,
            MEDIA_TITLE_ID,
            "media-title",
            mpv_format::MPV_FORMAT_STRING,
        );
        observe_property(
            event_client,
            EOF_REACHED_ID,
            "eof-reached",
            mpv_format::MPV_FORMAT_FLAG,
        );
        observe_property(
            event_client,
            DEMUXER_CACHE_TIME_ID,
            "demuxer-cache-time",
            mpv_format::MPV_FORMAT_DOUBLE,
        );

        debug!("MPV Event Loop: Started observing properties.");

        loop {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }
            let event = mpv_wait_event(event_client, 0.1);
            if event.is_null() {
                continue;
            }

            match (*event).event_id {
                mpv_event_id::MPV_EVENT_START_FILE => {
                    #[cfg(debug_assertions)]
                    debug!("MPV Event Loop: MPV_EVENT_START_FILE received.");
                    end_file_emitted_for_current_item = false;
                    eof_reached.store(false, Ordering::SeqCst);
                }
                mpv_event_id::MPV_EVENT_FILE_LOADED => {
                    #[cfg(debug_assertions)]
                    info!("MPV Event Loop: MPV_EVENT_FILE_LOADED received.");
                    notify_start = true;
                    width = 0;
                    height = 0;
                    last_emit_width = 0;
                    last_emit_height = 0;
                    #[cfg(any(target_os = "macos", target_os = "windows"))]
                    {
                        last_pip_aspect_width = 0;
                        last_pip_aspect_height = 0;
                    }
                    last_video_bitrate = 0.0;
                    last_demuxer_cache_time = 0.0;
                    last_buffered_pos = 0.0;
                    end_file_emitted_for_current_item = false;
                    eof_reached.store(false, Ordering::SeqCst);
                    is_playing.store(true, Ordering::Relaxed);
                }
                mpv_event_id::MPV_EVENT_PLAYBACK_RESTART => {
                    #[cfg(debug_assertions)]
                    debug!("MPV Event Loop: MPV_EVENT_PLAYBACK_RESTART received.");

                    if notify_start {
                        notify_start = false;
                        emit_event(&app_handle, "file_loaded", ());
                    }
                }
                mpv_event_id::MPV_EVENT_SHUTDOWN => {
                    #[cfg(debug_assertions)]
                    debug!("MPV Event Loop: MPV_EVENT_SHUTDOWN received. Exiting.");
                    break;
                }
                mpv_event_id::MPV_EVENT_PROPERTY_CHANGE => {
                    let prop_event = (*event).data as *mut MpvEventProperty;

                    if !prop_event.is_null() {
                        let value_ptr = (*prop_event).data;
                        let mut should_emit_progress = true;

                        match (*event).reply_usrdata {
                            TIME_POS_ID => {
                                if (*prop_event).format == mpv_format::MPV_FORMAT_DOUBLE
                                    && !value_ptr.is_null()
                                {
                                    last_time_pos = *(value_ptr as *mut f64);
                                    last_buffered_pos = compute_buffered_pos(
                                        last_time_pos,
                                        last_duration,
                                        last_demuxer_cache_time,
                                    );
                                    #[cfg(debug_assertions)]
                                    trace!("MPV time-pos updated: {}", last_time_pos);
                                }
                            }
                            DURATION_ID => {
                                if (*prop_event).format == mpv_format::MPV_FORMAT_DOUBLE
                                    && !value_ptr.is_null()
                                {
                                    last_duration = *(value_ptr as *mut f64);
                                    last_buffered_pos = compute_buffered_pos(
                                        last_time_pos,
                                        last_duration,
                                        last_demuxer_cache_time,
                                    );
                                }
                            }
                            PAUSE_ID => {
                                if (*prop_event).format == mpv_format::MPV_FORMAT_FLAG
                                    && !value_ptr.is_null()
                                {
                                    let is_paused_int = *(value_ptr as *mut c_int);
                                    last_is_paused = is_paused_int != 0;
                                    #[cfg(any(target_os = "macos", target_os = "windows"))]
                                    {
                                        emit_pip_state_on_pause_change(
                                            &app_handle,
                                            last_is_paused,
                                            width,
                                            height,
                                            &mut last_pip_paused,
                                        );
                                    }
                                }
                            }
                            VIDEO_BITRATE_ID => {
                                if (*prop_event).format == mpv_format::MPV_FORMAT_DOUBLE
                                    && !value_ptr.is_null()
                                {
                                    let bitrate = *(value_ptr as *mut f64);
                                    last_video_bitrate = if bitrate.is_finite() && bitrate > 0.0 {
                                        bitrate
                                    } else {
                                        0.0
                                    };
                                }
                            }
                            MEDIA_TITLE_ID => {
                                if (*prop_event).format == mpv_format::MPV_FORMAT_NONE {
                                    if last_media_title.is_some() {
                                        last_media_title = None;
                                        emit_event(&app_handle, "mpv-media-title", "");
                                    }
                                } else {
                                    let title_ptr = mpv_get_property_string(
                                        event_client,
                                        media_title_name.as_ptr(),
                                    );
                                    if title_ptr.is_null() {
                                        #[cfg(debug_assertions)]
                                        debug!("mpv media title: <null>");
                                    } else {
                                        let c_str = CStr::from_ptr(title_ptr);
                                        let title = c_str.to_string_lossy().into_owned();
                                        if last_media_title.as_deref() != Some(title.as_str()) {
                                            last_media_title = Some(title.clone());
                                            emit_event(
                                                &app_handle,
                                                "mpv-media-title",
                                                title.clone(),
                                            );
                                        }
                                        // println!("mpv media title: {}", title);
                                        mpv_free(title_ptr as *mut c_void);
                                    }
                                }
                            }
                            EOF_REACHED_ID => {
                                if (*prop_event).format == mpv_format::MPV_FORMAT_FLAG
                                    && !value_ptr.is_null()
                                {
                                    let eof_reached_value = *(value_ptr as *mut c_int) != 0;
                                    if eof_reached_value {
                                        eof_reached.store(true, Ordering::SeqCst);
                                        is_playing.store(false, Ordering::Relaxed);
                                        last_is_paused = true;
                                        if !end_file_emitted_for_current_item {
                                            #[cfg(debug_assertions)]
                                            debug!(
                                                "MPV Event Loop: eof-reached=true received; synthesizing EOF event."
                                            );
                                            emit_end_file_and_progress(
                                                &app_handle,
                                                0,
                                                &mut last_time_pos,
                                                last_duration,
                                                &mut last_buffered_pos,
                                                &mut last_video_bitrate,
                                            );
                                            end_file_emitted_for_current_item = true;
                                            should_emit_progress = false;
                                        }
                                    } else {
                                        eof_reached.store(false, Ordering::SeqCst);
                                        end_file_emitted_for_current_item = false;
                                    }
                                }
                            }
                            DEMUXER_CACHE_TIME_ID => {
                                if (*prop_event).format == mpv_format::MPV_FORMAT_DOUBLE
                                    && !value_ptr.is_null()
                                {
                                    let cache_time = *(value_ptr as *mut f64);
                                    last_demuxer_cache_time = if cache_time.is_finite() {
                                        cache_time.max(0.0)
                                    } else {
                                        0.0
                                    };
                                    last_buffered_pos = compute_buffered_pos(
                                        last_time_pos,
                                        last_duration,
                                        last_demuxer_cache_time,
                                    );
                                    trace!("last_buffered_pos: {}", last_buffered_pos);
                                }
                            }
                            WIDTH_ID => {
                                if (*prop_event).format == mpv_format::MPV_FORMAT_INT64
                                    && !value_ptr.is_null()
                                {
                                    width = *(value_ptr as *mut i64);
                                }
                                emit_resize_if_changed(
                                    &app_handle,
                                    width,
                                    height,
                                    &mut last_emit_width,
                                    &mut last_emit_height,
                                );
                                #[cfg(any(target_os = "macos", target_os = "windows"))]
                                {
                                    emit_pip_state_on_video_size_change(
                                        &app_handle,
                                        last_is_paused,
                                        width,
                                        height,
                                        &mut last_pip_aspect_width,
                                        &mut last_pip_aspect_height,
                                        &mut last_pip_paused,
                                    );
                                }
                            }
                            HEIGHT_ID => {
                                if (*prop_event).format == mpv_format::MPV_FORMAT_INT64
                                    && !value_ptr.is_null()
                                {
                                    height = *(value_ptr as *mut i64);
                                }

                                emit_resize_if_changed(
                                    &app_handle,
                                    width,
                                    height,
                                    &mut last_emit_width,
                                    &mut last_emit_height,
                                );
                                #[cfg(any(target_os = "macos", target_os = "windows"))]
                                {
                                    emit_pip_state_on_video_size_change(
                                        &app_handle,
                                        last_is_paused,
                                        width,
                                        height,
                                        &mut last_pip_aspect_width,
                                        &mut last_pip_aspect_height,
                                        &mut last_pip_paused,
                                    );
                                }
                            }
                            TRACK_ID => {
                                if (*prop_event).format == mpv_format::MPV_FORMAT_NODE
                                    && !value_ptr.is_null()
                                {
                                    let node = value_ptr as *mut mpv_node;
                                    let json_track_list = parse_node(node);
                                    let mut tracks = Vec::new();
                                    if let Some(list) = json_track_list.as_array() {
                                        for item in list {
                                            let as_string = |key: &str| {
                                                item.get(key)
                                                    .and_then(|value| value.as_str())
                                                    .map(ToString::to_string)
                                            };
                                            let as_i64 = |key: &str| {
                                                item.get(key).and_then(|value| value.as_i64())
                                            };
                                            let as_f64 = |key: &str| {
                                                item.get(key).and_then(|value| value.as_f64())
                                            };
                                            let as_bool = |key: &str| {
                                                item.get(key).and_then(|value| value.as_bool())
                                            };
                                            tracks.push(MediaTrack {
                                                id: item["id"].as_i64().unwrap_or(0),
                                                track_type: item["type"]
                                                    .as_str()
                                                    .unwrap_or("")
                                                    .to_string(),
                                                title: item["title"]
                                                    .as_str()
                                                    .or(item["lang"].as_str())
                                                    .unwrap_or("Unknown")
                                                    .to_string(),
                                                lang: item["lang"]
                                                    .as_str()
                                                    .unwrap_or("")
                                                    .to_string(),
                                                selected: item["selected"]
                                                    .as_bool()
                                                    .unwrap_or(false),
                                                codec: as_string("codec"),
                                                codec_desc: as_string("codec-desc"),
                                                decoder_desc: as_string("decoder-desc"),
                                                demux_w: as_i64("demux-w"),
                                                demux_h: as_i64("demux-h"),
                                                demux_fps: as_f64("demux-fps"),
                                                demux_bitrate: as_i64("demux-bitrate"),
                                                demux_samplerate: as_i64("demux-samplerate"),
                                                demux_channels: as_string("demux-channels"),
                                                fps: as_f64("fps"),
                                                w: as_i64("w"),
                                                h: as_i64("h"),
                                                is_default: as_bool("default"),
                                                forced: as_bool("forced"),
                                                external: as_bool("external"),
                                            });
                                        }
                                        if !tracks.is_empty() {
                                            emit_event(
                                                &app_handle,
                                                "mpv-tracks-update",
                                                TracksPayload { tracks },
                                            );
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }

                        if should_emit_progress {
                            emit_progress(
                                &app_handle,
                                last_time_pos,
                                last_duration,
                                last_buffered_pos,
                                !last_is_paused,
                                last_video_bitrate,
                            );
                        }
                    }
                }
                mpv_event_id::MPV_EVENT_END_FILE => {
                    is_playing.store(false, Ordering::Relaxed);
                    let reason = if !(*event).data.is_null() {
                        let end_file = &*((*event).data as *const MpvEventEndFile);
                        end_file.reason
                    } else {
                        -1
                    };
                    eof_reached.store(reason == 0, Ordering::SeqCst);
                    if reason == 0 && end_file_emitted_for_current_item {
                        #[cfg(debug_assertions)]
                        debug!(
                            "MPV Event Loop: Skipping duplicate EOF end event from MPV_EVENT_END_FILE."
                        );
                    } else {
                        emit_end_file_and_progress(
                            &app_handle,
                            reason,
                            &mut last_time_pos,
                            last_duration,
                            &mut last_buffered_pos,
                            &mut last_video_bitrate,
                        );
                    }
                    end_file_emitted_for_current_item = reason == 0;
                }
                _ => {}
            }
        }
    }

    unsafe {
        mpv_destroy(event_client);
    }
    eof_reached.store(false, Ordering::SeqCst);
    info!("MPV Event Loop: Thread exited cleanly.");
}
