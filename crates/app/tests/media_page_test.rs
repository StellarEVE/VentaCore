//! Stage 12: Media Page Integration Tests
//!
//! Verifies asset ingestion, trimming, exclude/favourite ranges,
//! song structure analysis, and manual beat-grid corrections.

use ventacore_app::app::VentaApp;
use ventacore_audio::corrections::{double_time, half_time, set_downbeat, shift_grid, tap_tempo};
use ventacore_audio::SongStructure;
use ventacore_core::command::Command;
use ventacore_core::model::{Asset, AssetId, ProbeData};
use ventacore_core::time::{Tick, TimeRange};

#[test]
fn test_media_asset_trims_and_ranges() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    let asset_id = AssetId(1);
    let mut asset = Asset::new(asset_id, "test_video.mp4".to_string());
    asset.probe_data = Some(ProbeData {
        duration: Tick::from_seconds(60),
        width: 1920,
        height: 1080,
        fps_num: 30,
        fps_den: 1,
        has_audio: true,
        audio_sample_rate: 48000,
    });

    app.state
        .apply_command(Command::AddAsset(asset))
        .expect("add asset");

    assert_eq!(app.state.project.assets.len(), 1);

    // Apply intro and outro trims
    let intro = Tick::from_seconds(2);
    let outro = Tick::from_seconds(5);
    app.state
        .apply_command(Command::SetAssetTrims {
            asset_id,
            intro,
            outro,
        })
        .expect("set trims");

    let saved_asset = app.state.project.assets.iter().find(|a| a.id == asset_id).unwrap();
    assert_eq!(saved_asset.intro_trim, intro);
    assert_eq!(saved_asset.outro_trim, outro);

    // Add exclude range
    let exclude = TimeRange::new(Tick::from_seconds(10), Tick::from_seconds(3));
    app.state
        .apply_command(Command::AddExcludeRange {
            asset_id,
            range: exclude,
        })
        .expect("add exclude");

    // Add favourite range
    let favourite = TimeRange::new(Tick::from_seconds(20), Tick::from_seconds(4));
    app.state
        .apply_command(Command::AddFavouriteMoment {
            asset_id,
            range: favourite,
        })
        .expect("add favourite");

    let updated_asset = app.state.project.assets.iter().find(|a| a.id == asset_id).unwrap();
    assert_eq!(updated_asset.exclude_ranges.len(), 1);
    assert_eq!(updated_asset.favourite_moments.len(), 1);
    assert_eq!(updated_asset.exclude_ranges[0], exclude);
    assert_eq!(updated_asset.favourite_moments[0], favourite);
}

#[test]
fn test_manual_song_grid_corrections_in_ui() {
    let song = SongStructure {
        bpm: 120.0,
        beats_per_bar: 4,
        bars_per_phrase: 4,
        grid_mode: ventacore_core::GridMode::Transient,
        beat_ticks: (0..120).map(|i| Tick::from_seconds(i / 2)).collect(),
        bar_ticks: (0..30).map(|i| Tick::from_seconds(i * 2)).collect(),
        phrase_ticks: vec![Tick::ZERO, Tick::from_seconds(16), Tick::from_seconds(32)],
        onsets: Vec::new(),
        change_points: Vec::new(),
        waveform_peaks: vec![0.5; 300],
        loudness_envelope: vec![0.5; 300],
        energy_per_second: vec![0.5; 60],
        energy_per_bar: vec![0.5; 30],
    };

    // Half time
    let halved = half_time(&song);
    assert_eq!(halved.bpm, 60.0);

    // Double time
    let doubled = double_time(&song);
    assert_eq!(doubled.bpm, 240.0);

    // Shift grid by 1 second
    let shifted = shift_grid(&song, Tick::from_seconds(1));
    assert_eq!(shifted.beat_ticks[0], Tick::from_seconds(1));

    // Tap tempo
    let tap_ticks = vec![
        Tick::ZERO,
        Tick::from_seconds(1),
        Tick::from_seconds(2),
        Tick::from_seconds(3),
    ];
    let calculated_bpm = tap_tempo(&tap_ticks);
    assert!(calculated_bpm.is_some());
    assert!((calculated_bpm.unwrap() - 60.0).abs() < 1.0);

    // Set downbeat
    let downbeated = set_downbeat(&song, Tick::from_seconds(4));
    assert!(!downbeated.beat_ticks.is_empty());
}

#[test]
fn test_media_page_render_headless() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    let ctx = egui::Context::default();
    ventacore_app::theme::apply_theme(&ctx);

    let _output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.media_page.render(ui, &mut app.state);
        });
    });

    // Ensure initial state clean
    assert_eq!(app.state.project.assets.len(), 0);
}
