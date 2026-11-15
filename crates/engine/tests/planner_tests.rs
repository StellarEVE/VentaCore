//! Stage 3 Integration Tests: The Montage Planner.
//!
//! Validates:
//! 1. Deterministic repeatability: same inputs + seed -> 100% identical plan.
//! 2. Seed variability: different seeds produce different montage plans.
//! 3. Pinned and touched clip preservation: pinned clips remain byte-for-byte unchanged.
//! 4. Hard constraint enforcement: no generated clip ever crosses intro/outro trims,
//!    exclude ranges, or black ranges.
//! 5. Grid alignment with cut offset: cuts land on beat grid shifted by cut_offset.
//! 6. Energy density scaling: higher energy passages produce shorter cut durations.
//! 7. Ranked alternatives: alternatives are sorted descending by score with zero self-inclusion.
//! 8. Placement modes: WindowedWalk with continue_start_points vs EvenWindows.
//! 9. Scene cut snapping: snaps to nearby scene cuts within threshold.
//! 10. Command application: applying plan to project modifies only unpinned items,
//!     maintaining 100% invariant validity through Command pattern.

use std::collections::HashMap;
use ventacore_audio::SongStructure;
use ventacore_core::*;
use ventacore_engine::planner::*;
use ventacore_media::FootageAnalysis;

/// Helper to construct a synthetic SongStructure.
fn create_test_song(bpm: f32, duration_sec: f64, energy: f32) -> SongStructure {
    let beat_duration_sec = 60.0 / bpm as f64;
    let beat_tick = Tick::from_seconds_f64(beat_duration_sec);
    let num_beats = (duration_sec / beat_duration_sec) as usize;

    let beat_ticks: Vec<Tick> = (1..=num_beats).map(|i| beat_tick * (i as i64)).collect();
    let bar_ticks: Vec<Tick> = beat_ticks.iter().step_by(4).copied().collect();
    let num_bars = bar_ticks.len();

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
        energy_per_second: vec![energy; duration_sec.ceil() as usize],
        energy_per_bar: vec![energy; num_bars + 1],
    }
}

/// Helper to create a test project with 1 video track and multiple assets.
fn create_test_project() -> (Project, TrackId, Vec<AssetId>) {
    let mut project = Project::new("Planner Test Project");
    let track_id = TrackId(10);
    project
        .apply_command(&Command::AddTrack(Track::new(
            track_id,
            "Video Track 1",
            TrackKind::Video,
        )))
        .unwrap();

    let mut asset_ids = Vec::new();
    for i in 1..=4 {
        let aid = AssetId(i);
        let mut asset = Asset::new(aid, format!("video_{}.mp4", i));
        asset.probe_data = Some(ProbeData {
            duration: Tick::from_seconds(60),
            width: 1920,
            height: 1080,
            fps_num: 30,
            fps_den: 1,
            has_audio: false,
            audio_sample_rate: 0,
        });
        project.apply_command(&Command::AddAsset(asset)).unwrap();
        asset_ids.push(aid);
    }

    (project, track_id, asset_ids)
}

#[test]
fn test_deterministic_repeatability() {
    let (project, track_id, _) = create_test_project();
    let song = create_test_song(128.0, 30.0, 0.7);
    let footage = HashMap::new();

    let mut settings = GenerationSettings::default();
    settings.seed = 987654321;

    let plan1 = plan_montage(&project, &song, &footage, Some(&settings), Some(track_id)).unwrap();
    let plan2 = plan_montage(&project, &song, &footage, Some(&settings), Some(track_id)).unwrap();

    assert_eq!(
        plan1.slots.len(),
        plan2.slots.len(),
        "Plans must have identical number of slots"
    );
    assert_eq!(
        plan1.generated_items, plan2.generated_items,
        "Generated items must be byte-for-byte identical across runs with same seed"
    );
    assert_eq!(
        plan1.slots, plan2.slots,
        "All slots and alternatives must match exactly"
    );
}

