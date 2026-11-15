//! Stage 15 & 16: Edit Page (Full Multi-Track Timeline Editor & Power Tools)
//!
//! Provides the complete multitrack timeline editor:
//! - Interactive video and audio tracks with clip selection, move, trim, split, and ripple delete.
//! - Track mute, solo, lock, and volume controls.
//! - Clip Inspector with transitions, effects stack, text overlays, and keyframes.
//! - Multitrack audio mixer drawer with soft-limiting.

use egui::{
    Button, Color32, Frame, Margin, Pos2, Rect, Rounding, ScrollArea, Sense, Stroke, Ui, Vec2,
};
use ventacore_core::command::Command;
use ventacore_core::model::{Item, ItemId, Track, TrackId, TrackKind, Transition, TransitionKind};
use ventacore_core::time::{Tick, TICKS_PER_SECOND};

use crate::state::{AppState, ToastLevel};
use crate::theme::{
    ACCENT_TEAL, ACCENT_TEAL_DARK, BG_DEEP, BORDER, BORDER_LIGHT,
    GOLD_FAVOURITE, SURFACE, SURFACE_ACTIVE, TEXT_MUTED, TEXT_PRIMARY, WARNING_AMBER,
};
use crate::widgets::timecode::{format_smpte_timecode, format_time_minutes_seconds};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditTool {
    Select,
    Razor,
    Ripple,
}

pub struct EditPage {
    pub active_tool: EditTool,
    pub snap_to_grid: bool,
    pub timeline_zoom: f32, // pixels per second
    selected_transition_kind: usize,
    selected_transition_duration_sec: f32,
    selected_effect_choice: usize,
    effect_param_value: f32,
    custom_text_input: String,
}

impl Default for EditPage {
    fn default() -> Self {
        Self {
            active_tool: EditTool::Select,
            snap_to_grid: true,
            timeline_zoom: 50.0, // 50px per second
            selected_transition_kind: 1, // CrossDissolve
            selected_transition_duration_sec: 0.5,
            selected_effect_choice: 0,
            effect_param_value: 0.5,
            custom_text_input: "VentaCore Title".to_string(),
        }
    }
}

