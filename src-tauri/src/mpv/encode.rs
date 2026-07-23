//! Clip and GIF export from a time range.
//!
//! The bundled ffmpeg is a *playback* build: it ships no muxers and no video
//! encoders (verified — `av_guess_format` fails for every container), so mpv's
//! own encode mode can't be used no matter how it's configured. Instead:
//!
//!   * **GIF** is produced entirely in-process. The headless frame extractor
//!     already works, so we dump the range as JPEGs at a fixed fps and encode
//!     the GIF with the pure-Rust `image` encoder. No external binary.
//!   * **Video clips** need a real muxer, which only a full ffmpeg has. We
//!     locate one at runtime (a bundled sidecar, the user's configured path, or
//!     `ffmpeg` on PATH) and stream-copy the range. Absent ffmpeg, the feature
//!     reports that clearly instead of failing obscurely.

use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, RgbaImage};
use std::path::{Path, PathBuf};
use std::process::Command;

/// GIF is a 256-colour, LZW-compressed format with no real interframe
/// compression, so size grows brutally with resolution and frame rate. The cap
/// is a total *pixel budget* rather than a duration limit: that lets a small
/// GIF run long and a full-resolution one stay short, instead of imposing one
/// arbitrary number on both.
pub const GIF_MAX_SECONDS: f64 = 60.0;
/// ~1.6 gigapixels ≈ 30s of 480p at 12fps, or ~4s of 1080p at 12fps.
const GIF_PIXEL_BUDGET: u64 = 1_600_000_000;
pub const GIF_DEFAULT_WIDTH: u32 = 720;
pub const GIF_DEFAULT_FPS: u32 = 12;

/// Collect the extracted frames in capture order (mpv names them sequentially).
fn sorted_frames(dir: &Path) -> Vec<PathBuf> {
    let mut frames: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case("jpg"))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    frames.sort();
    frames
}

/// Reject a request that would produce an absurd file before spending minutes
/// on it. `source_height` is only known for a source-resolution export.
pub fn check_gif_budget(
    seconds: f64,
    width: u32,
    height: u32,
    fps: u32,
) -> Result<(), String> {
    if seconds > GIF_MAX_SECONDS {
        return Err(format!("GIFs are limited to {GIF_MAX_SECONDS:.0} seconds"));
    }
    if width == 0 || height == 0 {
        return Ok(());
    }
    let pixels = (width as u64) * (height as u64) * (fps as u64).max(1)
        * seconds.max(0.0).ceil() as u64;
    if pixels > GIF_PIXEL_BUDGET {
        // Tell the user the length that *would* fit, which is more useful than
        // an abstract complaint about pixels.
        let per_second = (width as u64) * (height as u64) * (fps as u64).max(1);
        let allowed = GIF_PIXEL_BUDGET / per_second.max(1);
        return Err(format!(
            "Too large for a GIF at {width}px / {fps}fps — keep it under {allowed}s, \
             or pick a smaller size"
        ));
    }
    Ok(())
}

