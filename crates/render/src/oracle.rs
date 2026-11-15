//! CPU Reference Oracle for graphics rendering.
//!
//! Provides an exact analytical software renderer for hard cuts, transitions,
//! 2D transformations, and fit modes. Used as ground truth in automated tests
//! and as the guaranteed software fallback when GPU hardware is unavailable
//! (Charter Rule 5: Graphics first, never graphics only).

use crate::types::{FitMode, RenderFrame, Transform2D, TransitionKind, TransitionParams};

/// CPU software reference renderer.
#[derive(Debug, Clone, Default)]
pub struct CpuReferenceOracle;

impl CpuReferenceOracle {
    pub fn new() -> Self {
        Self
    }

    /// Renders a single video frame with fit mode and 2D transform onto a target canvas.
    pub fn render_cut(
        &self,
        source: &RenderFrame,
        target_width: u32,
        target_height: u32,
        fit: FitMode,
        transform: &Transform2D,
    ) -> RenderFrame {
        if target_width == 0 || target_height == 0 {
            return RenderFrame::new(0, 0);
        }

        let mut output = match fit {
            FitMode::Crop => self.render_crop(source, target_width, target_height),
            FitMode::Letterbox => self.render_letterbox(source, target_width, target_height),
            FitMode::BlurPadding => self.render_blur_padding(source, target_width, target_height),
        };

        // Apply 2D transform if not identity
        if is_non_identity_transform(transform) {
            output = self.apply_transform(&output, transform);
        }

        output
    }

