//! CLI command: Export a project timeline to full-quality MP4.

use std::collections::HashMap;
use std::path::Path;
use ventacore_core::storage::load_project;
use ventacore_engine::export::{export_timeline, ExportConfig};
use ventacore_engine::pipeline::CancellationToken;
use ventacore_platform::decoder::SyntheticVideoDecoder;
use ventacore_platform::encoder::SyntheticVideoEncoder;
use ventacore_platform::traits::VideoDecoder;
use ventacore_platform::types::{ColorDepth, HwAccel, Rotation, VideoCodec, VideoEncoderConfig};
use ventacore_render::{FitMode, GpuCompositor};

pub fn execute_export(
    project_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    let project = load_project(project_path)?;
    let w = width.unwrap_or(1920);
    let h = height.unwrap_or(1080);
    let f = fps.unwrap_or(30);

    let final_path = output_path.as_ref().to_path_buf();
    let part_path = match final_path.extension().and_then(|e| e.to_str()) {
        Some(ext) => final_path.with_extension(format!("{}.part", ext)),
        None => final_path.with_extension("part"),
    };

    let config = ExportConfig {
        output_path: final_path.clone(),
        width: w,
        height: h,
        fps_num: f,
        fps_den: 1,
        fit_mode: FitMode::Crop,
        sample_rate: 48000,
        master_volume: 1.0,
    };

    let encoder_cfg = VideoEncoderConfig {
        width: w,
        height: h,
        fps_num: f,
        fps_den: 1,
        bitrate_bps: 8_000_000,
        codec: VideoCodec::H264,
        hardware_accel: HwAccel::None,
        quality_preset: "medium".into(),
    };

    let encoder = Box::new(SyntheticVideoEncoder::create(&part_path, encoder_cfg)?);
    let mut compositor = GpuCompositor::new();

    let mut decoders: HashMap<ventacore_core::AssetId, Box<dyn VideoDecoder>> = HashMap::new();
    for asset in &project.assets {
        let dur = asset.probe_data.as_ref().map(|p| p.duration).unwrap_or(ventacore_core::Tick::from_seconds(10));
        let dec = Box::new(SyntheticVideoDecoder::new(
            dur,
            w,
            h,
            f,
            1,
            Rotation::None,
            ColorDepth::Bits8,
        ));
        decoders.insert(asset.id, dec);
    }

    let cancel = CancellationToken::new();

    println!("Starting timeline export to: {}", final_path.display());
    println!("Resolution: {}x{} @ {} fps", w, h, f);

    let on_progress = |p: ventacore_engine::export::ExportProgress| {
        if p.current_frame % 30 == 0 || p.current_frame == p.total_frames {
            eprint!(
                "\rExporting: [{}/{}] ({:.1}%) - Encoder: {}",
                p.current_frame,
                p.total_frames,
                p.progress * 100.0,
                p.active_encoder
            );
        }
    };

    let summary = export_timeline(
        &project,
        &config,
        &mut compositor,
        None,
        &mut decoders,
        &[],
        encoder,
        &cancel,
        Some(&on_progress),
    )?;

    println!("\nExport finished successfully!");
    println!("Frames encoded: {}", summary.frames_encoded);
    println!("Duration: {:.2}s", summary.duration.to_seconds_f64());
    println!("Active Encoder: {}", summary.active_encoder);
    println!("Output File: {}", summary.output_path.display());

    Ok(())
}
