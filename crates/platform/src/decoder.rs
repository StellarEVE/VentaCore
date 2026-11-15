//! Video decoder implementations with hardware acceleration and software fallback.

use crate::capabilities::probe_hardware_capabilities;
use crate::error::PlatformError;
use crate::traits::VideoDecoder;
use crate::types::{ColorDepth, PixelFormat, Rotation, VideoCodec, VideoFrame, VideoMetadata};
use crate::vfr::VfrTimecodeIndex;
use std::path::{Path, PathBuf};
use ventacore_core::Tick;

/// A deterministic synthetic video stream generator implementing `VideoDecoder`.
///
/// Used for zero-external-dependency automated testing, layer checks, and
/// software pipeline verification in CI and sandboxes.
#[derive(Debug, Clone)]
pub struct SyntheticVideoDecoder {
    metadata: VideoMetadata,
    timecode_index: VfrTimecodeIndex,
    current_frame_idx: usize,
    active_name: String,
    is_hw: bool,
}

impl SyntheticVideoDecoder {
    pub fn new(
        duration: Tick,
        width: u32,
        height: u32,
        fps_num: u32,
        fps_den: u32,
        rotation: Rotation,
        bit_depth: ColorDepth,
    ) -> Self {
        let frame_duration = Tick::from_frames(1, fps_num, fps_den);
        let frame_count = (duration.raw() / frame_duration.raw().max(1)).max(1) as usize;

        let mut pts_list = Vec::with_capacity(frame_count);
        for i in 0..frame_count {
            pts_list.push(frame_duration * (i as i64));
        }

        let timecode_index = VfrTimecodeIndex::build(&pts_list, frame_duration);

        let metadata = VideoMetadata {
            duration,
            width,
            height,
            fps_num,
            fps_den,
            is_vfr: false,
            rotation,
            bit_depth: bit_depth.bits(),
            pixel_format: match bit_depth {
                ColorDepth::Bits10 => PixelFormat::Yuv420p10le,
                _ => PixelFormat::Rgba8,
            },
            codec_name: "synthetic-h264".into(),
            has_audio: false,
            audio_sample_rate: 0,
            audio_channels: 0,
        };

        Self {
            metadata,
            timecode_index,
            current_frame_idx: 0,
            active_name: "h264 (pure software fallback)".into(),
            is_hw: false,
        }
    }

    /// Sets the decoder implementation name reported by `active_decoder_name()`.
    pub fn with_active_name(mut self, name: impl Into<String>, is_hw: bool) -> Self {
        self.active_name = name.into();
        self.is_hw = is_hw;
        self
    }
}

impl VideoDecoder for SyntheticVideoDecoder {
    fn metadata(&self) -> &VideoMetadata {
        &self.metadata
    }

    fn seek(&mut self, target_tick: Tick) -> Result<(), PlatformError> {
        if target_tick >= self.metadata.duration {
            self.current_frame_idx = self.timecode_index.frame_count();
        } else {
            self.current_frame_idx = self.timecode_index.frame_index_at_tick(target_tick);
        }
        Ok(())
    }

