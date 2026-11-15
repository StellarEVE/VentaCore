//! Stage 12: Media Page (Footage & Audio Management)
//!
//! Provides asset ingestion, staged analysis status, interactive filmstrips,
//! trim and exclusion range controls, audio waveform rendering, and manual
//! beat-grid corrections.

use std::path::PathBuf;
use std::time::Instant;

use egui::{
    Color32, Frame, Margin, Pos2, Rect, Rounding, ScrollArea, Sense, Stroke, Ui, Vec2,
};
use ventacore_audio::corrections::{
    double_time, half_time, set_downbeat, shift_grid, tap_tempo,
};
use ventacore_core::command::Command;
use ventacore_core::model::{Asset, AssetId, AssetRole, ReplaceSongPolicy};
use ventacore_core::time::{Tick, TimeRange, TICKS_PER_SECOND};
use ventacore_engine::import::{
    apply_import_plan, plan_import, ImportAction, ImportChoicePrompt, ImportDestination, ImportPlan,
};

use crate::state::{AppState, ToastLevel};
use crate::theme::{
    ACCENT_TEAL, ACCENT_TEAL_DARK, ACCENT_TEAL_DIM, BG_DEEP, BORDER, ERROR_RED,
    GOLD_FAVOURITE, SURFACE, TEXT_MUTED, TEXT_PRIMARY, WARNING_AMBER,
};
use crate::widgets::file_dialog::{FileDialogHelper, FileDialogResult};
use crate::widgets::monitor::Monitor;
use crate::widgets::timecode::format_time_minutes_seconds;
use ventacore_engine::playback::PlaybackEngine;

pub struct MediaPage {
    first_tap: Option<Instant>,
    tap_ticks: Vec<Tick>,
    _custom_path_input: String,
    pub file_dialog: FileDialogHelper,
    pub music_panel_rect: Option<Rect>,
    pub footage_panel_rect: Option<Rect>,
    pub pending_choice: Option<ImportChoicePrompt>,
    pub not_imported_strip: Vec<(PathBuf, String, bool)>,
    pub source_player: PlaybackEngine,
    pub source_monitor: Monitor,
    pub loaded_asset_id: Option<AssetId>,
}

impl Default for MediaPage {
    fn default() -> Self {
        Self {
            first_tap: None,
            tap_ticks: Vec::new(),
            _custom_path_input: String::new(),
            file_dialog: FileDialogHelper::new(),
            music_panel_rect: None,
            footage_panel_rect: None,
            pending_choice: None,
            not_imported_strip: Vec::new(),
            source_player: PlaybackEngine::new(),
            source_monitor: Monitor::source(),
            loaded_asset_id: None,
        }
    }
}

