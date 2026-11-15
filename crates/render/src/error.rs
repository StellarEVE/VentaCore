//! Error types for the graphics and rendering subsystem.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("GPU adapter not found")]
    AdapterNotFound,

    #[error("Failed to request GPU device: {0}")]
    DeviceRequestFailed(String),

    #[error("Buffer mapping failed: {0}")]
    BufferMappingFailed(String),

    #[error("Invalid render dimensions: {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },

    #[error("Pixel buffer size mismatch: expected {expected} bytes, got {actual} bytes")]
    BufferSizeMismatch { expected: usize, actual: usize },

    #[error("Shader creation error: {0}")]
    ShaderError(String),
}
