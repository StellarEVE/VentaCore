//! Transport bar widget for video playback monitors.
//!
//! Conforms to Part 4b & Stage R3:
//! - Play / Pause toggle with Spacebar shortcut.
//! - Step back 1 frame (left arrow / J key).
//! - Step forward 1 frame (right arrow / L key).
//! - SMPTE timecode display (HH:MM:SS:FF / HH:MM:SS:FF).
//! - Scrub slider with hover timecode preview.
//! - Loop playback toggle.
//! - Resolution tier selector (Full, Half, Quarter).

use eframe::egui;
use ventacore_core::time::Tick;
use ventacore_engine::playback::PlaybackEngine;
use ventacore_render::types::ResolutionTier;

use crate::theme::{ACCENT_TEAL, SURFACE, TEXT_MUTED, TEXT_PRIMARY};
use crate::widgets::timecode::format_smpte_timecode;

pub struct TransportBar;

impl TransportBar {
    /// Renders the transport controls below a monitor surface.
    pub fn show(ui: &mut egui::Ui, player: &mut PlaybackEngine) {
        let (fps_n, fps_d) = player.fps();
        let fps = fps_n as f64 / fps_d as f64;
        let duration = player.duration();
        let current_tick = player.current_tick();

        ui.vertical(|ui| {
            // 1. Scrub Slider with timecode hover
            let total_ticks = duration.raw().max(1) as f64;
            let mut scrub_pos = (current_tick.raw() as f64 / total_ticks).clamp(0.0, 1.0);

            ui.add_space(2.0);
            let slider_response = ui.add(
                egui::Slider::new(&mut scrub_pos, 0.0..=1.0)
                    .show_value(false)
                    .trailing_fill(true),
            );

            if slider_response.dragged() || slider_response.clicked() {
                let target_tick = Tick((scrub_pos * total_ticks) as i64);
                player.seek(target_tick);
            }

            if slider_response.hovered() {
                if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let rect = slider_response.rect;
                    if rect.width() > 0.0 {
                        let ratio = ((mouse_pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                        let hover_tick = Tick((ratio as f64 * total_ticks) as i64);
                        let hover_tc = format_smpte_timecode(hover_tick, fps);
                        slider_response.on_hover_text(format!("Scrub: {hover_tc}"));
                    }
                }
            }

            ui.add_space(4.0);

            // 2. Transport Buttons and Metadata
            ui.horizontal(|ui| {
                // Step Backward 1 Frame
                if ui.button("⏮").on_hover_text("Step Back 1 Frame (Left Arrow / J)").clicked() {
                    player.step_backward();
                }

                // Play / Pause Toggle
                let play_label = if player.is_playing() { "⏸" } else { "▶" };
                let play_tooltip = if player.is_playing() { "Pause (Space)" } else { "Play (Space)" };
                if ui.button(play_label).on_hover_text(play_tooltip).clicked() {
                    player.toggle_play();
                }

                // Step Forward 1 Frame
                if ui.button("⏭").on_hover_text("Step Forward 1 Frame (Right Arrow / L)").clicked() {
                    player.step_forward();
                }

                // Loop Toggle
                let loop_active = player.looping();
                let loop_btn = egui::Button::new("🔁").fill(if loop_active {
                    ACCENT_TEAL.linear_multiply(0.3)
                } else {
                    SURFACE
                });
                if ui.add(loop_btn).on_hover_text(if loop_active { "Looping: ON" } else { "Looping: OFF" }).clicked() {
                    player.set_looping(!loop_active);
                }

                ui.separator();

                // SMPTE Timecode Display
                let curr_tc = format_smpte_timecode(current_tick, fps);
                let dur_tc = format_smpte_timecode(duration, fps);
                ui.label(
                    egui::RichText::new(format!("{curr_tc} / {dur_tc}"))
                        .monospace()
                        .color(TEXT_PRIMARY)
                        .strong(),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Resolution Tier Dropdown
                    let current_tier = player.resolution_tier();
                    egui::ComboBox::from_id_source(ui.make_persistent_id("res_tier_combo"))
                        .selected_text(match current_tier {
                            ResolutionTier::Full => "Full 100%",
                            ResolutionTier::Half => "Half 50%",
                            ResolutionTier::Quarter => "Quarter 25%",
                            ResolutionTier::Eighth => "Eighth 12.5%",
                        })
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(current_tier == ResolutionTier::Full, "Full 100%").clicked() {
                                player.set_resolution_tier(ResolutionTier::Full);
                            }
                            if ui.selectable_label(current_tier == ResolutionTier::Half, "Half 50%").clicked() {
                                player.set_resolution_tier(ResolutionTier::Half);
                            }
                            if ui.selectable_label(current_tier == ResolutionTier::Quarter, "Quarter 25%").clicked() {
                                player.set_resolution_tier(ResolutionTier::Quarter);
                            }
                        });

                    ui.label(egui::RichText::new(format!("{:.2} fps", fps)).color(TEXT_MUTED).small());
                });
            });
        });
    }
}