/// Render `[start, end)` of `input` as an animated GIF at `width` (0 = keep the
/// source resolution) and `fps`. Returns the encoded pixel dimensions.
/// Blocking.
pub fn export_gif(
    input: &str,
    output: &Path,
    start: f64,
    end: f64,
    work_dir: &Path,
    width: u32,
    fps: u32,
) -> Result<(u32, u32), String> {
    if !(end > start) {
        return Err("Invalid range".to_string());
    }
    let fps = fps.clamp(5, 30);
    check_gif_budget(end - start, width, if width == 0 { 0 } else { width }, fps)?;

    // Clear leftovers so a previous run's frames can't leak into this GIF.
    let _ = std::fs::remove_dir_all(work_dir);
    super::generate_range_frames(input, work_dir, start, end, fps, width)?;

    let frames = sorted_frames(work_dir);
    if frames.is_empty() {
        let _ = std::fs::remove_dir_all(work_dir);
        return Err("No frames could be extracted".to_string());
    }

    // Now that the real frame size is known, re-check the budget — a
    // source-resolution export couldn't be judged up front.
    let first = image::open(&frames[0])
        .map_err(|error| format!("Failed to read frame: {error}"))?;
    let (real_w, real_h) = (first.width(), first.height());
    if let Err(error) = check_gif_budget(end - start, real_w, real_h, fps) {
        let _ = std::fs::remove_dir_all(work_dir);
        return Err(error);
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let file = std::fs::File::create(output)
        .map_err(|error| format!("Failed to create GIF: {error}"))?;

    // Speed is 1..30, lower being a better palette search. Large frames make
    // the search expensive, so ease off as the resolution climbs.
    let speed = if real_w >= 1280 { 20 } else { 10 };
    let mut encoder = GifEncoder::new_with_speed(std::io::BufWriter::new(file), speed);
    encoder
        .set_repeat(Repeat::Infinite)
        .map_err(|error| format!("Failed to start GIF: {error}"))?;

    let delay = Delay::from_numer_denom_ms(1000, fps);
    let mut written = 0u32;
    for frame_path in &frames {
        let Ok(image) = image::open(frame_path) else {
            continue;
        };
        let rgba: RgbaImage = image.to_rgba8();
        encoder
            .encode_frame(Frame::from_parts(rgba, 0, 0, delay))
            .map_err(|error| format!("Failed to encode GIF frame: {error}"))?;
        written += 1;
    }
    // Dropping the encoder writes the GIF trailer.
    drop(encoder);
    let _ = std::fs::remove_dir_all(work_dir);

    if written == 0 {
        let _ = std::fs::remove_file(output);
        return Err("No frames could be encoded".to_string());
    }
    Ok((real_w, real_h))
}

/// ffmpeg is a console application: launched normally on Windows it flashes a
/// console window over the video. Every spawn must go through this.
fn quiet_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    command
}

/// Locate an ffmpeg executable: an explicitly configured path, one shipped
/// beside our own binary, or whatever is on PATH.
pub fn find_ffmpeg(configured: Option<&str>) -> Option<PathBuf> {
    let runnable = |path: &Path| -> bool {
        quiet_command(path)
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    };

    if let Some(raw) = configured.map(str::trim).filter(|s| !s.is_empty()) {
        let path = PathBuf::from(raw);
        if runnable(&path) {
            return Some(path);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sidecar = dir.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
            if sidecar.exists() && runnable(&sidecar) {
                return Some(sidecar);
            }
        }
    }
    let bare = PathBuf::from("ffmpeg");
    if runnable(&bare) {
        return Some(bare);
    }
    None
}

