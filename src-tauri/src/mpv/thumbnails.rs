use std::ffi::{c_char, c_void, CString};
use std::path::{Path, PathBuf};

use super::ffi::{
    mpv_command, mpv_create, mpv_event_id, mpv_initialize, mpv_set_option_string,
    mpv_terminate_destroy, mpv_wait_event,
};

/// Set LUMO_MPV_DEBUG=1 to let the headless helpers log to the terminal; the
/// extractors are otherwise silent, which makes a failure look like "no frames".
fn apply_log_options(ctx: *mut c_void) {
    if std::env::var_os("LUMO_MPV_DEBUG").is_some() {
        set_opt(ctx, "terminal", "yes");
        set_opt(ctx, "msg-level", "all=debug");
    } else {
        set_opt(ctx, "really-quiet", "yes");
    }
}

fn set_opt(ctx: *mut c_void, name: &str, value: &str) {
    if let (Ok(c_name), Ok(c_val)) = (CString::new(name), CString::new(value)) {
        unsafe {
            mpv_set_option_string(ctx, c_name.as_ptr(), c_val.as_ptr());
        }
    }
}

fn command(ctx: *mut c_void, args: &[&str]) -> bool {
    let cstrings: Vec<CString> = args
        .iter()
        .filter_map(|arg| CString::new(*arg).ok())
        .collect();
    if cstrings.len() != args.len() {
        return false;
    }
    let mut ptrs: Vec<*const c_char> = cstrings.iter().map(|c| c.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    unsafe { mpv_command(ctx, ptrs.as_ptr()) == 0 }
}

fn count_jpgs(dir: &Path) -> u32 {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case("jpg"))
                        .unwrap_or(false)
                })
                .count() as u32
        })
        .unwrap_or(0)
}

/// Seek-bar preview width. Small enough that a full set is cheap to generate.
const SEEK_THUMB_WIDTH: u32 = 240;

/// Generate ~`count` downscaled JPEG thumbnails for `path` into `outdir`, evenly
/// spaced across the file, using a headless second libmpv instance. `vo=image`
/// writes each rendered frame to disk and `sstep` advances one frame per step,
/// so no window / render context / screenshot support is required. Blocking —
/// run this on a background thread.
pub fn generate_thumbnails(
    path: &str,
    outdir: &Path,
    duration: f64,
    count: u32,
) -> Result<u32, String> {
    generate_frames(path, outdir, duration, count, SEEK_THUMB_WIDTH)
}

