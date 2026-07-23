//! Contact-sheet (thumbnail grid) composition.
//!
//! Frames come from the same headless-mpv extractor that feeds the seek-bar
//! previews, just at a larger tile width. They're tiled into a single JPEG with
//! a burned-in timestamp per tile.
//!
//! Timestamps are drawn with a hand-rolled 5x7 bitmap font rather than a real
//! text stack: the only glyphs needed are digits and a colon, so pulling in a
//! font rasteriser and an embedded typeface would be all cost and no benefit.

use image::{ImageEncoder, RgbImage};
use std::path::Path;

const BACKGROUND: [u8; 3] = [18, 20, 24];
const GAP: u32 = 6;
const MARGIN: u32 = 10;
const JPEG_QUALITY: u8 = 90;

/// 5x7 glyphs, one byte per row, low 5 bits used (bit 4 = leftmost pixel).
const GLYPH_WIDTH: u32 = 5;
const GLYPH_HEIGHT: u32 = 7;

fn glyph(c: char) -> Option<[u8; 7]> {
    Some(match c {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        ':' => [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000],
        _ => return None,
    })
}

fn fill_rect(canvas: &mut RgbImage, x: u32, y: u32, w: u32, h: u32, colour: [u8; 3]) {
    let max_x = (x + w).min(canvas.width());
    let max_y = (y + h).min(canvas.height());
    for py in y..max_y {
        for px in x..max_x {
            canvas.put_pixel(px, py, image::Rgb(colour));
        }
    }
}

/// Draw `text` at `x`,`y` scaled by `scale`, returning the width drawn.
fn draw_text(canvas: &mut RgbImage, x: u32, y: u32, text: &str, scale: u32, colour: [u8; 3]) {
    let mut cursor = x;
    for c in text.chars() {
        let Some(rows) = glyph(c) else {
            cursor += (GLYPH_WIDTH + 1) * scale;
            continue;
        };
        for (row_index, row) in rows.iter().enumerate() {
            for col in 0..GLYPH_WIDTH {
                // bit 4 is the leftmost pixel of the 5-wide glyph
                if row & (1 << (GLYPH_WIDTH - 1 - col)) == 0 {
                    continue;
                }
                fill_rect(
                    canvas,
                    cursor + col * scale,
                    y + row_index as u32 * scale,
                    scale,
                    scale,
                    colour,
                );
            }
        }
        cursor += (GLYPH_WIDTH + 1) * scale;
    }
}

fn text_width(text: &str, scale: u32) -> u32 {
    if text.is_empty() {
        return 0;
    }
    (text.chars().count() as u32) * (GLYPH_WIDTH + 1) * scale - scale
}

fn format_timestamp(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        (total % 3600) / 60,
        total % 60
    )
}

/// Collect the extracted frames in capture order (mpv names them sequentially).
fn sorted_frames(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut frames: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
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

/// Extract `columns * rows` frames from `video_path` and tile them into a JPEG
/// at `output_path`. Blocking — run on a background thread.
pub fn export(
    video_path: &str,
    duration: f64,
    columns: u32,
    rows: u32,
    tile_width: u32,
    work_dir: &Path,
    output_path: &Path,
) -> Result<(), String> {
    let columns = columns.clamp(2, 8);
    let rows = rows.clamp(2, 12);
    let wanted = columns * rows;

    // Clear any leftovers so `sorted_frames` can't pick up a previous run's
    // frames and skew the timestamps.
    let _ = std::fs::remove_dir_all(work_dir);
    crate::mpv::generate_frames(video_path, work_dir, duration, wanted, tile_width)?;

    let frames = sorted_frames(work_dir);
    if frames.is_empty() {
        let _ = std::fs::remove_dir_all(work_dir);
        return Err("No frames could be extracted".to_string());
    }

    // mpv's step count isn't exact, so lay out whatever actually came back.
    let usable = frames.len().min(wanted as usize);
    let used_rows = ((usable as u32) + columns - 1) / columns;

    let first = image::open(&frames[0])
        .map_err(|error| format!("Failed to read frame: {error}"))?
        .to_rgb8();
    let (tile_w, tile_h) = (first.width(), first.height());

    let sheet_w = MARGIN * 2 + columns * tile_w + (columns - 1) * GAP;
    let sheet_h = MARGIN * 2 + used_rows * tile_h + used_rows.saturating_sub(1) * GAP;
    let mut canvas = RgbImage::from_pixel(sheet_w, sheet_h, image::Rgb(BACKGROUND));

    // Frame i sits at the midpoint of its slice of the timeline, matching how
    // the extractor steps through the file.
    let slice = duration / usable.max(1) as f64;
    let label_scale = (tile_w / 160).max(1);
    let label_h = GLYPH_HEIGHT * label_scale;

    for (index, frame_path) in frames.iter().take(usable).enumerate() {
        let Ok(tile) = image::open(frame_path).map(|img| img.to_rgb8()) else {
            continue;
        };
        let col = index as u32 % columns;
        let row = index as u32 / columns;
        let x0 = MARGIN + col * (tile_w + GAP);
        let y0 = MARGIN + row * (tile_h + GAP);

        for (px, py, pixel) in tile.enumerate_pixels() {
            if x0 + px >= sheet_w || y0 + py >= sheet_h {
                continue;
            }
            canvas.put_pixel(x0 + px, y0 + py, *pixel);
        }

        let label = format_timestamp(slice * (index as f64 + 0.5));
        let label_w = text_width(&label, label_scale);
        let pad = 3 * label_scale;
        let box_x = x0 + tile_w.saturating_sub(label_w + pad * 2 + 2);
        let box_y = y0 + tile_h.saturating_sub(label_h + pad * 2 + 2);
        fill_rect(
            &mut canvas,
            box_x,
            box_y,
            label_w + pad * 2,
            label_h + pad * 2,
            [12, 12, 12],
        );
        draw_text(
            &mut canvas,
            box_x + pad,
            box_y + pad,
            &label,
            label_scale,
            [255, 255, 255],
        );
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create output folder: {error}"))?;
    }
    let file = std::fs::File::create(output_path)
        .map_err(|error| format!("Failed to create contact sheet: {error}"))?;
    image::codecs::jpeg::JpegEncoder::new_with_quality(
        std::io::BufWriter::new(file),
        JPEG_QUALITY,
    )
    .write_image(
        canvas.as_raw(),
        canvas.width(),
        canvas.height(),
        image::ExtendedColorType::Rgb8,
    )
    .map_err(|error| format!("Failed to encode contact sheet: {error}"))?;

    let _ = std::fs::remove_dir_all(work_dir);
    Ok(())
}
