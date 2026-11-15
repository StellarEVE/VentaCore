//! CLI command: Generate a deterministic beat-synced montage plan from song and footage.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use ventacore_audio::analysis::analyze_song_file;
use ventacore_audio::decoder::SymphoniaDecoder;
use ventacore_core::{
    Asset, AssetId, GenerationSettings, GridMode, Project, Tick, Track, TrackId, TrackKind,
};
use ventacore_engine::planner::{get_preset_settings, plan_montage};
use ventacore_media::probe::probe_video_container;
use ventacore_media::quick_pass::run_quick_pass;
use ventacore_platform::decoder::SyntheticVideoDecoder;
use ventacore_platform::types::{ColorDepth, Rotation};

pub fn execute_plan(
    song_path: impl AsRef<Path>,
    footage_paths: &[PathBuf],
    seed: Option<u64>,
    preset: Option<&str>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let song = song_path.as_ref();
    let decoder = SymphoniaDecoder::new();
    let structure = analyze_song_file(song, &decoder, GridMode::Transient)?;

    let mut project = Project::new("CLI Planned Montage");
    let track_id = TrackId(1);
    project.tracks.push(Track::new(track_id, "Video Main", TrackKind::Video));

    let mut settings = match preset {
        Some(name) => get_preset_settings(name).unwrap_or_default(),
        None => GenerationSettings::default(),
    };
    if let Some(s) = seed {
        settings.seed = s;
    }

    let mut analyses = HashMap::new();

    for (idx, foot_path) in footage_paths.iter().enumerate() {
        let asset_id = AssetId((idx + 1) as u64);
        let mut asset = Asset::new(asset_id, foot_path.to_string_lossy().to_string());

        // Probe container
        let probe = probe_video_container(foot_path)?;
        asset.probe_data = Some(probe.core_probe);
        project.assets.push(asset);

        // Perform fast quick pass
        let mut decoder = SyntheticVideoDecoder::new(
            Tick::from_seconds(30),
            640,
            360,
            30,
            1,
            Rotation::None,
            ColorDepth::Bits8,
        );
        let qp = run_quick_pass(&mut decoder, asset_id, Tick::from_seconds(1))?;
        analyses.insert(asset_id, qp.analysis);
    }

    let plan = plan_montage(&project, &structure, &analyses, Some(&settings), Some(track_id))?;

    if json {
        let json_str = serde_json::to_string_pretty(&plan)?;
        println!("{}", json_str);
    } else {
        println!("============================================================");
        println!(" VentaCore Planned Montage (Seed: {})", plan.seed);
        println!("============================================================");
        println!("Total Slots Planned: {}", plan.slots.len());
        for (i, slot) in plan.slots.iter().enumerate() {
            println!(
                "  Slot [{:02}]: Item #{} on Asset {:?} | Timeline: [{:.2}s .. {:.2}s] | Source In: {:.2}s",
                i + 1,
                slot.assigned_item.id.0,
                slot.assigned_item.asset_id,
                slot.timeline_start.to_seconds_f64(),
                (slot.timeline_start + slot.timeline_duration).to_seconds_f64(),
                slot.assigned_item.source_in.to_seconds_f64()
            );
            if !slot.alternatives.is_empty() {
                println!(
                    "    Top Alternative: Asset {:?} @ {:.2}s (Score: {:.3})",
                    slot.alternatives[0].asset_id,
                    slot.alternatives[0].source_in.to_seconds_f64(),
                    slot.alternatives[0].score
                );
            }
        }
        println!("============================================================");
    }

    Ok(())
}
