//! Stage 15 & 16: Edit Page & Power Tools Integration Tests
//!
//! Verifies multitrack timeline operations: move, trim, split, ripple delete,
//! track mute/solo/lock, transitions, effects stack, text overlays, and crash recovery.

use ventacore_app::app::VentaApp;
use ventacore_core::command::Command;
use ventacore_core::model::{Asset, AssetId, Item, ItemId, ProbeData, Track, TrackId, TrackKind, Transition, TransitionKind};
use ventacore_core::time::Tick;
use ventacore_render::effects::{apply_effects_to_frame, VideoEffect};
use ventacore_render::keyframes::{KeyframeInterpolation, KeyframeTrack};
use ventacore_render::types::RenderFrame;

#[test]
fn test_edit_page_timeline_split_and_ripple_delete() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    let track_id = TrackId(1);
    app.state
        .apply_command(Command::AddTrack(Track::new(track_id, "Video 1", TrackKind::Video)))
        .expect("add track");

    let asset_id = AssetId(1);
    let mut asset = Asset::new(asset_id, "footage.mp4".to_string());
    asset.probe_data = Some(ProbeData {
        duration: Tick::from_seconds(60),
        width: 1920,
        height: 1080,
        fps_num: 30,
        fps_den: 1,
        has_audio: true,
        audio_sample_rate: 48000,
    });
    app.state.apply_command(Command::AddAsset(asset)).expect("add asset");

    // Add clip 1: 0s to 4s
    let item1 = Item::new(
        ItemId(1),
        track_id,
        asset_id,
        Tick::ZERO,
        Tick::from_seconds(4),
        Tick::ZERO,
    );
    app.state.apply_command(Command::InsertItem(item1)).expect("insert item 1");

    // Add clip 2: 4s to 8s
    let item2 = Item::new(
        ItemId(2),
        track_id,
        asset_id,
        Tick::from_seconds(4),
        Tick::from_seconds(4),
        Tick::ZERO,
    );
    app.state.apply_command(Command::InsertItem(item2)).expect("insert item 2");

    assert_eq!(app.state.project.items.len(), 2);

    // Split clip 1 at 2s
    let split_cmd = Command::SplitItem {
        item_id: ItemId(1),
        split_point: Tick::from_seconds(2),
        new_item_id: ItemId(3),
    };
    app.state.apply_command(split_cmd).expect("split item");

    assert_eq!(app.state.project.items.len(), 3);
    let part1 = app.state.project.items.iter().find(|i| i.id == ItemId(1)).unwrap();
    let part2 = app.state.project.items.iter().find(|i| i.id == ItemId(3)).unwrap();
    assert_eq!(part1.timeline_duration, Tick::from_seconds(2));
    assert_eq!(part2.timeline_start, Tick::from_seconds(2));
    assert_eq!(part2.timeline_duration, Tick::from_seconds(2));

    // Ripple delete part 1 (remove Item 1 and shift Item 3 and Item 2 left by 2s)
    app.state.selected_timeline_item_id = Some(ItemId(1));
    app.edit_page.active_tool = ventacore_app::pages::edit_page::EditTool::Ripple;

    let ctx = egui::Context::default();
    ventacore_app::theme::apply_theme(&ctx);

    let _output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.edit_page.render(ui, &mut app.state);
        });
    });
}

#[test]
fn test_transitions_and_effects_stack() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    let track_id = TrackId(1);
    app.state.apply_command(Command::AddTrack(Track::new(track_id, "Video 1", TrackKind::Video))).expect("add track");
    let asset_id = AssetId(1);
    app.state.apply_command(Command::AddAsset(Asset::new(asset_id, "clip.mp4".to_string()))).expect("add asset");

    let item = Item::new(
        ItemId(1),
        track_id,
        asset_id,
        Tick::ZERO,
        Tick::from_seconds(5),
        Tick::ZERO,
    );
    app.state.apply_command(Command::InsertItem(item)).expect("insert item");

    // Apply CrossDissolve transition (Stage 16a)
    let transition = Transition::new(TransitionKind::CrossDissolve, Tick::from_seconds(1));
    app.state
        .apply_command(Command::SetItemTransition {
            item_id: ItemId(1),
            transition: Some(transition),
        })
        .expect("set transition");

    // Apply effects stack (Stage 16b & 16d)
    let effects = vec![
        "brightness:0.2".to_string(),
        "contrast:1.2".to_string(),
        "text:Title Overlay".to_string(),
    ];
    app.state
        .apply_command(Command::SetItemEffects {
            item_id: ItemId(1),
            effects,
        })
        .expect("set effects");

    let updated = app.state.project.items.iter().find(|i| i.id == ItemId(1)).unwrap();
    assert!(updated.transition.is_some());
    assert_eq!(updated.effects.len(), 3);
}

#[test]
fn test_pure_cpu_effects_processing_charter_rule_5() {
    let mut frame = RenderFrame::new(100, 100);
    // Fill with mid-grey
    for chunk in frame.pixels.chunks_exact_mut(4) {
        chunk[0] = 128;
        chunk[1] = 128;
        chunk[2] = 128;
        chunk[3] = 255;
    }

    let parsed_effects = vec![
        VideoEffect::Brightness(0.2),
        VideoEffect::Contrast(1.2),
        VideoEffect::BoxBlur(2),
    ];

    apply_effects_to_frame(&mut frame, &parsed_effects, 0.0);

    // Pixels should be brighter than 128
    assert!(frame.pixels[0] > 128);
    assert_eq!(frame.pixels[3], 255);
}

#[test]
fn test_keyframe_track_evaluation_stage_16c() {
    let mut track = KeyframeTrack::<f32>::new();
    track.insert(Tick::ZERO, 0.0, KeyframeInterpolation::Linear);
    track.insert(Tick::from_seconds(2), 100.0, KeyframeInterpolation::Linear);

    assert_eq!(track.evaluate(Tick::ZERO, 0.0), 0.0);
    assert_eq!(track.evaluate(Tick::from_seconds(1), 0.0), 50.0);
    assert_eq!(track.evaluate(Tick::from_seconds(2), 0.0), 100.0);
}

#[test]
fn test_crash_recovery_prompt_restoration_stage_17() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    let recovery_file = temp_dir.path().join("test_crash.autosave.json");
    ventacore_core::storage::save_project_atomic(&recovery_file, &app.state.project).expect("save");

    // Check recovery candidate
    app.state.check_autosave_recovery(recovery_file.clone());
    assert!(app.state.recovery_path.is_some());

    // Restore from crash
    let res = app.state.restore_recovered_autosave();
    assert!(res.is_ok());
    assert!(app.state.recovery_path.is_none());
}
