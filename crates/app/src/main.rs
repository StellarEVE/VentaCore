//! Layer 5: App binary for VentaCore.
//!
//! Native desktop GUI shell built with egui and eframe.

use std::path::PathBuf;

use eframe::egui;
use ventacore_app::app::VentaApp;
use ventacore_app::theme::apply_theme;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([1024.0, 600.0])
            .with_title("VentaCore Studio"),
        ..Default::default()
    };

    let cache_dir = dirs_cache_root();

    eframe::run_native(
        "VentaCore Studio",
        native_options,
        Box::new(|cc| {
            apply_theme(&cc.egui_ctx);
            let mut app = VentaApp::new(cache_dir);
            app.probe_startup_hardware();
            Ok(Box::new(app))
        }),
    )
}

fn dirs_cache_root() -> PathBuf {
    std::env::temp_dir().join("ventacore_cache")
}
