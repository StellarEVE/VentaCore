//! Stage 0: Instant container and stream probe.
//!
//! Conforms to Charter Rule 7: Probe on import.
//! Runs in milliseconds to determine container properties without full decoding.

use crate::error::MediaError;
use std::path::Path;
use ventacore_core::ProbeData;
use ventacore_platform::{PlatformVideoDecoder, VideoDecoder, VideoMetadata};

/// Stage 0 probe result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage0ProbeResult {
    pub core_probe: ProbeData,
    pub full_metadata: VideoMetadata,
}

/// Performs Stage 0 probe on a media file.
pub fn probe_video_container(path: impl AsRef<Path>) -> Result<Stage0ProbeResult, MediaError> {
    let decoder = PlatformVideoDecoder::open(path)?;
    let meta = decoder.metadata().clone();

    let core_probe = ProbeData {
        duration: meta.duration,
        width: meta.width,
        height: meta.height,
        fps_num: meta.fps_num,
        fps_den: meta.fps_den,
        has_audio: meta.has_audio,
        audio_sample_rate: meta.audio_sample_rate,
    };

    Ok(Stage0ProbeResult {
        core_probe,
        full_metadata: meta,
    })
}