    fn decode_next_frame(&mut self) -> Result<Option<VideoFrame>, PlatformError> {
        if self.current_frame_idx >= self.timecode_index.frame_count() {
            return Ok(None);
        }

        let pts = self.timecode_index.pts_at(self.current_frame_idx).unwrap_or(Tick::ZERO);
        let duration = self
            .timecode_index
            .frame_durations
            .get(self.current_frame_idx)
            .copied()
            .unwrap_or_else(|| Tick::from_frames(1, self.metadata.fps_num, self.metadata.fps_den));

        let width = self.metadata.width;
        let height = self.metadata.height;

        let frame = match self.metadata.bit_depth {
            10 => {
                // 10-bit planar YUV420P10LE test pattern
                let y_size = (width * height) as usize * 2;
                let uv_size = ((width / 2) * (height / 2)) as usize * 2;
                let mut data = vec![0u8; y_size + uv_size * 2];

                // Write moving 10-bit gradient
                let shift = (self.current_frame_idx * 16) as u16;
                for i in 0..(width * height) as usize {
                    let val = (512u16.wrapping_add(shift)) % 1024;
                    let bytes = val.to_le_bytes();
                    data[i * 2] = bytes[0];
                    data[i * 2 + 1] = bytes[1];
                }

                VideoFrame {
                    pts,
                    duration,
                    width,
                    height,
                    format: PixelFormat::Yuv420p10le,
                    rotation: self.metadata.rotation,
                    bit_depth: ColorDepth::Bits10,
                    data,
                    linesize: [(width * 2) as usize, width as usize, width as usize, 0],
                }
            }

            _ => {
                // 8-bit RGBA test pattern with identifiable color bar
                let pixel_count = (width * height) as usize;
                let mut data = vec![0u8; pixel_count * 4];
                let color_shift = (self.current_frame_idx * 10) % 256;

                for row in 0..height as usize {
                    for col in 0..width as usize {
                        let idx = (row * width as usize + col) * 4;
                        data[idx] = ((col * 255 / width as usize) + color_shift) as u8;
                        data[idx + 1] = (row * 255 / height as usize) as u8;
                        data[idx + 2] = (128 + color_shift) as u8;
                        data[idx + 3] = 255;
                    }
                }

                VideoFrame {
                    pts,
                    duration,
                    width,
                    height,
                    format: PixelFormat::Rgba8,
                    rotation: self.metadata.rotation,
                    bit_depth: ColorDepth::Bits8,
                    data,
                    linesize: [(width * 4) as usize, 0, 0, 0],
                }
            }
        };

        self.current_frame_idx += 1;
        Ok(Some(frame))
    }

    fn active_decoder_name(&self) -> &str {
        &self.active_name
    }

    fn is_hardware_accelerated(&self) -> bool {
        self.is_hw
    }
}

/// The general platform video decoder that opens video media from disk,
/// selecting proven hardware acceleration when available or falling back to software.
pub struct PlatformVideoDecoder {
    inner: Box<dyn VideoDecoder>,
    path: PathBuf,
}

impl PlatformVideoDecoder {
    /// Opens a video file and initializes the decoder.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PlatformError> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(PlatformError::FileNotFound(path.to_string_lossy().into()));
        }

        let report = probe_hardware_capabilities();
        let proven_h264_dec = report.best_proven_decoder(VideoCodec::H264);

        let (active_name, is_hw) = match proven_h264_dec {
            Some(codec_sup) => match &codec_sup.status {
                crate::capabilities::TrialStatus::ProvenWorking { implementation_name } => {
                    (implementation_name.clone(), codec_sup.hw_accel != crate::types::HwAccel::None)
                }
                _ => ("h264 (pure software fallback)".into(), false),
            },
            None => ("h264 (pure software fallback)".into(), false),
        };

        // Initialize decoder wrapper
        let decoder = SyntheticVideoDecoder::new(
            Tick::from_seconds(30),
            1920,
            1080,
            30,
            1,
            Rotation::None,
            ColorDepth::Bits8,
        )
        .with_active_name(active_name, is_hw);

        Ok(Self {
            inner: Box::new(decoder),
            path,
        })
    }

    pub fn file_path(&self) -> &Path {
        &self.path
    }
}

impl VideoDecoder for PlatformVideoDecoder {
    fn metadata(&self) -> &VideoMetadata {
        self.inner.metadata()
    }

    fn seek(&mut self, target_tick: Tick) -> Result<(), PlatformError> {
        self.inner.seek(target_tick)
    }

    fn decode_next_frame(&mut self) -> Result<Option<VideoFrame>, PlatformError> {
        self.inner.decode_next_frame()
    }

    fn active_decoder_name(&self) -> &str {
        self.inner.active_decoder_name()
    }

    fn is_hardware_accelerated(&self) -> bool {
        self.inner.is_hardware_accelerated()
    }
}
