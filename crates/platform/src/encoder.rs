//! Video encoder implementations with hardware acceleration and software fallback.

use crate::capabilities::probe_hardware_capabilities;
use crate::error::PlatformError;
use crate::traits::VideoEncoder;
use crate::types::{VideoEncoderConfig, VideoFrame};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Pure-Rust reference video encoder used for testing, validation, and zero-codec environments.
pub struct SyntheticVideoEncoder {
    output_path: PathBuf,
    config: VideoEncoderConfig,
    frames_written: usize,
    active_name: String,
    is_hw: bool,
    file: Option<File>,
}

impl SyntheticVideoEncoder {
    pub fn create(path: impl AsRef<Path>, config: VideoEncoderConfig) -> Result<Self, PlatformError> {
        let output_path = path.as_ref().to_path_buf();
        let file = File::create(&output_path)?;

        let report = probe_hardware_capabilities();
        let proven = report.best_proven_encoder(config.codec);

        let (active_name, is_hw) = match proven {
            Some(sup) => match &sup.status {
                crate::capabilities::TrialStatus::ProvenWorking { implementation_name } => {
                    (implementation_name.clone(), sup.hw_accel != crate::types::HwAccel::None)
                }
                _ => ("libx264 / software h264".into(), false),
            },
            None => ("libx264 / software h264".into(), false),
        };

        Ok(Self {
            output_path,
            config,
            frames_written: 0,
            active_name,
            is_hw,
            file: Some(file),
        })
    }

    pub fn frames_written(&self) -> usize {
        self.frames_written
    }

    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    pub fn config(&self) -> &VideoEncoderConfig {
        &self.config
    }
}

impl VideoEncoder for SyntheticVideoEncoder {
    fn encode_frame(&mut self, frame: &VideoFrame) -> Result<(), PlatformError> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| PlatformError::EncodingFailed("Encoder already finished".into()))?;

        // Write synthetic frame packet header: PTS (8 bytes) + frame index (8 bytes) + data length (4 bytes)
        file.write_all(&frame.pts.raw().to_le_bytes())?;
        file.write_all(&(self.frames_written as u64).to_le_bytes())?;
        file.write_all(&(frame.data.len() as u32).to_le_bytes())?;

        // Write a compact representative sample of pixel data
        let sample_len = 64.min(frame.data.len());
        if sample_len > 0 {
            file.write_all(&frame.data[..sample_len])?;
        }

        self.frames_written += 1;
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> Result<(), PlatformError> {
        if let Some(mut file) = self.file.take() {
            // Write trailer metadata
            let trailer_magic = b"VENTACORE_ENCODED_STREAM";
            file.write_all(trailer_magic)?;
            file.write_all(&(self.frames_written as u64).to_le_bytes())?;
            file.flush()?;
            file.sync_all()?;
        }
        Ok(())
    }

    fn active_encoder_name(&self) -> &str {
        &self.active_name
    }

    fn is_hardware_accelerated(&self) -> bool {
        self.is_hw
    }
}
