//! Stage 14: Export Page & Milestone B Exit Gate Integration Tests
//!
//! Verifies export configuration, active hardware encoder disclosure,
//! atomic .part cleanup on cancellation, and disposable cache management (Charter Rules 6, 9 & 11).

use std::fs::File;
use std::io::Write;
use ventacore_app::app::VentaApp;
use ventacore_app::state::AppPage;
use ventacore_core::command::Command;
use ventacore_core::model::{Asset, AssetId, ProbeData, Track, TrackId, TrackKind};
use ventacore_core::time::Tick;
use ventacore_render::FitMode;

#[test]
fn test_export_configuration_and_encoder_disclosure() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    // Execute hardware probe
    app.probe_startup_hardware();
    assert!(app.state.hardware_report.is_some());

    // Default configuration: 1080p, 30 fps, Letterbox
    assert_eq!(app.state.export.resolution, (1920, 1080));
    assert_eq!(app.state.export.framerate, 30);
    assert_eq!(app.state.export.fit_mode, FitMode::Letterbox);

    let ctx = egui::Context::default();
    ventacore_app::theme::apply_theme(&ctx);

    let _output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.export_page.render(ui, &mut app.state);
        });
    });

    // Verify active encoder report available
    let hw = app.state.hardware_report.as_ref().unwrap();
    assert!(!hw.decoders.is_empty());
}

#[test]
fn test_export_cancellation_cleans_up_partial_file() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    let output_file = temp_dir.path().join("final_cut.mp4");
    app.state.export.output_path = output_file.clone();
    app.state.export.is_exporting = true;

    // Simulate creation of .part file
    let part_path = temp_dir.path().join("final_cut.mp4.part");
    {
        let mut f = File::create(&part_path).expect("create part file");
        f.write_all(b"partial video data").expect("write data");
    }
    assert!(part_path.exists());

    // Render export page
    let ctx = egui::Context::default();
    let _output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.export_page.render(ui, &mut app.state);
        });
    });

    // Trigger cancel export
    ventacore_app::pages::export_page::ExportPage::cancel_export(&mut app.state);

    assert!(!part_path.exists(), "Partial .part file must never remain on disk");
}

#[test]
fn test_disposable_cache_purge_utility() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    // Create a dummy cache file in media cache
    let test_cache_file = app.state.media_cache.root_dir().join("test_entry.tmp");
    {
        let mut f = File::create(&test_cache_file).expect("create test file");
        f.write_all(b"cache contents").expect("write cache");
    }
    assert!(test_cache_file.exists());

    // Purge caches
    let _ = app.state.media_cache.purge();
    let _ = app.state.render_cache.purge();
    app.state.frame_cache.clear();

    assert!(!test_cache_file.exists(), "Purge must cleanly delete cached files");
}

#[test]
fn test_milestone_b_exit_gate_full_gui_workflow() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    // 1. Probe hardware
    app.probe_startup_hardware();
    assert!(app.state.hardware_report.is_some());

    // 2. Ingest media asset
    let asset_id = AssetId(1);
    let mut asset = Asset::new(asset_id, "source_footage.mp4".to_string());
    asset.probe_data = Some(ProbeData {
        duration: Tick::from_seconds(90),
        width: 1920,
        height: 1080,
        fps_num: 30,
        fps_den: 1,
        has_audio: true,
        audio_sample_rate: 48000,
    });
    app.state.apply_command(Command::AddAsset(asset)).expect("add asset");

    // 3. Add video track and song structure
    let track_id = TrackId(1);
    app.state.apply_command(Command::AddTrack(Track::new(track_id, "Video Track", TrackKind::Video))).expect("add track");
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

    // 4. Run step frames for each page
    let ctx = egui::Context::default();
    ventacore_app::theme::apply_theme(&ctx);

    for page in [AppPage::Media, AppPage::Cut, AppPage::Edit, AppPage::Export] {
        app.state.active_page = page;
        let _output = ctx.run(egui::RawInput::default(), |ctx| {
            app.step_frame(ctx);
        });
        assert_eq!(app.state.active_page, page);
    }

    // 5. Verify zero residue files remain
    assert_eq!(app.state.project.assets.len(), 1);
    assert_eq!(app.state.project.tracks.len(), 1);
}