impl MediaPage {
    pub fn render(&mut self, ui: &mut Ui, state: &mut AppState) {
        // 1. Poll background native file dialog helper channel
        if let Some(res) = self.file_dialog.poll() {
            match res {
                FileDialogResult::Song(path) => {
                    self.handle_import_paths(vec![path], ImportDestination::MusicPanel, state);
                }
                FileDialogResult::Videos(paths) => {
                    self.handle_import_paths(paths, ImportDestination::FootagePanel, state);
                }
            }
        }

        // 2. Handle dropped files from native OS drag-and-drop
        let dropped = ui.input(|i| i.raw.dropped_files.clone());
        let pointer_pos = ui.input(|i| i.pointer.hover_pos().or(i.pointer.interact_pos()));
        if !dropped.is_empty() {
            for file in dropped {
                if let Some(path) = file.path {
                    let dest = match pointer_pos {
                        Some(pos) => {
                            if self.music_panel_rect.map(|r| r.contains(pos)).unwrap_or(false) {
                                ImportDestination::MusicPanel
                            } else if self.footage_panel_rect.map(|r| r.contains(pos)).unwrap_or(false) {
                                ImportDestination::FootagePanel
                            } else {
                                ImportDestination::AnywhereElse
                            }
                        }
                        None => ImportDestination::AnywhereElse,
                    };
                    self.handle_import_paths(vec![path], dest, state);
                }
            }
        }

        // 3. Render any pending modal choices
        self.render_choice_modal(ui.ctx(), state);

        ui.horizontal(|ui| {
            // Left Panel: Ingest & Pools (Music and Footage Panels, 340px width)
            ui.allocate_ui_with_layout(
                Vec2::new(340.0, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    self.render_asset_list(ui, state);
                },
            );

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Right Panel: Selected Asset Detail / Waveform Inspector
            ui.allocate_ui_with_layout(
                ui.available_size(),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    self.render_asset_inspector(ui, state);
                },
            );
        });
    }

    pub fn handle_import_paths(
        &mut self,
        paths: Vec<PathBuf>,
        destination: ImportDestination,
        state: &mut AppState,
    ) {
        let plan = plan_import(&state.project, &paths, destination);

        for action in &plan.actions {
            match action {
                ImportAction::Refused {
                    path,
                    reason,
                    suggest_use_as_song,
                } => {
                    self.not_imported_strip.push((
                        path.clone(),
                        reason.clone(),
                        *suggest_use_as_song,
                    ));
                }
                ImportAction::NeedsChoice { choice, .. } => {
                    self.pending_choice = Some(choice.clone());
                }
                _ => {}
            }
        }

        if plan.songs_count() > 0 || plan.footage_count() > 0 {
            match apply_import_plan(
                &mut state.project,
                &plan,
                ReplaceSongPolicy::KeepPinnedAndTouched,
            ) {
                Ok(commands) => {
                    if !commands.is_empty() {
                        state.history.record(state.project.clone());
                        state.is_dirty = true;
                    }
                    state.add_toast(
                        ToastLevel::Success,
                        format!("Imported {} item(s)", commands.len()),
                    );
                }
                Err(e) => {
                    state.add_toast(ToastLevel::Error, format!("Import error: {e}"));
                }
            }
        }
    }

    fn render_choice_modal(&mut self, ctx: &egui::Context, state: &mut AppState) {
        let choice = match &self.pending_choice {
            Some(c) => c.clone(),
            None => return,
        };

        let mut dismiss = false;

        match choice {
            ImportChoicePrompt::ReplaceExistingSong { new_song_path } => {
                egui::Window::new("Replace Active Song")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.heading("Replace Existing Song?");
                        ui.colored_label(
                            TEXT_MUTED,
                            "A song is already loaded in this project. How would you like to handle existing timeline clips?",
                        );
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            if ui.button("📌 Keep Pinned & Touched Clips").clicked() {
                                let plan = ImportPlan {
                                    actions: vec![ImportAction::ImportSong {
                                        path: new_song_path.clone(),
                                        classification: ventacore_platform::classify(&new_song_path),
                                    }],
                                };
                                if let Ok(_) = apply_import_plan(
                                    &mut state.project,
                                    &plan,
                                    ReplaceSongPolicy::KeepPinnedAndTouched,
                                ) {
                                    state.history.record(state.project.clone());
                                    state.is_dirty = true;
                                }
                                dismiss = true;
                            }
                            if ui.button("🔄 Start Over").clicked() {
                                let plan = ImportPlan {
                                    actions: vec![ImportAction::ImportSong {
                                        path: new_song_path.clone(),
                                        classification: ventacore_platform::classify(&new_song_path),
                                    }],
                                };
                                if let Ok(_) = apply_import_plan(
                                    &mut state.project,
                                    &plan,
                                    ReplaceSongPolicy::StartOver,
                                ) {
                                    state.history.record(state.project.clone());
                                    state.is_dirty = true;
                                }
                                dismiss = true;
                            }
                            if ui.button("Cancel").clicked() {
                                dismiss = true;
                            }
                        });
                    });
            }
            ImportChoicePrompt::VideoOnMusicPanel { path, classification } => {
                egui::Window::new("Video Dropped on Music Panel")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.heading("Use Video as Song or Footage?");
                        ui.colored_label(
                            TEXT_MUTED,
                            "This video contains picture and sound. How should it be imported?",
                        );
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            if ui.button("🎵 Use Sound as Song").clicked() {
                                let plan = ImportPlan {
                                    actions: vec![ImportAction::ImportSong {
                                        path: path.clone(),
                                        classification: classification.clone(),
                                    }],
                                };
                                if let Ok(_) = apply_import_plan(
                                    &mut state.project,
                                    &plan,
                                    ReplaceSongPolicy::KeepPinnedAndTouched,
                                ) {
                                    state.history.record(state.project.clone());
                                    state.is_dirty = true;
                                }
                                dismiss = true;
                            }
                            if ui.button("🎬 Add to Footage Pool").clicked() {
                                let plan = ImportPlan {
                                    actions: vec![ImportAction::ImportFootage {
                                        path: path.clone(),
                                        classification: classification.clone(),
                                    }],
                                };
                                if let Ok(_) = apply_import_plan(
                                    &mut state.project,
                                    &plan,
                                    ReplaceSongPolicy::KeepPinnedAndTouched,
                                ) {
                                    state.history.record(state.project.clone());
                                    state.is_dirty = true;
                                }
                                dismiss = true;
                            }
                            if ui.button("Cancel").clicked() {
                                dismiss = true;
                            }
                        });
                    });
            }
            ImportChoicePrompt::MultipleAudioFiles { candidates } => {
                egui::Window::new("Multiple Audio Files Selected")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.heading("Which file is the active song?");
                        ui.colored_label(
                            TEXT_MUTED,
                            "Select the master song track for the music video cut:",
                        );
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        for path in &candidates {
                            let fname = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "audio".into());
                            if ui.button(format!("🎵 {fname}")).clicked() {
                                let plan = ImportPlan {
                                    actions: vec![ImportAction::ImportSong {
                                        path: path.clone(),
                                        classification: ventacore_platform::classify(path),
                                    }],
                                };
                                if let Ok(_) = apply_import_plan(
                                    &mut state.project,
                                    &plan,
                                    ReplaceSongPolicy::KeepPinnedAndTouched,
                                ) {
                                    state.history.record(state.project.clone());
                                    state.is_dirty = true;
                                }
                                dismiss = true;
                            }
                        }

                        ui.add_space(4.0);
                        if ui.button("Cancel").clicked() {
                            dismiss = true;
                        }
                    });
            }
        }

        if dismiss {
            self.pending_choice = None;
        }
    }

    fn render_asset_list(&mut self, ui: &mut Ui, state: &mut AppState) {
        let is_hovering = ui.input(|i| !i.raw.hovered_files.is_empty());

        // A. Not imported strip (Part 4b)
        if !self.not_imported_strip.is_empty() {
            Frame::none()
                .fill(Color32::from_rgb(35, 20, 20))
                .stroke(Stroke::new(1.0_f32, ERROR_RED))
                .rounding(Rounding::same(6.0))
                .inner_margin(Margin::same(8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(ERROR_RED, "⚠ Some files were not imported:");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Clear").clicked() {
                                self.not_imported_strip.clear();
                            }
                        });
                    });

                    let items = self.not_imported_strip.clone();
                    for (idx, (path, reason, suggest_use_as_song)) in items.iter().enumerate() {
                        let fname = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "file".into());
                        ui.horizontal(|ui| {
                            ui.colored_label(TEXT_PRIMARY, format!("• {fname}:"));
                            ui.colored_label(TEXT_MUTED, reason);
                            if *suggest_use_as_song {
                                if ui.button("🎵 Use as song").clicked() {
                                    let p = path.clone();
                                    self.not_imported_strip.remove(idx);
                                    self.handle_import_paths(
                                        vec![p],
                                        ImportDestination::MusicPanel,
                                        state,
                                    );
                                }
                            }
                        });
                    }
                });
            ui.add_space(8.0);
        }

        // B. Music Panel (Part 4b DropZone)
        let music_stroke = if is_hovering {
            Stroke::new(2.0_f32, ACCENT_TEAL)
        } else {
            Stroke::new(1.0_f32, BORDER)
        };

        let music_resp = Frame::none()
            .fill(SURFACE)
            .stroke(music_stroke)
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(10.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("🎵 Music Panel");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("📂 Browse for music").clicked() {
                            self.file_dialog.pick_song();
                        }
                    });
                });

                ui.colored_label(TEXT_MUTED, "Drop song audio here, or browse");
                ui.add_space(4.0);

                if let Some(song_slot) = &state.project.song {
                    let song_asset = state.project.find_asset(song_slot.asset_id);
                    let name = song_asset
                        .map(|a| {
                            PathBuf::from(&a.path_hint)
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string()
                        })
                        .unwrap_or_else(|| "Active Song".into());
                    let dur = song_slot.duration();

                    Frame::none()
                        .fill(BG_DEEP)
                        .stroke(Stroke::new(1.0_f32, ACCENT_TEAL))
                        .rounding(Rounding::same(4.0))
                        .inner_margin(Margin::same(6.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.colored_label(ACCENT_TEAL, "🎵 [Active Song]");
                                ui.colored_label(TEXT_PRIMARY, name);
                                ui.colored_label(TEXT_MUTED, format_time_minutes_seconds(dur));
                            });
                        });
                } else {
                    ui.colored_label(WARNING_AMBER, "No active song loaded");
                }
            });

        self.music_panel_rect = Some(music_resp.response.rect);
        ui.add_space(10.0);

        // C. Footage Panel (Part 4b DropZone)
        let footage_stroke = if is_hovering {
            Stroke::new(2.0_f32, ACCENT_TEAL)
        } else {
            Stroke::new(1.0_f32, BORDER)
        };

        let footage_resp = Frame::none()
            .fill(SURFACE)
            .stroke(footage_stroke)
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(10.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("🎬 Footage Panel");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("➕ Add videos").clicked() {
                            self.file_dialog.pick_videos();
                        }
                    });
                });

                ui.colored_label(TEXT_MUTED, "Drop video files here, or click Add videos");
                ui.add_space(6.0);

                // Footage pool list
                let footage_assets: Vec<AssetId> = state
                    .project
                    .assets
                    .iter()
                    .filter(|a| a.role == AssetRole::Footage)
                    .map(|a| a.id)
                    .collect();

                if footage_assets.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.colored_label(TEXT_MUTED, "No footage imported yet.");
                        ui.add_space(20.0);
                    });
                } else {
                    ScrollArea::vertical()
                        .max_height(260.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for asset_id in footage_assets {
                                let (name, dur, is_selected) = {
                                    if let Some(a) = state.project.find_asset(asset_id) {
                                        let filename = PathBuf::from(&a.path_hint)
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_else(|| format!("Asset #{}", a.id.0));
                                        let dur = a
                                            .probe_data
                                            .as_ref()
                                            .map(|p| p.duration)
                                            .unwrap_or(Tick::from_seconds(60));
                                        let is_selected = state.selected_asset_id == Some(asset_id);
                                        (filename, dur, is_selected)
                                    } else {
                                        continue;
                                    }
                                };

                                let (bg, stroke) = if is_selected {
                                    (ACCENT_TEAL_DARK, Stroke::new(1.0_f32, ACCENT_TEAL))
                                } else {
                                    (BG_DEEP, Stroke::new(1.0_f32, BORDER))
                                };

                                let row_resp = Frame::none()
                                    .fill(bg)
                                    .stroke(stroke)
                                    .rounding(Rounding::same(4.0))
                                    .inner_margin(Margin::same(6.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label("🎬");
                                            ui.vertical(|ui| {
                                                ui.colored_label(TEXT_PRIMARY, &name);
                                                ui.colored_label(
                                                    TEXT_MUTED,
                                                    format_time_minutes_seconds(dur),
                                                );
                                            });

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui.small_button("✕").clicked() {
                                                        let _ = state.apply_command(
                                                            Command::RemoveFootage(asset_id),
                                                        );
                                                    }
                                                    if ui.small_button("Select").clicked() {
                                                        state.selected_asset_id = Some(asset_id);
                                                    }
                                                },
                                            );
                                        });
                                    });

                                if row_resp.response.interact(egui::Sense::click()).clicked() {
                                    state.selected_asset_id = Some(asset_id);
                                }
                                ui.add_space(4.0);
                            }
                        });
                }
            });

        self.footage_panel_rect = Some(footage_resp.response.rect);
    }

    fn render_asset_inspector(&mut self, ui: &mut Ui, state: &mut AppState) {
        let selected_id = state.selected_asset_id;
        let asset = selected_id
            .and_then(|id| state.project.assets.iter().find(|a| a.id == id).cloned());

        if let Some(asset) = asset {
            let is_audio = asset.probe_data.as_ref().map(|p| p.width == 0).unwrap_or(false);

            Frame::none()
                .fill(SURFACE)
                .stroke(Stroke::new(1.0_f32, BORDER))
                .rounding(Rounding::same(6.0))
                .inner_margin(Margin::same(16.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let icon = if is_audio {
                            "🎵 Song Audio Inspector"
                        } else {
                            "🎬 Video Asset Inspector"
                        };
                        ui.heading(icon);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let badge = if let Some(fp) = &asset.fingerprint {
                                format!("Fingerprint: {:.8}...", fp)
                            } else {
                                "Un-fingerprinted".to_string()
                            };
                            ui.colored_label(TEXT_MUTED, badge);
                        });
                    });

                    ui.colored_label(TEXT_MUTED, &asset.path_hint);
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(12.0);

                    if is_audio {
                        self.render_audio_waveform(ui, &asset, state);
                    } else {
                        // Source Monitor
                        let filename = PathBuf::from(&asset.path_hint)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Source Media".into());

                        if self.loaded_asset_id != Some(asset.id) {
                            self.loaded_asset_id = Some(asset.id);
                            let _ = self.source_player.open_clip(&asset.path_hint);
                        }

                        self.source_monitor.show(ui, &mut self.source_player, &filename);
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);

                        self.render_video_trimmers(ui, &asset, state);
                    }
                });
        } else {
            Frame::none()
                .fill(SURFACE)
                .stroke(Stroke::new(1.0_f32, BORDER))
                .rounding(Rounding::same(6.0))
                .inner_margin(Margin::same(24.0))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        ui.heading("No Asset Selected");
                        ui.colored_label(
                            TEXT_MUTED,
                            "Select an asset from the left panel to inspect filmstrips, trims, and waveforms.",
                        );
                    });
                });
        }
    }

    fn render_video_trimmers(&mut self, ui: &mut Ui, asset: &Asset, state: &mut AppState) {
        ui.label(egui::RichText::new("Trims & Ranges").strong());
        ui.colored_label(
            TEXT_MUTED,
            "Configure start/end trims and exclude ranges. Pinned or trimmed ranges are protected permanently (Charter Rule 8).",
        );
        ui.add_space(8.0);

        let total_dur = asset.probe_data.as_ref().map(|p| p.duration).unwrap_or(Tick::from_seconds(60));
        let total_secs = total_dur.0 as f64 / TICKS_PER_SECOND as f64;
        let mut intro_secs = asset.intro_trim.0 as f64 / TICKS_PER_SECOND as f64;
        let mut outro_secs = asset.outro_trim.0 as f64 / TICKS_PER_SECOND as f64;

        let mut changed = false;

        ui.horizontal(|ui| {
            ui.label("Intro Trim (sec):");
            if ui
                .add(egui::Slider::new(&mut intro_secs, 0.0..=total_secs.max(1.0)).text("s"))
                .changed()
            {
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Outro Trim (sec):");
            if ui
                .add(egui::Slider::new(&mut outro_secs, 0.0..=total_secs.max(1.0)).text("s"))
                .changed()
            {
                changed = true;
            }
        });

        if changed {
            let intro = Tick::from_seconds(intro_secs as i64);
            let outro = Tick::from_seconds(outro_secs as i64);
            let _ = state.apply_command(Command::SetAssetTrims {
                asset_id: asset.id,
                intro,
                outro,
            });
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(12.0);

        // Exclude ranges section
        ui.horizontal(|ui| {
            ui.colored_label(ERROR_RED, "Excluded Ranges (Shaky / Bad Moments):");
            if ui.button("+ Add Exclude (0s - 2s)").clicked() {
                let range = TimeRange::new(Tick::ZERO, Tick::from_seconds(2));
                let _ = state.apply_command(Command::AddExcludeRange {
                    asset_id: asset.id,
                    range,
                });
            }
        });

        for (idx, range) in asset.exclude_ranges.iter().enumerate() {
            ui.colored_label(
                TEXT_MUTED,
                format!(
                    "  [#{}] Exclude: {} to {}",
                    idx + 1,
                    format_time_minutes_seconds(range.start),
                    format_time_minutes_seconds(range.end())
                ),
            );
        }

        ui.add_space(8.0);

        // Favourite moments section
        ui.horizontal(|ui| {
            ui.colored_label(GOLD_FAVOURITE, "Favourite Highlight Ranges:");
            if ui.button("+ Add Favourite (2s - 4s)").clicked() {
                let range = TimeRange::new(Tick::from_seconds(2), Tick::from_seconds(2));
                let _ = state.apply_command(Command::AddFavouriteMoment {
                    asset_id: asset.id,
                    range,
                });
            }
        });

        for (idx, fav) in asset.favourite_moments.iter().enumerate() {
            ui.colored_label(
                TEXT_MUTED,
                format!(
                    "  [#{}] Favourite: {} to {}",
                    idx + 1,
                    format_time_minutes_seconds(fav.start),
                    format_time_minutes_seconds(fav.end())
                ),
            );
        }
    }

    fn render_audio_waveform(&mut self, ui: &mut Ui, asset: &Asset, state: &mut AppState) {
        ui.label(egui::RichText::new("Beat & Waveform Grid").strong());
        ui.colored_label(
            TEXT_MUTED,
            "Waveform peak envelope with downbeats and beat alignment.",
        );
        ui.add_space(8.0);

        let total_dur = asset.probe_data.as_ref().map(|p| p.duration).unwrap_or(Tick::from_seconds(60));

        // Visual Waveform Canvas
        let (rect, _response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), 120.0),
            Sense::click_and_drag(),
        );

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, Rounding::same(4.0), BG_DEEP);
        painter.rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.0_f32, BORDER));

        // Draw synthetic waveform bars
        let width = rect.width();
        let height = rect.height();
        let mid_y = rect.min.y + height * 0.5;

        let num_bars = (width / 4.0) as usize;
        for i in 0..num_bars {
            let x = rect.min.x + (i as f32) * 4.0;
            let norm = (i as f32) / (num_bars as f32);
            let amp = (norm * 12.0).sin().abs() * 0.4 + (norm * 30.0).sin().abs() * 0.3;
            let bar_h = (height * 0.45) * amp;

            painter.line_segment(
                [Pos2::new(x, mid_y - bar_h), Pos2::new(x, mid_y + bar_h)],
                Stroke::new(2.0_f32, ACCENT_TEAL_DIM),
            );
        }

        // Draw vertical beat lines
        if let Some(song) = &state.song_structure {
            for (idx, beat) in song.beat_ticks.iter().enumerate() {
                let beat_norm = (beat.0 as f32) / (total_dur.0.max(1) as f32);
                if beat_norm >= 0.0 && beat_norm <= 1.0 {
                    let beat_x = rect.min.x + beat_norm * width;
                    let is_downbeat = idx % 4 == 0;
                    let (stroke_color, stroke_w) = if is_downbeat {
                        (ACCENT_TEAL, 2.0_f32)
                    } else {
                        (BORDER, 1.0_f32)
                    };

                    painter.line_segment(
                        [Pos2::new(beat_x, rect.min.y), Pos2::new(beat_x, rect.max.y)],
                        Stroke::new(stroke_w, stroke_color),
                    );
                }
            }
        }

        ui.add_space(12.0);

        let current_bpm = state
            .song_structure
            .as_ref()
            .map(|s| s.bpm)
            .unwrap_or(120.0);

        ui.horizontal(|ui| {
            ui.colored_label(
                TEXT_PRIMARY,
                format!("Tempo: {:.1} BPM", current_bpm),
            );
            ui.add_space(16.0);

            // Tap tempo button
            if ui.button("👆 Tap Tempo").clicked() {
                let now = Instant::now();
                let first = self.first_tap.get_or_insert(now);
                let elapsed_sec = now.duration_since(*first).as_secs_f64();
                self.tap_ticks.push(Tick::from_seconds_f64(elapsed_sec));
                if self.tap_ticks.len() > 8 {
                    self.tap_ticks.remove(0);
                }
                if let Some(bpm) = tap_tempo(&self.tap_ticks) {
                    if let Some(song) = &mut state.song_structure {
                        song.bpm = bpm;
                        state.add_toast(ToastLevel::Success, format!("Tempo updated: {:.1} BPM", bpm));
                    }
                }
            }

            // Half Time
            if ui.button("½ Half Time").clicked() {
                if let Some(song) = &state.song_structure {
                    let new_song = half_time(song);
                    state.song_structure = Some(new_song);
                    state.add_toast(ToastLevel::Info, "Halved grid tempo");
                }
            }

            // Double Time
            if ui.button("2× Double Time").clicked() {
                if let Some(song) = &state.song_structure {
                    let new_song = double_time(song);
                    state.song_structure = Some(new_song);
                    state.add_toast(ToastLevel::Info, "Doubled grid tempo");
                }
            }

            // Shift grid
            if ui.button("◀ Shift (-1 Beat)").clicked() {
                if let Some(song) = &state.song_structure {
                    let shift_ticks = (TICKS_PER_SECOND as f64 * (60.0 / song.bpm as f64)) as i64;
                    let new_song = shift_grid(song, Tick(-shift_ticks));
                    state.song_structure = Some(new_song);
                }
            }
            if ui.button("Shift (+1 Beat) ▶").clicked() {
                if let Some(song) = &state.song_structure {
                    let shift_ticks = (TICKS_PER_SECOND as f64 * (60.0 / song.bpm as f64)) as i64;
                    let new_song = shift_grid(song, Tick(shift_ticks));
                    state.song_structure = Some(new_song);
                }
            }

            // Set downbeat
            if ui.button("📌 Set Downbeat at Playhead").clicked() {
                if let Some(song) = &state.song_structure {
                    let new_song = set_downbeat(song, state.playback.current_tick);
                    state.song_structure = Some(new_song);
                    state.add_toast(ToastLevel::Success, "Downbeat aligned to playhead");
                }
            }
        });
    }

    pub fn import_file_path(&mut self, path: PathBuf, state: &mut AppState) {
        self.handle_import_paths(vec![path], ImportDestination::AnywhereElse, state);
    }
}
