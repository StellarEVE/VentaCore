//! Owner Regression Reproduction Tests
//!
//! Reproduces the four major user failures identified by the owner:
//! 1. Import has no way to browse computer files (only drag & drop).
//! 2. Song treated as an ordinary asset and placed onto video cut timeline.
//! 3. No playing video anywhere in the interface (monitors missing or empty).
//! 4. Page layouts invented from scratch rather than standard editor conventions.
//!
//! Each test is marked #[ignore = "..."] with the stage that will fix it.

use std::collections::HashMap;
use ventacore_audio::SongStructure;
use ventacore_core::command::Command;
use ventacore_core::model::{Asset, AssetId, AssetRole, GridMode, Item, ItemId, ProbeData, Project, ReplaceSongPolicy, Track, TrackId, TrackKind};
use ventacore_core::time::Tick;
use ventacore_engine::planner::plan_montage;

/// Failure 2: Importing a song and videos must yield exactly one active song
/// and the right number of footage assets, with zero song items on video tracks.
#[test]
fn test_regression_failure_2_song_role_and_exclusion_from_video_tracks() {
    let mut project = Project::new("Music Video");

    // 1. Add 1 song asset and 2 footage assets
    let mut song_asset = Asset::song(AssetId(1), "test_song.mp3".into());
    song_asset.probe_data = Some(ProbeData {
        duration: Tick::from_seconds(10),
        width: 0,
        height: 0,
        fps_num: 0,
        fps_den: 1,
        has_audio: true,
        audio_sample_rate: 44100,
    });

    let mut foot1 = Asset::footage(AssetId(2), "drone_footage.mp4".into());
    foot1.probe_data = Some(ProbeData {
        duration: Tick::from_seconds(30),
        width: 1920,
        height: 1080,
        fps_num: 30,
        fps_den: 1,
        has_audio: true,
        audio_sample_rate: 48000,
    });

    let mut foot2 = Asset::footage(AssetId(3), "skate_footage.mp4".into());
    foot2.probe_data = Some(ProbeData {
        duration: Tick::from_seconds(30),
        width: 1920,
        height: 1080,
        fps_num: 30,
        fps_den: 1,
        has_audio: true,
        audio_sample_rate: 48000,
    });

    project.apply_command(&Command::AddAsset(song_asset)).expect("add song asset");
    project.apply_command(&Command::AddFootage(foot1)).expect("add footage 1");
    project.apply_command(&Command::AddFootage(foot2)).expect("add footage 2");

    // Add Video Track
    project.apply_command(&Command::AddTrack(Track::new(TrackId(1), "Video 1", TrackKind::Video)))
        .expect("add video track");

    // 2. Set active song
    let song_duration = Tick::from_seconds(10);
    project.apply_command(&Command::SetSong {
        asset_id: AssetId(1),
        start: Tick::ZERO,
        end: song_duration,
        policy: ReplaceSongPolicy::StartOver,
    }).expect("set song must succeed");

    assert!(project.song.is_some());
    assert_eq!(project.song.as_ref().unwrap().asset_id, AssetId(1));
    assert_eq!(project.find_asset(AssetId(1)).unwrap().role, AssetRole::Song);
    assert_eq!(project.find_asset(AssetId(2)).unwrap().role, AssetRole::Footage);
    assert_eq!(project.find_asset(AssetId(3)).unwrap().role, AssetRole::Footage);

    // 3. Verify: Attempting to insert song item on video track is strictly rejected by validation
    let bad_song_item = Item::new(
        ItemId(99),
        TrackId(1), // Video track
        AssetId(1), // Song asset!
        Tick::ZERO,
        Tick::from_seconds(3),
        Tick::ZERO,
    );
    let result = project.apply_command(&Command::InsertItem(bad_song_item));
    assert!(result.is_err(), "Placing a song asset on a video track must be rejected!");

    // 4. Verify: Song track contains exactly 1 item and it is the active song
    let song_track = project.song_track().expect("song track exists");
    let song_items = project.track_items(song_track.id);
    assert_eq!(song_items.len(), 1);
    assert_eq!(song_items[0].asset_id, AssetId(1));

    // 5. Run montage planner and verify song is NEVER placed as a candidate
    let beat_ticks = vec![
        Tick::from_seconds(0),
        Tick::from_seconds(2),
        Tick::from_seconds(4),
        Tick::from_seconds(6),
        Tick::from_seconds(8),
        Tick::from_seconds(10),
    ];
    let song_structure = SongStructure {
        bpm: 120.0,
        beats_per_bar: 4,
        bars_per_phrase: 4,
        grid_mode: GridMode::Transient,
        beat_ticks: beat_ticks.clone(),
        bar_ticks: vec![Tick::from_seconds(0), Tick::from_seconds(4), Tick::from_seconds(8)],
        phrase_ticks: vec![Tick::from_seconds(0), Tick::from_seconds(8)],
        onsets: Vec::new(),
        change_points: Vec::new(),
        waveform_peaks: vec![0.5; 50],
        loudness_envelope: vec![0.8; 50],
        energy_per_second: vec![0.5; 10],
        energy_per_bar: vec![0.6; 3],
    };

    let plan = plan_montage(&project, &song_structure, &HashMap::new(), None, Some(TrackId(1)))
        .expect("planner must succeed");

    assert!(!plan.generated_items.is_empty(), "Plan must have generated clips");
    for item in &plan.generated_items {
        // Must NEVER point to the song asset
        assert_ne!(item.asset_id, AssetId(1), "Cut clip must never be the song asset!");
        assert!(item.asset_id == AssetId(2) || item.asset_id == AssetId(3));
        assert!(item.timeline_end() <= song_duration, "Clip must not exceed song duration");
    }

    // 6. Test replacing song with KeepPinnedAndTouched policy
    // Pin first clip
    let first_clip = plan.generated_items[0].clone();
    let mut pinned_clip = first_clip.clone();
    pinned_clip.pinned = true;
    project.apply_command(&Command::InsertItem(pinned_clip)).expect("insert pinned clip");

    // Replace song with shorter duration (5s)
    let shorter_duration = Tick::from_seconds(5);
    project.apply_command(&Command::SetSong {
        asset_id: AssetId(1),
        start: Tick::ZERO,
        end: shorter_duration,
        policy: ReplaceSongPolicy::KeepPinnedAndTouched,
    }).expect("replace song with keep pinned must succeed");

    assert_eq!(project.timeline_duration(), shorter_duration);
}

