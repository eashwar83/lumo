//! Scene detection.
//!
//! Old films routinely ship with no chapters at all, which makes a two- or
//! three-hour runtime navigable only by blind scrubbing. Rather than require a
//! full decode, this reuses the seek-thumbnail extractor: sample the film on a
//! coarse grid, compare consecutive frames, and call a large jump a cut.
//!
//! That means the boundaries land on the sampling grid, not on the exact frame
//! — which is the right trade for navigation. Someone jumping to "the scene
//! where they arrive at the house" does not care about a few seconds, and an
//! exact-cut detector would need to decode every frame of the file.

use image::GenericImageView;
use serde::Serialize;
use std::path::Path;

/// Seconds between samples. Sparse sampling cannot see individual cuts — two
/// frames ten seconds apart are usually different shots regardless — so this
/// looks for *scene-level* change (a new location, a lighting change), which is
/// what someone navigating a film actually wants.
const SAMPLE_INTERVAL: f64 = 10.0;
const SAMPLE_WIDTH: u32 = 160;
/// Below this many samples the file is too short to be worth indexing.
const MIN_SAMPLES: u32 = 8;
const MAX_SAMPLES: u32 = 1200;
/// Fraction of sample gaps treated as ordinary change; only the tail above this
/// counts as a scene boundary. Adaptive, because absolute difference varies
/// hugely between a dark 1980s transfer and a bright modern one.
const CHANGE_PERCENTILE: f64 = 0.90;
/// Floor for the adaptive threshold, so a static film doesn't produce noise.
const MIN_CHANGE: f64 = 18.0;
/// Never emit scenes closer together than this.
const MIN_SCENE_SECONDS: f64 = 45.0;
/// Upper bound on markers; beyond this the strip stops being navigable.
const MAX_SCENES: usize = 80;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    /// Seconds from the start of the file.
    pub start: f64,
}

/// A coarse signature of one frame: an 8x8 grid of average grey levels. Robust
/// to compression noise and cheap to compare.
fn signature(path: &Path) -> Option<Vec<f64>> {
    let image = image::open(path).ok()?;
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let grid = 8u32;
    let mut cells = vec![0f64; (grid * grid) as usize];
    let mut counts = vec![0f64; (grid * grid) as usize];
    let rgb = image.to_rgb8();
    for (x, y, pixel) in rgb.enumerate_pixels() {
        let cx = (x * grid / w).min(grid - 1);
        let cy = (y * grid / h).min(grid - 1);
        let index = (cy * grid + cx) as usize;
        let [r, g, b] = pixel.0;
        cells[index] += 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
        counts[index] += 1.0;
    }
    for (cell, count) in cells.iter_mut().zip(counts.iter()) {
        if *count > 0.0 {
            *cell /= *count;
        }
    }
    Some(cells)
}

fn distance(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .sum::<f64>()
        / a.len() as f64
}

/// Detect scene starts across `duration` seconds of `video_path`. Blocking.
pub fn detect(
    video_path: &str,
    duration: f64,
    work_dir: &Path,
) -> Result<Vec<Scene>, String> {
    if duration <= 0.0 {
        return Err("Unknown duration".to_string());
    }
    let count = ((duration / SAMPLE_INTERVAL).round() as u32).clamp(MIN_SAMPLES, MAX_SAMPLES);

    let _ = std::fs::remove_dir_all(work_dir);
    crate::mpv::generate_frames(video_path, work_dir, duration, count, SAMPLE_WIDTH)?;

    let mut frames: Vec<std::path::PathBuf> = std::fs::read_dir(work_dir)
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

    if frames.len() < 2 {
        let _ = std::fs::remove_dir_all(work_dir);
        return Err("Not enough frames to analyse".to_string());
    }

    // Frames are evenly spaced across the file, so derive the spacing from how
    // many actually came back rather than from the requested count.
    let step = duration / frames.len() as f64;

    // Pass one: how different is each sample from the one before it?
    let mut changes: Vec<(f64, f64)> = Vec::new(); // (time, distance)
    let mut previous = signature(&frames[0]);
    for (index, frame) in frames.iter().enumerate().skip(1) {
        let Some(current) = signature(frame) else {
            continue;
        };
        if let Some(prev) = &previous {
            changes.push((index as f64 * step, distance(prev, &current)));
        }
        previous = Some(current);
    }
    let _ = std::fs::remove_dir_all(work_dir);

    if changes.is_empty() {
        return Ok(vec![Scene { start: 0.0 }]);
    }

    // Pass two: an adaptive threshold. Absolute frame difference depends
    // entirely on the transfer, so what matters is which gaps are unusual
    // *for this film*.
    let mut sorted: Vec<f64> = changes.iter().map(|(_, d)| *d).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let cut_index = ((sorted.len() as f64) * CHANGE_PERCENTILE).floor() as usize;
    let threshold = sorted
        .get(cut_index.min(sorted.len() - 1))
        .copied()
        .unwrap_or(MIN_CHANGE)
        .max(MIN_CHANGE);

    // Pass three: keep the strongest changes, spaced out.
    let mut candidates: Vec<(f64, f64)> = changes
        .into_iter()
        .filter(|(_, d)| *d >= threshold)
        .collect();
    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut starts: Vec<f64> = vec![0.0];
    for (at, _) in candidates {
        if starts.len() >= MAX_SCENES {
            break;
        }
        if starts
            .iter()
            .all(|existing| (at - existing).abs() >= MIN_SCENE_SECONDS)
        {
            starts.push(at);
        }
    }
    starts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    Ok(starts.into_iter().map(|start| Scene { start }).collect())
}
