//! Video surface widget for egui.
//!
//! Conforms to Part 4b & Stage R3:
//! - Takes raw RGBA frame bytes from the playback engine.
//! - Uploads to an in-place `egui::TextureHandle`.
//! - Draws the frame aspect-fitted inside the monitor bounds with solid black letterbox/pillarbox bars.

use eframe::egui;
use ventacore_engine::playback::PlayerFrame;

pub struct VideoSurface {
    texture: Option<egui::TextureHandle>,
}

impl Default for VideoSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoSurface {
    pub fn new() -> Self {
        Self { texture: None }
    }

    /// Renders the video surface inside `desired_size`.
    /// Uploads frame pixels to the GPU texture handle, maintains aspect ratio,
    /// and draws black padding bars when aspect ratios differ.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        frame: Option<&PlayerFrame>,
        desired_size: egui::Vec2,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

        // Always paint background with solid black
        ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);

        if let Some(f) = frame {
            if f.width > 0 && f.height > 0 && !f.rgba.is_empty() {
                let img = egui::ColorImage::from_rgba_unmultiplied(
                    [f.width as usize, f.height as usize],
                    &f.rgba,
                );

                match &mut self.texture {
                    Some(tex) => {
                        tex.set(img, egui::TextureOptions::LINEAR);
                    }
                    None => {
                        self.texture = Some(ui.ctx().load_texture(
                            "video_surface_texture",
                            img,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }

                if let Some(tex) = &self.texture {
                    let video_aspect = f.width as f32 / f.height as f32;
                    let area_aspect = rect.width() / rect.height().max(1.0);

                    let fitted_rect = if area_aspect > video_aspect {
                        // Pillarbox: video touches top and bottom, black bars on sides
                        let fitted_w = rect.height() * video_aspect;
                        let offset_x = (rect.width() - fitted_w) / 2.0;
                        egui::Rect::from_min_size(
                            egui::pos2(rect.min.x + offset_x, rect.min.y),
                            egui::vec2(fitted_w, rect.height()),
                        )
                    } else {
                        // Letterbox: video touches left and right, black bars on top and bottom
                        let fitted_h = rect.width() / video_aspect;
                        let offset_y = (rect.height() - fitted_h) / 2.0;
                        egui::Rect::from_min_size(
                            egui::pos2(rect.min.x, rect.min.y + offset_y),
                            egui::vec2(rect.width(), fitted_h),
                        )
                    };

                    ui.painter().image(
                        tex.id(),
                        fitted_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
        }

        response
    }
}
