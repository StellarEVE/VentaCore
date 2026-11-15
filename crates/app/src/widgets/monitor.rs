//! Monitor widget encapsulating VideoSurface, TransportBar, and hotkeys.
//!
//! Conforms to Part 4b & Stage R3:
//! - Reusable for Source Monitor (clip auditioning) and Program Monitor (timeline cut).
//! - Handles Spacebar play/pause, J/K/L transport shortcuts, and arrow frame steps.
//! - Requests continuous repaints while playing at display rate.

use eframe::egui;
use ventacore_engine::playback::PlaybackEngine;

use crate::theme::{
    ACCENT_TEAL, BG_DEEP, BORDER, TEXT_MUTED, TEXT_PRIMARY, WARNING_AMBER,
};
use crate::widgets::transport_bar::TransportBar;
use crate::widgets::video_surface::VideoSurface;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorKind {
    Source,
    Program,
}

pub struct Monitor {
    pub surface: VideoSurface,
    pub kind: MonitorKind,
}

impl Monitor {
    pub fn new(kind: MonitorKind) -> Self {
        Self {
            surface: VideoSurface::new(),
            kind,
        }
    }

    pub fn source() -> Self {
        Self::new(MonitorKind::Source)
    }

    pub fn program() -> Self {
        Self::new(MonitorKind::Program)
    }

    /// Renders the complete monitor widget including header, video surface, and transport bar.
    pub fn show(&mut self, ui: &mut egui::Ui, player: &mut PlaybackEngine, title: &str) {
        // 1. Advance player clock and check for repaint
        let frame_updated = player.update();
        if player.is_playing() || frame_updated {
            ui.ctx().request_repaint();
        }

        // 2. Keyboard shortcuts handling
        if ui.input(|i| i.key_pressed(egui::Key::Space)) {
            player.toggle_play();
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::J)) {
            player.step_backward();
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::L)) {
            player.step_forward();
        }
        if ui.input(|i| i.key_pressed(egui::Key::K)) {
            player.pause();
        }

        // 3. Container Card
        egui::Frame::canvas(ui.style())
            .fill(BG_DEEP)
            .stroke(egui::Stroke::new(1.0_f32, BORDER))
            .inner_margin(egui::Margin::same(6.0))
            .show(ui, |ui| {
                // Header Bar
                ui.horizontal(|ui| {
                    let (badge_text, badge_color) = match self.kind {
                        MonitorKind::Source => ("SOURCE", ACCENT_TEAL),
                        MonitorKind::Program => ("PROGRAM", WARNING_AMBER),
                    };

                    ui.colored_label(badge_color, egui::RichText::new(badge_text).strong().small());
                    ui.label(egui::RichText::new(title).strong().color(TEXT_PRIMARY));

                    let (w, h) = player.native_dimensions();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.colored_label(TEXT_MUTED, format!("{w}x{h}"));
                    });
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // Video Surface
                // Allocate remaining vertical space minus transport bar height (~48px)
                let avail = ui.available_size();
                let surface_h = (avail.y - 48.0).max(120.0);
                let surface_size = egui::vec2(avail.x, surface_h);

                self.surface.show(ui, player.current_frame(), surface_size);

                ui.add_space(4.0);

                // Transport Controls
                TransportBar::show(ui, player);
            });
    }
}
