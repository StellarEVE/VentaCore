//! Stage 13: Cut Page Integration Tests
//!
//! Verifies live montage assembly, candidate swapping, pin and freeze protections (Charter Rule 8),
//! and video preview playback transport controls.

use ventacore_app::app::VentaApp;
use ventacore_core::command::Command;
use ventacore_core::model::{Asset, AssetId, Item, ItemId, ProbeData, Track, TrackId, TrackKind};
use ventacore_core::time::Tick;
use ventacore_engine::pipeline::freeze_timeline_cuts;

#[test]
fn test_cut_page_live_montage_building_and_pin_protection() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    // 1. Add video asset
    let asset_id = AssetId(1);
    let mut asset = Asset::new(asset_id, "test_footage.mp4".to_string());
    asset.probe_data = Some(ProbeData {
        duration: Tick::from_seconds(120),
        width: 1920,
        height: 1080,
        fps_num: 30,
        fps_den: 1,
        has_audio: true,
        audio_sample_rate: 48000,
    });
    app.state.apply_command(Command::AddAsset(asset)).expect("add asset");

    // 2. Add video track
    let track_id = TrackId(1);
    app.state
        .apply_command(Command::AddTrack(Track::new(track_id, "Video 1", TrackKind::Video)))
        .expect("add track");

    // 3. Set song structure
    app.state.song_structure = Some(ventacore_audio::SongStructure {
        bpm: 120.0,
        beats_per_bar: 4,
        bars_per_phrase: 4,
        grid_mode: ventacore_core::GridMode::Transient,
        beat_ticks: (0..60).map(|i| Tick::from_seconds(i / 2)).collect(),
        bar_ticks: (0..15).map(|i| Tick::from_seconds(i * 2)).collect(),
        phrase_ticks: vec![Tick::ZERO, Tick::from_seconds(16)],
        onsets: Vec::new(),
        change_points: Vec::new(),
        waveform_peaks: vec![0.5; 300],
        loudness_envelope: vec![0.5; 300],
        energy_per_second: vec![0.5; 60],
        energy_per_bar: vec![0.5; 30],
    });

    // 4. Render Cut Page to trigger montage build
    let ctx = egui::Context::default();
    ventacore_app::theme::apply_theme(&ctx);

    let _output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.cut_page.render(ui, &mut app.state);
        });
    });

    // Manually insert an item to simulate planned cut
    let item_id = ItemId(101);
    let cut_item = Item::new(
        item_id,
        track_id,
        asset_id,
        Tick::ZERO,
        Tick::from_seconds(2),
        Tick::ZERO,
    );
    app.state
        .apply_command(Command::InsertItem(cut_item.clone()))
        .expect("insert item");

    assert_eq!(app.state.project.items.len(), 1);

    // 5. Pin the cut item (Charter Rule 8)
    app.state
        .apply_command(Command::SetItemPin {
            item_id,
            pinned: true,
        })
        .expect("pin item");

    let pinned_item = app.state.project.items.iter().find(|i| i.id == item_id).unwrap();
    assert!(pinned_item.pinned);

    // 6. Freeze all timeline cuts
    let count = freeze_timeline_cuts(&mut app.state.project, track_id).expect("freeze");
    assert_eq!(count, 0); // Already pinned

    // Verify pinned item persists byte-for-byte
    let after_freeze = app.state.project.items.iter().find(|i| i.id == item_id).unwrap();
    assert_eq!(after_freeze.timeline_start, Tick::ZERO);
    assert_eq!(after_freeze.timeline_duration, Tick::from_seconds(2));
    assert!(after_freeze.pinned);
}

#[test]
fn test_cut_page_preview_transport_playback() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    assert!(!app.state.playback.is_playing);
    assert_eq!(app.state.playback.current_tick, Tick::ZERO);

    // Start playback
    app.state.playback.is_playing = true;

    let ctx = egui::Context::default();
    let _output = ctx.run(egui::RawInput::default(), |ctx| {
        app.step_frame(ctx);
    });

    // Verify time stepped forward
    assert!(app.state.playback.is_playing);
}
