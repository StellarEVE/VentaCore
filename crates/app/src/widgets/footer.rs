//! Persistent Four-Page Footer Navigation Bar
//!
//! Handles primary navigation tabs: Media, Cut, Edit, Export,
//! plus keyboard shortcuts (Ctrl+1 through Ctrl+4) and hardware capability disclosures.

use egui::{Button, Color32, Frame, Key, Margin, Response, Rounding, Stroke, Ui};

use crate::state::{AppPage, AppState};
use crate::theme::{
    ACCENT_TEAL, ACCENT_TEAL_DARK, BORDER, SURFACE, TEXT_MUTED, TEXT_PRIMARY, WARNING_AMBER,
};

pub fn render_footer(ui: &mut Ui, state: &mut AppState) {
    // Keyboard shortcuts for quick tab switching
    let input = ui.input(|i| {
        (
            i.modifiers.command && i.key_pressed(Key::Num1),
            i.modifiers.command && i.key_pressed(Key::Num2),
            i.modifiers.command && i.key_pressed(Key::Num3),
            i.modifiers.command && i.key_pressed(Key::Num4),
        )
    });

    if input.0 {
        state.active_page = AppPage::Media;
    } else if input.1 {
        state.active_page = AppPage::Cut;
    } else if input.2 {
        state.active_page = AppPage::Edit;
    } else if input.3 {
        state.active_page = AppPage::Export;
    }

    Frame::none()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .rounding(Rounding::ZERO)
        .inner_margin(Margin::symmetric(16.0, 8.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Navigation tabs
                render_nav_tab(ui, "1  Media", AppPage::Media, &mut state.active_page);
                render_nav_tab(ui, "2  Cut", AppPage::Cut, &mut state.active_page);
                render_nav_tab(ui, "3  Edit", AppPage::Edit, &mut state.active_page);
                render_nav_tab(ui, "4  Export", AppPage::Export, &mut state.active_page);

                ui.add_space(20.0);
                ui.separator();
                ui.add_space(20.0);

                // Conductor indicator if generating
                if state.conductor.is_running {
                    ui.colored_label(
                        WARNING_AMBER,
                        format!(
                            "⚙ Building: {:.0}% ({})",
                            state.conductor.progress * 100.0,
                            state.conductor.status
                        ),
                    );
                } else if state.export.is_exporting {
                    ui.colored_label(
                        ACCENT_TEAL,
                        format!(
                            "🎬 Exporting: {:.0}% ({})",
                            state.export.progress * 100.0,
                            state.export.status
                        ),
                    );
                } else {
                    let dirty_label = if state.is_dirty {
                        "● Modified"
                    } else {
                        "✔ Saved"
                    };
                    let dirty_color = if state.is_dirty {
                        WARNING_AMBER
                    } else {
                        TEXT_MUTED
                    };
                    ui.colored_label(dirty_color, dirty_label);
                }

                // Push hardware disclosures to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(hw) = &state.hardware_report {
                        let gpu_info = if !hw.adapters.is_empty() {
                            format!("GPU: {}", hw.adapters[0].name)
                        } else {
                            "GPU: Software Fallback".to_string()
                        };

                        let proven_decoders: Vec<String> = hw
                            .decoders
                            .iter()
                            .filter_map(|d| d.status.implementation_name().map(|s| s.to_string()))
                            .collect();

                        let decoders_info = if !proven_decoders.is_empty() {
                            format!("Decoders: {}", proven_decoders.join(", "))
                        } else {
                            "Decoders: CPU Reference".to_string()
                        };

                        ui.colored_label(TEXT_MUTED, format!("{gpu_info} | {decoders_info}"));
                    } else {
                        ui.colored_label(TEXT_MUTED, "Hardware: Probing...");
                    }
                });
            });
        });
}

fn render_nav_tab(
    ui: &mut Ui,
    title: &str,
    page: AppPage,
    active_page: &mut AppPage,
) -> Response {
    let is_active = *active_page == page;
    let (bg, text_color, border_stroke) = if is_active {
        (
            ACCENT_TEAL_DARK,
            ACCENT_TEAL,
            Stroke::new(1.0_f32, ACCENT_TEAL),
        )
    } else {
        (
            SURFACE,
            TEXT_PRIMARY,
            Stroke::new(1.0_f32, Color32::TRANSPARENT),
        )
    };

    let btn = Button::new(egui::RichText::new(title).color(text_color).strong())
        .fill(bg)
        .stroke(border_stroke)
        .rounding(Rounding::same(4.0));

    let response = ui.add(btn);
    if response.clicked() {
        *active_page = page;
    }
    response
}
