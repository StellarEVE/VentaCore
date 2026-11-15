//! Media file classifier for VentaCore.
//!
//! Conforms to Charter Rules & Part 4b:
//! - Classifies by container and stream contents, NEVER by file extension.
//! - Attached pictures (cover art inside MP3, FLAC, or M4A) are treated as album art, not video.
//! - Videos with pictures are classified as Video (with or without audio).
//! - Audio-only files are classified as AudioOnly.
//! - Text, documents, still images, and damaged files are classified as Unsupported with reasons.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use ventacore_core::Tick;

/// Result of media classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaClassification {
    AudioOnly {
        sample_rate: u32,
        channels: u16,
        duration: Tick,
        has_cover_art: bool,
    },
    Video {
        width: u32,
        height: u32,
        fps_num: u32,
        fps_den: u32,
        duration: Tick,
        has_audio: bool,
    },
    Unsupported {
        reason: String,
    },
}

impl MediaClassification {
    pub fn is_audio_only(&self) -> bool {
        matches!(self, MediaClassification::AudioOnly { .. })
    }

    pub fn is_video(&self) -> bool {
        matches!(self, MediaClassification::Video { .. })
    }

    pub fn is_unsupported(&self) -> bool {
        matches!(self, MediaClassification::Unsupported { .. })
    }
}

/// Classifies a media file by inspecting its raw container header and streams.
pub fn classify<P: AsRef<Path>>(path: P) -> MediaClassification {
    let path_ref = path.as_ref();
    if !path_ref.exists() {
        return MediaClassification::Unsupported {
            reason: format!("File does not exist: {}", path_ref.display()),
        };
    }

    let mut file = match File::open(path_ref) {
        Ok(f) => f,
        Err(e) => {
            return MediaClassification::Unsupported {
                reason: format!("Cannot open file: {e}"),
            };
        }
    };

    let file_len = match file.metadata() {
        Ok(m) => m.len(),
        Err(e) => {
            return MediaClassification::Unsupported {
                reason: format!("Cannot read file metadata: {e}"),
            };
        }
    };

    if file_len == 0 {
        return MediaClassification::Unsupported {
            reason: "File is empty (0 bytes)".into(),
        };
    }

    let mut header = [0u8; 4096];
    let bytes_read = match file.read(&mut header) {
        Ok(n) => n,
        Err(e) => {
            return MediaClassification::Unsupported {
                reason: format!("Cannot read file header: {e}"),
            };
        }
    };

    let buf = &header[..bytes_read];

    // 1. Text / Document Check
    if is_text_or_document(buf) {
        return MediaClassification::Unsupported {
            reason: "Text or document file is not supported media".into(),
        };
    }

    // 2. Still Image Check
    if is_still_image(buf) {
        return MediaClassification::Unsupported {
            reason: "Still image cannot be used as video footage or song audio".into(),
        };
    }

    // 3. MP3 Check (with or without ID3v2 APIC cover art)
    if let Some(res) = probe_mp3(&mut file, buf, file_len) {
        return res;
    }

    // 4. FLAC Check
    if let Some(res) = probe_flac(buf, file_len) {
        return res;
    }

    // 5. WAV Check
    if let Some(res) = probe_wav(buf, file_len) {
        return res;
    }

    // 6. MP4 / MOV / ISO BMFF Check
    if let Some(res) = probe_isobmff(&mut file, buf, file_len) {
        return refine_video_classification(path_ref, res);
    }

    // 7. Matroska / WebM Check
    if let Some(res) = probe_matroska(buf, file_len) {
        return refine_video_classification(path_ref, res);
    }

    // 8. General fallback probe with ffprobe for other video formats (AVI, etc.)
    if let Some(ffprobe_exe) = crate::decoder::find_ffprobe_binary() {
        if let Ok(meta) = crate::decoder::probe_video_with_ffprobe(path_ref, &ffprobe_exe) {
            if meta.width > 0 && meta.height > 0 {
                return MediaClassification::Video {
                    width: meta.width,
                    height: meta.height,
                    fps_num: meta.fps_num,
                    fps_den: meta.fps_den,
                    duration: meta.duration,
                    has_audio: meta.has_audio,
                };
            }
        }
    }

    // Default unrecognized
    MediaClassification::Unsupported {
        reason: "Unrecognized media container or unsupported codec".into(),
    }
}

fn refine_video_classification(path: &Path, raw: MediaClassification) -> MediaClassification {
    match raw {
        MediaClassification::Video {
            width,
            height,
            fps_num,
            fps_den,
            duration,
            has_audio,
        } => {
            if let Some(ffprobe_exe) = crate::decoder::find_ffprobe_binary() {
                if let Ok(meta) = crate::decoder::probe_video_with_ffprobe(path, &ffprobe_exe) {
                    return MediaClassification::Video {
                        width: meta.width,
                        height: meta.height,
                        fps_num: meta.fps_num,
                        fps_den: meta.fps_den,
                        duration: meta.duration,
                        has_audio: meta.has_audio,
                    };
                }
            }
            MediaClassification::Video {
                width,
                height,
                fps_num,
                fps_den,
                duration,
                has_audio,
            }
        }
        other => other,
    }
}

