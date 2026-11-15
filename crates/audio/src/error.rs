//! Error types for VentaCore audio analysis and decoding.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Audio decode error: {0}")]
    Decode(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Audio format unsupported: {0}")]
    UnsupportedFormat(String),

    #[error("No audio track found in file")]
    NoAudioTrack,

    #[error("Audio data is empty or invalid")]
    EmptyAudio,

    #[error("Invalid sample rate: {0}")]
    InvalidSampleRate(u32),
}
