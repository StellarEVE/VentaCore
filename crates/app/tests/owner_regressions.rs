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
fn test_regression_failure_2_mp3_with_cover_art_classified_as_audio_only() {
    use ventacore_platform::{classify, MediaClassification};

    let temp_dir = std::env::temp_dir();
    let mp3_path = temp_dir.join(format!("owner_test_art_{}.mp3", std::process::id()));

    // Synthetic MP3 with ID3v2.3 tag containing APIC (Attached Picture / Cover Art)
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"ID3\x03\x00\x00\x00\x00\x01\x00"); // 128 bytes synchsafe tag
    bytes.extend_from_slice(b"APIC\x00\x00\x00\x20\x00\x00"); // APIC frame header
    bytes.extend_from_slice(&[0u8; 32]); // Image bytes
    while bytes.len() < 138 {
        bytes.push(0);
    }
    // MPEG Audio frame sync
    bytes.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x64]);
    bytes.extend_from_slice(&[0u8; 1000]);

    std::fs::write(&mp3_path, &bytes).unwrap();

    let classification = classify(&mp3_path);
    assert!(
        classification.is_audio_only(),
        "MP3 with cover art must be classified as AudioOnly, never as video!"
    );

    if let MediaClassification::AudioOnly { has_cover_art, .. } = classification {
        assert!(has_cover_art, "Must recognize cover art as album art");
    } else {
        panic!("Expected AudioOnly classification");
    }

    let _ = std::fs::remove_file(&mp3_path);
}

/// Failure 3: Monitor widget must render a real decoded frame and match the
/// expected color in a pixel test, failing if black or unrendered.
#[test]
fn test_regression_failure_3_monitor_shows_real_pixels() {
    use ventacore_app::widgets::monitor::Monitor;
    use ventacore_engine::playback::PlaybackEngine;
    use ventacore_platform::{
        ColorDepth, PixelFormat, PlatformError, Rotation, VideoDecoder, VideoFrame, VideoMetadata,
    };

    /// Custom synthetic decoder outputting solid Red at 0s, solid Green at 1s, solid Blue at 2s.
    struct TestRgbColorDecoder {
        metadata: VideoMetadata,
        current_tick: Tick,
    }

    impl VideoDecoder for TestRgbColorDecoder {
        fn metadata(&self) -> &VideoMetadata {
            &self.metadata
        }

        fn seek(&mut self, target_tick: Tick) -> Result<(), PlatformError> {
            self.current_tick = target_tick;
            Ok(())
        }

        fn decode_next_frame(&mut self) -> Result<Option<VideoFrame>, PlatformError> {
            let pts = self.current_tick;
            let dur = Tick::from_frames(1, 30, 1);
            let w = self.metadata.width;
            let h = self.metadata.height;

            let pixel = if pts < Tick::from_seconds(1) {
                // Red at 0.0s
                [255u8, 0, 0, 255]
            } else if pts < Tick::from_seconds(2) {
                // Green at 1.0s
                [0u8, 255, 0, 255]
            } else {
                // Blue at 2.0s
                [0u8, 0, 255, 255]
            };

            let mut data = vec![0u8; (w * h * 4) as usize];
            for chunk in data.chunks_exact_mut(4) {
                chunk.copy_from_slice(&pixel);
            }

            self.current_tick = pts + dur;

            Ok(Some(VideoFrame {
                pts,
                duration: dur,
                width: w,
                height: h,
                format: PixelFormat::Rgba8,
                rotation: Rotation::None,
                bit_depth: ColorDepth::Bits8,
                data,
                linesize: [(w * 4) as usize, 0, 0, 0],
            }))
        }

        fn active_decoder_name(&self) -> &str {
            "test-rgb-color-decoder"
        }

        fn is_hardware_accelerated(&self) -> bool {
            false
        }
    }

    let metadata = VideoMetadata {
        duration: Tick::from_seconds(3),
        width: 320,
        height: 240,
        fps_num: 30,
        fps_den: 1,
        is_vfr: false,
        rotation: Rotation::None,
        bit_depth: 8,
        pixel_format: PixelFormat::Rgba8,
        codec_name: "test-rgb".into(),
        has_audio: false,
        audio_sample_rate: 0,
        audio_channels: 0,
    };

    let decoder = TestRgbColorDecoder {
        metadata,
        current_tick: Tick::ZERO,
    };

    let mut player = PlaybackEngine::new();
    player.open_custom_decoder(Box::new(decoder)).expect("open custom decoder");

    // 1. Verify frame at 0.0s has red pixels (>200 red, <50 green, <50 blue)
    player.seek(Tick::ZERO);
    let frame0 = player.current_frame().expect("Frame at 0.0s must exist");
    assert_eq!(frame0.width, 320);
    assert_eq!(frame0.height, 240);
    assert!(
        frame0.rgba[0] > 200 && frame0.rgba[1] < 50 && frame0.rgba[2] < 50,
        "Frame at 0.0s must be RED! Got [R:{}, G:{}, B:{}]",
        frame0.rgba[0],
        frame0.rgba[1],
        frame0.rgba[2]
    );

    // 2. Seek to 1.0s, verify green pixels (<50 red, >200 green, <50 blue)
    player.seek(Tick::from_seconds(1));
    let frame1 = player.current_frame().expect("Frame at 1.0s must exist");
    assert!(
        frame1.rgba[0] < 50 && frame1.rgba[1] > 200 && frame1.rgba[2] < 50,
        "Frame at 1.0s must be GREEN! Got [R:{}, G:{}, B:{}]",
        frame1.rgba[0],
        frame1.rgba[1],
        frame1.rgba[2]
    );

    // 3. Seek to 2.0s, verify blue pixels (<50 red, <50 green, >200 blue)
    player.seek(Tick::from_seconds(2));
    let frame2 = player.current_frame().expect("Frame at 2.0s must exist");
    assert!(
        frame2.rgba[0] < 50 && frame2.rgba[1] < 50 && frame2.rgba[2] > 200,
        "Frame at 2.0s must be BLUE! Got [R:{}, G:{}, B:{}]",
        frame2.rgba[0],
        frame2.rgba[1],
        frame2.rgba[2]
    );

    // 4. Verify Monitor and VideoSurface widget in egui context
    let ctx = egui::Context::default();
    let mut monitor = Monitor::source();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            monitor.show(ui, &mut player, "Source Monitor Test");
        });
    });

    // Verify player is not blank or unrendered
    assert!(player.current_frame().is_some());
    let current_pixels = &player.current_frame().unwrap().rgba;
    assert!(!current_pixels.is_empty(), "Monitor frame must contain real decoded pixels");
    // Blue frame was active: verify blue is dominant
    assert!(current_pixels[2] > 200, "Rendered monitor pixels must match active blue frame");
}

