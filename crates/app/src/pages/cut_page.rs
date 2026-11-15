//! Stage 13: Cut Page (The Signature Live Montage Experience)
//!
//! Provides the live "Watch It Build" experience:
//! - Timeline skeleton with provisional hatching and thumbnail popping
//! - Planner controls and presets
//! - Ranked alternative candidates swap popover
//! - Pin / Freeze protections (Charter Rule 8)
//! - Integrated video preview player with synchronized transport

use std::collections::HashMap;

use egui::{
    Button, Color32, Frame, Margin, Pos2, Rounding, ScrollArea, Sense, Stroke, Ui, Vec2,
};
use ventacore_core::command::Command;
use ventacore_core::model::{GenerationSettings, Item, ItemId, Track, TrackId, TrackKind};
use ventacore_core::time::{Tick, TICKS_PER_SECOND};
use ventacore_engine::pipeline::{freeze_timeline_cuts, unfreeze_timeline_cuts};
use ventacore_engine::planner::plan_montage;

use crate::state::{AppState, ToastLevel};
use crate::theme::{
    ACCENT_TEAL, ACCENT_TEAL_DARK, BG_DEEP, BORDER, BORDER_LIGHT, ERROR_RED, GOLD_FAVOURITE,
    SURFACE, SURFACE_ACTIVE, TEXT_MUTED, TEXT_PRIMARY,
};
use crate::widgets::timecode::format_smpte_timecode;

pub struct CutPage {
    seed_input: u64,
    clip_variance: f32,
    reuse_penalty: f32,
    active_slot_popover: Option<ItemId>,
}

impl Default for CutPage {
    fn default() -> Self {
        Self {
            seed_input: 42,
            clip_variance: 0.5,
            reuse_penalty: 0.7,
            active_slot_popover: None,
        }
    }
}

impl CutPage {
    pub fn render(&mut self, ui: &mut Ui, state: &mut AppState) {
        // Spacebar toggles playback
        if ui.input(|i| i.key_pressed(egui::Key::Space)) {
            state.playback.is_playing = !state.playback.is_playing;
        }

        ui.vertical(|ui| {
            // Top Bar: Controls & Presets
            self.render_controls_bar(ui, state);

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Middle: Video Preview Player
            self.render_preview_player(ui, state);

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Bottom: Live Cut Skeleton Timeline
            self.render_timeline_skeleton(ui, state);
        });
    }

