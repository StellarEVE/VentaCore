//! Loudness envelope calculation and structural change-point detection.
//!
//! Identifies macro-level energy shifts (quiet-to-loud or loud-to-quiet) marking
//! musical transitions such as verses, drops, breakdowns, and build-ups.

use serde::{Deserialize, Serialize};
use ventacore_core::Tick;

/// A detected structural turning point in song loudness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoudnessChangePoint {
    pub tick: Tick,
    pub label: String,
    pub energy_regime: f32,
}

/// Computes RMS loudness envelope over sliding frames.
pub fn compute_loudness_envelope(samples: &[f32], sample_rate: u32, frame_size: usize) -> Vec<f32> {
    if samples.is_empty() || frame_size == 0 || sample_rate == 0 {
        return Vec::new();
    }

    let hop_size = frame_size / 2;
    let num_frames = (samples.len().saturating_sub(frame_size)) / hop_size + 1;
    let mut envelope = Vec::with_capacity(num_frames);

    for f in 0..num_frames {
        let start = f * hop_size;
        let mut sum_sq = 0.0_f32;
        for i in 0..frame_size {
            let s = samples[start + i];
            sum_sq += s * s;
        }
        let rms = (sum_sq / frame_size as f32).sqrt();
        envelope.push(rms);
    }

    // Normalize envelope into [0.0, 1.0]
    let max_val = envelope.iter().copied().fold(0.0_f32, f32::max);
    if max_val > 0.0 {
        for v in &mut envelope {
            *v /= max_val;
        }
    }

    envelope
}

/// Detects structural change points on the loudness envelope using sliding-window comparison.
pub fn detect_loudness_change_points(
    envelope: &[f32],
    envelope_fps: f32,
) -> Vec<LoudnessChangePoint> {
    let mut points = Vec::new();
    let window = ((envelope_fps * 1.5).round() as usize).max(2); // 1.5s window

    if envelope.len() < window * 2 {
        return points;
    }

    let mut deltas = vec![0.0_f32; envelope.len()];
    for i in window..(envelope.len() - window) {
        let prev_mean = envelope[(i - window)..i].iter().sum::<f32>() / window as f32;
        let next_mean = envelope[i..(i + window)].iter().sum::<f32>() / window as f32;
        deltas[i] = next_mean - prev_mean;
    }

    let min_dist_frames = ((envelope_fps * 2.0).round() as usize).max(window);
    let mut last_trigger = 0usize;

    for i in (window + 1)..(envelope.len() - window - 1) {
        let abs_d = deltas[i].abs();
        if abs_d > 0.16
            && abs_d >= deltas[i - 1].abs()
            && abs_d >= deltas[i + 1].abs()
            && (last_trigger == 0 || i >= last_trigger + min_dist_frames)
        {
            let time_sec = i as f64 / envelope_fps as f64;
            let tick = Tick::from_seconds_f64(time_sec);

            let label = if deltas[i] > 0.0 {
                "Energy Lift / Chorus Drop".to_string()
            } else {
                "Breakdown / Verse".to_string()
            };

            let next_mean = envelope[i..(i + window)].iter().sum::<f32>() / window as f32;
            points.push(LoudnessChangePoint {
                tick,
                label,
                energy_regime: next_mean.clamp(0.0, 1.0),
            });

            last_trigger = i;
        }
    }

    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_quiet_to_loud_change_point() {
        let sample_rate = 48000;
        let duration_sec = 6.0;
        let total_samples = (duration_sec * sample_rate as f64) as usize;
        let mut samples = vec![0.0_f32; total_samples];

        // First 3 seconds: quiet (amplitude 0.05)
        for i in 0..(3 * sample_rate) {
            samples[i] = 0.05 * ((i as f32 * 0.1).sin());
        }
        // Next 3 seconds: loud (amplitude 0.9)
        for i in (3 * sample_rate)..total_samples {
            samples[i] = 0.9 * ((i as f32 * 0.1).sin());
        }

        let frame_size = 2048;
        let hop_size = frame_size / 2;
        let envelope = compute_loudness_envelope(&samples, sample_rate as u32, frame_size);
        let envelope_fps = sample_rate as f32 / hop_size as f32;

        let points = detect_loudness_change_points(&envelope, envelope_fps);

        assert!(!points.is_empty(), "Must detect quiet-to-loud change point");
        let first = &points[0];
        assert_eq!(first.label, "Energy Lift / Chorus Drop");
        let trigger_sec = first.tick.to_seconds_f64();
        assert!(
            (trigger_sec - 3.0).abs() < 0.5,
            "Change point should land near 3.0 seconds, got {:.2}",
            trigger_sec
        );
    }
}
