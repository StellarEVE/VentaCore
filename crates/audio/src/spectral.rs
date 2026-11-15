//! Multiband spectral flux onset detection.
//!
//! Uses FFT to compute short-time magnitude spectra, calculates spectral flux
//! separated into Low, Mid, and High frequency bands, and detects transient onsets.
//! Runs 100% on the CPU.

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;
use ventacore_core::Tick;

/// A detected musical onset with band flux details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnsetPeak {
    pub tick: Tick,
    pub strength: f32,
    pub low_band_flux: f32,
    pub mid_band_flux: f32,
    pub high_band_flux: f32,
}

/// Computes multiband spectral flux and onset peaks from audio samples.
///
/// Returns:
/// - List of detected `OnsetPeak`s.
/// - Continuous onset strength envelope (sampled at `sample_rate / hop_size`).
/// - Envelope frames per second.
pub fn compute_spectral_flux_onsets(
    samples: &[f32],
    sample_rate: u32,
) -> (Vec<OnsetPeak>, Vec<f32>, f32) {
    let window_size = 1024usize;
    let hop_size = 512usize;

    if samples.len() < window_size {
        return (Vec::new(), Vec::new(), 0.0);
    }

    let envelope_fps = sample_rate as f32 / hop_size as f32;

    // Precompute Hanning window
    let mut window = vec![0.0_f32; window_size];
    for n in 0..window_size {
        window[n] = 0.5 * (1.0 - (2.0 * PI * n as f32 / (window_size - 1) as f32).cos());
    }

    // Set up FFT planner
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(window_size);

    let num_bins = window_size / 2 + 1;
    let bin_hz = sample_rate as f32 / window_size as f32;

    // Frequency bands
    // Low: 0 - 250 Hz (bass / kicks)
    // Mid: 250 - 2500 Hz (snare / vocal body)
    // High: 2500+ Hz (hi-hats / cymbals / transients)
    let low_cutoff_bin = ((250.0 / bin_hz).round() as usize).clamp(1, num_bins);
    let mid_cutoff_bin = ((2500.0 / bin_hz).round() as usize).clamp(low_cutoff_bin + 1, num_bins);

    let num_frames = (samples.len() - window_size) / hop_size + 1;
    let mut prev_magnitude = vec![0.0_f32; num_bins];
    let mut buffer = vec![Complex::new(0.0_f32, 0.0); window_size];

    let mut onset_envelope = Vec::with_capacity(num_frames);
    let mut low_flux_env = Vec::with_capacity(num_frames);
    let mut mid_flux_env = Vec::with_capacity(num_frames);
    let mut high_flux_env = Vec::with_capacity(num_frames);

    for frame_idx in 0..num_frames {
        let start = frame_idx * hop_size;
        for i in 0..window_size {
            buffer[i] = Complex::new(samples[start + i] * window[i], 0.0);
        }

        fft.process(&mut buffer);

        let mut low_flux = 0.0_f32;
        let mut mid_flux = 0.0_f32;
        let mut high_flux = 0.0_f32;

        for k in 0..num_bins {
            let mag = (buffer[k].re * buffer[k].re + buffer[k].im * buffer[k].im).sqrt();
            let diff = (mag - prev_magnitude[k]).max(0.0); // Half-wave rectification

            if k < low_cutoff_bin {
                low_flux += diff;
            } else if k < mid_cutoff_bin {
                mid_flux += diff;
            } else {
                high_flux += diff;
            }

            prev_magnitude[k] = mag;
        }

        let combined = 1.35 * low_flux + 1.0 * mid_flux + 0.85 * high_flux;
        onset_envelope.push(combined);
        low_flux_env.push(low_flux);
        mid_flux_env.push(mid_flux);
        high_flux_env.push(high_flux);
    }

    // Peak picking over onset envelope
    let mut onsets = Vec::new();
    let local_win = 5usize; // ~50 ms window
    let min_dist_frames = ((0.06 * envelope_fps).round() as usize).max(1); // at least 60ms between onsets

    // Normalize envelope
    let max_val = onset_envelope
        .iter()
        .copied()
        .fold(0.0_f32, f32::max);
    if max_val > 0.0 {
        for v in &mut onset_envelope {
            *v /= max_val;
        }
        for v in &mut low_flux_env {
            *v /= max_val;
        }
        for v in &mut mid_flux_env {
            *v /= max_val;
        }
        for v in &mut high_flux_env {
            *v /= max_val;
        }
    }

    let mut last_onset_frame = 0usize;

    for i in local_win..(onset_envelope.len().saturating_sub(local_win)) {
        let val = onset_envelope[i];
        if val < 0.08 {
            continue;
        }

        // Must be a local maximum
        let is_peak = onset_envelope[i - 1] <= val
            && onset_envelope[i + 1] <= val
            && val > onset_envelope[i - 2]
            && val > onset_envelope[i + 2];

        if !is_peak {
            continue;
        }

        // Local dynamic threshold: mean of neighborhood
        let local_slice = &onset_envelope[(i - local_win)..=(i + local_win)];
        let local_mean = local_slice.iter().sum::<f32>() / local_slice.len() as f32;

        if val > local_mean + 0.04 && (i == 0 || i >= last_onset_frame + min_dist_frames) {
            let time_sec = (i * hop_size) as f64 / sample_rate as f64;
            let tick = Tick::from_seconds_f64(time_sec);

            onsets.push(OnsetPeak {
                tick,
                strength: val.clamp(0.0, 1.0),
                low_band_flux: low_flux_env[i],
                mid_band_flux: mid_flux_env[i],
                high_band_flux: high_flux_env[i],
            });

            last_onset_frame = i;
        }
    }

    (onsets, onset_envelope, envelope_fps)
}
