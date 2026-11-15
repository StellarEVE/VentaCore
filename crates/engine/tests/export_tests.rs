//! Tests for Stage 9 full timeline export and atomic finalization.

use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use ventacore_audio::AudioMixTrack;
use ventacore_core::{Asset, AssetId, Item, ItemId, Project, Tick, Track, TrackId, TrackKind};
use ventacore_engine::export::{export_timeline, ExportConfig};
use ventacore_engine::pipeline::CancellationToken;
use ventacore_platform::encoder::SyntheticVideoEncoder;
use ventacore_platform::traits::VideoDecoder;
use ventacore_platform::types::{ColorDepth, HwAccel, Rotation, VideoCodec, VideoEncoderConfig};
use ventacore_render::{FitMode, GpuCompositor};

#[test]
fn test_export_timeline_atomic_finalization() {
    let temp_dir = std::env::temp_dir().join("ventacore_test_export");
    let _ = fs::create_dir_all(&temp_dir);
    let output_file = temp_dir.join("test_output.mp4");
    let part_file = temp_dir.join("test_output.mp4.part");

    // Clean up if existed
    let _ = fs::remove_file(&output_file);
    let _ = fs::remove_file(&part_file);

    // Setup Project: 1 video track with 2 items (total 3s @ 30fps = 90 frames)
    let mut project = Project::new("Export Test Project");
    let track_id = TrackId(1);
    project.tracks.push(Track::new(track_id, "Video 1", TrackKind::Video));

    let asset1_id = AssetId(101);
    let mut asset1 = Asset::new(asset1_id, "video1.mp4".into());
    asset1.fingerprint = Some("fp_video1".into());
    project.assets.push(asset1);

    // Item 1: 0s..1.5s
    project.items.push(Item::new(
        ItemId(1),
        track_id,
        asset1_id,
        Tick::ZERO,
        Tick::from_seconds_f64(1.5),
        Tick::ZERO,
    ));

    // Item 2: 1.5s..3.0s
    project.items.push(Item::new(
        ItemId(2),
        track_id,
        asset1_id,
        Tick::from_seconds_f64(1.5),
        Tick::from_seconds_f64(1.5),
        Tick::from_seconds(2),
    ));

    let config = ExportConfig {
        output_path: output_file.clone(),
        width: 320,
        height: 180,
        fps_num: 30,
        fps_den: 1,
        fit_mode: FitMode::Crop,
        sample_rate: 48000,
        master_volume: 1.0,
    };

    let encoder_cfg = VideoEncoderConfig {
        width: 320,
        height: 180,
        fps_num: 30,
        fps_den: 1,
        bitrate_bps: 1_000_000,
        codec: VideoCodec::H264,
        hardware_accel: HwAccel::None,
        quality_preset: "medium".into(),
    };

    // Encoder writes to .part path
    let encoder = Box::new(SyntheticVideoEncoder::create(&part_file, encoder_cfg).unwrap());
    let mut compositor = GpuCompositor::new_cpu_only();

    // Decoder for asset1
    let decoder = Box::new(
        ventacore_platform::decoder::SyntheticVideoDecoder::new(
            Tick::from_seconds(10),
            320,
            180,
            30,
            1,
            Rotation::None,
            ColorDepth::Bits8,
        ),
    );

    let mut decoders: HashMap<AssetId, Box<dyn VideoDecoder>> = HashMap::new();
    decoders.insert(asset1_id, decoder);

    let audio_track = AudioMixTrack::new(
        Tick::ZERO,
        Tick::from_seconds(3),
        vec![0.3f32; 48000 * 2 * 3],
    );

    let cancel = CancellationToken::new();
    let progress_calls = Arc::new(AtomicUsize::new(0));
    let progress_counter = progress_calls.clone();

    let on_progress = move |_p: ventacore_engine::export::ExportProgress| {
        progress_counter.fetch_add(1, Ordering::SeqCst);
    };

    let summary = export_timeline(
        &project,
        &config,
        &mut compositor,
        None,
        &mut decoders,
        &[audio_track],
        encoder,
        &cancel,
        Some(&on_progress),
    )
    .expect("Export must succeed");

    assert_eq!(summary.frames_encoded, 90);
    assert_eq!(progress_calls.load(Ordering::SeqCst), 90);

    // Verify Charter Rule 9: .part file has been atomically renamed to target file!
    assert!(!part_file.exists(), ".part file must not remain after successful export");
    assert!(output_file.exists(), "Target output file must exist");

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_export_timeline_cancellation_cleans_up_partial_file() {
    let temp_dir = std::env::temp_dir().join("ventacore_test_export_cancel");
    let _ = fs::create_dir_all(&temp_dir);
    let output_file = temp_dir.join("test_cancel.mp4");
    let part_file = temp_dir.join("test_cancel.mp4.part");

    let mut project = Project::new("Export Cancel Project");
    let track_id = TrackId(1);
    project.tracks.push(Track::new(track_id, "Video 1", TrackKind::Video));
    let asset1_id = AssetId(101);
    project.items.push(Item::new(
        ItemId(1),
        track_id,
        asset1_id,
        Tick::ZERO,
        Tick::from_seconds(10), // 300 frames
        Tick::ZERO,
    ));

    let config = ExportConfig::new(&output_file, 320, 180);
    let encoder_cfg = VideoEncoderConfig {
        width: 320,
        height: 180,
        fps_num: 30,
        fps_den: 1,
        bitrate_bps: 1_000_000,
        codec: VideoCodec::H264,
        hardware_accel: HwAccel::None,
        quality_preset: "medium".into(),
    };

    let encoder = Box::new(SyntheticVideoEncoder::create(&part_file, encoder_cfg).unwrap());
    let mut compositor = GpuCompositor::new_cpu_only();
    let mut decoders: HashMap<AssetId, Box<dyn VideoDecoder>> = HashMap::new();

    let cancel = CancellationToken::new();
    // Cancel immediately before export
    cancel.cancel();

    let result = export_timeline(
        &project,
        &config,
        &mut compositor,
        None,
        &mut decoders,
        &[],
        encoder,
        &cancel,
        None,
    );

    assert!(result.is_err());
    // Partial file must be deleted (Charter Rule 9: no corrupt partial exports)
    assert!(!part_file.exists(), "Partial .part file must be cleaned up on cancellation");
    assert!(!output_file.exists(), "Output file must not exist if cancelled");

    let _ = fs::remove_dir_all(temp_dir);
}