fn is_text_or_document(buf: &[u8]) -> bool {
    if buf.starts_with(b"%PDF-") || buf.starts_with(b"{\\rtf") {
        return true;
    }
    if buf.starts_with(b"<!DOCTYPE") || buf.starts_with(b"<?xml") {
        return true;
    }

    // Check if buffer is predominantly UTF-8 text
    if buf.len() >= 16 {
        let printable = buf.iter().take(256).filter(|&&b| {
            b == b'\n' || b == b'\r' || b == b'\t' || (b >= 32 && b <= 126)
        }).count();
        let total = buf.len().min(256);
        if printable == total {
            return true;
        }
    }

    false
}

fn is_still_image(buf: &[u8]) -> bool {
    // PNG
    if buf.starts_with(b"\x89PNG\r\n\x1a\n") {
        return true;
    }
    // JPEG
    if buf.starts_with(b"\xFF\xD8\xFF") {
        return true;
    }
    // GIF
    if buf.starts_with(b"GIF87a") || buf.starts_with(b"GIF89a") {
        return true;
    }
    // BMP
    if buf.starts_with(b"BM") && buf.len() > 14 {
        let reserved = &buf[6..10];
        if reserved == [0, 0, 0, 0] {
            return true;
        }
    }
    false
}

fn probe_mp3(file: &mut File, buf: &[u8], file_len: u64) -> Option<MediaClassification> {
    let mut has_cover_art = false;
    let mut audio_offset = 0usize;

    // Check for ID3v2 tag
    if buf.starts_with(b"ID3") && buf.len() >= 10 {
        let size = ((buf[6] as usize & 0x7F) << 21)
            | ((buf[7] as usize & 0x7F) << 14)
            | ((buf[8] as usize & 0x7F) << 7)
            | (buf[9] as usize & 0x7F);

        let total_id3_len = 10 + size;
        audio_offset = total_id3_len;

        // Read more of ID3 tag to check for APIC (Attached Picture)
        let check_len = total_id3_len.min(1024 * 1024);
        let mut id3_buf = vec![0u8; check_len];
        let _ = file.seek(SeekFrom::Start(0));
        if file.read_exact(&mut id3_buf).is_ok() {
            if id3_buf.windows(4).any(|w| w == b"APIC" || w == b"PIC ") {
                has_cover_art = true;
            }
        }
    }

    // Search for MPEG Audio Sync 0xFFE0..0xFFFF
    let _ = file.seek(SeekFrom::Start(audio_offset as u64));
    let mut sync_buf = [0u8; 4096];
    let n = file.read(&mut sync_buf).ok()?;
    if n < 4 {
        // If starts with ID3 but sync wasn't found in first chunk, it is still audio
        if audio_offset > 0 {
            return Some(MediaClassification::AudioOnly {
                sample_rate: 44100,
                channels: 2,
                duration: Tick::from_seconds(180),
                has_cover_art,
            });
        }
        return None;
    }

    for i in 0..n.saturating_sub(3) {
        if sync_buf[i] == 0xFF && (sync_buf[i + 1] & 0xE0) == 0xE0 {
            let b1 = sync_buf[i + 1];
            let b2 = sync_buf[i + 2];

            let mpeg_ver = (b1 >> 3) & 0x03;
            let layer = (b1 >> 1) & 0x03;
            let sample_rate_idx = ((b2 >> 2) & 0x03) as usize;

            if layer != 0 && sample_rate_idx < 3 {
                let sample_rate = match (mpeg_ver, sample_rate_idx) {
                    (3, 0) => 44100,
                    (3, 1) => 48000,
                    (3, 2) => 32000,
                    (2, 0) => 22050,
                    (2, 1) => 24000,
                    (2, 2) => 16000,
                    _ => 44100,
                };

                let channels = if ((sync_buf[i + 3] >> 6) & 0x03) == 3 { 1 } else { 2 };
                let est_duration_secs = (file_len.saturating_sub(audio_offset as u64) as f64) / (128_000.0 / 8.0);
                let duration = Tick::from_seconds(est_duration_secs.max(1.0) as i64);

                return Some(MediaClassification::AudioOnly {
                    sample_rate,
                    channels,
                    duration,
                    has_cover_art,
                });
            }
        }
    }

    if audio_offset > 0 {
        Some(MediaClassification::AudioOnly {
            sample_rate: 44100,
            channels: 2,
            duration: Tick::from_seconds(180),
            has_cover_art,
        })
    } else {
        None
    }
}

