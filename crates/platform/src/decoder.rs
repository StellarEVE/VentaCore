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
                // Fast 8-bit RGBA test pattern with moving color bar
                let pixel_count = (width * height) as usize;
                let mut data = vec![0u8; pixel_count * 4];
                let color_shift = ((self.current_frame_idx * 10) % 256) as u8;
                let r_base = color_shift;
                let g_base = ((self.current_frame_idx * 5) % 256) as u8;
                let b_base = color_shift.wrapping_add(128);

                let pixel = [r_base, g_base, b_base, 255];
                for chunk in data.chunks_exact_mut(4) {
                    chunk.copy_from_slice(&pixel);
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

        // 1. Try real FFmpeg video decoder if tools are available and file probes successfully
        if let (Some(ffprobe_exe), Some(ffmpeg_exe)) = (find_ffprobe_binary(), find_ffmpeg_binary()) {
            if let Ok(metadata) = probe_video_with_ffprobe(&path, &ffprobe_exe) {
                let active_name = format!("ffmpeg ({})", metadata.codec_name);
                let decoder = FfmpegVideoDecoder::new(path.clone(), metadata, ffmpeg_exe, active_name);
                return Ok(Self {
                    inner: Box::new(decoder),
                    path,
                });
            }
        }

        // 2. Fallback: Synthetic decoder for automated testing, sandboxes, and mock files
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

        let decoder = SyntheticVideoDecoder::new(
            Tick::from_seconds(10),
            640,
            360,
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

/// Real FFmpeg-powered video decoder reading raw RGBA frames from stdout pipe.
pub struct FfmpegVideoDecoder {
    path: PathBuf,
    metadata: VideoMetadata,
    ffmpeg_exe: PathBuf,
    active_name: String,
    current_tick: Tick,
    child: Option<std::process::Child>,
    child_tick: Tick,
    last_frame: Option<VideoFrame>,
}

impl FfmpegVideoDecoder {
    pub fn new(
        path: PathBuf,
        metadata: VideoMetadata,
        ffmpeg_exe: PathBuf,
        active_name: String,
    ) -> Self {
        Self {
            path,
            metadata,
            ffmpeg_exe,
            active_name,
            current_tick: Tick::ZERO,
            child: None,
            child_tick: Tick::ZERO,
            last_frame: None,
        }
    }

    fn terminate_child(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn spawn_stream(&mut self) -> Result<(), PlatformError> {
        self.terminate_child();

        let secs = self.current_tick.to_seconds_f64();
        let mut cmd = std::process::Command::new(&self.ffmpeg_exe);
        cmd.args(&[
            "-nostdin",
            "-noautorotate",
            "-ss",
            &format!("{:.6}", secs),
            "-i",
        ]);
        cmd.arg(&self.path);
        cmd.args(&[
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-",
        ]);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::null());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let child = cmd.spawn().map_err(|e| {
            PlatformError::DecodingFailed(format!("Failed to spawn ffmpeg: {}", e))
        })?;

        self.child = Some(child);
        self.child_tick = self.current_tick;
        Ok(())
    }
}

impl VideoDecoder for FfmpegVideoDecoder {
    fn metadata(&self) -> &VideoMetadata {
        &self.metadata
    }

    fn seek(&mut self, target_tick: Tick) -> Result<(), PlatformError> {
        let clamped = target_tick.min(self.metadata.duration);
        let frame_dur = Tick::from_frames(1, self.metadata.fps_num, self.metadata.fps_den);

        // If child is running and clamped matches the next frame in the stream:
        let is_next_frame = self.child.is_some()
            && (clamped >= self.child_tick && clamped < self.child_tick + frame_dur);

        // If clamped matches the frame we already decoded:
        let is_same_frame = self
            .last_frame
            .as_ref()
            .map(|f| clamped >= f.pts && clamped < f.pts + f.duration)
            .unwrap_or(false);

        if !is_next_frame && !is_same_frame {
            self.terminate_child();
        }

        self.current_tick = clamped;
        Ok(())
    }

    fn decode_next_frame(&mut self) -> Result<Option<VideoFrame>, PlatformError> {
        if self.current_tick >= self.metadata.duration {
            return Ok(None);
        }

        // Return cached frame if we are already at current_tick (e.g. paused repainting)
        if let Some(ref frame) = self.last_frame {
            let frame_dur = Tick::from_frames(1, self.metadata.fps_num, self.metadata.fps_den);
            if self.current_tick >= frame.pts && self.current_tick < frame.pts + frame_dur {
                return Ok(Some(frame.clone()));
            }
        }

        if self.child.is_none() {
            self.spawn_stream()?;
        }

        let frame_size = (self.metadata.width * self.metadata.height * 4) as usize;
        let mut buffer = vec![0u8; frame_size];

        if let Some(child) = self.child.as_mut() {
            if let Some(stdout) = child.stdout.as_mut() {
                use std::io::Read;
                match stdout.read_exact(&mut buffer) {
                    Ok(()) => {
                        let pts = self.child_tick;
                        let frame_duration =
                            Tick::from_frames(1, self.metadata.fps_num, self.metadata.fps_den);
                        self.child_tick += frame_duration;
                        self.current_tick = self.child_tick;

                        let frame = VideoFrame {
                            pts,
                            duration: frame_duration,
                            width: self.metadata.width,
                            height: self.metadata.height,
                            format: PixelFormat::Rgba8,
                            rotation: self.metadata.rotation,
                            bit_depth: ColorDepth::Bits8,
                            data: buffer,
                            linesize: [(self.metadata.width * 4) as usize, 0, 0, 0],
                        };
                        self.last_frame = Some(frame.clone());
                        return Ok(Some(frame));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                        self.terminate_child();
                        return Ok(None);
                    }
                    Err(e) => {
                        self.terminate_child();
                        return Err(PlatformError::DecodingFailed(format!(
                            "Read error from ffmpeg stream: {}",
                            e
                        )));
                    }
                }
            }
        }

        Ok(None)
    }

    fn active_decoder_name(&self) -> &str {
        &self.active_name
    }

    fn is_hardware_accelerated(&self) -> bool {
        false
    }
}

impl Drop for FfmpegVideoDecoder {
    fn drop(&mut self) {
        self.terminate_child();
    }
}

/// Locates the ffmpeg executable on Windows or POSIX.
pub fn find_ffmpeg_binary() -> Option<PathBuf> {
    find_tool_binary("ffmpeg")
}

/// Locates the ffprobe executable on Windows or POSIX.
pub fn find_ffprobe_binary() -> Option<PathBuf> {
    find_tool_binary("ffprobe")
}

fn find_tool_binary(tool_name: &str) -> Option<PathBuf> {
    let exe_name = if cfg!(windows) {
        format!("{}.exe", tool_name)
    } else {
        tool_name.to_string()
    };

    // 1. Check PATH
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(&exe_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // 2. Check FFMPEG_BIN or FFMPEG_DLL_PATH
    if let Ok(bin_dir) = std::env::var("FFMPEG_BIN") {
        let candidate = PathBuf::from(bin_dir).join(&exe_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Ok(dll_dir) = std::env::var("FFMPEG_DLL_PATH") {
        let candidate = PathBuf::from(dll_dir).join(&exe_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // 3. Check Windows common directories
    let candidates = [
        r"C:\ffmpeg\bin",
        r"C:\tools\ffmpeg\bin",
        r"C:\Program Files\ffmpeg\bin",
    ];
    for dir in &candidates {
        let candidate = PathBuf::from(dir).join(&exe_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // 4. Check LocalAppData WinGet packages
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let winget_path = PathBuf::from(local_app_data).join(r"Microsoft\WinGet\Packages");
        if winget_path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&winget_path) {
                for entry in entries.flatten() {
                    let folder = entry.file_name().to_string_lossy().to_string();
                    if folder.contains("FFmpeg") {
                        let bin_direct = entry.path().join("bin").join(&exe_name);
                        if bin_direct.is_file() {
                            return Some(bin_direct);
                        }
                        if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                            for sub in sub_entries.flatten() {
                                let sub_bin = sub.path().join("bin").join(&exe_name);
                                if sub_bin.is_file() {
                                    return Some(sub_bin);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

/// Probes a media file using ffprobe and extracts comprehensive container metadata.
pub fn probe_video_with_ffprobe(
    path: &Path,
    ffprobe_exe: &Path,
) -> Result<VideoMetadata, PlatformError> {
    let mut cmd = std::process::Command::new(ffprobe_exe);
    cmd.args(&[
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ]);
    cmd.arg(path);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let output = cmd.output().map_err(|e| {
        PlatformError::InitializationFailed(format!("Failed to execute ffprobe: {}", e))
    })?;

    if !output.status.success() {
        return Err(PlatformError::InitializationFailed(
            "ffprobe returned non-zero status".into(),
        ));
    }

    let json_val: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| {
        PlatformError::InitializationFailed(format!("Failed to parse ffprobe json: {}", e))
    })?;

    let streams = json_val["streams"].as_array().ok_or_else(|| {
        PlatformError::InitializationFailed("No streams in ffprobe output".into())
    })?;

    // Find first non-cover-art video stream
    let video_stream = streams.iter().find(|s| {
        s["codec_type"].as_str() == Some("video")
            && s["disposition"]["attached_pic"].as_i64() != Some(1)
    }).ok_or_else(|| {
        PlatformError::InitializationFailed("No real video stream found".into())
    })?;

    let width = video_stream["width"].as_u64().unwrap_or(0) as u32;
    let height = video_stream["height"].as_u64().unwrap_or(0) as u32;
    if width == 0 || height == 0 {
        return Err(PlatformError::InitializationFailed(
            "Invalid video dimensions (0x0)".into(),
        ));
    }

    let (fps_num, fps_den) =
        parse_frame_rate(video_stream["r_frame_rate"].as_str().unwrap_or("30/1"));

    let dur_secs: f64 = video_stream["duration"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            json_val["format"]["duration"]
                .as_str()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(10.0);

    let duration = Tick::from_seconds_f64(dur_secs);
    let rotation = parse_rotation(video_stream);

    let pix_fmt_str = video_stream["pix_fmt"].as_str().unwrap_or("yuv420p");
    let (pixel_format, bit_depth) = if pix_fmt_str.contains("10") {
        (PixelFormat::Yuv420p10le, 10)
    } else {
        (PixelFormat::Rgba8, 8)
    };

    let codec_name = video_stream["codec_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    let audio_stream = streams
        .iter()
        .find(|s| s["codec_type"].as_str() == Some("audio"));
    let has_audio = audio_stream.is_some();
    let audio_sample_rate = audio_stream
        .and_then(|s| s["sample_rate"].as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let audio_channels = audio_stream
        .and_then(|s| s["channels"].as_u64())
        .unwrap_or(0) as u16;

    Ok(VideoMetadata {
        duration,
        width,
        height,
        fps_num,
        fps_den,
        is_vfr: false,
        rotation,
        bit_depth,
        pixel_format,
        codec_name,
        has_audio,
        audio_sample_rate,
        audio_channels,
    })
}

fn parse_rotation(video_stream: &serde_json::Value) -> Rotation {
    if let Some(rotate_str) = video_stream["tags"]["rotate"].as_str() {
        if let Ok(deg) = rotate_str.parse::<i32>() {
            return match deg.rem_euclid(360) {
                90 => Rotation::Rotate90,
                180 => Rotation::Rotate180,
                270 => Rotation::Rotate270,
                _ => Rotation::None,
            };
        }
    }
    if let Some(side_data) = video_stream["side_data_list"].as_array() {
        for item in side_data {
            if let Some(deg) = item["rotation"].as_i64() {
                return match deg.rem_euclid(360) {
                    90 => Rotation::Rotate90,
                    180 => Rotation::Rotate180,
                    270 => Rotation::Rotate270,
                    _ => Rotation::None,
                };
            }
        }
    }
    Rotation::None
}

fn parse_frame_rate(s: &str) -> (u32, u32) {
    if let Some((num, den)) = s.split_once('/') {
        let n: u32 = num.parse().unwrap_or(30);
        let d: u32 = den.parse().unwrap_or(1);
        if d == 0 {
            (30, 1)
        } else {
            (n, d)
        }
    } else if let Ok(fps) = s.parse::<f64>() {
        let n = (fps * 1000.0).round() as u32;
        (n, 1000)
    } else {
        (30, 1)
    }
}