/// Grab a single representative frame for use as a poster, written into
/// `outdir` as a JPEG. Seeks to 20% of the file so it lands past titles and
/// fade-ins, and bounds the decode with `length` so a long film doesn't dump
/// thousands of frames. Returns the written file. Blocking.
pub fn generate_poster(path: &str, outdir: &Path, width: u32) -> Result<PathBuf, String> {
    let _ = std::fs::remove_dir_all(outdir);
    std::fs::create_dir_all(outdir).map_err(|err| err.to_string())?;

    let ctx = unsafe { mpv_create() };
    if ctx.is_null() {
        return Err("mpv_create failed".to_string());
    }

    let outdir_str = outdir.to_string_lossy().to_string();
    set_opt(ctx, "audio", "no");
    set_opt(ctx, "hwdec", "no");
    set_opt(ctx, "sub", "no");
    set_opt(ctx, "load-scripts", "no");
    set_opt(ctx, "osc", "no");
    set_opt(ctx, "vo", "image");
    set_opt(ctx, "vo-image-format", "jpg");
    set_opt(ctx, "vo-image-jpeg-quality", "85");
    set_opt(ctx, "vo-image-outdir", &outdir_str);
    // Percentage start works without knowing the duration up front.
    set_opt(ctx, "start", "20%");
    set_opt(ctx, "length", "0.2");
    set_opt(ctx, "vf", &format!("scale={}:-2", width.max(16)));
    apply_log_options(ctx);

    if unsafe { mpv_initialize(ctx) } < 0 {
        unsafe { mpv_terminate_destroy(ctx) };
        return Err("mpv_initialize failed".to_string());
    }
    if !command(ctx, &["loadfile", path]) {
        unsafe { mpv_terminate_destroy(ctx) };
        return Err("loadfile failed".to_string());
    }

    loop {
        let event = unsafe { mpv_wait_event(ctx, 25.0) };
        if event.is_null() {
            break;
        }
        match unsafe { &(*event).event_id } {
            mpv_event_id::MPV_EVENT_END_FILE
            | mpv_event_id::MPV_EVENT_SHUTDOWN
            | mpv_event_id::MPV_EVENT_NONE => break,
            _ => {}
        }
    }

    unsafe { mpv_terminate_destroy(ctx) };

    let mut frames: Vec<PathBuf> = std::fs::read_dir(outdir)
        .map(|rd| {
            rd.flatten()
                .map(|entry| entry.path())
                .filter(|p| {
                    p.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case("jpg"))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    frames.sort();
    frames
        .into_iter()
        .next()
        .ok_or_else(|| "No frame could be extracted".to_string())
}

/// Extract every frame of `[start, end)` at a fixed `fps` and `width`, in order,
/// as JPEGs in `outdir`. Unlike `generate_frames` this decodes the range
/// straight through (an `fps` filter rather than per-frame seeking), which is
/// both far faster and frame-accurate — what a GIF needs. Blocking.
pub fn generate_range_frames(
    path: &str,
    outdir: &Path,
    start: f64,
    end: f64,
    fps: u32,
    width: u32,
) -> Result<u32, String> {
    if !(end > start) {
        return Err("Invalid range".to_string());
    }
    std::fs::create_dir_all(outdir).map_err(|err| err.to_string())?;

    let ctx = unsafe { mpv_create() };
    if ctx.is_null() {
        return Err("mpv_create failed".to_string());
    }

    let outdir_str = outdir.to_string_lossy().to_string();
    set_opt(ctx, "audio", "no");
    set_opt(ctx, "hwdec", "no");
    set_opt(ctx, "sub", "no");
    set_opt(ctx, "load-scripts", "no");
    set_opt(ctx, "osc", "no");
    set_opt(ctx, "vo", "image");
    set_opt(ctx, "vo-image-format", "jpg");
    set_opt(ctx, "vo-image-jpeg-quality", "92");
    set_opt(ctx, "vo-image-outdir", &outdir_str);
    set_opt(ctx, "start", &format!("{start}"));
    set_opt(ctx, "end", &format!("{end}"));
    // width == 0 means "keep the source resolution" — skip the scaler entirely
    // rather than round-tripping through an identity scale.
    let filters = if width == 0 {
        format!("fps={}", fps.max(1))
    } else {
        format!("fps={},scale={}:-2:flags=lanczos", fps.max(1), width.max(16))
    };
    set_opt(ctx, "vf", &filters);
    apply_log_options(ctx);

    if unsafe { mpv_initialize(ctx) } < 0 {
        unsafe { mpv_terminate_destroy(ctx) };
        return Err("mpv_initialize failed".to_string());
    }
    if !command(ctx, &["loadfile", path]) {
        unsafe { mpv_terminate_destroy(ctx) };
        return Err("loadfile failed".to_string());
    }

    loop {
        let event = unsafe { mpv_wait_event(ctx, 30.0) };
        if event.is_null() {
            break;
        }
        match unsafe { &(*event).event_id } {
            mpv_event_id::MPV_EVENT_END_FILE
            | mpv_event_id::MPV_EVENT_SHUTDOWN
            | mpv_event_id::MPV_EVENT_NONE => break,
            _ => {}
        }
    }

    unsafe { mpv_terminate_destroy(ctx) };
    Ok(count_jpgs(outdir))
}

/// As `generate_thumbnails`, but with an explicit tile width — the contact
/// sheet wants frames several times larger than the seek-bar previews.
pub fn generate_frames(
    path: &str,
    outdir: &Path,
    duration: f64,
    count: u32,
    width: u32,
) -> Result<u32, String> {
    if duration <= 0.0 {
        return Err("Unknown duration".to_string());
    }
    std::fs::create_dir_all(outdir).map_err(|err| err.to_string())?;
    let step = (duration / count.max(1) as f64).max(0.05);

    let ctx = unsafe { mpv_create() };
    if ctx.is_null() {
        return Err("mpv_create failed".to_string());
    }

    let outdir_str = outdir.to_string_lossy().to_string();
    set_opt(ctx, "audio", "no");
    set_opt(ctx, "hwdec", "no");
    set_opt(ctx, "sub", "no");
    set_opt(ctx, "load-scripts", "no");
    set_opt(ctx, "osc", "no");
    set_opt(ctx, "vo", "image");
    set_opt(ctx, "vo-image-format", "jpg");
    set_opt(ctx, "vo-image-jpeg-quality", "80");
    set_opt(ctx, "vo-image-outdir", &outdir_str);
    set_opt(ctx, "sstep", &format!("{step}"));
    set_opt(ctx, "vf", &format!("scale={}:-2", width.max(16)));
    apply_log_options(ctx);

    if unsafe { mpv_initialize(ctx) } < 0 {
        unsafe { mpv_terminate_destroy(ctx) };
        return Err("mpv_initialize failed".to_string());
    }

    if !command(ctx, &["loadfile", path]) {
        unsafe { mpv_terminate_destroy(ctx) };
        return Err("loadfile failed".to_string());
    }

    // Pump events until the file ends. A per-wait timeout doubles as a stall
    // guard (a hung decode won't block the thread forever).
    loop {
        let event = unsafe { mpv_wait_event(ctx, 20.0) };
        if event.is_null() {
            break;
        }
        match unsafe { &(*event).event_id } {
            mpv_event_id::MPV_EVENT_END_FILE
            | mpv_event_id::MPV_EVENT_SHUTDOWN
            | mpv_event_id::MPV_EVENT_NONE => break,
            _ => {}
        }
    }

    unsafe { mpv_terminate_destroy(ctx) };
    Ok(count_jpgs(outdir))
}
