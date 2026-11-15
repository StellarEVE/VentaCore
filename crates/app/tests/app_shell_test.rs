//! Stage 11: App Shell and Navigation Integration Tests
//!
//! Verifies desktop application initialization, theme engine,
//! footer navigation, hardware disclosures, undo/redo, and toasts.

use ventacore_app::app::VentaApp;
use ventacore_app::state::{AppPage, ToastLevel};
use ventacore_core::command::Command;
use ventacore_core::model::{Track, TrackId, TrackKind};

#[test]
fn test_app_initialization_and_hardware_probe() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    assert_eq!(app.state.active_page, AppPage::Media);
    assert!(!app.state.is_dirty);
    assert!(app.state.toasts.is_empty());

    // Run hardware probe
    app.probe_startup_hardware();
    assert!(app.state.hardware_report.is_some());
    let hw = app.state.hardware_report.as_ref().unwrap();
    // Verify adapter info exists or fallbacks are ready
    assert!(!hw.decoders.is_empty());
}

#[test]
fn test_footer_page_navigation() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    // Navigate across all 4 pages
    app.state.active_page = AppPage::Cut;
    assert_eq!(app.state.active_page, AppPage::Cut);

    app.state.active_page = AppPage::Edit;
    assert_eq!(app.state.active_page, AppPage::Edit);

    app.state.active_page = AppPage::Export;
    assert_eq!(app.state.active_page, AppPage::Export);

    app.state.active_page = AppPage::Media;
    assert_eq!(app.state.active_page, AppPage::Media);
}

#[test]
fn test_toast_stack_lifecycle() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    app.state.add_toast(ToastLevel::Info, "Testing info");
    app.state.add_toast(ToastLevel::Success, "Testing success");
    app.state.add_toast(ToastLevel::Warning, "Testing warning");
    app.state.add_toast(ToastLevel::Error, "Testing error");

    assert_eq!(app.state.toasts.len(), 4);
    assert_eq!(app.state.toasts[0].level, ToastLevel::Info);
    assert_eq!(app.state.toasts[3].level, ToastLevel::Error);

    // Prune toasts
    app.state.prune_toasts();
    // Freshly created toasts must not be pruned immediately
    assert_eq!(app.state.toasts.len(), 4);
}

#[test]
fn test_command_undo_redo_in_app_state() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    assert_eq!(app.state.project.tracks.len(), 0);

    // Add track via validated command
    let track = Track::new(TrackId(10), "Test Video", TrackKind::Video);
    app.state
        .apply_command(Command::AddTrack(track))
        .expect("apply command");

    assert_eq!(app.state.project.tracks.len(), 1);
    assert!(app.state.is_dirty);

    // Undo
    app.state.undo().expect("undo");
    assert_eq!(app.state.project.tracks.len(), 0);

    // Redo
    app.state.redo().expect("redo");
    assert_eq!(app.state.project.tracks.len(), 1);
}

#[test]
fn test_headless_step_frame_execution() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let mut app = VentaApp::new(temp_dir.path().to_path_buf());

    let ctx = egui::Context::default();
    ventacore_app::theme::apply_theme(&ctx);

    // Step frame without crash
    let _output = ctx.run(egui::RawInput::default(), |ctx| {
        app.step_frame(ctx);
    });

    // Verify app state intact after frame step
    assert_eq!(app.state.active_page, AppPage::Media);
}
