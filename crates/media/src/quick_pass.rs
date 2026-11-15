//! Stage 1: Quick pass sparse analysis and thumbnail generation.
//!
//! Conforms to Charter Rule 7: A quick pass over every video.
//! Samples frames sparsely (e.g. 1 frame/sec) to generate thumbnail filmstrips,
//! detect coarse scene cuts, and provide provisional motion curves.

use crate::analysis::{AnalysisLevel, FootageAnalysis, MotionPoint};
use crate::error::MediaError;
use crate::thumbnail::ThumbnailFrame;
use ventacore_core::{AssetId, Tick, TimeRange};
use ventacore_platform::VideoDecoder;

/// Result of Stage 1 quick pass.
#[derive(Debug, Clone)]
pub struct QuickPassResult {
    pub analysis: FootageAnalysis,
    pub thumbnails: Vec<ThumbnailFrame>,
}

/// Runs Stage 1 quick pass analysis over a decoder.
pub fn run_quick_pass(
    decoder: &mut dyn VideoDecoder,
    asset_id: AssetId,
    sample_interval: Tick,
) -> Result<QuickPassResult, MediaError> {
    let duration = decoder.metadata().duration;
    let mut analysis = FootageAnalysis::new_provisional(asset_id, duration);
    let mut thumbnails = Vec::new();

    if duration <= Tick::ZERO {
        return Ok(QuickPassResult { analysis, thumbnails });
    }

    let interval = if sample_interval <= Tick::ZERO {
        Tick::from_seconds(1)
    } else {
        sample_interval
    };

    let mut cursor = Tick::ZERO;
    let mut prev_sample_pixels: Option<Vec<u8>> = None;
    let mut dark_start: Option<Tick> = None;

    while cursor < duration {
        decoder.seek(cursor)?;
        if let Some(frame) = decoder.decode_next_frame()? {
            let (oriented_rgba, _w, _h) = frame.to_oriented_rgba8();

            // 1. Generate downscaled thumbnail (160x90)
            let thumb = ThumbnailFrame::from_video_frame(&frame, 160, 90);
            thumbnails.push(thumb);

            // 2. Measure average luminance to check for dark frames
            let luminance = compute_average_luminance(&oriented_rgba);
            if luminance < 10.0 {
                if dark_start.is_none() {
                    dark_start = Some(cursor);
                }
            } else if let Some(start) = dark_start.take() {
                if cursor > start {
                    analysis.black_ranges.push(TimeRange::from_start_end(start, cursor));
                }
            }

            // 3. Difference with previous frame for scene cuts and rough motion
            if let Some(ref prev) = prev_sample_pixels {
                let diff_score = compute_frame_difference(&oriented_rgba, prev);

                // If massive jump, mark as provisional scene cut
                if diff_score > 0.40 {
                    analysis.scene_cuts.push(cursor);
                }

                // Add rough motion point
                analysis.motion_scores.push(MotionPoint {
                    time: cursor,
                    score: diff_score.clamp(0.0, 1.0),
                });
            } else {
                analysis.motion_scores.push(MotionPoint {
                    time: cursor,
                    score: 0.5,
                });
            }

            prev_sample_pixels = Some(oriented_rgba);
        }

        cursor += interval;
    }

    if let Some(start) = dark_start {
        if duration > start {
            analysis.black_ranges.push(TimeRange::from_start_end(start, duration));
        }
    }

    analysis.level = AnalysisLevel::Provisional;
    Ok(QuickPassResult { analysis, thumbnails })
}

/// Computes average luminance (0.0 to 255.0) of an RGBA8 buffer.
pub fn compute_average_luminance(rgba: &[u8]) -> f32 {
    let pixel_count = rgba.len() / 4;
    if pixel_count == 0 {
        return 0.0;
    }

    let mut sum_lum = 0.0f64;
    for chunk in rgba.chunks_exact(4) {
        let r = chunk[0] as f64;
        let g = chunk[1] as f64;
        let b = chunk[2] as f64;
        sum_lum += 0.2126 * r + 0.7152 * g + 0.0722 * b;
    }

    (sum_lum / pixel_count as f64) as f32
}

/// Computes normalized RGB color difference (0.0 to 1.0) between two equal-sized RGBA buffers,
/// ignoring the constant alpha channel.
pub fn compute_frame_difference(curr: &[u8], prev: &[u8]) -> f32 {
    let min_len = curr.len().min(prev.len());
    let pixel_count = min_len / 4;
    if pixel_count == 0 {
        return 0.0;
    }

    let mut sum_diff = 0u64;
    for (c, p) in curr.chunks_exact(4).zip(prev.chunks_exact(4)) {
        let dr = c[0].abs_diff(p[0]) as u64;
        let dg = c[1].abs_diff(p[1]) as u64;
        let db = c[2].abs_diff(p[2]) as u64;
        sum_diff += dr + dg + db;
    }

    (sum_diff as f64 / (pixel_count as f64 * 3.0 * 255.0)) as f32
}
