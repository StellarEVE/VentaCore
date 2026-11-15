//! Floating Toast Notification Banner
//!
//! Renders non-intrusive floating toasts in the top-right corner.

use egui::{Align2, Context, Frame, Margin, Pos2, Rounding, Stroke, Vec2};

use crate::state::{AppState, ToastLevel};
use crate::theme::{BORDER, ERROR_RED, SUCCESS_GREEN, SURFACE, TEXT_PRIMARY, WARNING_AMBER};

pub fn render_toasts(ctx: &Context, state: &mut AppState) {
    state.prune_toasts();
    if state.toasts.is_empty() {
        return;
    }

    let screen_rect = ctx.screen_rect();
    let mut top_offset = 24.0;

    for toast in &state.toasts {
        let (border_color, icon) = match toast.level {
            ToastLevel::Info => (BORDER, "ℹ"),
            ToastLevel::Success => (SUCCESS_GREEN, "✔"),
            ToastLevel::Warning => (WARNING_AMBER, "⚠"),
            ToastLevel::Error => (ERROR_RED, "✖"),
        };

        egui::Area::new(egui::Id::new(format!("toast_{}", toast.id)))
            .fixed_pos(Pos2::new(screen_rect.max.x - 320.0, top_offset))
            .anchor(Align2::LEFT_TOP, Vec2::ZERO)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(SURFACE)
                    .stroke(Stroke::new(1.0_f32, border_color))
                    .rounding(Rounding::same(6.0))
                    .inner_margin(Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.set_width(290.0);
                        ui.horizontal(|ui| {
                            ui.colored_label(border_color, icon);
                            ui.add_space(4.0);
                            ui.colored_label(TEXT_PRIMARY, &toast.message);
                        });
                    });
            });

        top_offset += 46.0;
    }
}