#[test]
fn test_seed_variability() {
    let (project, track_id, _) = create_test_project();
    let song = create_test_song(120.0, 30.0, 0.6);
    let footage = HashMap::new();

    let mut settings1 = GenerationSettings::default();
    settings1.seed = 11111;
    settings1.clip_variance = 0.8;

    let mut settings2 = GenerationSettings::default();
    settings2.seed = 99999;
    settings2.clip_variance = 0.8;

    let plan1 = plan_montage(&project, &song, &footage, Some(&settings1), Some(track_id)).unwrap();
    let plan2 = plan_montage(&project, &song, &footage, Some(&settings2), Some(track_id)).unwrap();

    assert_ne!(
        plan1.generated_items, plan2.generated_items,
        "Different seeds must produce different cut selections"
    );
}

#[test]
fn test_pinned_and_touched_clips_are_preserved_byte_for_byte() {
    let (mut project, track_id, asset_ids) = create_test_project();
    let song = create_test_song(120.0, 40.0, 0.5);
    let footage = HashMap::new();

    // Add a pinned clip at [4s, 8s)
    let pinned_item = Item {
        id: ItemId(101),
        track_id,
        asset_id: asset_ids[0],
        timeline_start: Tick::from_seconds(4),
        timeline_duration: Tick::from_seconds(4),
        source_in: Tick::from_seconds(15),
        speed: 1.0,
        pinned: true,
        touched: false,
        enabled: true,
        transition: None,
        effects: Vec::new(),
        volume: 1.0,
    };
    project
        .apply_command(&Command::InsertItem(pinned_item.clone()))
        .unwrap();

    // Add a touched clip at [16s, 20s)
    let touched_item = Item {
        id: ItemId(102),
        track_id,
        asset_id: asset_ids[1],
        timeline_start: Tick::from_seconds(16),
        timeline_duration: Tick::from_seconds(4),
        source_in: Tick::from_seconds(8),
        speed: 1.0,
        pinned: false,
        touched: true,
        enabled: true,
        transition: None,
        effects: Vec::new(),
        volume: 1.0,
    };
    project
        .apply_command(&Command::InsertItem(touched_item.clone()))
        .unwrap();

    // Generate plan
    let plan = plan_montage(&project, &song, &footage, None, Some(track_id)).unwrap();

    assert_eq!(plan.pinned_items_count, 2);

    // Apply plan to project
    apply_plan_to_project(&mut project, &plan).unwrap();

    // Invariants: pinned and touched items must still be in the project byte-for-byte!
    let found_pinned = project.find_item(ItemId(101)).expect("Pinned item must remain");
    assert_eq!(found_pinned, &pinned_item);

    let found_touched = project.find_item(ItemId(102)).expect("Touched item must remain");
    assert_eq!(found_touched, &touched_item);

    // Generated items must never overlap with [4s, 8s) or [16s, 20s)
    let pinned_range = TimeRange::from_start_end(Tick::from_seconds(4), Tick::from_seconds(8));
    let touched_range = TimeRange::from_start_end(Tick::from_seconds(16), Tick::from_seconds(20));

    for item in &plan.generated_items {
        let item_range = TimeRange::new(item.timeline_start, item.timeline_duration);
        assert!(
            !item_range.overlaps(pinned_range),
            "Generated item {:?} must not overlap pinned range",
            item
        );
        assert!(
            !item_range.overlaps(touched_range),
            "Generated item {:?} must not overlap touched range",
            item
        );
    }
}

