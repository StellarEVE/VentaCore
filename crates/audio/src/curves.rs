//! Waveform peaks, meter loudness, and per-second/per-bar energy curves.

use ventacore_core::Tick;

/// Downsamples samples into peak amplitudes for UI waveform drawing.
pub fn compute_waveform_peaks(
    samples: &[f32],
    sample_rate: u32,
    points_per_sec: usize,
) -> Vec<f32> {
    if samples.is_empty() || sample_rate == 0 || points_per_sec == 0 {
        return Vec::new();
    }

    let samples_per_point = ((sample_rate as f32) / (points_per_sec as f32)).round() as usize;
    if samples_per_point == 0 {
        return Vec::new();
    }

    let num_points = samples.len() / samples_per_point;
    let mut peaks = Vec::with_capacity(num_points);

    for p in 0..num_points {
        let start = p * samples_per_point;
        let end = (start + samples_per_point).min(samples.len());
        let mut max_abs = 0.0_f32;
        for i in start..end {
            let abs = samples[i].abs();
            if abs > max_abs {
                max_abs = abs;
            }
        }
        peaks.push(max_abs.clamp(0.0, 1.0));
    }

    peaks
}

/// Computes average RMS energy for each 1-second slice of audio.
pub fn compute_energy_per_second(samples: &[f32], sample_rate: u32) -> Vec<f32> {
    if samples.is_empty() || sample_rate == 0 {
        return Vec::new();
    }

    let chunk_size = sample_rate as usize;
    let num_secs = (samples.len() + chunk_size - 1) / chunk_size;
    let mut energies = Vec::with_capacity(num_secs);

    for s in 0..num_secs {
        let start = s * chunk_size;
        let end = (start + chunk_size).min(samples.len());
        let count = end - start;
        if count == 0 {
            break;
        }

        let mut sum_sq = 0.0_f32;
        for i in start..end {
            let val = samples[i];
            sum_sq += val * val;
        }
        let rms = (sum_sq / count as f32).sqrt();
        energies.push(rms);
    }

    // Normalize
    let max_val = energies.iter().copied().fold(0.0_f32, f32::max);
    if max_val > 0.0 {
        for v in &mut energies {
            *v /= max_val;
        }
    }

    energies
}

/// Computes average RMS energy for each musical bar between bar ticks.
pub fn compute_energy_per_bar(
    samples: &[f32],
    sample_rate: u32,
    bar_ticks: &[Tick],
) -> Vec<f32> {
    if samples.is_empty() || sample_rate == 0 || bar_ticks.is_empty() {
        return Vec::new();
    }

    let total_samples = samples.len();
    let mut bar_energies = Vec::with_capacity(bar_ticks.len());

    for i in 0..bar_ticks.len() {
        let start_sec = bar_ticks[i].to_seconds_f64();
        let end_sec = if i + 1 < bar_ticks.len() {
            bar_ticks[i + 1].to_seconds_f64()
        } else {
            start_sec + 2.0 // Fallback estimate for final bar
        };

        let start_sample = ((start_sec * sample_rate as f64).round() as usize).min(total_samples);
        let end_sample = ((end_sec * sample_rate as f64).round() as usize).min(total_samples);

        if start_sample >= end_sample {
            bar_energies.push(0.0);
            continue;
        }

        let count = end_sample - start_sample;
        let mut sum_sq = 0.0_f32;
        for s in start_sample..end_sample {
            let val = samples[s];
            sum_sq += val * val;
        }
        let rms = (sum_sq / count as f32).sqrt();
        bar_energies.push(rms);
    }

    // Normalize
    let max_val = bar_energies.iter().copied().fold(0.0_f32, f32::max);
    if max_val > 0.0 {
        for v in &mut bar_energies {
            *v /= max_val;
        }
    }

    bar_energies
}
