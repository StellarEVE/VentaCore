//! Error definitions for the platform and codec abstraction layer.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Unsupported codec: {0}")]
    UnsupportedCodec(String),

    #[error("Failed to decode video frame: {0}")]
    DecodingFailed(String),

    #[error("Failed to encode video frame: {0}")]
    EncodingFailed(String),

    #[error("Hardware capability probe failed: {0}")]
    HardwareProbingFailed(String),

    #[error("Hardware trial failed for {codec}: {reason}")]
    HardwareTrialFailed { codec: String, reason: String },

    #[error("Invalid frame data or buffer layout: {0}")]
    InvalidFrameData(String),

    #[error("Codec or hardware context initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Seek operation failed: {0}")]
    SeekFailed(String),
}
