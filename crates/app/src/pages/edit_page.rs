//! Stage 15 Preview: Edit Page (Multi-Track Timeline Overview)
//!
//! Provides a track-by-track overview of video and audio items,
//! mute/solo/lock flags, and volume levels.
//! Full interactive multi-track editing tools (ripple, trim, split) arrive in Milestone C.

use egui::{Color32, Frame, Margin, Rounding, ScrollArea, Stroke, Ui};
use ventacore_core::model::{Item, Track, TrackKind};

use crate::state::AppState;
use crate::theme::{
    ACCENT_TEAL, ACCENT_TEAL_DARK, BG_DEEP, BORDER, GOLD_FAVOURITE, SURFACE, TEXT_MUTED,
    TEXT_PRIMARY, WARNING_AMBER,
};
use crate::widgets::timecode::format_time_minutes_seconds;

#[derive(Default)]
pub struct EditPage;

impl EditPage {
    pub fn render(&mut self, ui: &mut Ui, state: &mut AppState) {
        ui.vertical(|ui| {
            // Milestone C Banner
            Frame::none()
                .fill(SURFACE)
                .stroke(Stroke::new(1.0_f32, ACCENT_TEAL))
                .rounding(Rounding::same(6.0))
                .inner_margin(Margin::same(12.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("🎬 Edit Page (Timeline Overview)");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.colored_label(
                                ACCENT_TEAL,
                                "Milestone C: Interactive Multi-Track Editor arrives in Stage 15",
                            );
                        });
                    });
                    ui.colored_label(
                        TEXT_MUTED,
                        "Inspect track channels, placed clips, track volumes, and locks below.",
                    );
                });

            ui.add_space(8.0);

            // Multitrack Overview
            Frame::none()
                .fill(BG_DEEP)
                .stroke(Stroke::new(1.0_f32, BORDER))
                .rounding(Rounding::same(6.0))
                .inner_margin(Margin::same(12.0))
                .show(ui, |ui| {
                    if state.project.tracks.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.colored_label(
                                TEXT_MUTED,
                                "No tracks present in current project. Build a montage in the Cut Page first.",
                            );
                        });
                        return;
                    }

                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let tracks = state.project.tracks.clone();
                            let items = state.project.items.clone();
                            for track in tracks {
                                let track_items: Vec<Item> = items
                                    .iter()
                                    .filter(|i| i.track_id == track.id)
                                    .cloned()
                                    .collect();
                                self.render_track_lane(ui, &track, &track_items);
                                ui.add_space(8.0);
                            }
                        });
                });
        });
    }

    fn render_track_lane(&self, ui: &mut Ui, track: &Track, items: &[Item]) {
        let (icon, label) = match track.kind {
            TrackKind::Video => ("📹", "Video Track"),
            TrackKind::Audio => ("🔊", "Audio Track"),
        };

        Frame::none()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .rounding(Rounding::same(4.0))
            .inner_margin(Margin::same(8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(icon);
                    ui.colored_label(TEXT_PRIMARY, format!("{} ({label} #{})", track.name, track.id.0));

                    ui.add_space(16.0);
                    if track.muted {
                        ui.colored_label(WARNING_AMBER, "[MUTED]");
                    }
                    if track.solo {
                        ui.colored_label(ACCENT_TEAL, "[SOLO]");
                    }
                    if track.locked {
                        ui.colored_label(GOLD_FAVOURITE, "[LOCKED]");
                    }

                    ui.colored_label(
                        TEXT_MUTED,
                        format!("Items: {}", items.len()),
                    );
                });

                ui.add_space(4.0);

                // Horizontal strip showing items on this track
                ScrollArea::horizontal()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for item in items {
                                let item_dur = format_time_minutes_seconds(item.timeline_duration);
                                Frame::none()
                                    .fill(ACCENT_TEAL_DARK)
                                    .stroke(Stroke::new(1.0_f32, ACCENT_TEAL))
                                    .rounding(Rounding::same(3.0))
                                    .inner_margin(Margin::same(4.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.colored_label(Color32::WHITE, format!("#{}", item.id.0));
                                            ui.colored_label(TEXT_MUTED, item_dur);
                                            if item.pinned {
                                                ui.colored_label(GOLD_FAVOURITE, "📌");
                                            }
                                        });
                                    });
                                ui.add_space(4.0);
                            }
                        });
                    });
            });
    }
}