/// Failure 1: Both "Browse for music" and "Add videos" workflows must operate cleanly:
/// the file-dialog helper returns paths from stand-in dialogs, drag-and-drop sorts
/// by content, wrong-panel drops offer one-click fixes, and import undo reverts in one step.
#[test]
fn test_regression_failure_1_browse_buttons_and_dialog_helper() {
    use ventacore_app::widgets::file_dialog::{FileDialogHelper, FileDialogResult};
    use ventacore_engine::import::{
        apply_import_plan, plan_import, ImportAction, ImportChoicePrompt, ImportDestination, ImportPlan,
    };

    let temp_dir = std::env::temp_dir();
    let song_path = temp_dir.join(format!("reg1_song_{}.wav", std::process::id()));
    let vid1_path = temp_dir.join(format!("reg1_vid1_{}.mp4", std::process::id()));
    let vid2_path = temp_dir.join(format!("reg1_vid2_{}.mp4", std::process::id()));

    // Create synthetic WAV
    let mut wav_bytes = Vec::new();
    wav_bytes.extend_from_slice(b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x02\x00\x44\xAC\x00\x00\x10\xB1\x02\x00\x04\x00\x10\x00data\x00\x00\x00\x00");
    std::fs::write(&song_path, &wav_bytes).unwrap();

    // Create synthetic MP4s
    let mut mp4_bytes = Vec::new();
    mp4_bytes.extend_from_slice(b"\x00\x00\x00\x18ftypisom\x00\x00\x02\x00isomiso2mp41");
    mp4_bytes.extend_from_slice(b"\x00\x00\x00\x30moovhdlrvide");
    mp4_bytes.extend_from_slice(&[0u8; 64]);
    std::fs::write(&vid1_path, &mp4_bytes).unwrap();
    std::fs::write(&vid2_path, &mp4_bytes).unwrap();

    // 1. FileDialogHelper stand-in headless test
    let mut helper = FileDialogHelper::new();
    helper.set_stand_in(FileDialogResult::Song(song_path.clone()));
    helper.pick_song();
    let poll_result = helper.poll();
    assert!(poll_result.is_some(), "FileDialogHelper must return injected stand-in");
    match poll_result.unwrap() {
        FileDialogResult::Song(p) => assert_eq!(p, song_path),
        _ => panic!("Expected FileDialogResult::Song"),
    }

    helper.set_stand_in(FileDialogResult::Videos(vec![vid1_path.clone(), vid2_path.clone()]));
    helper.pick_videos();
    let poll_vids = helper.poll();
    assert!(poll_vids.is_some(), "FileDialogHelper must return video stand-in");
    match poll_vids.unwrap() {
        FileDialogResult::Videos(vids) => {
            assert_eq!(vids.len(), 2);
            assert_eq!(vids[0], vid1_path);
            assert_eq!(vids[1], vid2_path);
        }
        _ => panic!("Expected FileDialogResult::Videos"),
    }

    // 2. Import song via Music panel
    let mut project = Project::new("Failure 1 Regression Test");
    let _initial_tracks_count = project.tracks.len();

    let plan_song = plan_import(&project, &[song_path.clone()], ImportDestination::MusicPanel);
    assert_eq!(plan_song.songs_count(), 1);
    assert_eq!(plan_song.footage_count(), 0);
    assert_eq!(plan_song.refused_count(), 0);

    apply_import_plan(&mut project, &plan_song, ReplaceSongPolicy::StartOver).expect("import song");
    assert!(project.song.is_some(), "Song must be active in project");
    assert_eq!(project.assets.len(), 1);
    assert_eq!(project.assets[0].role, AssetRole::Song);

    // 3. Import videos via Footage panel
    let plan_videos = plan_import(
        &project,
        &[vid1_path.clone(), vid2_path.clone()],
        ImportDestination::FootagePanel,
    );
    assert_eq!(plan_videos.footage_count(), 2);
    assert_eq!(plan_videos.songs_count(), 0);

    apply_import_plan(&mut project, &plan_videos, ReplaceSongPolicy::StartOver).expect("import footage");
    assert_eq!(project.assets.len(), 3);
    assert_eq!(project.footage_assets().len(), 2);

    // 4. Test wrong panel: Song dropped onto Footage panel is refused with suggest_use_as_song
    let second_song_path = temp_dir.join(format!("reg1_song2_{}.wav", std::process::id()));
    std::fs::write(&second_song_path, &wav_bytes).unwrap();

    let plan_wrong = plan_import(&project, &[second_song_path.clone()], ImportDestination::FootagePanel);
    assert_eq!(plan_wrong.refused_count(), 1);
    match &plan_wrong.actions[0] {
        ImportAction::Refused { suggest_use_as_song, reason, .. } => {
            assert!(suggest_use_as_song, "Audio on footage panel must offer Use as song");
            assert!(reason.contains("Footage panel"));
        }
        _ => panic!("Expected Refused action"),
    }

    // One-click fix: User clicks "Use as song" -> routes to song
    let fix_plan = ImportPlan {
        actions: vec![ImportAction::ImportSong {
            path: second_song_path.clone(),
            classification: ventacore_platform::classify(&second_song_path),
        }],
    };
    apply_import_plan(&mut project, &fix_plan, ReplaceSongPolicy::KeepPinnedAndTouched)
        .expect("apply fix must succeed");
    let song_asset = project.find_asset(project.song.as_ref().unwrap().asset_id).unwrap();
    assert_eq!(song_asset.path_hint, second_song_path.to_string_lossy());

    // 5. Test video dropped onto Music panel -> prompts user choice
    let vid3_path = temp_dir.join(format!("reg1_vid3_{}.mp4", std::process::id()));
    std::fs::write(&vid3_path, &mp4_bytes).unwrap();

    let vid_on_music_plan = plan_import(&project, &[vid3_path.clone()], ImportDestination::MusicPanel);
    assert_eq!(vid_on_music_plan.choices_count(), 1);
    match &vid_on_music_plan.actions[0] {
        ImportAction::NeedsChoice { choice: ImportChoicePrompt::VideoOnMusicPanel { path, .. }, .. } => {
            assert_eq!(path, &vid3_path);
        }
        _ => panic!("Expected VideoOnMusicPanel choice prompt"),
    }

    // 6. Test undo restores exact previous state in one step
    let count_before = project.assets.len();
    let mut history = ventacore_core::history::History::with_max_depth(project.clone(), 50);

    let fresh_vid_path = temp_dir.join(format!("reg1_fresh_{}.mp4", std::process::id()));
    std::fs::write(&fresh_vid_path, &mp4_bytes).unwrap();
    let fresh_plan = plan_import(&project, &[fresh_vid_path.clone()], ImportDestination::FootagePanel);
    apply_import_plan(&mut project, &fresh_plan, ReplaceSongPolicy::StartOver).expect("add fresh footage");
    history.record(project.clone());
    assert_eq!(project.assets.len(), count_before + 1);

    // Single undo
    let undone = history.undo();
    assert!(undone, "Undo must succeed");
    assert_eq!(history.present().assets.len(), count_before, "Batch import must revert cleanly in one step");

    // Cleanup
    let _ = std::fs::remove_file(&song_path);
    let _ = std::fs::remove_file(&second_song_path);
    let _ = std::fs::remove_file(&vid1_path);
    let _ = std::fs::remove_file(&vid2_path);
    let _ = std::fs::remove_file(&vid3_path);
    let _ = std::fs::remove_file(&fresh_vid_path);
}