#[test]
fn test_hard_constraints_trims_exclusions_and_black_ranges_never_crossed() {
    let mut project = Project::new("Hard Constraints Test");
    let track_id = TrackId(1);
    project
        .apply_command(&Command::AddTrack(Track::new(
            track_id,
            "Video",
            TrackKind::Video,
        )))
        .unwrap();

    let asset_id = AssetId(1);
    let mut asset = Asset::new(asset_id, "source.mp4".into());
    asset.probe_data = Some(ProbeData {
        duration: Tick::from_seconds(60),
        width: 1920,
        height: 1080,
        fps_num: 30,
        fps_den: 1,
        has_audio: false,
        audio_sample_rate: 0,
    });
    // Intro trim = 5s, Outro trim = 10s -> Usable [5s, 50s)
    asset.intro_trim = Tick::from_seconds(5);
    asset.outro_trim = Tick::from_seconds(10);

    // Exclude [15s, 25s)
    let exclude = TimeRange::from_start_end(Tick::from_seconds(15), Tick::from_seconds(25));
    asset.exclude_ranges.push(exclude);

    project.apply_command(&Command::AddAsset(asset)).unwrap();

    let mut footage_map = HashMap::new();
    let mut analysis = FootageAnalysis::new(asset_id, Tick::from_seconds(60));
    // Black range: [35s, 40s)
    let black = TimeRange::from_start_end(Tick::from_seconds(35), Tick::from_seconds(40));
    analysis.black_ranges.push(black);
    footage_map.insert(asset_id, analysis);

    let song = create_test_song(120.0, 30.0, 0.5);

    let mut settings = GenerationSettings::default();
    settings.min_clip_duration = Tick::from_seconds(1);
    settings.max_clip_duration = Tick::from_seconds(3);

    let plan = plan_montage(&project, &song, &footage_map, Some(&settings), Some(track_id)).unwrap();

    assert!(!plan.generated_items.is_empty());

    for item in &plan.generated_items {
        let source_in = item.source_in;
        let source_end = item.source_in + item.timeline_duration;
        let item_source_range = TimeRange::from_start_end(source_in, source_end);

        // 1. Intro trim: source_in >= 5s
        assert!(
            source_in >= Tick::from_seconds(5),
            "source_in ({}) must be >= intro_trim (5s)",
            source_in
        );

        // 2. Outro trim: source_end <= 50s
        assert!(
            source_end <= Tick::from_seconds(50),
            "source_end ({}) must be <= duration - outro_trim (50s)",
            source_end
        );

        // 3. Exclude ranges: must not overlap [15s, 25s)
        assert!(
            !item_source_range.overlaps(exclude),
            "Clip source range {:?} overlaps exclude range {:?}",
            item_source_range,
            exclude
        );

        // 4. Black ranges: must not overlap [35s, 40s)
        assert!(
            !item_source_range.overlaps(black),
            "Clip source range {:?} overlaps black range {:?}",
            item_source_range,
            black
        );
    }
}

#[test]
fn test_cuts_land_on_grid_with_offset() {
    let (project, track_id, _) = create_test_project();
    let song = create_test_song(120.0, 20.0, 0.5);
    let footage = HashMap::new();

    let mut settings = GenerationSettings::default();
    let offset = Tick::from_seconds_f64(-0.04); // -40ms
    settings.cut_offset = offset;
    settings.clip_variance = 0.0;
    settings.min_clip_duration = Tick::from_seconds(1);

    let plan = plan_montage(&project, &song, &footage, Some(&settings), Some(track_id)).unwrap();

    for slot in &plan.slots[..plan.slots.len() - 1] {
        let cut_point = slot.assigned_item.timeline_end();
        let matches_grid = song.beat_ticks.iter().any(|&b| b + offset == cut_point);
        assert!(
            matches_grid,
            "Cut point {} must land on beat shifted by offset {}",
            cut_point, offset
        );
    }
}

#[test]
fn test_higher_energy_produces_shorter_cuts() {
    let (project, track_id, _) = create_test_project();
    let song_high = create_test_song(130.0, 30.0, 0.95);
    let song_low = create_test_song(130.0, 30.0, 0.15);
    let footage = HashMap::new();

    let mut settings = GenerationSettings::default();
    settings.clip_variance = 0.0;
    settings.energy_density = 1.0;
    settings.min_clip_duration = Tick::from_seconds_f64(0.3);
    settings.max_clip_duration = Tick::from_seconds_f64(6.0);

    let plan_high = plan_montage(&project, &song_high, &footage, Some(&settings), Some(track_id)).unwrap();
    let plan_low = plan_montage(&project, &song_low, &footage, Some(&settings), Some(track_id)).unwrap();

    let avg_dur_high: f64 = plan_high.generated_items.iter().map(|i| i.timeline_duration.to_seconds_f64()).sum::<f64>()
        / plan_high.generated_items.len() as f64;
    let avg_dur_low: f64 = plan_low.generated_items.iter().map(|i| i.timeline_duration.to_seconds_f64()).sum::<f64>()
        / plan_low.generated_items.len() as f64;

    assert!(
        avg_dur_high < avg_dur_low,
        "High energy average cut duration ({:.2}s) must be shorter than low energy ({:.2}s)",
        avg_dur_high,
        avg_dur_low
    );
    assert!(
        plan_high.generated_items.len() > plan_low.generated_items.len(),
        "High energy plan must contain more cuts than low energy"
    );
}

