//! Tempo estimation via autocorrelation of the onset envelope.
//!
//! Evaluates candidate lags corresponding to 60.0 - 200.0 BPM, applies
//! harmonic weighting to resolve half-time/double-time ambiguities, and
//! uses sub-sample parabolic interpolation for sub-BPM accuracy.

/// Estimate tempo in BPM from an onset strength envelope.
///
/// # Arguments
/// * `envelope` - Floating point onset flux or envelope samples.
/// * `fps` - Envelope sampling rate (samples per second).
///
/// Returns estimated tempo in beats per minute (BPM).
pub fn estimate_tempo_autocorrelation(envelope: &[f32], fps: f32) -> f32 {
    if envelope.len() < 10 || fps <= 0.0 {
        return 120.0;
    }

    let min_bpm = 60.0_f32;
    let max_bpm = 200.0_f32;

    let min_lag = ((fps * 60.0 / max_bpm).round() as usize).max(1);
    let max_lag = ((fps * 60.0 / min_bpm).round() as usize).min(envelope.len() / 2);

    if min_lag >= max_lag {
        return 120.0;
    }

    // Compute normalized autocorrelation for each candidate lag
    let n = envelope.len();
    let mut corr = vec![0.0_f32; max_lag + 2];

    for lag in min_lag..=max_lag {
        let mut sum = 0.0_f32;
        let count = n - lag;
        for i in 0..count {
            sum += envelope[i] * envelope[i + lag];
        }
        corr[lag] = sum / (count as f32);
    }

    // Find best lag with tempo prior weighting (perceptual center ~120 BPM)
    let mut best_lag = min_lag;
    let mut best_score = f32::MIN;

    for lag in min_lag..=max_lag {
        let raw_val = corr[lag];
        let bpm = (60.0 * fps) / (lag as f32);

        // Gaussian prior centered at 120 BPM in log2 space (std dev ~ 0.5 octaves)
        let log_ratio = (bpm / 120.0).log2();
        let prior = (-0.5 * (log_ratio / 0.55).powi(2)).exp();
        let weighted_score = raw_val * (0.65 + 0.35 * prior);

        if weighted_score > best_score {
            best_score = weighted_score;
            best_lag = lag;
        }
    }

    // Check octave harmonics: test if half-lag (double tempo) or double-lag (half tempo)
    // is a cleaner fundamental peak
    let half_lag = best_lag / 2;
    if half_lag >= min_lag && corr[half_lag] > 0.82 * corr[best_lag] {
        let bpm_half = (60.0 * fps) / (half_lag as f32);
        if (bpm_half - 120.0).abs() < ((60.0 * fps / best_lag as f32) - 120.0).abs() {
            best_lag = half_lag;
        }
    }

    // Parabolic sub-bin interpolation around best_lag
    let refined_lag = if best_lag > min_lag && best_lag < max_lag {
        let y0 = corr[best_lag - 1];
        let y1 = corr[best_lag];
        let y2 = corr[best_lag + 1];
        let denom = 2.0 * y1 - y0 - y2;
        if denom > 1e-6 {
            let delta = 0.5 * (y2 - y0) / denom;
            best_lag as f32 + delta.clamp(-0.5, 0.5)
        } else {
            best_lag as f32
        }
    } else {
        best_lag as f32
    };

    (60.0 * fps) / refined_lag
}

#[cfg(test)]
mod tests {
    use super::*;
    use ventacore_core::generate_click_train;

    #[test]
    fn test_autocorrelation_measures_known_tempos_within_one_bpm() {
        let test_tempos = [90.0_f32, 120.0_f32, 140.0_f32];

        for &target_bpm in &test_tempos {
            // Generate 10 seconds of click train at 48000 Hz
            let samples = generate_click_train(target_bpm, 10.0, 48000);

            // Compute onset flux
            let (_, envelope, fps) =
                crate::spectral::compute_spectral_flux_onsets(&samples, 48000);

            let measured = estimate_tempo_autocorrelation(&envelope, fps);
            let diff = (measured - target_bpm).abs();

            assert!(
                diff <= 1.0,
                "Expected {} BPM +/- 1.0, but measured {:.2} BPM (diff: {:.3})",
                target_bpm,
                measured,
                diff
            );
        }
    }
}
