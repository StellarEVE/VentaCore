//! Master Application Container: VentaApp
//!
//! Implements eframe::App and coordinates the page routers, footer,
//! toasts, crash recovery prompt, and background tasks.

use std::path::PathBuf;
use std::time::Instant;

use egui::{CentralPanel, Context, Frame, Margin, Rounding, Stroke, TopBottomPanel};
use ventacore_core::time::Tick;
use ventacore_platform::capabilities::probe_hardware_capabilities;

use crate::pages::cut_page::CutPage;
use crate::pages::edit_page::EditPage;
use crate::pages::export_page::ExportPage;
use crate::pages::media_page::MediaPage;
use crate::state::{AppPage, AppState};
use crate::theme::{SURFACE, WARNING_AMBER};
use crate::widgets::footer::render_footer;
use crate::widgets::settings_dialog::render_settings_dialog;
use crate::widgets::toast::render_toasts;

pub struct VentaApp {
    pub state: AppState,
    pub media_page: MediaPage,
    pub cut_page: CutPage,
    pub edit_page: EditPage,
    pub export_page: ExportPage,
    last_frame_instant: Instant,
    probed_hardware: bool,
}

impl VentaApp {
    pub fn new(cache_root: PathBuf) -> Self {
        Self {
            state: AppState::new(cache_root),
            media_page: MediaPage::default(),
            cut_page: CutPage::default(),
            edit_page: EditPage::default(),
            export_page: ExportPage::default(),
            last_frame_instant: Instant::now(),
            probed_hardware: false,
        }
    }

    /// Perform startup hardware probe (Charter Rule 6).
    pub fn probe_startup_hardware(&mut self) {
        if !self.probed_hardware {
            let hw = probe_hardware_capabilities();
            self.state.hardware_report = Some(hw);
            self.probed_hardware = true;
        }
    }

    /// Handle one logical simulation / render frame.
    pub fn step_frame(&mut self, ctx: &Context) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_instant).as_secs_f64();
        self.last_frame_instant = now;

        // Advance playback if active
        if self.state.playback.is_playing {
            let delta_ticks = (dt * 254_016_000_000.0 * self.state.playback.playback_speed) as i64;
            self.state.playback.current_tick = Tick(self.state.playback.current_tick.0 + delta_ticks);
            ctx.request_repaint();
        }

        // Check periodic autosave
        self.state.autosave_heartbeat();

        // Footer Navigation Bar
        TopBottomPanel::bottom("app_footer")
            .resizable(false)
            .show(ctx, |ui| {
                render_footer(ui, &mut self.state);
            });

        // Central Page Routing
        CentralPanel::default().show(ctx, |ui| {
            // Stage 17: Autosave Crash Recovery Prompt Banner
            if self.state.recovery_path.is_some() {
                Frame::none()
                    .fill(SURFACE)
                    .stroke(Stroke::new(1.0_f32, WARNING_AMBER))
                    .rounding(Rounding::same(4.0))
                    .inner_margin(Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.colored_label(
                                WARNING_AMBER,
                                "⚠ An unsaved session was recovered from crash.",
                            );
                            if ui.button("⚡ Restore Recovered Session").clicked() {
                                let _ = self.state.restore_recovered_autosave();
                            }
                            if ui.button("✕ Dismiss").clicked() {
                                self.state.recovery_path = None;
                            }
                        });
                    });
                ui.add_space(4.0);
            }

            match self.state.active_page {
                AppPage::Media => self.media_page.render(ui, &mut self.state),
                AppPage::Cut => self.cut_page.render(ui, &mut self.state),
                AppPage::Edit => self.edit_page.render(ui, &mut self.state),
                AppPage::Export => self.export_page.render(ui, &mut self.state),
            }
        });

        // Stage 17: Settings Preferences Modal
        let mut prefs_open = self.state.preferences_open;
        let mut prefs = self.state.preferences.clone();
        render_settings_dialog(ctx, &mut prefs_open, &mut prefs);
        self.state.preferences_open = prefs_open;
        self.state.preferences = prefs;

        // Floating Toast Stack
        render_toasts(ctx, &mut self.state);
    }
}

impl eframe::App for VentaApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.probe_startup_hardware();
        self.step_frame(ctx);
    }
}