#[test]
fn test_alternatives_ranked_without_self_inclusion() {
    let (project, track_id, _) = create_test_project();
    let song = create_test_song(120.0, 20.0, 0.5);
    let footage = HashMap::new();

    let plan = plan_montage(&project, &song, &footage, None, Some(track_id)).unwrap();

    for slot in &plan.slots {
        let assigned = &slot.assigned_item;

        for alt in &slot.alternatives {
            // Must not match assigned clip
            let is_self = alt.asset_id == assigned.asset_id && alt.source_in == assigned.source_in;
            assert!(
                !is_self,
                "Alternatives must never include the assigned clip (slot {})",
                slot.slot_index
            );
        }

        // Must be sorted descending by score
        for window in slot.alternatives.windows(2) {
            assert!(
                window[0].score >= window[1].score,
                "Alternatives must be sorted in descending order of score"
            );
        }
    }
}

#[test]
fn test_scene_snapping() {
    let mut project = Project::new("Scene Snapping Test");
    let track_id = TrackId(1);
    project
        .apply_command(&Command::AddTrack(Track::new(
            track_id,
            "Video",
            TrackKind::Video,
        )))
        .unwrap();

    let asset_id = AssetId(1);
    let mut asset = Asset::new(asset_id, "video_with_cuts.mp4".into());
    asset.probe_data = Some(ProbeData {
        duration: Tick::from_seconds(60),
        width: 1920,
        height: 1080,
        fps_num: 30,
        fps_den: 1,
        has_audio: false,
        audio_sample_rate: 0,
    });
    project.apply_command(&Command::AddAsset(asset)).unwrap();

    let mut footage_map = HashMap::new();
    let mut analysis = FootageAnalysis::new(asset_id, Tick::from_seconds(60));
    // Scene cut at 5.2 seconds
    let scene_cut_tick = Tick::from_seconds_f64(5.2);
    analysis.scene_cuts.push(scene_cut_tick);
    footage_map.insert(asset_id, analysis);

    let song = create_test_song(120.0, 15.0, 0.5);

    let mut settings = GenerationSettings::default();
    settings.snap_to_scenes = true;

    let plan = plan_montage(&project, &song, &footage_map, Some(&settings), Some(track_id)).unwrap();

    // Check if any slot's assigned or alternative clips snapped to 5.2s
    let mut found_snap = false;
    for slot in &plan.slots {
        if slot.assigned_item.source_in == scene_cut_tick {
            found_snap = true;
            break;
        }
        for alt in &slot.alternatives {
            if alt.source_in == scene_cut_tick {
                found_snap = true;
                break;
            }
        }
    }

    assert!(found_snap, "Scene cut at 5.2s should be picked up or snapped to by the planner");
}

#[test]
fn test_apply_plan_updates_project_invariants_cleanly() {
    let (mut project, track_id, _) = create_test_project();
    let song = create_test_song(120.0, 25.0, 0.5);
    let footage = HashMap::new();

    let plan = plan_montage(&project, &song, &footage, None, Some(track_id)).unwrap();

    // Applying plan must succeed and leave a valid project
    apply_plan_to_project(&mut project, &plan).expect("Plan application must succeed");

    // Project must be fully valid according to all project invariants
    project.validate().expect("Project must be valid after plan application");

    // Track items must match plan generated items
    let track_items = project.track_items(track_id);
    assert_eq!(track_items.len(), plan.generated_items.len());

    // Non-overlapping check
    for window in track_items.windows(2) {
        assert!(
            window[0].timeline_end() <= window[1].timeline_start,
            "Items on track must never overlap"
        );
    }
}

#[test]
fn test_presets_coverage() {
    for name in PRESET_NAMES {
        let preset = get_preset_settings(name).unwrap_or_else(|| panic!("Preset {} must be found", name));
        assert!(preset.min_clip_duration > Tick::ZERO);
        assert!(preset.max_clip_duration >= preset.min_clip_duration);
        assert!(preset.energy_density >= 0.0 && preset.energy_density <= 1.0);
    }

    assert!(get_preset_settings("nonexistent_preset").is_none());
}

