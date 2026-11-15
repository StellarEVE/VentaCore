//! Audio decoding trait and pure-Rust Symphonia implementation.
//!
//! Provides an abstract `AudioDecoder` trait with a built-in pure Rust decoder
//! and an extension hook for a platform fallback decoder (e.g. FFmpeg in Layer 2).

use crate::error::AudioError;
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decoded audio buffer containing mono-mixed floating point samples in `[-1.0, 1.0]`.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

/// Abstract audio decoder trait.
///
/// Implementations can be pure-Rust (Symphonia) or hardware/system (FFmpeg in Layer 2).
pub trait AudioDecoder: Send + Sync {
    fn decode(&self, path: &Path) -> Result<DecodedAudio, AudioError>;
}

/// Pure-Rust audio decoder powered by Symphonia.
/// Supports MP3, WAV, FLAC, AAC, OGG/Vorbis, and MP4/M4A containers.
#[derive(Debug, Default, Clone)]
pub struct SymphoniaDecoder;

impl SymphoniaDecoder {
    pub fn new() -> Self {
        Self
    }
}

impl AudioDecoder for SymphoniaDecoder {
    fn decode(&self, path: &Path) -> Result<DecodedAudio, AudioError> {
        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = DecoderOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| AudioError::Decode(format!("Probe failed: {e}")))?;

        let mut format = probed.format;

        // Find the first audio track with a supported codec
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(AudioError::NoAudioTrack)?;

        let track_id = track.id;
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .map_err(|e| AudioError::Decode(format!("Codec creation failed: {e}")))?;

        let mut sample_rate = 0u32;
        let mut channels = 0u16;
        let mut mono_samples = Vec::new();

        while let Ok(packet) = format.next_packet() {
            if packet.track_id() != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    if sample_rate == 0 {
                        let spec = *decoded.spec();
                        sample_rate = spec.rate;
                        channels = spec.channels.count() as u16;
                    }
                    extract_mono_samples(&decoded, &mut mono_samples);
                }
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(SymphoniaError::DecodeError(_)) => {
                    // Skip corrupt or unreadable packet
                    continue;
                }
                Err(e) => return Err(AudioError::Decode(format!("Decode failure: {e}"))),
            }
        }

        if mono_samples.is_empty() || sample_rate == 0 {
            return Err(AudioError::EmptyAudio);
        }

        Ok(DecodedAudio {
            sample_rate,
            channels: channels.max(1),
            samples: mono_samples,
        })
    }
}

fn extract_mono_samples(buf_ref: &AudioBufferRef<'_>, out: &mut Vec<f32>) {
    match buf_ref {
        AudioBufferRef::F32(buf) => {
            let n_channels = buf.spec().channels.count();
            let n_frames = buf.frames();
            out.reserve(n_frames);
            if n_channels == 1 {
                out.extend_from_slice(buf.chan(0));
            } else {
                for i in 0..n_frames {
                    let mut sum = 0.0_f32;
                    for c in 0..n_channels {
                        sum += buf.chan(c)[i];
                    }
                    out.push(sum / (n_channels as f32));
                }
            }
        }
        AudioBufferRef::S16(buf) => {
            let n_channels = buf.spec().channels.count();
            let n_frames = buf.frames();
            out.reserve(n_frames);
            if n_channels == 1 {
                for &s in buf.chan(0) {
                    out.push(s as f32 / 32768.0);
                }
            } else {
                for i in 0..n_frames {
                    let mut sum = 0.0_f32;
                    for c in 0..n_channels {
                        sum += buf.chan(c)[i] as f32 / 32768.0;
                    }
                    out.push(sum / (n_channels as f32));
                }
            }
        }
        AudioBufferRef::U8(buf) => {
            let n_channels = buf.spec().channels.count();
            let n_frames = buf.frames();
            out.reserve(n_frames);
            for i in 0..n_frames {
                let mut sum = 0.0_f32;
                for c in 0..n_channels {
                    sum += (buf.chan(c)[i] as f32 - 128.0) / 128.0;
                }
                out.push(sum / (n_channels as f32));
            }
        }
        AudioBufferRef::S32(buf) => {
            let n_channels = buf.spec().channels.count();
            let n_frames = buf.frames();
            out.reserve(n_frames);
            for i in 0..n_frames {
                let mut sum = 0.0_f32;
                for c in 0..n_channels {
                    sum += buf.chan(c)[i] as f32 / 2147483648.0;
                }
                out.push(sum / (n_channels as f32));
            }
        }
        _ => {}
    }
}