    /// Renders a transition between two video frames onto a target canvas.
    pub fn render_transition(
        &self,
        from_frame: &RenderFrame,
        to_frame: &RenderFrame,
        target_width: u32,
        target_height: u32,
        fit: FitMode,
        params: &TransitionParams,
    ) -> RenderFrame {
        let t = params.progress.clamp(0.0, 1.0);

        // Fast shortcuts for boundary values
        if t <= 0.0 {
            return self.render_cut(from_frame, target_width, target_height, fit, &Transform2D::default());
        }
        if t >= 1.0 {
            return self.render_cut(to_frame, target_width, target_height, fit, &Transform2D::default());
        }

        let canvas_a = self.render_cut(from_frame, target_width, target_height, fit, &Transform2D::default());
        let canvas_b = self.render_cut(to_frame, target_width, target_height, fit, &Transform2D::default());

        let mut output = RenderFrame::new(target_width, target_height);
        let pixel_count = (target_width * target_height) as usize;

        match params.kind {
            TransitionKind::None => {
                if t < 0.5 {
                    return canvas_a;
                } else {
                    return canvas_b;
                }
            }
            TransitionKind::CrossDissolve => {
                let inv_t = 1.0 - t;
                for i in 0..pixel_count {
                    let idx = i * 4;
                    for c in 0..4 {
                        let val = inv_t * canvas_a.pixels[idx + c] as f32 + t * canvas_b.pixels[idx + c] as f32;
                        output.pixels[idx + c] = (val + 0.5).clamp(0.0, 255.0) as u8;
                    }
                }
            }
            TransitionKind::DipToColor(dip_color) => {
                if t <= 0.5 {
                    // Blend canvas_a to dip_color
                    let k = t * 2.0;
                    let inv_k = 1.0 - k;
                    for i in 0..pixel_count {
                        let idx = i * 4;
                        for c in 0..4 {
                            let val = inv_k * canvas_a.pixels[idx + c] as f32 + k * dip_color[c] as f32;
                            output.pixels[idx + c] = (val + 0.5).clamp(0.0, 255.0) as u8;
                        }
                    }
                } else {
                    // Blend dip_color to canvas_b
                    let k = (t - 0.5) * 2.0;
                    let inv_k = 1.0 - k;
                    for i in 0..pixel_count {
                        let idx = i * 4;
                        for c in 0..4 {
                            let val = inv_k * dip_color[c] as f32 + k * canvas_b.pixels[idx + c] as f32;
                            output.pixels[idx + c] = (val + 0.5).clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
            TransitionKind::WipeLeft => {
                let split_x = (t * target_width as f32).round() as u32;
                for y in 0..target_height {
                    for x in 0..target_width {
                        let pixel = if x < split_x {
                            canvas_b.get_pixel(x, y)
                        } else {
                            canvas_a.get_pixel(x, y)
                        };
                        output.set_pixel(x, y, pixel);
                    }
                }
            }
            TransitionKind::WipeRight => {
                let split_x = ((1.0 - t) * target_width as f32).round() as u32;
                for y in 0..target_height {
                    for x in 0..target_width {
                        let pixel = if x >= split_x {
                            canvas_b.get_pixel(x, y)
                        } else {
                            canvas_a.get_pixel(x, y)
                        };
                        output.set_pixel(x, y, pixel);
                    }
                }
            }
            TransitionKind::PushLeft => {
                let shift = (t * target_width as f32).round() as i32;
                for y in 0..target_height {
                    for x in 0..target_width {
                        let src_x_a = x as i32 + shift;
                        let src_x_b = x as i32 - (target_width as i32 - shift);
                        let pixel = if src_x_a < target_width as i32 {
                            canvas_a.get_pixel(src_x_a as u32, y)
                        } else if src_x_b >= 0 {
                            canvas_b.get_pixel(src_x_b as u32, y)
                        } else {
                            [0, 0, 0, 255]
                        };
                        output.set_pixel(x, y, pixel);
                    }
                }
            }
            TransitionKind::PushRight => {
                let shift = (t * target_width as f32).round() as i32;
                for y in 0..target_height {
                    for x in 0..target_width {
                        let src_x_a = x as i32 - shift;
                        let src_x_b = x as i32 + (target_width as i32 - shift);
                        let pixel = if src_x_a >= 0 {
                            canvas_a.get_pixel(src_x_a as u32, y)
                        } else if src_x_b < target_width as i32 {
                            canvas_b.get_pixel(src_x_b as u32, y)
                        } else {
                            [0, 0, 0, 255]
                        };
                        output.set_pixel(x, y, pixel);
                    }
                }
            }
        }

        output
    }

    /// Renders with FitMode::Letterbox: fits source inside target, adds solid black bars.
    fn render_letterbox(&self, source: &RenderFrame, target_width: u32, target_height: u32) -> RenderFrame {
        let mut output = RenderFrame::from_solid_color(target_width, target_height, [0, 0, 0, 255]);
        if source.width == 0 || source.height == 0 {
            return output;
        }

        let ar_src = source.width as f32 / source.height as f32;
        let ar_tgt = target_width as f32 / target_height as f32;

        let (fit_w, fit_h, offset_x, offset_y) = if ar_src >= ar_tgt {
            let fw = target_width as f32;
            let fh = (fw / ar_src).round();
            let oy = ((target_height as f32 - fh) * 0.5).floor();
            (fw as u32, fh as u32, 0, oy as i32)
        } else {
            let fh = target_height as f32;
            let fw = (fh * ar_src).round();
            let ox = ((target_width as f32 - fw) * 0.5).floor();
            (fw as u32, fh as u32, ox as i32, 0)
        };

        for y in 0..fit_h {
            let dst_y = (offset_y + y as i32) as u32;
            if dst_y >= target_height {
                continue;
            }
            let v = (y as f32 + 0.5) * (source.height as f32 / fit_h as f32);

            for x in 0..fit_w {
                let dst_x = (offset_x + x as i32) as u32;
                if dst_x >= target_width {
                    continue;
                }
                let u = (x as f32 + 0.5) * (source.width as f32 / fit_w as f32);
                let pixel = sample_bilinear(source, u, v);
                output.set_pixel(dst_x, dst_y, pixel);
            }
        }

        output
    }

    /// Renders with FitMode::Crop: scales source to completely cover target, crops overflow symmetrically.
    fn render_crop(&self, source: &RenderFrame, target_width: u32, target_height: u32) -> RenderFrame {
        let mut output = RenderFrame::new(target_width, target_height);
        if source.width == 0 || source.height == 0 {
            return output;
        }

        let ar_src = source.width as f32 / source.height as f32;
        let ar_tgt = target_width as f32 / target_height as f32;

        let (crop_w, crop_h, crop_u0, crop_v0) = if ar_src >= ar_tgt {
            // Source is wider: match height, crop width
            let ch = source.height as f32;
            let cw = ch * ar_tgt;
            let u0 = (source.width as f32 - cw) * 0.5;
            (cw, ch, u0, 0.0)
        } else {
            // Source is taller: match width, crop height
            let cw = source.width as f32;
            let ch = cw / ar_tgt;
            let v0 = (source.height as f32 - ch) * 0.5;
            (cw, ch, 0.0, v0)
        };

        for y in 0..target_height {
            let v = crop_v0 + (y as f32 + 0.5) * (crop_h / target_height as f32);
            for x in 0..target_width {
                let u = crop_u0 + (x as f32 + 0.5) * (crop_w / target_width as f32);
                let pixel = sample_bilinear(source, u, v);
                output.set_pixel(x, y, pixel);
            }
        }

        output
    }

    /// Renders with FitMode::BlurPadding: pads borders with stretched, blurred source image.
    fn render_blur_padding(&self, source: &RenderFrame, target_width: u32, target_height: u32) -> RenderFrame {
        if source.width == 0 || source.height == 0 {
            return RenderFrame::from_solid_color(target_width, target_height, [0, 0, 0, 255]);
        }

        // 1. Generate low-res thumbnail and blur it for background fill
        let low_w = 16u32;
        let low_h = 9u32;
        let mut low_res = RenderFrame::new(low_w, low_h);
        for ly in 0..low_h {
            let v = (ly as f32 + 0.5) * (source.height as f32 / low_h as f32);
            for lx in 0..low_w {
                let u = (lx as f32 + 0.5) * (source.width as f32 / low_w as f32);
                low_res.set_pixel(lx, ly, sample_bilinear(source, u, v));
            }
        }

        // Smooth low-res buffer with 3x3 box blur
        let mut blurred_low = low_res.clone();
        for ly in 0..low_h {
            for lx in 0..low_w {
                let mut sum_r = 0u32;
                let mut sum_g = 0u32;
                let mut sum_b = 0u32;
                let mut count = 0u32;

                for dy in -1i32..=1i32 {
                    let ny = ly as i32 + dy;
                    if ny < 0 || ny >= low_h as i32 {
                        continue;
                    }
                    for dx in -1i32..=1i32 {
                        let nx = lx as i32 + dx;
                        if nx < 0 || nx >= low_w as i32 {
                            continue;
                        }
                        let p = low_res.get_pixel(nx as u32, ny as u32);
                        sum_r += p[0] as u32;
                        sum_g += p[1] as u32;
                        sum_b += p[2] as u32;
                        count += 1;
                    }
                }
                blurred_low.set_pixel(
                    lx,
                    ly,
                    [
                        (sum_r / count) as u8,
                        (sum_g / count) as u8,
                        (sum_b / count) as u8,
                        255,
                    ],
                );
            }
        }

        // 2. Upscale blurred background onto target canvas
        let mut output = RenderFrame::new(target_width, target_height);
        for y in 0..target_height {
            let v = (y as f32 + 0.5) * (low_h as f32 / target_height as f32);
            for x in 0..target_width {
                let u = (x as f32 + 0.5) * (low_w as f32 / target_width as f32);
                // Slight darken for contrast behind active video
                let bg = sample_bilinear(&blurred_low, u, v);
                let dimmed = [
                    ((bg[0] as f32 * 0.7) + 0.5) as u8,
                    ((bg[1] as f32 * 0.7) + 0.5) as u8,
                    ((bg[2] as f32 * 0.7) + 0.5) as u8,
                    255,
                ];
                output.set_pixel(x, y, dimmed);
            }
        }

        // 3. Draw the crisp letterboxed source over the blurred background
        let ar_src = source.width as f32 / source.height as f32;
        let ar_tgt = target_width as f32 / target_height as f32;

        let (fit_w, fit_h, offset_x, offset_y) = if ar_src >= ar_tgt {
            let fw = target_width as f32;
            let fh = (fw / ar_src).round();
            let oy = ((target_height as f32 - fh) * 0.5).floor();
            (fw as u32, fh as u32, 0, oy as i32)
        } else {
            let fh = target_height as f32;
            let fw = (fh * ar_src).round();
            let ox = ((target_width as f32 - fw) * 0.5).floor();
            (fw as u32, fh as u32, ox as i32, 0)
        };

        for y in 0..fit_h {
            let dst_y = (offset_y + y as i32) as u32;
            if dst_y >= target_height {
                continue;
            }
            let v = (y as f32 + 0.5) * (source.height as f32 / fit_h as f32);

            for x in 0..fit_w {
                let dst_x = (offset_x + x as i32) as u32;
                if dst_x >= target_width {
                    continue;
                }
                let u = (x as f32 + 0.5) * (source.width as f32 / fit_w as f32);
                let pixel = sample_bilinear(source, u, v);
                output.set_pixel(dst_x, dst_y, pixel);
            }
        }

        output
    }

    /// Applies 2D translation, scale, rotation, and opacity.
    fn apply_transform(&self, input: &RenderFrame, transform: &Transform2D) -> RenderFrame {
        let mut output = RenderFrame::from_solid_color(input.width, input.height, [0, 0, 0, 255]);
        let cx = input.width as f32 * 0.5;
        let cy = input.height as f32 * 0.5;

        let tx = transform.position[0] * cx;
        let ty = transform.position[1] * cy;

        let sx = if transform.scale[0].abs() < 1e-6 { 1e-6 } else { transform.scale[0] };
        let sy = if transform.scale[1].abs() < 1e-6 { 1e-6 } else { transform.scale[1] };

        let cos_r = transform.rotation_radians.cos();
        let sin_r = transform.rotation_radians.sin();
        let opacity = transform.opacity.clamp(0.0, 1.0);

        for y in 0..input.height {
            for x in 0..input.width {
                // Shift to center and un-translate
                let px = (x as f32 + 0.5) - cx - tx;
                let py = (y as f32 + 0.5) - cy - ty;

                // Inverse rotate
                let rx = px * cos_r + py * sin_r;
                let ry = -px * sin_r + py * cos_r;

                // Inverse scale
                let u = (rx / sx) + cx;
                let v = (ry / sy) + cy;

                if u >= 0.0 && u < input.width as f32 && v >= 0.0 && v < input.height as f32 {
                    let mut sampled = sample_bilinear(input, u, v);
                    if opacity < 1.0 {
                        sampled[0] = ((sampled[0] as f32 * opacity) + 0.5) as u8;
                        sampled[1] = ((sampled[1] as f32 * opacity) + 0.5) as u8;
                        sampled[2] = ((sampled[2] as f32 * opacity) + 0.5) as u8;
                        sampled[3] = ((sampled[3] as f32 * opacity) + 0.5) as u8;
                    }
                    output.set_pixel(x, y, sampled);
                }
            }
        }

        output
    }
}

/// Helper: evaluates bilinear pixel sample at float coordinates (u, v).
#[inline]
fn sample_bilinear(frame: &RenderFrame, u: f32, v: f32) -> [u8; 4] {
    let u_clamped = u.clamp(0.5, frame.width as f32 - 0.5) - 0.5;
    let v_clamped = v.clamp(0.5, frame.height as f32 - 0.5) - 0.5;

    let x0 = u_clamped.floor() as u32;
    let y0 = v_clamped.floor() as u32;
    let x1 = (x0 + 1).min(frame.width.saturating_sub(1));
    let y1 = (y0 + 1).min(frame.height.saturating_sub(1));

    let fx = u_clamped - x0 as f32;
    let fy = v_clamped - y0 as f32;

    let p00 = frame.get_pixel(x0, y0);
    let p10 = frame.get_pixel(x1, y0);
    let p01 = frame.get_pixel(x0, y1);
    let p11 = frame.get_pixel(x1, y1);

    let mut out = [0u8; 4];
    for c in 0..4 {
        let top = (1.0 - fx) * p00[c] as f32 + fx * p10[c] as f32;
        let bottom = (1.0 - fx) * p01[c] as f32 + fx * p11[c] as f32;
        let val = (1.0 - fy) * top + fy * bottom;
        out[c] = (val + 0.5).clamp(0.0, 255.0) as u8;
    }
    out
}

#[inline]
fn is_non_identity_transform(t: &Transform2D) -> bool {
    t.position[0] != 0.0
        || t.position[1] != 0.0
        || t.scale[0] != 1.0
        || t.scale[1] != 1.0
        || t.rotation_radians != 0.0
        || t.opacity != 1.0
}