#[test]
fn test_error_handling() {
    let (mut project, track_id, _) = create_test_project();
    let song = create_test_song(120.0, 20.0, 0.5);
    let footage = HashMap::new();

    // 1. Empty song error
    let empty_song = SongStructure {
        bpm: 120.0,
        beats_per_bar: 4,
        bars_per_phrase: 4,
        grid_mode: GridMode::Grid,
        beat_ticks: Vec::new(),
        bar_ticks: Vec::new(),
        phrase_ticks: Vec::new(),
        onsets: Vec::new(),
        change_points: Vec::new(),
        waveform_peaks: Vec::new(),
        loudness_envelope: Vec::new(),
        energy_per_second: Vec::new(),
        energy_per_bar: Vec::new(),
    };
    let err = plan_montage(&project, &empty_song, &footage, None, Some(track_id)).unwrap_err();
    assert_eq!(err, PlannerError::EmptySong);

    // 2. Target track locked error
    project
        .apply_command(&Command::SetTrackLock {
            track_id,
            locked: true,
        })
        .unwrap();
    let err = plan_montage(&project, &song, &footage, None, Some(track_id)).unwrap_err();
    assert_eq!(err, PlannerError::TargetTrackLocked(track_id));

    // Unlock track
    project
        .apply_command(&Command::SetTrackLock {
            track_id,
            locked: false,
        })
        .unwrap();

    // 3. No eligible assets
    let mut empty_project = Project::new("Empty Assets Project");
    empty_project
        .apply_command(&Command::AddTrack(Track::new(
            track_id,
            "Video",
            TrackKind::Video,
        )))
        .unwrap();
    let err = plan_montage(&empty_project, &song, &footage, None, Some(track_id)).unwrap_err();
    assert_eq!(err, PlannerError::NoEligibleAssets);
}

#[test]
fn test_motion_matching_rewards_high_motion_in_high_energy_passages() {
    let mut project = Project::new("Motion Matching Test");
    let track_id = TrackId(1);
    project
        .apply_command(&Command::AddTrack(Track::new(
            track_id,
            "Video",
            TrackKind::Video,
        )))
        .unwrap();

    // Asset 1: Action video (high motion: 0.95)
    let aid_action = AssetId(1);
    let mut asset_action = Asset::new(aid_action, "action.mp4".into());
    asset_action.probe_data = Some(ProbeData {
        duration: Tick::from_seconds(60),
        width: 1920,
        height: 1080,
        fps_num: 30,
        fps_den: 1,
        has_audio: false,
        audio_sample_rate: 0,
    });
    project.apply_command(&Command::AddAsset(asset_action)).unwrap();

    // Asset 2: Calm video (low motion: 0.10)
    let aid_calm = AssetId(2);
    let mut asset_calm = Asset::new(aid_calm, "calm.mp4".into());
    asset_calm.probe_data = Some(ProbeData {
        duration: Tick::from_seconds(60),
        width: 1920,
        height: 1080,
        fps_num: 30,
        fps_den: 1,
        has_audio: false,
        audio_sample_rate: 0,
    });
    project.apply_command(&Command::AddAsset(asset_calm)).unwrap();

    let mut footage_map = HashMap::new();

    let mut ana_action = FootageAnalysis::new(aid_action, Tick::from_seconds(60));
    ana_action.level = ventacore_media::AnalysisLevel::Full;
    for s in 0..60 {
        ana_action.motion_scores.push(ventacore_media::MotionPoint {
            time: Tick::from_seconds(s),
            score: 0.95,
        });
    }
    footage_map.insert(aid_action, ana_action);

    let mut ana_calm = FootageAnalysis::new(aid_calm, Tick::from_seconds(60));
    ana_calm.level = ventacore_media::AnalysisLevel::Full;
    for s in 0..60 {
        ana_calm.motion_scores.push(ventacore_media::MotionPoint {
            time: Tick::from_seconds(s),
            score: 0.10,
        });
    }
    footage_map.insert(aid_calm, ana_calm);

    // High energy song (0.95)
    let song = create_test_song(130.0, 10.0, 0.95);
    let mut settings = GenerationSettings::default();
    settings.energy_density = 1.0;
    settings.source_reuse_penalty = 0.0; // let score purely drive choice

    let plan = plan_montage(&project, &song, &footage_map, Some(&settings), Some(track_id)).unwrap();

    let action_picks = plan.generated_items.iter().filter(|i| i.asset_id == aid_action).count();
    let calm_picks = plan.generated_items.iter().filter(|i| i.asset_id == aid_calm).count();

    assert!(
        action_picks > calm_picks,
        "High energy song should pick high-motion asset (action: {}, calm: {})",
        action_picks, calm_picks
    );
}

