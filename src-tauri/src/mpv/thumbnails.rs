use std::ffi::{c_char, c_void, CString};
use std::path::Path;

use super::ffi::{
    mpv_command, mpv_create, mpv_event_id, mpv_initialize, mpv_set_option_string,
    mpv_terminate_destroy, mpv_wait_event,
};

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
    set_opt(ctx, "vf", "scale=240:-2");
    set_opt(ctx, "really-quiet", "yes");

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
