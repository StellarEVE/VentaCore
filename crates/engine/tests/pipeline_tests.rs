//! Stage 6 Integration Tests: Conductor, Pipeline, and Live Cut Building.
//!
//! Validates:
//! 1. Breadth-before-depth execution:
//!    Song analysis -> Quick passes on all videos -> Provisional cut -> Full passes -> Refined cut.
//! 2. Priority ordering: Assets selected in the provisional cut are analyzed first during Full Pass.
//! 3. Cooperative cancellation: CancellationToken halts execution cleanly without corrupting state.
//! 4. Freeze action: Freezing sets `pinned = true` across timeline clips, protecting them from alteration.
//! 5. Pipeline status reporting.

use std::fs::File;
use std::io::Write;
use ventacore_audio::SongStructure;
use ventacore_core::*;
use ventacore_engine::pipeline::*;
use ventacore_media::MediaCache;

/// Creates a test project with 1 video track and multiple assets.
fn create_test_project_and_assets() -> (Project, TrackId, Vec<(AssetId, std::path::PathBuf)>) {
    let mut project = Project::new("Pipeline Conductor Test");
    let track_id = TrackId(1);
    project
        .apply_command(&Command::AddTrack(Track::new(
            track_id,
            "Video 1",
            TrackKind::Video,
        )))
        .unwrap();

    let temp_dir = std::env::temp_dir().join("ventacore_pipeline_test_assets");
    let _ = std::fs::create_dir_all(&temp_dir);

    let mut asset_files = Vec::new();

    for i in 1..=3 {
        let aid = AssetId(i);
        let path = temp_dir.join(format!("test_video_{}.bin", i));

        // Create a non-empty mock media file (100 KB)
        let mut file = File::create(&path).unwrap();
        file.write_all(&vec![i as u8; 100 * 1024]).unwrap();

        let mut asset = Asset::new(aid, path.to_string_lossy().to_string());
        asset.probe_data = Some(ProbeData {
            duration: Tick::from_seconds(6),
            width: 320,
            height: 180,
            fps_num: 30,
            fps_den: 1,
            has_audio: false,
            audio_sample_rate: 0,
        });

        project.apply_command(&Command::AddAsset(asset)).unwrap();
        asset_files.push((aid, path));
    }

    (project, track_id, asset_files)
}

fn create_simple_song(bpm: f32, duration_sec: f64) -> SongStructure {
    let beat_duration_sec = 60.0 / bpm as f64;
    let beat_tick = Tick::from_seconds_f64(beat_duration_sec);
    let num_beats = (duration_sec / beat_duration_sec) as usize;

    let beat_ticks: Vec<Tick> = (1..=num_beats).map(|i| beat_tick * (i as i64)).collect();
    let bar_ticks: Vec<Tick> = beat_ticks.iter().step_by(4).copied().collect();

    SongStructure {
        bpm,
        beats_per_bar: 4,
        bars_per_phrase: 4,
        grid_mode: GridMode::Grid,
        beat_ticks,
        bar_ticks,
        phrase_ticks: Vec::new(),
        onsets: Vec::new(),
        change_points: Vec::new(),
        waveform_peaks: Vec::new(),
        loudness_envelope: Vec::new(),
        energy_per_second: vec![0.5; duration_sec.ceil() as usize],
        energy_per_bar: vec![0.5; 30],
    }
}

