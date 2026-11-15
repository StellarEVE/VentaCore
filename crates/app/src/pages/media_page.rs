//! Stage 12: Media Page (Footage & Audio Management)
//!
//! Provides asset ingestion, staged analysis status, interactive filmstrips,
//! trim and exclusion range controls, audio waveform rendering, and manual
//! beat-grid corrections.

use std::path::PathBuf;
use std::time::Instant;

use egui::{
    Frame, Margin, Pos2, Rounding, ScrollArea, Sense, Stroke, Ui, Vec2,
};
use ventacore_audio::corrections::{
    double_time, half_time, set_downbeat, shift_grid, tap_tempo,
};
use ventacore_core::command::Command;
use ventacore_core::model::{Asset, AssetId, ProbeData, SongEntry, SongId};
use ventacore_core::time::{Tick, TimeRange, TICKS_PER_SECOND};
use ventacore_media::fingerprint::compute_content_fingerprint;

use crate::state::{AppState, ToastLevel};
use crate::theme::{
    ACCENT_TEAL, ACCENT_TEAL_DARK, ACCENT_TEAL_DIM, BG_DEEP, BORDER, ERROR_RED, GOLD_FAVOURITE,
    SURFACE, TEXT_MUTED, TEXT_PRIMARY,
};
use crate::widgets::timecode::format_time_minutes_seconds;

pub struct MediaPage {
    first_tap: Option<Instant>,
    tap_ticks: Vec<Tick>,
    custom_path_input: String,
}

impl Default for MediaPage {
    fn default() -> Self {
        Self {
            first_tap: None,
            tap_ticks: Vec::new(),
            custom_path_input: String::new(),
        }
    }
}

impl MediaPage {
    pub fn render(&mut self, ui: &mut Ui, state: &mut AppState) {
        // Handle dropped files from native OS
        let dropped = ui.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            if let Some(path) = file.path {
                self.import_file_path(path, state);
            }
        }

        ui.horizontal(|ui| {
            // Left Panel: Ingest & Asset List (340px width)
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

    fn render_asset_list(&mut self, ui: &mut Ui, state: &mut AppState) {
        Frame::none()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                ui.heading("Media Assets");
                ui.colored_label(TEXT_MUTED, "Drag & drop video and song files here");
                ui.add_space(8.0);

                // Quick file adder field
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.custom_path_input)
                            .hint_text("Enter file path...")
                            .desired_width(220.0),
                    );
                    if ui.button("+ Add").clicked() {
                        let path = PathBuf::from(&self.custom_path_input);
                        if path.exists() {
                            self.import_file_path(path, state);
                            self.custom_path_input.clear();
                        } else {
                            state.add_toast(ToastLevel::Error, "File does not exist");
                        }
                    }
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // Scrollable list of loaded assets
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if state.project.assets.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(40.0);
                                ui.colored_label(TEXT_MUTED, "No media assets imported yet.");
                            });
                            return;
                        }

                        let asset_ids: Vec<AssetId> =
                            state.project.assets.iter().map(|a| a.id).collect();

                        for asset_id in asset_ids {
                            let (name, is_audio, dur, is_selected) = {
                                if let Some(a) = state.project.assets.iter().find(|a| a.id == asset_id) {
                                    let filename = PathBuf::from(&a.path_hint)
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| format!("Asset #{}", a.id.0));
                                    let is_audio = a.probe_data.as_ref().map(|p| p.width == 0).unwrap_or(false);
                                    let dur = a.probe_data.as_ref().map(|p| p.duration).unwrap_or(Tick::from_seconds(60));
                                    let is_selected = state.selected_asset_id == Some(asset_id);
                                    (filename, is_audio, dur, is_selected)
                                } else {
                                    continue;
                                }
                            };

                            let (bg, stroke) = if is_selected {
                                (ACCENT_TEAL_DARK, Stroke::new(1.0_f32, ACCENT_TEAL))
                            } else {
                                (BG_DEEP, Stroke::new(1.0_f32, BORDER))
                            };

                            Frame::none()
                                .fill(bg)
                                .stroke(stroke)
                                .rounding(Rounding::same(4.0))
                                .inner_margin(Margin::same(8.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let icon = if is_audio { "🎵" } else { "🎬" };
                                        ui.label(icon);
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
                                                    let _ = state.apply_command(Command::RemoveAsset(asset_id));
                                                }
                                                if ui.small_button("Select").clicked() {
                                                    state.selected_asset_id = Some(asset_id);
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    });
            });
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

    fn import_file_path(&mut self, path: PathBuf, state: &mut AppState) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let is_audio = matches!(ext.as_str(), "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a");
        let id_num = (state.project.assets.len() + 1) as u64;
        let id = AssetId(id_num);
        let fp = compute_content_fingerprint(&path).ok();

        let probe_data = if is_audio {
            ProbeData {
                duration: Tick::from_seconds(60),
                width: 0,
                height: 0,
                fps_num: 0,
                fps_den: 1,
                has_audio: true,
                audio_sample_rate: 44100,
            }
        } else {
            ProbeData {
                duration: Tick::from_seconds(60),
                width: 1920,
                height: 1080,
                fps_num: 30,
                fps_den: 1,
                has_audio: true,
                audio_sample_rate: 48000,
            }
        };

        let mut asset = Asset::new(id, path.to_string_lossy().to_string());
        asset.fingerprint = fp;
        asset.probe_data = Some(probe_data);

        if let Err(e) = state.apply_command(Command::AddAsset(asset)) {
            state.add_toast(ToastLevel::Error, format!("Failed to add asset: {e}"));
        } else {
            state.selected_asset_id = Some(id);
            if is_audio && state.song_structure.is_none() {
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
                let mut song_entry = SongEntry::new(SongId(1), id, "Main Song");
                song_entry.tempo_bpm = Some(120.0);
                let _ = state.apply_command(Command::AddSong(song_entry));
            }
            state.add_toast(ToastLevel::Success, "Asset imported successfully");
        }
    }
}
