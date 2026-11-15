//! Error definitions for the media analysis and cache subsystem.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Platform decoding error: {0}")]
    Platform(#[from] ventacore_platform::PlatformError),

    #[error("Serialization / JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Media file not found: {0}")]
    FileNotFound(String),

    #[error("Empty or invalid media file: {0}")]
    InvalidMedia(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Operation was cancelled")]
    Cancelled,
}