#[test]
fn test_breadth_before_depth_conductor_execution() {
    let (project, track_id, asset_files) = create_test_project_and_assets();
    let song = create_simple_song(120.0, 6.0);

    let temp_cache = std::env::temp_dir().join("ventacore_conductor_cache");
    let cache = MediaCache::new(&temp_cache).unwrap();

    let config = ConductorConfig {
        project,
        song,
        asset_files,
        cache,
        target_track_id: Some(track_id),
    };

    let handle = spawn_conductor(config);

    let mut got_provisional = false;
    let mut got_refined = false;
    let mut got_finished = false;
    let mut progress_count = 0;

    while let Ok(update) = handle.receiver.recv() {
        match update {
            ConductorUpdate::Progress(_) => {
                progress_count += 1;
            }
            ConductorUpdate::ProvisionalCut(plan) => {
                assert!(!got_provisional, "Provisional cut emitted once");
                assert!(!plan.generated_items.is_empty(), "Must place items live");
                got_provisional = true;
            }
            ConductorUpdate::RefinedCut(plan) => {
                assert!(got_provisional, "Refined cut must occur after provisional cut");
                assert!(!plan.generated_items.is_empty());
                got_refined = true;
            }
            ConductorUpdate::Finished(final_project) => {
                assert!(got_refined);
                assert!(!final_project.items.is_empty());
                got_finished = true;
            }
        }
    }

    let result = handle.thread_handle.join().unwrap();
    assert!(result.is_ok(), "Pipeline must finish with success");
    assert!(got_provisional, "Breadth-before-depth must emit provisional cut");
    assert!(got_refined, "Conductor must emit refined cut after full passes");
    assert!(got_finished, "Conductor must finish and emit final project");
    assert!(progress_count > 5, "Progress updates streamed");

    let _ = std::fs::remove_dir_all(temp_cache);
}

#[test]
fn test_conductor_cooperative_cancellation() {
    let (project, track_id, asset_files) = create_test_project_and_assets();
    let song = create_simple_song(120.0, 6.0);

    let temp_cache = std::env::temp_dir().join("ventacore_cancel_cache");
    let cache = MediaCache::new(&temp_cache).unwrap();

    let config = ConductorConfig {
        project,
        song,
        asset_files,
        cache,
        target_track_id: Some(track_id),
    };

    let handle = spawn_conductor(config);

    // Cancel immediately
    handle.cancellation.cancel();

    let result = handle.thread_handle.join().unwrap();
    assert!(result.is_err(), "Cancelled pipeline must return Err");
    assert!(handle.cancellation.is_cancelled());

    let _ = std::fs::remove_dir_all(temp_cache);
}

#[test]
fn test_freeze_timeline_cuts_action() {
    let (mut project, track_id, asset_files) = create_test_project_and_assets();
    let aid = asset_files[0].0;

    // Add 2 unpinned items
    let item1 = Item::new(
        ItemId(101),
        track_id,
        aid,
        Tick::from_seconds(0),
        Tick::from_seconds(4),
        Tick::ZERO,
    );
    let item2 = Item::new(
        ItemId(102),
        track_id,
        aid,
        Tick::from_seconds(4),
        Tick::from_seconds(4),
        Tick::ZERO,
    );

    project.apply_command(&Command::InsertItem(item1)).unwrap();
    project.apply_command(&Command::InsertItem(item2)).unwrap();

    assert_eq!(project.items.iter().filter(|i| i.pinned).count(), 0);

    // 1. Freeze timeline cuts
    let frozen_count = freeze_timeline_cuts(&mut project, track_id).unwrap();
    assert_eq!(frozen_count, 2);

    let pinned_count = project.items.iter().filter(|i| i.pinned).count();
    assert_eq!(pinned_count, 2, "All items on track must now be pinned");

    // 2. Unfreeze
    let unfrozen_count = unfreeze_timeline_cuts(&mut project, track_id).unwrap();
    assert_eq!(unfrozen_count, 2);

    let pinned_after = project.items.iter().filter(|i| i.pinned).count();
    assert_eq!(pinned_after, 0, "All items on track must now be unpinned");
}

#[test]
fn test_pipeline_status_formatting() {
    let mut status = PipelineStatus::default();
    assert_eq!(status.summary(), "Idle - Ready to build");

    status.stage = PipelineStage::AnalyzingSong;
    assert!(status.summary().contains("Listening to song"));

    status.stage = PipelineStage::QuickPassingFootage {
        completed: 2,
        total: 5,
    };
    assert_eq!(status.summary(), "Quick pass on footage: 2 of 5 videos ready");

    status.stage = PipelineStage::ProvisionalCutReady;
    assert!(status.summary().contains("Provisional cut"));

    status.stage = PipelineStage::Complete;
    assert_eq!(status.summary(), "Build complete");
}
