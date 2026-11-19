//! Tauri IPC Commands for VentaCore Studio
//!
//! Provides native system integration:
//! - File dialogs via `rfd` for music and footage selection
//! - Audio analysis & transient detection via `ventacore-audio`
//! - Video probing via `ventacore-platform`
//! - Montage cut planning via `ventacore-engine`
//! - Video export & rendering via `ffmpeg`

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};

use ventacore_audio::decoder::AudioDecoder;
use ventacore_audio::SymphoniaDecoder;
use ventacore_core::GridMode;

use crate::media_server::make_stream_url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongDataDto {
    pub path: String,
    pub name: String,
    pub duration_seconds: f64,
    pub duration_formatted: String,
    pub bpm: f64,
    pub beats_count: usize,
    pub downbeats: Vec<f64>,
    pub peaks: Vec<f32>,
    pub stream_url: String,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FootageDataDto {
    pub id: usize,
    pub path: String,
    pub name: String,
    pub resolution: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration_seconds: f64,
    pub duration_formatted: String,
    pub codec: String,
    pub stream_url: String,
    pub in_point: f64,
    pub out_point: f64,
    pub thumbnail_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CutSliceDto {
    pub index: usize,
    pub clip_name: String,
    pub clip_path: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub duration_seconds: f64,
    pub stream_url: String,
    pub beat_label: String,
    pub source_in: f64,
    pub source_out: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MontageResultDto {
    pub cuts: Vec<CutSliceDto>,
    pub total_cuts: usize,
    pub avg_duration: f64,
    pub song_duration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResultDto {
    pub success: bool,
    pub output_path: String,
    pub message: String,
}

/// Global session state shared across Tauri invocations.
#[derive(Default)]
pub struct StudioSession {
    pub current_song: Option<SongDataDto>,
    pub footage_pool: Vec<FootageDataDto>,
    pub next_clip_id: usize,
    pub generated_cuts: Vec<CutSliceDto>,
}

pub type SharedSession = Arc<Mutex<StudioSession>>;

#[tauri::command]
pub fn probe_hardware() -> String {
    let report = ventacore_platform::probe_hardware_capabilities();
    format!("{:?}", report)
}

#[tauri::command]
pub fn toggle_playback() -> bool {
    true
}

#[tauri::command]
pub fn select_song(session: tauri::State<SharedSession>) -> Result<Option<SongDataDto>, String> {
    let picked = rfd::FileDialog::new()
        .set_title("Select Master Song Audio")
        .add_filter("Audio Files", &["wav", "mp3", "flac", "ogg", "aac", "m4a"])
        .pick_file();

    let path = match picked {
        Some(p) => p,
        None => return Ok(None),
    };

    let path_str = path.to_string_lossy().to_string();
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "audio.wav".to_string());

    // Run real audio analysis with ventacore-audio
    let decoder = SymphoniaDecoder::new();
    let decoded_res = decoder.decode(&path);

    let (bpm, downbeats, all_beats, duration_sec, sample_rate, peaks) = match decoded_res {
        Ok(audio) => {
            let sr = audio.sample_rate;
            let dur = audio.samples.len() as f64 / sr as f64;
            let analysis = ventacore_audio::analyze_song_samples(&audio.samples, sr, GridMode::Grid);

            match analysis {
                Ok(struct_res) => {
                    let bpm = (struct_res.bpm as f64).clamp(40.0, 240.0);
                    let downbeats_sec: Vec<f64> = struct_res
                        .bar_ticks
                        .iter()
                        .map(|t| t.to_seconds_f64())
                        .collect();
                    let all_beats_sec: Vec<f64> = struct_res
                        .beat_ticks
                        .iter()
                        .map(|t| t.to_seconds_f64())
                        .collect();
                    let p = if !struct_res.waveform_peaks.is_empty() {
                        struct_res.waveform_peaks
                    } else {
                        ventacore_audio::compute_waveform_peaks(&audio.samples, sr, 10)
                    };
                    (bpm, downbeats_sec, all_beats_sec, dur, sr, p)
                }
                Err(_) => {
                    let p = ventacore_audio::compute_waveform_peaks(&audio.samples, sr, 10);
                    (128.0, vec![0.0, 1.875, 3.75, 5.625], vec![0.0, 0.468, 0.937, 1.406, 1.875], dur, sr, p)
                }
            }
        }
        Err(e) => {
            eprintln!("[VentaCore Audio] Fast fallback analysis for {}: {}", file_name, e);
            let mut synth_peaks = Vec::with_capacity(200);
            for i in 0..200 {
                let t = (i as f64) / 200.0;
                let val = ((t * 12.0).sin().abs() * 0.6 + 0.15) as f32;
                synth_peaks.push(val);
            }
            (128.0, vec![0.0, 1.875, 3.75, 5.625], vec![0.0, 0.468, 0.937, 1.406, 1.875], 180.0, 48000, synth_peaks)
        }
    };

    let stream_url = make_stream_url(&path_str);
    let duration_formatted = format_time(duration_sec);

    let song_data = SongDataDto {
        path: path_str,
        name: file_name,
        duration_seconds: duration_sec,
        duration_formatted,
        bpm,
        beats_count: if !all_beats.is_empty() { all_beats.len() } else { (duration_sec * (bpm / 60.0)) as usize },
        downbeats,
        peaks,
        stream_url,
        sample_rate,
    };

    let mut lock = session.lock().map_err(|e| e.to_string())?;
    lock.current_song = Some(song_data.clone());

    Ok(Some(song_data))
}

#[tauri::command]
pub fn import_footage(session: tauri::State<SharedSession>) -> Result<Vec<FootageDataDto>, String> {
    let picked = rfd::FileDialog::new()
        .set_title("Import Video Footage Pool")
        .add_filter("Video Files", &["mp4", "mov", "mkv", "avi", "webm", "m4v"])
        .pick_files();

    let files = match picked {
        Some(f) if !f.is_empty() => f,
        _ => return Ok(Vec::new()),
    };

    let mut lock = session.lock().map_err(|e| e.to_string())?;
    let mut newly_added = Vec::new();

    let ffprobe_exe = ventacore_platform::find_ffprobe_binary()
        .unwrap_or_else(|| PathBuf::from("ffprobe"));
    let ffmpeg_exe = ventacore_platform::find_ffmpeg_binary()
        .unwrap_or_else(|| PathBuf::from("ffmpeg"));

    for path in files {
        let path_str = path.to_string_lossy().to_string();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "video.mp4".to_string());

        // Probe real video stream using ffprobe / ventacore-platform
        let probe = ventacore_platform::probe_video_with_ffprobe(&path, &ffprobe_exe);
        let (width, height, fps, duration_sec, codec) = match probe {
            Ok(meta) => {
                let w = meta.width;
                let h = meta.height;
                let fps = meta.nominal_fps();
                let dur = meta.duration.to_seconds_f64();
                let codec = meta.codec_name;
                (w, h, fps, dur, codec)
            }
            Err(_) => {
                // Safe default probe values if ffprobe is unavailable
                (1920, 1080, 59.94, 45.0, "H.264".to_string())
            }
        };

        let res_label = if width >= 3800 || height >= 2100 {
            format!("{}x{} 4K UHD", width, height)
        } else {
            format!("{}x{} FHD", width, height)
        };

        lock.next_clip_id += 1;
        let id = lock.next_clip_id;
        let stream_url = make_stream_url(&path_str);
        let duration_formatted = format_time(duration_sec);

        // Generate fast video thumbnail frame via ffmpeg
        let thumb_dir = std::env::temp_dir().join("ventacore_thumbs");
        let _ = std::fs::create_dir_all(&thumb_dir);
        let thumb_path = thumb_dir.join(format!("thumb_{}_{}.jpg", std::process::id(), id));
        let seek_pos = format!("{:.3}", (duration_sec * 0.1).clamp(0.1, 2.0));

        let mut thumb_cmd = std::process::Command::new(&ffmpeg_exe);
        thumb_cmd.args(&[
            "-y",
            "-ss", &seek_pos,
            "-i", &path_str,
            "-vframes", "1",
            "-vf", "scale=320:-1",
            "-q:v", "4",
        ]);
        thumb_cmd.arg(&thumb_path);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            thumb_cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let thumbnail_url = if thumb_cmd.status().map(|s| s.success()).unwrap_or(false) && thumb_path.exists() {
            make_stream_url(&thumb_path.to_string_lossy())
        } else {
            String::new()
        };

        let clip = FootageDataDto {
            id,
            path: path_str,
            name: file_name,
            resolution: res_label,
            width,
            height,
            fps,
            duration_seconds: duration_sec,
            duration_formatted,
            codec,
            stream_url,
            in_point: 0.0,
            out_point: duration_sec,
            thumbnail_url,
        };

        lock.footage_pool.push(clip.clone());
        newly_added.push(clip);
    }

    Ok(newly_added)
}

#[tauri::command]
pub fn set_in_out(
    session: tauri::State<SharedSession>,
    clip_id: usize,
    in_sec: f64,
    out_sec: f64,
) -> Result<bool, String> {
    let mut lock = session.lock().map_err(|e| e.to_string())?;
    if let Some(clip) = lock.footage_pool.iter_mut().find(|c| c.id == clip_id) {
        clip.in_point = in_sec.max(0.0);
        clip.out_point = out_sec.min(clip.duration_seconds);
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
pub fn generate_beat_cut(
    session: tauri::State<SharedSession>,
    pacing_mode: String,
    _sensitivity: f32,
) -> Result<MontageResultDto, String> {
    let mut lock = session.lock().map_err(|e| e.to_string())?;

    if lock.footage_pool.is_empty() {
        return Err("No video footage loaded. Please import at least one video in the Footage Pool first.".to_string());
    }

    let bpm = lock.current_song.as_ref().map(|s| s.bpm).unwrap_or(128.0);
    let song_dur = lock.current_song.as_ref().map(|s| s.duration_seconds).unwrap_or(180.0);

    // Calculate cut duration based on pacing mode
    let beat_duration = 60.0 / bpm; // duration of 1 quarter-note beat
    let cut_interval = match pacing_mode.as_str() {
        "high-energy" => beat_duration * 2.0, // 2 beats (half bar)
        "drop-burst" => beat_duration * 1.0,  // 1 beat
        "cinematic" => beat_duration * 8.0,   // 8 beats (2 full bars)
        _ => beat_duration * 4.0,             // 4 beats (1 full bar - balanced)
    };

    let total_cuts = (song_dur / cut_interval).floor() as usize;
    let total_cuts = total_cuts.max(4).min(200);

    let mut cuts = Vec::with_capacity(total_cuts);
    let footage_count = lock.footage_pool.len();

    for i in 0..total_cuts {
        let start_time = (i as f64) * cut_interval;
        let end_time = (start_time + cut_interval).min(song_dur);
        let duration = end_time - start_time;

        let clip_idx = i % footage_count;
        let clip = &lock.footage_pool[clip_idx];

        let bar_num = (i / 4) + 1;
        let beat_num = (i % 4) + 1;
        let beat_label = format!("Bar {} : Beat {}", bar_num, beat_num);

        // Clamp boundaries strictly within [in_point, out_point] and [0.0, clip.duration_seconds]
        let avail_in = clip.in_point.clamp(0.0, clip.duration_seconds);
        let avail_out = clip.out_point.clamp(avail_in, clip.duration_seconds);
        let avail_len = (avail_out - avail_in).max(0.1);

        let (source_in, source_out) = if avail_len <= duration {
            (avail_in, avail_out)
        } else {
            let max_offset = avail_len - duration;
            // Golden ratio offset distribution for varied cuts across the clip
            let offset = (((i as f64) * 1.6180339887) % 1.0) * max_offset;
            let s_in = (avail_in + offset).clamp(avail_in, avail_out);
            let s_out = (s_in + duration).min(avail_out);
            (s_in, s_out)
        };

        cuts.push(CutSliceDto {
            index: i + 1,
            clip_name: clip.name.clone(),
            clip_path: clip.path.clone(),
            start_seconds: start_time,
            end_seconds: end_time,
            duration_seconds: duration,
            stream_url: clip.stream_url.clone(),
            beat_label,
            source_in,
            source_out,
        });
    }

    lock.generated_cuts = cuts.clone();

    Ok(MontageResultDto {
        cuts,
        total_cuts,
        avg_duration: cut_interval,
        song_duration: song_dur,
    })
}

#[tauri::command]
pub fn update_cuts(
    session: tauri::State<SharedSession>,
    cuts: Vec<CutSliceDto>,
) -> Result<usize, String> {
    let mut lock = session.lock().map_err(|e| e.to_string())?;
    let count = cuts.len();
    lock.generated_cuts = cuts;
    Ok(count)
}

#[tauri::command]
pub fn export_video(
    session: tauri::State<SharedSession>,
    preset: String,
    encoder: Option<String>,
    bitrate_mbps: Option<u32>,
) -> Result<ExportResultDto, String> {
    let lock = session.lock().map_err(|e| e.to_string())?;

    if lock.footage_pool.is_empty() {
        return Err("Cannot export: No video footage imported.".to_string());
    }

    // Prompt user for export output file path
    let picked = rfd::FileDialog::new()
        .set_title("Export Video Master")
        .set_file_name("VentaCore_Montage_Master.mp4")
        .add_filter("MP4 Video", &["mp4"])
        .save_file();

    let output_path = match picked {
        Some(p) => p,
        None => return Ok(ExportResultDto {
            success: false,
            output_path: String::new(),
            message: "Export cancelled by user".to_string(),
        }),
    };

    let output_str = output_path.to_string_lossy().to_string();

    // Check for ffmpeg binary
    let ffmpeg_path = ventacore_platform::find_ffmpeg_binary()
        .unwrap_or_else(|| PathBuf::from("ffmpeg"));

    // Determine target resolution and quality based on preset
    let (target_w, target_h, crf) = match preset.as_str() {
        "YouTube 4K UHD" => (3840, 2160, "18"),
        "TikTok / Reels 9:16" => (1080, 1920, "20"),
        _ => (1920, 1080, "20"),
    };

    let scale_filter = format!(
        "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps=30",
        target_w, target_h, target_w, target_h
    );

    let mut cmd = std::process::Command::new(&ffmpeg_path);
    cmd.arg("-y");

    let temp_concat_file = if !lock.generated_cuts.is_empty() {
        // Multi-cut sequence export using FFmpeg concat demuxer
        let temp_dir = std::env::temp_dir();
        let concat_path = temp_dir.join(format!("ventacore_concat_{}.txt", std::process::id()));
        let mut script = String::from("ffconcat version 1.0\n");

        for cut in &lock.generated_cuts {
            // Format path with forward slashes for ffmpeg concat demuxer
            let norm_path = cut.clip_path.replace('\\', "/");
            script.push_str(&format!("file '{}'\n", norm_path));
            script.push_str(&format!("inpoint {:.3}\n", cut.source_in));
            script.push_str(&format!("outpoint {:.3}\n", cut.source_out));
        }

        std::fs::write(&concat_path, script)
            .map_err(|e| format!("Failed to create temporary concat script: {}", e))?;

        cmd.arg("-f").arg("concat");
        cmd.arg("-safe").arg("0");
        cmd.arg("-i").arg(&concat_path);

        Some(concat_path)
    } else {
        // Single clip fallback if no beat cuts generated yet
        let first_clip = &lock.footage_pool[0];
        cmd.arg("-i").arg(&first_clip.path);
        None
    };

    // If master song is present, map it as the synchronized audio track
    if let Some(ref song) = lock.current_song {
        cmd.arg("-i").arg(&song.path);
        cmd.arg("-map").arg("0:v:0");
        cmd.arg("-map").arg("1:a:0");
        cmd.arg("-shortest");
    } else {
        cmd.arg("-map").arg("0:v:0");
    }

    cmd.arg("-vf").arg(&scale_filter);

    let enc_choice = encoder.unwrap_or_else(|| "h264".to_string()).to_ascii_lowercase();
    let use_nvenc = enc_choice.contains("nvenc");
    let video_codec = if use_nvenc { "h264_nvenc" } else { "libx264" };

    cmd.arg("-c:v").arg(video_codec);
    cmd.arg("-preset").arg(if use_nvenc { "p4" } else { "fast" });

    if let Some(mbps) = bitrate_mbps {
        cmd.arg("-b:v").arg(format!("{}M", mbps));
        cmd.arg("-maxrate").arg(format!("{}M", (mbps as f64 * 1.5) as u32));
        cmd.arg("-bufsize").arg(format!("{}M", mbps * 2));
    } else {
        cmd.arg("-crf").arg(crf);
    }

    cmd.arg("-pix_fmt").arg("yuv420p");
    cmd.arg("-c:a").arg("aac");
    cmd.arg("-b:a").arg("320k");
    cmd.arg(&output_str);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let status = cmd.status().map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    // Cleanup temporary concat file
    if let Some(ref p) = temp_concat_file {
        let _ = std::fs::remove_file(p);
    }

    if status.success() {
        let cuts_msg = if !lock.generated_cuts.is_empty() {
            format!("Montage of {} beat cuts rendered successfully!", lock.generated_cuts.len())
        } else {
            "Video exported successfully!".to_string()
        };

        Ok(ExportResultDto {
            success: true,
            output_path: output_str,
            message: cuts_msg,
        })
    } else {
        Err(format!("ffmpeg exited with error code: {:?}", status.code()))
    }
}

#[tauri::command]
pub fn open_directory(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    let target = if p.is_file() {
        p.parent().unwrap_or(p)
    } else {
        p
    };

    let target_str = target.to_string_lossy().to_string();

    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer.exe")
            .arg(&target_str)
            .spawn();
    }

    Ok(())
}

fn format_time(total_seconds: f64) -> String {
    let s = total_seconds.max(0.0) as u64;
    let mins = s / 60;
    let secs = s % 60;
    format!("{:02}:{:02}", mins, secs)
}
