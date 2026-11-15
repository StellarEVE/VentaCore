//! Visual Theme System for VentaCore App Shell
//!
//! Enforces the Charter visual identity:
//! - Dark silver/black surface palette
//! - Teal brand accents
//! - 4-6px rounded corners
//! - High-contrast readable typography and crisp borders

use egui::{Color32, Context, Margin, Rounding, Stroke, Visuals};

pub const BG_DEEP: Color32 = Color32::from_rgb(20, 21, 23);       // #141517
pub const SURFACE: Color32 = Color32::from_rgb(30, 32, 35);       // #1e2023
pub const SURFACE_HOVER: Color32 = Color32::from_rgb(43, 46, 52); // #2b2e34
pub const SURFACE_ACTIVE: Color32 = Color32::from_rgb(52, 56, 64);// #343840
pub const BORDER: Color32 = Color32::from_rgb(54, 58, 66);        // #363a42
pub const BORDER_LIGHT: Color32 = Color32::from_rgb(72, 78, 88);  // #484e58

pub const ACCENT_TEAL: Color32 = Color32::from_rgb(0, 229, 192);       // #00e5c0
pub const ACCENT_TEAL_DIM: Color32 = Color32::from_rgb(0, 168, 141);   // #00a88d
pub const ACCENT_TEAL_DARK: Color32 = Color32::from_rgb(0, 60, 50);    // Background tint

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(245, 246, 248);   // #f5f6f8
pub const TEXT_MUTED: Color32 = Color32::from_rgb(155, 161, 173);     // #9ba1ad
pub const TEXT_DISABLED: Color32 = Color32::from_rgb(100, 105, 115);  // #646973

pub const WARNING_AMBER: Color32 = Color32::from_rgb(255, 184, 108);  // #ffb86c (provisional)
pub const ERROR_RED: Color32 = Color32::from_rgb(255, 85, 85);        // #ff5555 (errors)
pub const SUCCESS_GREEN: Color32 = Color32::from_rgb(80, 250, 123);   // #50fa7b (verified)
pub const GOLD_FAVOURITE: Color32 = Color32::from_rgb(255, 215, 0);   // #ffd700 (favourites)

/// Apply the VentaCore dark theme to an egui Context.
pub fn apply_theme(ctx: &Context) {
    let mut visuals = Visuals::dark();

    visuals.panel_fill = BG_DEEP;
    visuals.window_fill = SURFACE;
    visuals.extreme_bg_color = Color32::from_rgb(14, 15, 17);

    visuals.window_rounding = Rounding::same(6.0);
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);

    // Non-interactive widgets (labels, panels)
    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.noninteractive.rounding = Rounding::same(4.0);

    // Inactive widgets (buttons at rest)
    visuals.widgets.inactive.bg_fill = SURFACE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.inactive.rounding = Rounding::same(4.0);

    // Hovered widgets
    visuals.widgets.hovered.bg_fill = SURFACE_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT_TEAL_DIM);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.hovered.rounding = Rounding::same(4.0);

    // Active widgets (pressed)
    visuals.widgets.active.bg_fill = SURFACE_ACTIVE;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT_TEAL);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.active.rounding = Rounding::same(4.0);

    // Open/selected widgets
    visuals.widgets.open.bg_fill = SURFACE_ACTIVE;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, ACCENT_TEAL);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.open.rounding = Rounding::same(4.0);

    visuals.selection.bg_fill = ACCENT_TEAL_DARK;
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT_TEAL);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.window_margin = Margin::same(12.0);
    ctx.set_style(style);
}
