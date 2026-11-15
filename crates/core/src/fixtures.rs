//! Synthetic audio test fixture generators for VentaCore.
//!
//! Conforms to Stage 0 requirements:
//! - Accepts tempo in BPM, duration in seconds, and sample rate in Hz.
//! - Deterministic: produces identical samples every time.
//! - Accurate click placement for tempo verification tests in later stages.

/// Generate a synthetic click-train audio signal at an exactly known tempo.
///
/// # Arguments
/// * `tempo_bpm` - Tempo in beats per minute (e.g. 120.0).
/// * `duration_sec` - Total audio duration in seconds.
/// * `sample_rate` - Audio sample rate in Hz (e.g. 44100 or 48000).
///
/// Returns a `Vec<f32>` containing mono audio samples in `[-1.0, 1.0]`.
pub fn generate_click_train(tempo_bpm: f32, duration_sec: f32, sample_rate: u32) -> Vec<f32> {
    assert!(tempo_bpm > 0.0, "tempo must be positive");
    assert!(duration_sec >= 0.0, "duration must be non-negative");
    assert!(sample_rate > 0, "sample rate must be non-zero");

    let total_samples = (duration_sec * sample_rate as f32).round() as usize;
    let mut samples = vec![0.0_f32; total_samples];

    if total_samples == 0 {
        return samples;
    }

    let period_samples = (sample_rate as f32 * 60.0 / tempo_bpm).round() as usize;
    if period_samples == 0 {
        return samples;
    }

    // A realistic 1 kHz decaying click lasting ~10 ms
    let click_length = ((sample_rate as f32 * 0.01).round() as usize).min(period_samples);
    let mut click_wave = Vec::with_capacity(click_length);
    for n in 0..click_length {
        let t = n as f32 / sample_rate as f32;
        // 1000 Hz sine with exponential decay
        let env = (-t * 300.0).exp();
        let val = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * env;
        click_wave.push(val);
    }

    let mut beat_idx = 0;
    while beat_idx < total_samples {
        let end = (beat_idx + click_length).min(total_samples);
        for i in beat_idx..end {
            samples[i] = click_wave[i - beat_idx];
        }
        beat_idx += period_samples;
    }

    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_click_train_produces_identical_samples_every_time() {
        let run1 = generate_click_train(120.0, 5.0, 48000);
        let run2 = generate_click_train(120.0, 5.0, 48000);

        assert_eq!(run1.len(), 5 * 48000);
        assert_eq!(run1, run2, "Click train generation must be strictly deterministic");
    }

    #[test]
    fn test_click_train_lengths_and_tempos() {
        let run_90 = generate_click_train(90.0, 2.0, 44100);
        let run_140 = generate_click_train(140.0, 3.0, 48000);

        assert_eq!(run_90.len(), 2 * 44100);
        assert_eq!(run_140.len(), 3 * 48000);

        // Verify non-empty and non-silent
        assert!(run_90.iter().any(|&s| s != 0.0));
        assert!(run_140.iter().any(|&s| s != 0.0));
    }
}
