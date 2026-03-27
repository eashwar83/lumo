use std::ffi::c_void;
use std::os::raw::{c_char, c_int};

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn ensure_numeric_locale_for_mpv() {
    // libmpv requires LC_NUMERIC to be "C" for mpv_create().
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, b"C\0".as_ptr().cast());
    }
}

#[repr(C)]
#[derive(Debug, PartialEq)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
pub(crate) enum mpv_format {
    MPV_FORMAT_NONE = 0,
    MPV_FORMAT_STRING = 1,
    MPV_FORMAT_OSD_STRING = 2,
    MPV_FORMAT_FLAG = 3,
    MPV_FORMAT_INT64 = 4,
    MPV_FORMAT_DOUBLE = 5,
    MPV_FORMAT_NODE = 6,
    MPV_FORMAT_NODE_ARRAY = 7,
    MPV_FORMAT_NODE_MAP = 8,
}

impl std::fmt::Display for mpv_format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            mpv_format::MPV_FORMAT_NONE => write!(f, "MPV_FORMAT_NONE"),
            mpv_format::MPV_FORMAT_STRING => write!(f, "MPV_FORMAT_STRING"),
            mpv_format::MPV_FORMAT_OSD_STRING => write!(f, "MPV_FORMAT_OSD_STRING"),
            mpv_format::MPV_FORMAT_FLAG => write!(f, "MPV_FORMAT_FLAG"),
            mpv_format::MPV_FORMAT_INT64 => write!(f, "MPV_FORMAT_INT64"),
            mpv_format::MPV_FORMAT_DOUBLE => write!(f, "MPV_FORMAT_DOUBLE"),
            mpv_format::MPV_FORMAT_NODE => write!(f, "MPV_FORMAT_NODE"),
            mpv_format::MPV_FORMAT_NODE_ARRAY => write!(f, "MPV_FORMAT_NODE_ARRAY"),
            mpv_format::MPV_FORMAT_NODE_MAP => write!(f, "MPV_FORMAT_NODE_MAP"),
        }
    }
}

#[repr(C)]
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub(crate) enum mpv_event_id {
    MPV_EVENT_NONE = 0,
    MPV_EVENT_SHUTDOWN = 1,
    MPV_EVENT_LOG_MESSAGE = 2,
    MPV_EVENT_GET_PROPERTY_REPLY = 3,
    MPV_EVENT_SET_PROPERTY_REPLY = 4,
    MPV_EVENT_COMMAND_REPLY = 5,
    MPV_EVENT_START_FILE = 6,
    MPV_EVENT_END_FILE = 7,
    MPV_EVENT_FILE_LOADED = 8,
    MPV_EVENT_IDLE = 11,
    MPV_EVENT_TICK = 14,
    MPV_EVENT_CLIENT_MESSAGE = 16,
    MPV_EVENT_VIDEO_RECONFIG = 17,
    MPV_EVENT_AUDIO_RECONFIG = 18,
    MPV_EVENT_SEEK = 20,
    MPV_EVENT_PLAYBACK_RESTART = 21,
    MPV_EVENT_PROPERTY_CHANGE = 22,
    MPV_EVENT_QUEUE_OVERFLOW = 24,
    MPV_EVENT_HOOK = 25,
}

impl From<i32> for mpv_event_id {
    fn from(id: i32) -> Self {
        match id {
            0 => mpv_event_id::MPV_EVENT_NONE,
            1 => mpv_event_id::MPV_EVENT_SHUTDOWN,
            2 => mpv_event_id::MPV_EVENT_LOG_MESSAGE,
            3 => mpv_event_id::MPV_EVENT_GET_PROPERTY_REPLY,
            4 => mpv_event_id::MPV_EVENT_SET_PROPERTY_REPLY,
            5 => mpv_event_id::MPV_EVENT_COMMAND_REPLY,
            6 => mpv_event_id::MPV_EVENT_START_FILE,
            7 => mpv_event_id::MPV_EVENT_END_FILE,
            8 => mpv_event_id::MPV_EVENT_FILE_LOADED,
            11 => mpv_event_id::MPV_EVENT_IDLE,
            14 => mpv_event_id::MPV_EVENT_TICK,
            16 => mpv_event_id::MPV_EVENT_CLIENT_MESSAGE,
            17 => mpv_event_id::MPV_EVENT_VIDEO_RECONFIG,
            18 => mpv_event_id::MPV_EVENT_AUDIO_RECONFIG,
            20 => mpv_event_id::MPV_EVENT_SEEK,
            21 => mpv_event_id::MPV_EVENT_PLAYBACK_RESTART,
            22 => mpv_event_id::MPV_EVENT_PROPERTY_CHANGE,
            24 => mpv_event_id::MPV_EVENT_QUEUE_OVERFLOW,
            25 => mpv_event_id::MPV_EVENT_HOOK,
            _ => {
                eprintln!("Received unknown MPV event ID: {}", id);
                mpv_event_id::MPV_EVENT_NONE
            }
        }
    }
}

