//! Layer 2: Platform crate for VentaCore.
//!
//! Sole interface for hardware probing, in-process FFmpeg video and audio
//! decoding and encoding, and trial capability verification.

pub mod audio_clock;
pub mod capabilities;
pub mod classify;
pub mod config;
pub mod decoder;
pub mod encoder;
pub mod error;
pub mod pixel;
pub mod traits;
pub mod types;
pub mod vfr;

pub use audio_clock::{ClockSource, PlatformAudioClock};
pub use capabilities::{
    probe_hardware_capabilities, run_decoder_trial, run_encoder_trial, CodecSupport,
    GraphicsAdapterInfo, HardwareCapabilityReport, TrialStatus,
};
pub use classify::{classify, MediaClassification};
pub use config::PlatformEnvironmentConfig;
pub use decoder::{
    find_ffmpeg_binary, find_ffprobe_binary, probe_video_with_ffprobe, FfmpegVideoDecoder,
    PlatformVideoDecoder, SyntheticVideoDecoder,
};
pub use encoder::SyntheticVideoEncoder;
pub use error::PlatformError;
pub use pixel::{rotate_rgba8, yuv420p10le_to_rgba8, yuv420p_to_rgba8};
pub use traits::{VideoDecoder, VideoEncoder};
pub use types::{
    AudioCodec, ColorDepth, HwAccel, PixelFormat, Rotation, VideoCodec, VideoEncoderConfig,
    VideoFrame, VideoMetadata,
};
pub use vfr::VfrTimecodeIndex;