/// Export `[start, end)` of `input` into `output`, re-encoded so the cut is
/// frame-exact. Blocking.
///
/// A stream copy would be faster and lossless, but it can only cut on a
/// keyframe: on a typical 10-second GOP, asking for 60s-64s yields 51s-64s.
/// Silently handing back nine seconds of extra footage is worse than spending a
/// few seconds re-encoding, so the export is precise and the quality is set
/// high enough (CRF 18) to be visually transparent.
pub fn export_clip(
    ffmpeg: &Path,
    input: &str,
    output: &Path,
    start: f64,
    end: f64,
) -> Result<(), String> {
    if !(end > start) {
        return Err("Invalid range".to_string());
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let _ = std::fs::remove_file(output);

    // `-ss` before `-i` seeks by keyframe then decodes forward to the exact
    // point, so it is both fast and accurate once we are re-encoding. Only the
    // first video and audio streams are mapped; stray data/subtitle streams
    // otherwise break the mp4 muxer.
    let mut command = quiet_command(ffmpeg);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-ss")
        .arg(format!("{start}"))
        .arg("-i")
        .arg(input)
        .arg("-t")
        .arg(format!("{}", end - start))
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a:0?")
        .arg("-c:v")
        .arg("libx264")
        .arg("-crf")
        .arg("18")
        .arg("-preset")
        .arg("veryfast")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("192k")
        .arg("-movflags")
        .arg("+faststart")
        .arg(output);

    let result = command
        .output()
        .map_err(|error| format!("Failed to run ffmpeg: {error}"))?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let detail = stderr.lines().last().unwrap_or("unknown error").trim();
        let _ = std::fs::remove_file(output);
        return Err(format!("ffmpeg failed: {detail}"));
    }
    match std::fs::metadata(output) {
        Ok(meta) if meta.len() > 0 => Ok(()),
        _ => {
            let _ = std::fs::remove_file(output);
            Err("ffmpeg produced no output".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scene detection on a real film. Opt-in; set LUMO_ENCODE_TEST_INPUT and
    /// LUMO_TEST_DURATION. `cargo test --release --lib -- --nocapture --ignored scenes`
    #[test]
    #[ignore]
    fn detects_scenes() {
        let Ok(input) = std::env::var("LUMO_ENCODE_TEST_INPUT") else {
            panic!("set LUMO_ENCODE_TEST_INPUT");
        };
        let duration: f64 = std::env::var("LUMO_TEST_DURATION")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(600.0);
        let dir = std::env::temp_dir().join("lumo_scene_test");
        let started = std::time::Instant::now();
        let scenes = crate::scenes::detect(&input, duration, &dir).expect("detect");
        println!(
            "{} scenes over {duration:.0}s in {:.1}s",
            scenes.len(),
            started.elapsed().as_secs_f64()
        );
        for scene in scenes.iter().take(12) {
            let t = scene.start as u64;
            println!("  {:02}:{:02}:{:02}", t / 3600, (t % 3600) / 60, t % 60);
        }
        assert!(scenes.len() > 1, "no scenes detected");
    }

    /// Poster extraction seeks by percentage, which only works if mpv accepts
    /// `start=20%` without knowing the duration. Opt-in:
    /// `cargo test --release --lib -- --nocapture --ignored poster`
    #[test]
    #[ignore]
    fn extracts_a_poster() {
        let Ok(input) = std::env::var("LUMO_ENCODE_TEST_INPUT") else {
            panic!("set LUMO_ENCODE_TEST_INPUT");
        };
        let dir = std::env::temp_dir().join("lumo_poster_test");
        let result = super::super::generate_poster(&input, &dir, 320);
        match &result {
            Ok(path) => {
                let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                println!("poster: {} ({size} bytes)", path.display());
                assert!(size > 0, "poster is empty");
            }
            Err(error) => panic!("poster failed: {error}"),
        }
    }

    /// End-to-end check against a real file. Opt-in because it needs media on
    /// disk: set LUMO_ENCODE_TEST_INPUT to a video path and run
    /// `cargo test --release --lib -- --nocapture --ignored exports`.
    #[test]
    #[ignore]
    fn exports_gif_and_clip() {
        let Ok(input) = std::env::var("LUMO_ENCODE_TEST_INPUT") else {
            panic!("set LUMO_ENCODE_TEST_INPUT");
        };
        let dir = std::env::temp_dir().join("lumo_encode_test");
        let _ = std::fs::create_dir_all(&dir);

        for (name, width) in [("gif720.gif", 720u32), ("gifsrc.gif", 0u32)] {
            let gif = dir.join(name);
            let _ = std::fs::remove_file(&gif);
            let result = export_gif(
                &input,
                &gif,
                60.0,
                63.0,
                &dir.join("work"),
                width,
                GIF_DEFAULT_FPS,
            );
            let size = std::fs::metadata(&gif).map(|m| m.len()).unwrap_or(0);
            println!("{name}: {result:?} size={size}");
            assert!(result.is_ok(), "{name} failed: {result:?}");
            assert!(size > 0, "{name} is empty");
        }

        match find_ffmpeg(None) {
            Some(ffmpeg) => {
                println!("ffmpeg: {}", ffmpeg.display());
                let clip = dir.join("clip.mp4");
                let result = export_clip(&ffmpeg, &input, &clip, 60.0, 64.0);
                let size = std::fs::metadata(&clip).map(|m| m.len()).unwrap_or(0);
                println!("clip: {result:?} size={size}");
                assert!(result.is_ok(), "clip failed: {result:?}");
                assert!(size > 0, "clip is empty");
            }
            None => println!("ffmpeg not found — clip export skipped"),
        }
    }
}