/// Failure 2 (Audio probe): An MP3 with cover art must be classified as audio only,
/// never misidentified as a video.
#[test]
#[ignore = "Stage R2"]
fn test_regression_failure_2_mp3_with_cover_art_classified_as_audio_only() {
    // Stage R2 will introduce the classify function in platform/engine that detects
    // ID3 APIC / attached pictures as cover art and classifies file as AudioOnly.
    panic!("Stage R2 not yet applied: classify() not yet implemented");
}

/// Failure 3: Monitor widget must render a real decoded frame and match the
/// expected color in a pixel test, failing if black or unrendered.
#[test]
#[ignore = "Stage R3"]
fn test_regression_failure_3_monitor_shows_real_pixels() {
    // Stage R3 will introduce the shared player, egui texture upload, and Monitor widget.
    panic!("Stage R3 not yet applied: no video monitor or texture pipeline in UI");
}

/// Failure 1: Both "Browse for music" and "Add videos" buttons must exist on the Media page,
/// and the file-dialog helper must return paths from a stand-in dialog.
#[test]
#[ignore = "Stage R2"]
fn test_regression_failure_1_browse_buttons_and_dialog_helper() {
    // Stage R2 will add Browse for music and Add videos buttons, and native file dialog helper.
    panic!("Stage R2 not yet applied: Browse buttons and dialog helper missing");
}