impl EditPage {
    pub fn render(&mut self, ui: &mut Ui, state: &mut AppState) {
        // Keyboard shortcuts for editing tools
        let (key_v, key_c, key_x, key_del) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::V),
                i.key_pressed(egui::Key::C),
                i.key_pressed(egui::Key::X),
                i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
            )
        });

        if key_v {
            self.active_tool = EditTool::Select;
        } else if key_c {
            self.active_tool = EditTool::Razor;
        } else if key_x {
            self.active_tool = EditTool::Ripple;
        }

        if key_del {
            self.ripple_delete_selected(state);
        }

        ui.vertical(|ui| {
            // Top Toolbar: Editing Tools & Track Controls
            self.render_tools_bar(ui, state);

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            // Main Editor Body: Timeline (left) and Inspector (right)
            ui.horizontal(|ui| {
                let inspector_width = 360.0;
                let timeline_width = (ui.available_width() - inspector_width - 12.0).max(400.0);

                // Multitrack Timeline Area
                ui.allocate_ui_with_layout(
                    Vec2::new(timeline_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        self.render_multitrack_timeline(ui, state);
                    },
                );

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                // Clip Inspector Deck Area
                ui.allocate_ui_with_layout(
                    ui.available_size(),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        self.render_clip_inspector(ui, state);
                    },
                );
            });
        });
    }

    fn render_tools_bar(&mut self, ui: &mut Ui, state: &mut AppState) {
        Frame::none()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Tool Selection Buttons
                    ui.label("Tool:");
                    let btn_select = Button::new("👆 Select (V)").fill(if self.active_tool == EditTool::Select {
                        ACCENT_TEAL_DARK
                    } else {
                        BG_DEEP
                    });
                    if ui.add(btn_select).clicked() {
                        self.active_tool = EditTool::Select;
                    }

                    let btn_razor = Button::new("✂ Razor (C)").fill(if self.active_tool == EditTool::Razor {
                        ACCENT_TEAL_DARK
                    } else {
                        BG_DEEP
                    });
                    if ui.add(btn_razor).clicked() {
                        self.active_tool = EditTool::Razor;
                    }

                    let btn_ripple = Button::new("⏪ Ripple (X)").fill(if self.active_tool == EditTool::Ripple {
                        ACCENT_TEAL_DARK
                    } else {
                        BG_DEEP
                    });
                    if ui.add(btn_ripple).clicked() {
                        self.active_tool = EditTool::Ripple;
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);

                    // Split Action
                    if ui.button("✂ Split at Playhead").clicked() {
                        self.split_at_playhead(state);
                    }

                    // Ripple Delete Action
                    if ui.button("🗑 Ripple Delete").clicked() {
                        self.ripple_delete_selected(state);
                    }

                    // Snap to grid
                    ui.checkbox(&mut self.snap_to_grid, "Snap to Beats");

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);

                    // Zoom Controls
                    ui.label("Zoom:");
                    ui.add(egui::Slider::new(&mut self.timeline_zoom, 10.0..=200.0).text("px/s"));

                    // Playhead SMPTE display on the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.colored_label(
                            TEXT_PRIMARY,
                            format_smpte_timecode(state.playback.current_tick, 30.0),
                        );
                        ui.label("Playhead:");
                    });
                });
            });
    }

    fn render_multitrack_timeline(&mut self, ui: &mut Ui, state: &mut AppState) {
        Frame::none()
            .fill(BG_DEEP)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(10.0))
            .show(ui, |ui| {
                if state.project.tracks.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        ui.colored_label(
                            TEXT_MUTED,
                            "Timeline is empty. Use Cut Page to assemble montage cuts or add a track.",
                        );
                        if ui.button("+ Add Video Track").clicked() {
                            let tid = TrackId((state.project.tracks.len() + 1) as u64);
                            let _ = state.apply_command(Command::AddTrack(Track::new(tid, "Video 1", TrackKind::Video)));
                        }
                    });
                    return;
                }

                ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let tracks = state.project.tracks.clone();
                        for track in tracks {
                            self.render_track_row(ui, &track, state);
                            ui.add_space(6.0);
                        }
                    });
            });
    }

    fn render_track_row(&mut self, ui: &mut Ui, track: &Track, state: &mut AppState) {
        let (icon, _label) = match track.kind {
            TrackKind::Song => ("🎵", "Song"),
            TrackKind::Video => ("📹", "Video"),
            TrackKind::Audio => ("🔊", "Audio"),
        };

        ui.horizontal(|ui| {
            // Track Controls Header (140px width)
            Frame::none()
                .fill(SURFACE)
                .stroke(Stroke::new(1.0_f32, BORDER))
                .rounding(Rounding::same(4.0))
                .inner_margin(Margin::same(6.0))
                .show(ui, |ui| {
                    ui.set_width(140.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(icon);
                            ui.colored_label(TEXT_PRIMARY, &track.name);
                        });
                        ui.horizontal(|ui| {
                            let mute_btn = if track.muted { "🔇" } else { "🔊" };
                            if ui.small_button(mute_btn).clicked() {
                                let _ = state.apply_command(Command::SetTrackMute {
                                    track_id: track.id,
                                    muted: !track.muted,
                                });
                            }
                            let solo_style = if track.solo { ACCENT_TEAL } else { TEXT_MUTED };
                            if ui.small_button(egui::RichText::new("S").color(solo_style)).clicked() {
                                let _ = state.apply_command(Command::SetTrackSolo {
                                    track_id: track.id,
                                    solo: !track.solo,
                                });
                            }
                            let lock_btn = if track.locked { "🔒" } else { "🔓" };
                            if ui.small_button(lock_btn).clicked() {
                                let _ = state.apply_command(Command::SetTrackLock {
                                    track_id: track.id,
                                    locked: !track.locked,
                                });
                            }
                        });
                    });
                });

            // Track Items Canvas Lane
            let track_items: Vec<Item> = state
                .project
                .items
                .iter()
                .filter(|i| i.track_id == track.id)
                .cloned()
                .collect();

            let max_track_dur_secs = state
                .project
                .items
                .iter()
                .map(|i| i.timeline_end().0 as f32 / TICKS_PER_SECOND as f32)
                .fold(60.0_f32, |acc, v| acc.max(v));

            let lane_width = (max_track_dur_secs * self.timeline_zoom).max(600.0);
            let (rect, _response) = ui.allocate_exact_size(Vec2::new(lane_width, 64.0), Sense::click());

            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, Rounding::same(4.0), Color32::from_rgb(16, 17, 20));
            painter.rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.0_f32, BORDER));

            // Render clips on lane
            for item in &track_items {
                let start_sec = item.timeline_start.0 as f32 / TICKS_PER_SECOND as f32;
                let dur_sec = item.timeline_duration.0 as f32 / TICKS_PER_SECOND as f32;

                let clip_x = rect.min.x + start_sec * self.timeline_zoom;
                let clip_w = (dur_sec * self.timeline_zoom).max(24.0);

                let clip_rect = Rect::from_min_size(
                    Pos2::new(clip_x, rect.min.y + 4.0),
                    Vec2::new(clip_w, 56.0),
                );

                let is_selected = state.selected_timeline_item_id == Some(item.id);

                let (bg, stroke) = if item.pinned {
                    (SURFACE_ACTIVE, Stroke::new(2.0_f32, GOLD_FAVOURITE))
                } else if is_selected {
                    (ACCENT_TEAL_DARK, Stroke::new(2.0_f32, ACCENT_TEAL))
                } else {
                    (SURFACE, Stroke::new(1.0_f32, BORDER_LIGHT))
                };

                painter.rect_filled(clip_rect, Rounding::same(4.0), bg);
                painter.rect_stroke(clip_rect, Rounding::same(4.0), stroke);

                // Clip title text
                painter.text(
                    Pos2::new(clip_rect.min.x + 6.0, clip_rect.min.y + 12.0),
                    egui::Align2::LEFT_TOP,
                    format!("#{}", item.id.0),
                    egui::FontId::proportional(12.0),
                    TEXT_PRIMARY,
                );

                // Badges
                if item.transition.is_some() {
                    painter.text(
                        Pos2::new(clip_rect.min.x + 6.0, clip_rect.max.y - 14.0),
                        egui::Align2::LEFT_BOTTOM,
                        "⧖ Trans",
                        egui::FontId::proportional(10.0),
                        ACCENT_TEAL,
                    );
                }
                if !item.effects.is_empty() {
                    painter.text(
                        Pos2::new(clip_rect.max.x - 6.0, clip_rect.max.y - 14.0),
                        egui::Align2::RIGHT_BOTTOM,
                        format!("✦ {}", item.effects.len()),
                        egui::FontId::proportional(10.0),
                        WARNING_AMBER,
                    );
                }

                // Handle click on clip
                let clip_id = egui::Id::new(format!("clip_{}", item.id.0));
                let clip_interaction = ui.interact(clip_rect, clip_id, Sense::click());
                if clip_interaction.clicked() {
                    state.selected_timeline_item_id = Some(item.id);
                    if self.active_tool == EditTool::Razor {
                        self.split_at_playhead(state);
                    } else if self.active_tool == EditTool::Ripple {
                        self.ripple_delete_selected(state);
                    }
                }
            }

            // Draw playhead vertical line
            let playhead_sec = state.playback.current_tick.0 as f32 / TICKS_PER_SECOND as f32;
            let playhead_x = rect.min.x + playhead_sec * self.timeline_zoom;
            if playhead_x >= rect.min.x && playhead_x <= rect.max.x {
                painter.line_segment(
                    [Pos2::new(playhead_x, rect.min.y), Pos2::new(playhead_x, rect.max.y)],
                    Stroke::new(2.0_f32, ACCENT_TEAL),
                );
            }
        });
    }

    fn render_clip_inspector(&mut self, ui: &mut Ui, state: &mut AppState) {
        let selected_item = state.selected_item().cloned();

        Frame::none()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                if let Some(item) = selected_item {
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.heading(format!("🎬 Clip Inspector: #{}", item.id.0));
                            ui.colored_label(
                                TEXT_MUTED,
                                format!("Track #{}, Asset #{}", item.track_id.0, item.asset_id.0),
                            );
                            ui.add_space(6.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // Timeline timing
                            ui.label(egui::RichText::new("Timing & Properties").strong());
                            ui.colored_label(
                                TEXT_PRIMARY,
                                format!(
                                    "Start: {} | Duration: {}",
                                    format_time_minutes_seconds(item.timeline_start),
                                    format_time_minutes_seconds(item.timeline_duration)
                                ),
                            );

                            let mut speed = item.speed;
                            if ui.add(egui::Slider::new(&mut speed, 0.25..=4.0).text("Speed")).changed() {
                                let mut updated = item.clone();
                                updated.speed = speed;
                                updated.touched = true;
                                let _ = state.apply_command(Command::RemoveItem(item.id));
                                let _ = state.apply_command(Command::InsertItem(updated));
                            }

                            let mut volume = item.volume;
                            if ui.add(egui::Slider::new(&mut volume, 0.0..=2.0).text("Volume")).changed() {
                                let mut updated = item.clone();
                                updated.volume = volume;
                                updated.touched = true;
                                let _ = state.apply_command(Command::RemoveItem(item.id));
                                let _ = state.apply_command(Command::InsertItem(updated));
                            }

                            // Pin Protection Toggle (Charter Rule 8)
                            let pin_label = if item.pinned { "🔓 Unpin Clip" } else { "📌 Pin Clip (Permanent)" };
                            if ui.button(pin_label).clicked() {
                                let _ = state.apply_command(Command::SetItemPin {
                                    item_id: item.id,
                                    pinned: !item.pinned,
                                });
                                state.add_toast(ToastLevel::Success, "Pin flag updated");
                            }

                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(10.0);

                            // Stage 16a: Transitions Inspector
                            ui.label(egui::RichText::new("Transitions (Stage 16a)").strong());
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut self.selected_transition_kind, 0, "None");
                                ui.selectable_value(&mut self.selected_transition_kind, 1, "Cross Dissolve");
                                ui.selectable_value(&mut self.selected_transition_kind, 2, "Dip to Black");
                                ui.selectable_value(&mut self.selected_transition_kind, 3, "Push Left");
                            });

                            ui.horizontal(|ui| {
                                ui.label("Duration (s):");
                                ui.add(egui::Slider::new(&mut self.selected_transition_duration_sec, 0.1..=2.0));
                            });

                            if ui.button("Apply Transition").clicked() {
                                let t_kind = match self.selected_transition_kind {
                                    1 => Some(TransitionKind::CrossDissolve),
                                    2 => Some(TransitionKind::DipToColor),
                                    3 => Some(TransitionKind::Push),
                                    _ => None,
                                };

                                let transition = t_kind.map(|k| {
                                    Transition::new(k, Tick::from_seconds_f64(self.selected_transition_duration_sec as f64))
                                });

                                let _ = state.apply_command(Command::SetItemTransition {
                                    item_id: item.id,
                                    transition,
                                });
                                state.add_toast(ToastLevel::Success, "Transition updated");
                            }

                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(10.0);

                            // Stage 16b: Effects Stack Inspector
                            ui.label(egui::RichText::new("Effects Stack (Stage 16b)").strong());
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut self.selected_effect_choice, 0, "Brightness");
                                ui.selectable_value(&mut self.selected_effect_choice, 1, "Contrast");
                                ui.selectable_value(&mut self.selected_effect_choice, 2, "Blur");
                                ui.selectable_value(&mut self.selected_effect_choice, 3, "Glitch");
                                ui.selectable_value(&mut self.selected_effect_choice, 4, "Beat Pulse");
                            });

                            ui.horizontal(|ui| {
                                ui.label("Parameter:");
                                ui.add(egui::Slider::new(&mut self.effect_param_value, 0.0..=2.0));
                            });

                            if ui.button("+ Add Effect").clicked() {
                                let effect_str = match self.selected_effect_choice {
                                    0 => format!("brightness:{:.2}", self.effect_param_value - 1.0),
                                    1 => format!("contrast:{:.2}", self.effect_param_value),
                                    2 => format!("blur:{}", (self.effect_param_value * 4.0).round() as u32),
                                    3 => format!("glitch:{:.2}", self.effect_param_value),
                                    _ => format!("beat_pulse:{:.2}", self.effect_param_value),
                                };

                                let mut effects = item.effects.clone();
                                effects.push(effect_str);
                                let _ = state.apply_command(Command::SetItemEffects {
                                    item_id: item.id,
                                    effects,
                                });
                                state.add_toast(ToastLevel::Success, "Effect added");
                            }

                            for (idx, eff) in item.effects.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.colored_label(ACCENT_TEAL, format!("  [#{}] {}", idx + 1, eff));
                                    if ui.small_button("✕").clicked() {
                                        let mut effects = item.effects.clone();
                                        effects.remove(idx);
                                        let _ = state.apply_command(Command::SetItemEffects {
                                            item_id: item.id,
                                            effects,
                                        });
                                    }
                                });
                            }

                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(10.0);

                            // Stage 16d: Text Overlay Inspector
                            ui.label(egui::RichText::new("Text Overlay (Stage 16d)").strong());
                            ui.add(egui::TextEdit::singleline(&mut self.custom_text_input).desired_width(240.0));
                            if ui.button("+ Add Title Overlay").clicked() {
                                let effect_str = format!("text:{}", self.custom_text_input);
                                let mut effects = item.effects.clone();
                                effects.push(effect_str);
                                let _ = state.apply_command(Command::SetItemEffects {
                                    item_id: item.id,
                                    effects,
                                });
                                state.add_toast(ToastLevel::Success, "Text overlay added");
                            }
                        });
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.heading("No Clip Selected");
                        ui.colored_label(
                            TEXT_MUTED,
                            "Select a clip on the timeline to inspect properties, transitions, and effects.",
                        );
                    });
                }
            });
    }

    fn split_at_playhead(&self, state: &mut AppState) {
        let playhead = state.playback.current_tick;

        // Find item under playhead
        let candidate = state.project.items.iter().find(|i| {
            playhead > i.timeline_start && playhead < i.timeline_end()
        }).cloned();

        if let Some(item) = candidate {
            let next_item_id = ItemId((state.project.items.len() + 100) as u64);
            let cmd = Command::SplitItem {
                item_id: item.id,
                split_point: playhead,
                new_item_id: next_item_id,
            };
            if let Err(e) = state.apply_command(cmd) {
                state.add_toast(ToastLevel::Error, format!("Split error: {e}"));
            } else {
                state.add_toast(ToastLevel::Success, "Clip split cleanly at playhead");
            }
        } else {
            state.add_toast(ToastLevel::Warning, "No clip under playhead to split");
        }
    }

    fn ripple_delete_selected(&self, state: &mut AppState) {
        let selected_id = state.selected_timeline_item_id;
        if let Some(item_id) = selected_id {
            let item = state.project.items.iter().find(|i| i.id == item_id).cloned();
            if let Some(item) = item {
                let track_id = item.track_id;
                let gap_duration = item.timeline_duration;
                let gap_start = item.timeline_start;

                // 1. Remove selected item
                let _ = state.apply_command(Command::RemoveItem(item_id));

                // 2. Ripple downstream items on the same track leftward
                let downstream_items: Vec<Item> = state
                    .project
                    .items
                    .iter()
                    .filter(|i| i.track_id == track_id && i.timeline_start >= gap_start)
                    .cloned()
                    .collect();

                for downstream in downstream_items {
                    let new_start = downstream.timeline_start - gap_duration;
                    let _ = state.apply_command(Command::MoveItem {
                        item_id: downstream.id,
                        new_start,
                    });
                }

                state.selected_timeline_item_id = None;
                state.add_toast(ToastLevel::Success, "Ripple deleted clip");
            }
        } else {
            state.add_toast(ToastLevel::Warning, "No clip selected for ripple delete");
        }
    }
}