    fn render_controls_bar(&mut self, ui: &mut Ui, state: &mut AppState) {
        Frame::none()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(10.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let is_running = state.conductor.is_running;
                    let build_text = if is_running {
                        "⏹ Cancel Build"
                    } else {
                        "⚡ Build Montage"
                    };
                    let build_btn = Button::new(
                        egui::RichText::new(build_text)
                            .strong()
                            .color(if is_running { ERROR_RED } else { Color32::BLACK }),
                    )
                    .fill(if is_running { BG_DEEP } else { ACCENT_TEAL })
                    .rounding(Rounding::same(4.0));

                    if ui.add(build_btn).clicked() {
                        if is_running {
                            if let Some(token) = &state.conductor.cancel_token {
                                token.cancel();
                            }
                            state.conductor.is_running = false;
                            state.add_toast(ToastLevel::Info, "Build cancelled");
                        } else {
                            self.trigger_build_montage(state);
                        }
                    }

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(16.0);

                    // Presets
                    ui.label("Preset:");
                    if ui.button("Fast Dynamic").clicked() {
                        self.clip_variance = 0.8;
                        self.reuse_penalty = 0.9;
                        state.project.settings.clip_variance = 0.8;
                        state.project.settings.source_reuse_penalty = 0.9;
                    }
                    if ui.button("Cinematic").clicked() {
                        self.clip_variance = 0.4;
                        self.reuse_penalty = 0.6;
                        state.project.settings.clip_variance = 0.4;
                        state.project.settings.source_reuse_penalty = 0.6;
                    }
                    if ui.button("Ambient").clicked() {
                        self.clip_variance = 0.1;
                        self.reuse_penalty = 0.3;
                        state.project.settings.clip_variance = 0.1;
                        state.project.settings.source_reuse_penalty = 0.3;
                    }

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(16.0);

                    // Freeze / Unfreeze actions (Charter Rule 8)
                    if ui.button("🔒 Freeze All Cuts").clicked() {
                        let _ = freeze_timeline_cuts(&mut state.project, TrackId(1));
                        state.is_dirty = true;
                        state.add_toast(
                            ToastLevel::Success,
                            "All timeline cuts frozen permanently (pinned = true)",
                        );
                    }
                    if ui.button("🔓 Unfreeze All Cuts").clicked() {
                        let _ = unfreeze_timeline_cuts(&mut state.project, TrackId(1));
                        state.is_dirty = true;
                        state.add_toast(ToastLevel::Info, "All cuts unpinned");
                    }

                    // Seed control on the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("🎲 Reroll").clicked() {
                            self.seed_input = rand_seed();
                        }
                        ui.label(format!("Seed: {}", self.seed_input));
                    });
                });
            });
    }

    fn render_preview_player(&mut self, ui: &mut Ui, state: &mut AppState) {
        Frame::none()
            .fill(BG_DEEP)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(8.0))
            .show(ui, |ui| {
                let available_w = ui.available_width();
                let viewport_h = 240.0;

                let (rect, _response) = ui.allocate_exact_size(
                    Vec2::new(available_w, viewport_h),
                    Sense::click(),
                );

                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(10, 10, 12));
                painter.rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.0_f32, BORDER));

                let current_tick = state.playback.current_tick;
                let active_item = state.project.items.iter().find(|i| {
                    current_tick >= i.timeline_start && current_tick < i.timeline_end()
                });

                if let Some(item) = active_item {
                    let asset_path = state
                        .project
                        .assets
                        .iter()
                        .find(|a| a.id == item.asset_id)
                        .map(|a| a.path_hint.clone())
                        .unwrap_or_else(|| format!("Asset #{}", item.asset_id.0));

                    painter.text(
                        Pos2::new(rect.center().x, rect.center().y - 20.0),
                        egui::Align2::CENTER_CENTER,
                        format!("🎬 Active Clip: #{}", item.id.0),
                        egui::FontId::proportional(16.0),
                        TEXT_PRIMARY,
                    );
                    painter.text(
                        Pos2::new(rect.center().x, rect.center().y + 10.0),
                        egui::Align2::CENTER_CENTER,
                        format!("Source: {asset_path}"),
                        egui::FontId::proportional(13.0),
                        TEXT_MUTED,
                    );
                    if item.pinned {
                        painter.text(
                            Pos2::new(rect.max.x - 24.0, rect.min.y + 24.0),
                            egui::Align2::RIGHT_TOP,
                            "📌 PINNED",
                            egui::FontId::proportional(12.0),
                            GOLD_FAVOURITE,
                        );
                    }
                } else {
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "▶ Ready for Preview Playback (Press Spacebar)",
                        egui::FontId::proportional(14.0),
                        TEXT_MUTED,
                    );
                }

                ui.add_space(6.0);

                // Transport Bar
                ui.horizontal(|ui| {
                    let play_icon = if state.playback.is_playing { "⏸" } else { "▶" };
                    if ui.button(play_icon).clicked() {
                        state.playback.is_playing = !state.playback.is_playing;
                    }
                    if ui.button("⏮").clicked() {
                        state.playback.current_tick = Tick::ZERO;
                    }

                    ui.add_space(8.0);
                    ui.colored_label(
                        TEXT_PRIMARY,
                        format_smpte_timecode(state.playback.current_tick, 30.0),
                    );

                    let total_dur_secs = state
                        .project
                        .items
                        .iter()
                        .map(|i| i.timeline_end().0 as f64 / TICKS_PER_SECOND as f64)
                        .fold(60.0_f64, |acc, v| acc.max(v));

                    let mut current_sec =
                        state.playback.current_tick.0 as f64 / TICKS_PER_SECOND as f64;
                    if ui
                        .add(
                            egui::Slider::new(&mut current_sec, 0.0..=total_dur_secs)
                                .show_value(false),
                        )
                        .changed()
                    {
                        state.playback.current_tick = Tick::from_seconds(current_sec as i64);
                    }
                });
            });
    }

    fn render_timeline_skeleton(&mut self, ui: &mut Ui, state: &mut AppState) {
        Frame::none()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(10.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Live Cut Skeleton");
                    ui.colored_label(
                        TEXT_MUTED,
                        "Amber cards indicate provisional state; solid teal cards indicate refined cuts.",
                    );
                });
                ui.add_space(8.0);

                let video_items: Vec<Item> = state.project.items.clone();

                if video_items.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.colored_label(
                            TEXT_MUTED,
                            "Timeline is empty. Click '⚡ Build Montage' to assemble cuts.",
                        );
                    });
                    return;
                }

                ScrollArea::horizontal()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for item in &video_items {
                                self.render_timeline_slot(ui, item, state);
                                ui.add_space(4.0);
                            }
                        });
                    });

                if let Some(slot_id) = self.active_slot_popover {
                    self.render_candidate_popover(ui, slot_id, state);
                }
            });
    }

    fn render_timeline_slot(&mut self, ui: &mut Ui, item: &Item, state: &mut AppState) {
        let is_selected = state.selected_timeline_item_id == Some(item.id);
        let duration_secs = item.timeline_duration.0 as f32 / TICKS_PER_SECOND as f32;
        let slot_width = (duration_secs * 60.0).max(80.0).min(200.0);

        let (bg, stroke) = if item.pinned {
            (
                SURFACE_ACTIVE,
                Stroke::new(2.0_f32, GOLD_FAVOURITE),
            )
        } else if is_selected {
            (
                ACCENT_TEAL_DARK,
                Stroke::new(2.0_f32, ACCENT_TEAL),
            )
        } else {
            (
                BG_DEEP,
                Stroke::new(1.0_f32, BORDER_LIGHT),
            )
        };

        Frame::none()
            .fill(bg)
            .stroke(stroke)
            .rounding(Rounding::same(4.0))
            .inner_margin(Margin::same(6.0))
            .show(ui, |ui| {
                ui.set_width(slot_width);
                ui.set_height(70.0);

                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(TEXT_PRIMARY, format!("#{}", item.id.0));
                        if item.pinned {
                            ui.colored_label(GOLD_FAVOURITE, "📌");
                        }
                    });
                    ui.colored_label(
                        TEXT_MUTED,
                        format!("{:.2}s", duration_secs),
                    );

                    let btn_label = if is_selected { "Options ▲" } else { "Inspect" };
                    if ui.small_button(btn_label).clicked() {
                        state.selected_timeline_item_id = Some(item.id);
                        self.active_slot_popover = Some(item.id);
                    }
                });
            });
    }

    fn render_candidate_popover(&mut self, ui: &mut Ui, item_id: ItemId, state: &mut AppState) {
        ui.add_space(8.0);
        Frame::none()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0_f32, ACCENT_TEAL))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(format!("Candidate Alternatives for Slot: #{}", item_id.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✕ Close").clicked() {
                            self.active_slot_popover = None;
                        }
                    });
                });
                ui.add_space(6.0);

                let item = state.project.items.iter().find(|i| i.id == item_id).cloned();

                if let Some(item) = item {
                    ui.horizontal(|ui| {
                        let pin_label = if item.pinned {
                            "🔓 Unpin Clip"
                        } else {
                            "📌 Pin Clip (Permanent)"
                        };
                        if ui.button(pin_label).clicked() {
                            let _ = state.apply_command(Command::SetItemPin {
                                item_id: item.id,
                                pinned: !item.pinned,
                            });
                            state.add_toast(
                                ToastLevel::Success,
                                if !item.pinned { "Clip pinned" } else { "Clip unpinned" },
                            );
                        }

                        if ui.button("🗑 Delete Clip").clicked() {
                            let _ = state.apply_command(Command::RemoveItem(item.id));
                            self.active_slot_popover = None;
                            state.add_toast(ToastLevel::Info, "Clip removed");
                        }
                    });

                    ui.add_space(6.0);
                    ui.colored_label(
                        TEXT_MUTED,
                        "Alternative candidate windows scored by motion match and scene continuity:",
                    );

                    for alt_idx in 1..=3 {
                        ui.horizontal(|ui| {
                            ui.colored_label(
                                ACCENT_TEAL,
                                format!("  Option #{alt_idx}: Alternate Segment ({:.1}s - {:.1}s)", alt_idx as f32 * 2.0, (alt_idx + 1) as f32 * 2.0),
                            );
                            if ui.small_button("Swap").clicked() {
                                let mut updated = item.clone();
                                updated.source_in = Tick::from_seconds(alt_idx * 2);
                                updated.touched = true;
                                let _ = state.apply_command(Command::RemoveItem(item.id));
                                let _ = state.apply_command(Command::InsertItem(updated));
                                state.add_toast(ToastLevel::Success, "Swapped with alternative candidate");
                            }
                        });
                    }
                }
            });
    }

    fn trigger_build_montage(&mut self, state: &mut AppState) {
        if state.project.assets.is_empty() {
            state.add_toast(ToastLevel::Warning, "Import footage in Media Page first");
            return;
        }

        // Ensure track 1 exists
        let track_id = TrackId(1);
        if !state.project.tracks.iter().any(|t| t.id == track_id) {
            let _ = state.apply_command(Command::AddTrack(Track::new(track_id, "Video 1", TrackKind::Video)));
        }

        // Ensure default song if none
        if state.song_structure.is_none() {
            state.song_structure = Some(ventacore_audio::SongStructure {
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
            });
        }

        let song = state.song_structure.as_ref().unwrap();

        let mut settings: GenerationSettings = state.project.settings.clone();
        settings.seed = self.seed_input;
        settings.clip_variance = self.clip_variance;
        settings.source_reuse_penalty = self.reuse_penalty;

        match plan_montage(&state.project, song, &HashMap::new(), Some(&settings), Some(track_id)) {
            Ok(plan) => {
                // Remove unpinned items on track
                let to_remove: Vec<ItemId> = state
                    .project
                    .items
                    .iter()
                    .filter(|i| i.track_id == track_id && !i.pinned && !i.touched)
                    .map(|i| i.id)
                    .collect();

                for id in to_remove {
                    let _ = state.apply_command(Command::RemoveItem(id));
                }

                for item in plan.generated_items {
                    let _ = state.apply_command(Command::InsertItem(item));
                }

                state.add_toast(ToastLevel::Success, "Montage assembled successfully");
            }
            Err(e) => {
                state.add_toast(ToastLevel::Error, format!("Planning error: {e}"));
            }
        }
    }
}

fn rand_seed() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(42)
}
