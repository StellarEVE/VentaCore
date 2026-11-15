//! Stage 2: Full pass dense decode, exact scene detection, and black range identification.
//!
//! Conforms to Charter Rule 7: One full decode per video with progress reported per chunk.
//! Pinpoints exact shot cut boundaries, black intervals, and dense motion curves.

use crate::analysis::{AnalysisLevel, FootageAnalysis, MotionPoint};
use crate::error::MediaError;
use crate::quick_pass::{compute_average_luminance, compute_frame_difference};
use ventacore_core::{AssetId, Tick, TimeRange};
use ventacore_platform::VideoDecoder;

/// Runs Stage 2 full analysis decode over a video stream.
pub fn run_full_pass(
    decoder: &mut dyn VideoDecoder,
    asset_id: AssetId,
    progress_callback: Option<&dyn Fn(f32)>,
) -> Result<FootageAnalysis, MediaError> {
    decoder.seek(Tick::ZERO)?;
    let duration = decoder.metadata().duration;

    let mut analysis = FootageAnalysis::new(asset_id, duration);
    analysis.level = AnalysisLevel::Full;

    if duration <= Tick::ZERO {
        return Ok(analysis);
    }

    let mut prev_frame_pixels: Option<Vec<u8>> = None;
    let mut dark_interval_start: Option<Tick> = None;
    let mut dark_frame_count = 0usize;

    let mut motion_accumulator = 0.0f32;
    let mut motion_frame_count = 0usize;
    let mut last_motion_tick = Tick::ZERO;
    let motion_interval = Tick::from_seconds_f64(0.25); // 4 points/sec

    let mut frame_index = 0usize;

    while let Some(frame) = decoder.decode_next_frame()? {
        let (rgba, _, _) = frame.to_oriented_rgba8();
        let pts = frame.pts;

        // 1. Black range detection
        let luminance = compute_average_luminance(&rgba);
        if luminance < 8.0 {
            dark_frame_count += 1;
            if dark_interval_start.is_none() {
                dark_interval_start = Some(pts);
            }
        } else {
            if dark_frame_count >= 2 {
                if let Some(start) = dark_interval_start.take() {
                    if pts > start {
                        analysis.black_ranges.push(TimeRange::from_start_end(start, pts));
                    }
                }
            } else {
                dark_interval_start = None;
            }
            dark_frame_count = 0;
        }

        // 2. Inter-frame difference for scene cuts and motion
        if let Some(ref prev) = prev_frame_pixels {
            let diff = compute_frame_difference(&rgba, prev);

            // Exact scene cut spike
            if diff > 0.20 {
                analysis.scene_cuts.push(pts);
            }

            motion_accumulator += diff;
            motion_frame_count += 1;

            if pts - last_motion_tick >= motion_interval {
                let avg_motion = motion_accumulator / motion_frame_count.max(1) as f32;
                analysis.motion_scores.push(MotionPoint {
                    time: pts,
                    score: (avg_motion * 2.0).clamp(0.0, 1.0),
                });
                motion_accumulator = 0.0;
                motion_frame_count = 0;
                last_motion_tick = pts;
            }
        }

        prev_frame_pixels = Some(rgba);
        frame_index += 1;

        if frame_index % 30 == 0 {
            if let Some(cb) = progress_callback {
                let progress = (pts.raw() as f64 / duration.raw() as f64).clamp(0.0, 1.0) as f32;
                cb(progress);
            }
        }
    }

    // Flush trailing dark interval
    if dark_frame_count >= 2 {
        if let Some(start) = dark_interval_start {
            if duration > start {
                analysis.black_ranges.push(TimeRange::from_start_end(start, duration));
            }
        }
    }

    // Flush remaining motion
    if motion_frame_count > 0 {
        let avg_motion = motion_accumulator / motion_frame_count as f32;
        analysis.motion_scores.push(MotionPoint {
            time: duration,
            score: (avg_motion * 2.0).clamp(0.0, 1.0),
        });
    }

    // Extract highlight moments (top 20% motion regions)
    analysis.interesting_ranges = extract_highlight_ranges(&analysis.motion_scores);

    if let Some(cb) = progress_callback {
        cb(1.0);
    }

    Ok(analysis)
}

/// Identifies segments with sustained above-average motion.
fn extract_highlight_ranges(motion_scores: &[MotionPoint]) -> Vec<TimeRange> {
    let mut highlights = Vec::new();
    let mut in_highlight = false;
    let mut highlight_start = Tick::ZERO;

    for point in motion_scores {
        if point.score >= 0.70 {
            if !in_highlight {
                in_highlight = true;
                highlight_start = point.time;
            }
        } else if in_highlight {
            in_highlight = false;
            let dur = point.time - highlight_start;
            if dur >= Tick::from_seconds(1) {
                highlights.push(TimeRange::from_start_end(highlight_start, point.time));
            }
        }
    }

    highlights
}
