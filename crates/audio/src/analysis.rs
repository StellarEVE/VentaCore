//! Unified song structure analysis coordinator.
//!
//! Orchestrates spectral flux onset detection, autocorrelation tempo estimation,
//! grid generation (Transient or Grid mode), structural change point detection,
//! waveform downsampling, and per-second/per-bar energy curves.

use crate::curves::{compute_energy_per_bar, compute_energy_per_second, compute_waveform_peaks};
use crate::decoder::AudioDecoder;
use crate::error::AudioError;
use crate::grid::build_beat_grid;
use crate::loudness::{compute_loudness_envelope, detect_loudness_change_points, LoudnessChangePoint};
use crate::spectral::{compute_spectral_flux_onsets, OnsetPeak};
use crate::tempo::estimate_tempo_autocorrelation;
use serde::{Deserialize, Serialize};
use std::path::Path;
use ventacore_core::{GridMode, Tick};

/// The complete structural analysis report of a song.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SongStructure {
    pub bpm: f32,
    pub beats_per_bar: u32,
    pub bars_per_phrase: u32,
    pub grid_mode: GridMode,
    pub beat_ticks: Vec<Tick>,
    pub bar_ticks: Vec<Tick>,
    pub phrase_ticks: Vec<Tick>,
    pub onsets: Vec<OnsetPeak>,
    pub change_points: Vec<LoudnessChangePoint>,
    pub waveform_peaks: Vec<f32>,
    pub loudness_envelope: Vec<f32>,
    pub energy_per_second: Vec<f32>,
    pub energy_per_bar: Vec<f32>,
}

/// Analyze raw floating point PCM audio samples.
pub fn analyze_song_samples(
    samples: &[f32],
    sample_rate: u32,
    grid_mode: GridMode,
) -> Result<SongStructure, AudioError> {
    if samples.is_empty() {
        return Err(AudioError::EmptyAudio);
    }
    if sample_rate == 0 {
        return Err(AudioError::InvalidSampleRate(sample_rate));
    }

    let total_duration_sec = samples.len() as f64 / sample_rate as f64;
    let total_duration_ticks = Tick::from_seconds_f64(total_duration_sec);

    // 1. Spectral flux onsets and onset envelope
    let (onsets, onset_envelope, envelope_fps) =
        compute_spectral_flux_onsets(samples, sample_rate);

    // 2. Tempo autocorrelation
    let bpm = estimate_tempo_autocorrelation(&onset_envelope, envelope_fps);

    // 3. Grid derivation (Beats, Bars, Phrases)
    let beats_per_bar = 4u32;
    let bars_per_phrase = 4u32;
    let (beat_ticks, bar_ticks, phrase_ticks) = build_beat_grid(
        bpm,
        total_duration_ticks,
        &onsets,
        grid_mode,
        beats_per_bar,
        bars_per_phrase,
    );

    // 4. Loudness envelope and change points
    let loudness_frame_size = 2048usize;
    let loudness_envelope =
        compute_loudness_envelope(samples, sample_rate, loudness_frame_size);
    let loudness_fps = sample_rate as f32 / (loudness_frame_size / 2) as f32;
    let change_points = detect_loudness_change_points(&loudness_envelope, loudness_fps);

    // 5. Waveform display peaks and energy curves
    let waveform_peaks = compute_waveform_peaks(samples, sample_rate, 50); // 50 points/sec
    let energy_per_second = compute_energy_per_second(samples, sample_rate);
    let energy_per_bar = compute_energy_per_bar(samples, sample_rate, &bar_ticks);

    Ok(SongStructure {
        bpm,
        beats_per_bar,
        bars_per_phrase,
        grid_mode,
        beat_ticks,
        bar_ticks,
        phrase_ticks,
        onsets,
        change_points,
        waveform_peaks,
        loudness_envelope,
        energy_per_second,
        energy_per_bar,
    })
}

/// Decode an audio file from disk using the specified decoder and analyze it.
pub fn analyze_song_file<D: AudioDecoder>(
    path: &Path,
    decoder: &D,
    grid_mode: GridMode,
) -> Result<SongStructure, AudioError> {
    let decoded = decoder.decode(path)?;
    analyze_song_samples(&decoded.samples, decoded.sample_rate, grid_mode)
}