#[repr(C)]
pub(crate) struct mpv_node {
    pub u: mpv_node_union,
    pub format: mpv_format,
}

#[repr(C)]
pub(crate) union mpv_node_union {
    pub string: *mut c_char,
    pub flag: i32,
    pub int64: i64,
    pub double: f64,
    pub list: *mut mpv_node_list,
}

#[repr(C)]
pub(crate) struct mpv_node_list {
    pub num: i32,
    pub values: *mut mpv_node,
    pub keys: *mut *mut c_char,
}

#[repr(C)]
#[derive(Debug)]
pub(crate) struct MpvEventProperty {
    pub name: *const c_char,
    pub format: mpv_format,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Debug)]
pub(crate) struct MpvEvent {
    pub event_id: mpv_event_id,
    pub error: c_int,
    pub reply_usrdata: u64,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Debug)]
pub(crate) struct MpvEventEndFile {
    pub reason: c_int,
    pub error: c_int,
    pub playlist_entry_id: i64,
    pub playlist_insert_id: i64,
    pub playlist_insert_num_entries: c_int,
}

unsafe extern "C" {
    pub(super) fn mpv_create() -> *mut c_void;
    pub(super) fn mpv_initialize(ctx: *mut c_void) -> c_int;
    pub(super) fn mpv_command(ctx: *mut c_void, args: *const *const c_char) -> c_int;
    pub(super) fn mpv_set_option(
        ctx: *mut c_void,
        name: *const c_char,
        format: mpv_format,
        data: *mut c_void,
    ) -> c_int;
    pub(super) fn mpv_set_option_string(
        ctx: *mut c_void,
        name: *const c_char,
        data: *const c_char,
    ) -> c_int;
    pub(super) fn mpv_destroy(ctx: *mut c_void);
    pub(super) fn mpv_terminate_destroy(ctx: *mut c_void);
    pub(super) fn mpv_create_client(ctx: *mut c_void, name: *const c_char) -> *mut c_void;
    pub(super) fn mpv_observe_property(
        ctx: *mut c_void,
        reply_usrdata: u64,
        name: *const c_char,
        format: mpv_format,
    ) -> c_int;
    pub(super) fn mpv_wait_event(ctx: *mut c_void, timeout: f64) -> *mut MpvEvent;
    pub(super) fn mpv_get_property_string(ctx: *mut c_void, name: *const c_char) -> *mut c_char;
    pub(super) fn mpv_free(data: *mut c_void);
}

#[repr(C)]
pub(crate) struct SoiaUtils {
    _private: [u8; 0],
}

#[link(name = "soia_utils")]
unsafe extern "C" {
    pub(super) fn soia_utils_create(
        mpv_ctx: *mut c_void,
        window: *const c_void,
        display: *const c_void,
        mode: i32,
    ) -> *mut SoiaUtils;
    pub(super) fn soia_utils_render_target_resize(
        utils: *mut SoiaUtils,
        width: u32,
        height: u32,
    );
    pub(super) fn soia_utils_render_context_update(utils: *mut SoiaUtils);
    pub(super) fn soia_utils_uses_render_context(utils: *mut SoiaUtils) -> c_int;
    pub(super) fn soia_utils_destroy(utils: *mut SoiaUtils);
}
