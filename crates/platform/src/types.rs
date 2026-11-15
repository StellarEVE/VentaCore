//! Video, audio, and graphics data types for the platform layer.

use serde::{Deserialize, Serialize};
use std::fmt;
use ventacore_core::Tick;

/// Container rotation flags (counteracting smartphone and camera metadata orientation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Rotation {
    #[default]
    None,
    Rotate90,
    Rotate180,
    Rotate270,
}

impl Rotation {
    /// Creates a Rotation from orientation angle in degrees (0, 90, 180, 270).
    pub fn from_degrees(degrees: i32) -> Self {
        let normalized = ((degrees % 360) + 360) % 360;
        match normalized {
            90 => Rotation::Rotate90,
            180 => Rotation::Rotate180,
            270 => Rotation::Rotate270,
            _ => Rotation::None,
        }
    }

    /// Converts Rotation to angle in degrees.
    pub fn to_degrees(self) -> i32 {
        match self {
            Rotation::None => 0,
            Rotation::Rotate90 => 90,
            Rotation::Rotate180 => 180,
            Rotation::Rotate270 => 270,
        }
    }

    /// Returns true if width and height swap after rotation (90 or 270 degrees).
    pub fn is_transposed(self) -> bool {
        matches!(self, Rotation::Rotate90 | Rotation::Rotate270)
    }
}

/// Bit depth of pixel color channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ColorDepth {
    #[default]
    Bits8,
    Bits10,
    Bits12,
}

impl ColorDepth {
    pub fn bits(self) -> u8 {
        match self {
            ColorDepth::Bits8 => 8,
            ColorDepth::Bits10 => 10,
            ColorDepth::Bits12 => 12,
        }
    }
}

/// Pixel format of decoded video frames or graphics buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PixelFormat {
    #[default]
    Rgba8,
    Rgb24,
    /// Standard Rec.709 8-bit planar YUV (4:2:0).
    Yuv420p,
    /// 10-bit planar YUV (4:2:0) little-endian (e.g. HDR / Log sources).
    Yuv420p10le,
    /// Semi-planar YUV (4:2:0) common in hardware decoder outputs (NV12).
    Nv12,
}

/// Video codecs supported by the platform layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VideoCodec {
    H264,
    Hevc,
    Av1,
    Vp9,
    ProRes,
}

impl fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VideoCodec::H264 => write!(f, "H.264 / AVC"),
            VideoCodec::Hevc => write!(f, "H.265 / HEVC"),
            VideoCodec::Av1 => write!(f, "AV1"),
            VideoCodec::Vp9 => write!(f, "VP9"),
            VideoCodec::ProRes => write!(f, "Apple ProRes"),
        }
    }
}

/// Audio codecs supported by the platform layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioCodec {
    Aac,
    Mp3,
    Pcm,
    Flac,
    Opus,
}

/// Hardware acceleration backend for video decoding or encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum HwAccel {
    #[default]
    None,
    Nvdec,
    Nvenc,
    D3d11va,
    D3d12va,
    Dxva2,
    Amf,
    Qsv,
    MediaFoundation,
    Vaapi,
}

impl fmt::Display for HwAccel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HwAccel::None => write!(f, "Software (CPU)"),
            HwAccel::Nvdec => write!(f, "NVIDIA NVDEC"),
            HwAccel::Nvenc => write!(f, "NVIDIA NVENC"),
            HwAccel::D3d11va => write!(f, "Direct3D 11 Video Acceleration"),
            HwAccel::D3d12va => write!(f, "Direct3D 12 Video Acceleration"),
            HwAccel::Dxva2 => write!(f, "DirectX Video Acceleration 2.0"),
            HwAccel::Amf => write!(f, "AMD Advanced Media Framework"),
            HwAccel::Qsv => write!(f, "Intel QuickSync Video"),
            HwAccel::MediaFoundation => write!(f, "Windows Media Foundation"),
            HwAccel::Vaapi => write!(f, "VA-API"),
        }
    }
}

/// Metadata extracted from a video file container and bitstream probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub duration: Tick,
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    /// True if frame durations vary across the stream (Variable Frame Rate).
    pub is_vfr: bool,
    /// Container or display matrix rotation.
    pub rotation: Rotation,
    /// Bit depth per color channel (8, 10, or 12).
    pub bit_depth: u8,
    pub pixel_format: PixelFormat,
    pub codec_name: String,
    pub has_audio: bool,
    pub audio_sample_rate: u32,
    pub audio_channels: u16,
}

impl VideoMetadata {
    /// Nominal frame rate in floating point seconds.
    pub fn nominal_fps(&self) -> f64 {
        if self.fps_den == 0 {
            0.0
        } else {
            self.fps_num as f64 / self.fps_den as f64
        }
    }

    /// Dimensions after container rotation is applied.
    pub fn oriented_dimensions(&self) -> (u32, u32) {
        if self.rotation.is_transposed() {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }
}

/// A decoded raw video frame with precise integer timestamp and color representation.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoFrame {
    pub pts: Tick,
    pub duration: Tick,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub rotation: Rotation,
    pub bit_depth: ColorDepth,
    pub data: Vec<u8>,
    pub linesize: [usize; 4],
}

impl VideoFrame {
    /// Creates a new RGBA8 video frame.
    pub fn new_rgba8(pts: Tick, duration: Tick, width: u32, height: u32, data: Vec<u8>) -> Self {
        let stride = width as usize * 4;
        Self {
            pts,
            duration,
            width,
            height,
            format: PixelFormat::Rgba8,
            rotation: Rotation::None,
            bit_depth: ColorDepth::Bits8,
            data,
            linesize: [stride, 0, 0, 0],
        }
    }

    /// Returns width and height after taking rotation into account.
    pub fn oriented_dimensions(&self) -> (u32, u32) {
        if self.rotation.is_transposed() {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }
}

/// Configuration settings for creating a video encoder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoEncoderConfig {
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub bitrate_bps: u64,
    pub codec: VideoCodec,
    pub hardware_accel: HwAccel,
    pub quality_preset: String,
}

impl Default for VideoEncoderConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps_num: 30,
            fps_den: 1,
            bitrate_bps: 12_000_000, // 12 Mbps
            codec: VideoCodec::H264,
            hardware_accel: HwAccel::None,
            quality_preset: "medium".into(),
        }
    }
}