fn probe_flac(buf: &[u8], file_len: u64) -> Option<MediaClassification> {
    if !buf.starts_with(b"fLaC") || buf.len() < 42 {
        return None;
    }

    let has_cover_art = buf.windows(4).any(|w| w == b"\x06\x00\x00" || w == b"PICTURE");
    let streaminfo = &buf[8..42];

    let sample_rate = ((streaminfo[10] as u32) << 12)
        | ((streaminfo[11] as u32) << 4)
        | ((streaminfo[12] as u32) >> 4);
    let channels = (((streaminfo[12] >> 1) & 0x07) + 1) as u16;

    let total_samples = (((streaminfo[13] as u64) & 0x0F) << 32)
        | ((streaminfo[14] as u64) << 24)
        | ((streaminfo[15] as u64) << 16)
        | ((streaminfo[16] as u64) << 8)
        | (streaminfo[17] as u64);

    let duration = if sample_rate > 0 && total_samples > 0 {
        Tick::from_samples(total_samples as i64, sample_rate)
    } else {
        Tick::from_seconds((file_len / 100_000).max(1) as i64)
    };

    Some(MediaClassification::AudioOnly {
        sample_rate: if sample_rate > 0 { sample_rate } else { 44100 },
        channels: if channels > 0 { channels } else { 2 },
        duration,
        has_cover_art,
    })
}

fn probe_wav(buf: &[u8], file_len: u64) -> Option<MediaClassification> {
    if buf.len() < 36 || !buf.starts_with(b"RIFF") || &buf[8..12] != b"WAVE" {
        return None;
    }

    let mut channels = 2u16;
    let mut sample_rate = 44100u32;
    let mut byte_rate = 176400u32;

    for i in 12..buf.len().saturating_sub(16) {
        if &buf[i..i + 4] == b"fmt " {
            channels = u16::from_le_bytes([buf[i + 10], buf[i + 11]]);
            sample_rate = u32::from_le_bytes([buf[i + 12], buf[i + 13], buf[i + 14], buf[i + 15]]);
            byte_rate = u32::from_le_bytes([buf[i + 16], buf[i + 17], buf[i + 18], buf[i + 19]]);
            break;
        }
    }

    let duration_secs = if byte_rate > 0 {
        (file_len as f64) / (byte_rate as f64)
    } else {
        10.0
    };

    Some(MediaClassification::AudioOnly {
        sample_rate,
        channels,
        duration: Tick::from_seconds(duration_secs.max(1.0) as i64),
        has_cover_art: false,
    })
}

fn probe_isobmff(file: &mut File, buf: &[u8], file_len: u64) -> Option<MediaClassification> {
    if buf.len() < 8 {
        return None;
    }

    let mut is_iso = false;
    if &buf[4..8] == b"ftyp" || &buf[4..8] == b"moov" || &buf[4..8] == b"mdat" {
        is_iso = true;
    }

    if !is_iso {
        return None;
    }

    // Read initial 64KB to scan for video and audio handler boxes ('hdlr' with 'vide' or 'soun')
    let scan_len = (file_len as usize).min(64 * 1024);
    let mut scan_buf = vec![0u8; scan_len];
    let _ = file.seek(SeekFrom::Start(0));
    let read_bytes = file.read(&mut scan_buf).ok()?;
    let scan = &scan_buf[..read_bytes];

    let has_video_track = scan.windows(8).any(|w| &w[..4] == b"hdlr" && &w[4..8] == b"vide")
        || scan.windows(4).any(|w| w == b"vide" || w == b"avc1" || w == b"hvc1" || w == b"vp09");

    let has_audio_track = scan.windows(8).any(|w| &w[..4] == b"hdlr" && &w[4..8] == b"soun")
        || scan.windows(4).any(|w| w == b"soun" || w == b"mp4a");

    let has_cover_art = scan.windows(4).any(|w| w == b"covr");

    if has_video_track {
        Some(MediaClassification::Video {
            width: 1920,
            height: 1080,
            fps_num: 30,
            fps_den: 1,
            duration: Tick::from_seconds(30),
            has_audio: has_audio_track,
        })
    } else if has_audio_track {
        Some(MediaClassification::AudioOnly {
            sample_rate: 44100,
            channels: 2,
            duration: Tick::from_seconds(180),
            has_cover_art,
        })
    } else {
        // Default video if ftyp matches common video profiles
        if scan.windows(4).any(|w| w == b"isom" || w == b"mp42" || w == b"qt  ") {
            Some(MediaClassification::Video {
                width: 1920,
                height: 1080,
                fps_num: 30,
                fps_den: 1,
                duration: Tick::from_seconds(30),
                has_audio: false,
            })
        } else {
            None
        }
    }
}

fn probe_matroska(buf: &[u8], _file_len: u64) -> Option<MediaClassification> {
    if !buf.starts_with(b"\x1A\x45\xDF\xA3") {
        return None;
    }

    let has_video = buf.windows(6).any(|w| w == b"V_MPEG" || w == b"V_MS/V" || w == b"V_VP8" || w == b"V_VP9");
    let has_audio = buf.windows(6).any(|w| w == b"A_AAC" || w == b"A_VORB" || w == b"A_OPUS" || w == b"A_MPEG");

    if has_video || (!has_video && !has_audio) {
        Some(MediaClassification::Video {
            width: 1920,
            height: 1080,
            fps_num: 30,
            fps_den: 1,
            duration: Tick::from_seconds(30),
            has_audio,
        })
    } else {
        Some(MediaClassification::AudioOnly {
            sample_rate: 48000,
            channels: 2,
            duration: Tick::from_seconds(180),
            has_cover_art: false,
        })
    }
}
