//! Stage 17: Application Settings and Preferences Modal
//!
//! Provides user preferences for default project settings, autosave intervals,
//! shortcut profiles, and accessible UI scaling.

use egui::{Color32, Context, Frame, Margin, Stroke, Window};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShortcutProfile {
    VentaCoreDefault,
    PremiereStyle,
    DaVinciStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPreferences {
    pub autosave_interval_sec: u64,
    pub default_fps: f64,
    pub default_width: u32,
    pub default_height: u32,
    pub shortcut_profile: ShortcutProfile,
    pub ui_scale: f32,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            autosave_interval_sec: 30,
            default_fps: 30.0,
            default_width: 1920,
            default_height: 1080,
            shortcut_profile: ShortcutProfile::VentaCoreDefault,
            ui_scale: 1.0,
        }
    }
}

pub fn render_settings_dialog(
    ctx: &Context,
    open: &mut bool,
    prefs: &mut AppPreferences,
) {
    if !*open {
        return;
    }

    let mut is_open = true;
    let mut close_clicked = false;

    Window::new("⚙ Application Preferences")
        .open(&mut is_open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(
            Frame::window(&ctx.style())
                .inner_margin(Margin::same(16.0))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0, 229, 192))),
        )
        .show(ctx, |ui| {
            ui.set_width(380.0);

            ui.heading("VentaCore Studio Settings");
            ui.colored_label(
                Color32::from_rgb(155, 161, 173),
                "Configure default workspace preferences and shortcut profiles.",
            );
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(10.0);

            // Autosave Interval
            ui.label(egui::RichText::new("Autosave Interval (Seconds):").strong());
            ui.add(egui::Slider::new(&mut prefs.autosave_interval_sec, 10..=300).text("s"));

            ui.add_space(8.0);

            // Default Resolution
            ui.label(egui::RichText::new("Default Resolution:").strong());
            ui.horizontal(|ui| {
                if ui.selectable_label(prefs.default_width == 1920, "1080p Full HD").clicked() {
                    prefs.default_width = 1920;
                    prefs.default_height = 1080;
                }
                if ui.selectable_label(prefs.default_width == 3840, "4K UHD").clicked() {
                    prefs.default_width = 3840;
                    prefs.default_height = 2160;
                }
                if ui.selectable_label(prefs.default_width == 1280, "720p HD").clicked() {
                    prefs.default_width = 1280;
                    prefs.default_height = 720;
                }
            });

            ui.add_space(8.0);

            // Shortcut Profiles
            ui.label(egui::RichText::new("Shortcut Profile:").strong());
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut prefs.shortcut_profile,
                    ShortcutProfile::VentaCoreDefault,
                    "VentaCore",
                );
                ui.selectable_value(
                    &mut prefs.shortcut_profile,
                    ShortcutProfile::PremiereStyle,
                    "Premiere",
                );
                ui.selectable_value(
                    &mut prefs.shortcut_profile,
                    ShortcutProfile::DaVinciStyle,
                    "DaVinci",
                );
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("✔ Done").clicked() {
                    close_clicked = true;
                }
            });
        });

    if !is_open || close_clicked {
        *open = false;
    }
}
