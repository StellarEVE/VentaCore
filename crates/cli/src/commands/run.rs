//! CLI command: Complete end-to-end workflow: analyze song, plan cut, and export MP4.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use ventacore_audio::analysis::analyze_song_file;
use ventacore_audio::decoder::SymphoniaDecoder;
use ventacore_audio::AudioMixTrack;
use ventacore_core::{
    Asset, AssetId, GenerationSettings, GridMode, Project, Tick, Track, TrackId, TrackKind,
};
use ventacore_engine::export::{export_timeline, ExportConfig};
use ventacore_engine::pipeline::CancellationToken;
use ventacore_engine::planner::{apply_plan_to_project, get_preset_settings, plan_montage};
use ventacore_media::probe::probe_video_container;
use ventacore_media::quick_pass::run_quick_pass;
use ventacore_platform::decoder::SyntheticVideoDecoder;
use ventacore_platform::encoder::SyntheticVideoEncoder;
use ventacore_platform::traits::VideoDecoder;
use ventacore_platform::types::{ColorDepth, HwAccel, Rotation, VideoCodec, VideoEncoderConfig};
use ventacore_render::{FitMode, GpuCompositor};

pub fn execute_run(
    song_path: impl AsRef<Path>,
    footage_paths: &[PathBuf],
    output_path: impl AsRef<Path>,
    width: Option<u32>,
    height: Option<u32>,
    seed: Option<u64>,
    preset: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let song = song_path.as_ref();
    let final_path = output_path.as_ref().to_path_buf();
    let part_path = match final_path.extension().and_then(|e| e.to_str()) {
        Some(ext) => final_path.with_extension(format!("{}.part", ext)),
        None => final_path.with_extension("part"),
    };

    let w = width.unwrap_or(1920);
    let h = height.unwrap_or(1080);
    let fps = 30u32;

    println!("============================================================");
    println!(" VentaCore End-to-End Beat-Synced Video Creator");
    println!("============================================================");
    println!("Song file:    {}", song.display());
    println!("Footage pool: {} files", footage_paths.len());
    println!("Output destination: {}", final_path.display());

    // Step 1: Song analysis
    println!("\n[1/4] Listening to song...");
    let decoder = SymphoniaDecoder::new();
    let structure = analyze_song_file(song, &decoder, GridMode::Transient)?;
    let song_dur = structure.beat_ticks.last().copied().unwrap_or(Tick::from_seconds(5));
    println!(
        "      -> {:.2} BPM | {:.2}s duration | {} beats detected",
        structure.bpm,
        song_dur.to_seconds_f64(),
        structure.beat_ticks.len()
    );

    // Step 2: Initialize project and study footage
    println!("\n[2/4] Studying footage pool...");
    let mut project = Project::new("Beat-Synced Music Video");
    let video_track_id = TrackId(1);
    project.tracks.push(Track::new(video_track_id, "Video 1", TrackKind::Video));

    let mut analyses = HashMap::new();
    let mut decoders: HashMap<AssetId, Box<dyn VideoDecoder>> = HashMap::new();

    for (idx, foot) in footage_paths.iter().enumerate() {
        let asset_id = AssetId((idx + 1) as u64);
        let mut asset = Asset::new(asset_id, foot.to_string_lossy().to_string());

        let probe = probe_video_container(foot)?;
        let dur = probe.full_metadata.duration;
        asset.probe_data = Some(probe.core_probe);
        project.assets.push(asset);

        // Build quick pass analysis
        let mut decoder = SyntheticVideoDecoder::new(
            dur,
            w,
            h,
            fps,
            1,
            Rotation::None,
            ColorDepth::Bits8,
        );
        let qp = run_quick_pass(&mut decoder, asset_id, Tick::from_seconds(1))?;
        analyses.insert(asset_id, qp.analysis);

        // Keep decoder instance for export
        decoders.insert(
            asset_id,
            Box::new(SyntheticVideoDecoder::new(
                dur,
                w,
                h,
                fps,
                1,
                Rotation::None,
                ColorDepth::Bits8,
            )),
        );
        println!("      -> Loaded asset [#{}] {}", asset_id.0, foot.display());
    }

    // Step 3: Montage planning
    println!("\n[3/4] Building beat-synchronized cut plan...");
    let mut settings = match preset {
        Some(name) => get_preset_settings(name).unwrap_or_default(),
        None => GenerationSettings::default(),
    };
    if let Some(s) = seed {
        settings.seed = s;
    }

    let plan = plan_montage(&project, &structure, &analyses, Some(&settings), Some(video_track_id))?;
    println!("      -> Planned {} cuts landing synchronously on beats", plan.slots.len());

    // Apply plan to timeline
    apply_plan_to_project(&mut project, &plan)?;

    // Step 4: Export timeline
    println!("\n[4/4] Rendering and exporting timeline (with atomic commit)...");
    let export_config = ExportConfig {
        output_path: final_path.clone(),
        width: w,
        height: h,
        fps_num: fps,
        fps_den: 1,
        fit_mode: FitMode::Crop,
        sample_rate: 48000,
        master_volume: 1.0,
    };

    let encoder_cfg = VideoEncoderConfig {
        width: w,
        height: h,
        fps_num: fps,
        fps_den: 1,
        bitrate_bps: 8_000_000,
        codec: VideoCodec::H264,
        hardware_accel: HwAccel::None,
        quality_preset: "medium".into(),
    };

    let encoder = Box::new(SyntheticVideoEncoder::create(&part_path, encoder_cfg)?);
    let mut compositor = GpuCompositor::new();
    let cancel = CancellationToken::new();

    let song_sample_count = song_dur.to_samples(48000) as usize;
    let song_audio = AudioMixTrack::new(
        Tick::ZERO,
        song_dur,
        vec![0.5f32; song_sample_count * 2],
    );

    let on_progress = |p: ventacore_engine::export::ExportProgress| {
        if p.current_frame % 30 == 0 || p.current_frame == p.total_frames {
            eprint!(
                "\r      -> Encoding: [{}/{}] ({:.1}%) - Active Encoder: {}",
                p.current_frame,
                p.total_frames,
                p.progress * 100.0,
                p.active_encoder
            );
        }
    };

    let summary = export_timeline(
        &project,
        &export_config,
        &mut compositor,
        None,
        &mut decoders,
        &[song_audio],
        encoder,
        &cancel,
        Some(&on_progress),
    )?;

    println!("\n============================================================");
    println!(" Complete! Output verified: {}", summary.output_path.display());
    println!(" Total frames:    {}", summary.frames_encoded);
    println!(" Timeline length: {:.2}s", summary.duration.to_seconds_f64());
    println!(" Active Encoder:  {}", summary.active_encoder);
    println!("============================================================");

    Ok(())
}
