//! Layer 2: Audio crate for VentaCore.
//!
//! Handles song decoding, multiband spectral flux onsets, tempo autocorrelation,
//! beat/bar/phrase grids, energy curves, change points, and manual corrections.
//!
//! Deliberately runs on the CPU. Does not import beat lists from other tools.

pub mod analysis;
pub mod corrections;
pub mod curves;
pub mod decoder;
pub mod error;
pub mod grid;
pub mod loudness;
pub mod mixer;
pub mod spectral;
pub mod tempo;

pub use analysis::{analyze_song_file, analyze_song_samples, SongStructure};
pub use corrections::{double_time, half_time, set_downbeat, shift_grid, tap_tempo};
pub use curves::{compute_energy_per_bar, compute_energy_per_second, compute_waveform_peaks};
pub use decoder::{AudioDecoder, DecodedAudio, SymphoniaDecoder};
pub use error::AudioError;
pub use grid::build_beat_grid;
pub use loudness::{compute_loudness_envelope, detect_loudness_change_points, LoudnessChangePoint};
pub use mixer::{AudioMixTrack, AudioMixer};
pub use spectral::{compute_spectral_flux_onsets, OnsetPeak};
pub use tempo::estimate_tempo_autocorrelation;
