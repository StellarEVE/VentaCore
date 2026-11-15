//! Abstract traits for in-process video decoding and encoding.
//!
//! Follows Charter Rule 4: No other module names a codec library or external process.
//! Follows Charter Rule 5: Every GPU path has a working software fallback.
//! Follows Charter Rule 6: Active encoder and decoder names are always exposed.

use crate::error::PlatformError;
use crate::types::{VideoFrame, VideoMetadata};
use ventacore_core::Tick;

/// Common interface for video file decoders.
pub trait VideoDecoder: Send + Sync {
    /// Container and video stream metadata.
    fn metadata(&self) -> &VideoMetadata;

    /// Seeks playback to the frame nearest to `target_tick`.
    fn seek(&mut self, target_tick: Tick) -> Result<(), PlatformError>;

    /// Decodes and returns the next video frame in presentation order.
    /// Returns `Ok(None)` when end of stream is reached.
    fn decode_next_frame(&mut self) -> Result<Option<VideoFrame>, PlatformError>;

    /// Returns the human-readable implementation name of the active decoder
    /// (e.g. "h264_nvdec", "h264_d3d11va", or "h264 (software)").
    fn active_decoder_name(&self) -> &str;

    /// Returns true if this decoder is actively using hardware acceleration.
    fn is_hardware_accelerated(&self) -> bool;
}

/// Common interface for video file encoders.
pub trait VideoEncoder: Send {
    /// Encodes a single uncompressed video frame.
    fn encode_frame(&mut self, frame: &VideoFrame) -> Result<(), PlatformError>;

    /// Flushes any pending packets, finalizes container muxing, and closes the file.
    fn finish(self: Box<Self>) -> Result<(), PlatformError>;

    /// Returns the exact name of the encoder actually used
    /// (e.g. "h264_nvenc", "h264_amf", or "libx264").
    fn active_encoder_name(&self) -> &str;

    /// Returns true if this encoder is actively using hardware acceleration.
    fn is_hardware_accelerated(&self) -> bool;
}
