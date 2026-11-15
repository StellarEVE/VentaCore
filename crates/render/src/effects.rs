//! Video Effects and Adjustment Layers for VentaCore
//!
//! Conforms to Stage 16b requirements:
//! - Color adjustments: Brightness, Contrast, Saturation.
//! - Dynamic motion: BoxBlur, Shake, Strobe, Glitch, BeatPulse.
//! - Pure CPU reference implementation conforming to Charter Rule 5.

use crate::types::RenderFrame;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VideoEffect {
    Brightness(f32),
    Contrast(f32),
    Saturation(f32),
    BoxBlur(u32),
    Shake { intensity: f32, speed: f32 },
    Strobe { frequency_hz: f32, duty_cycle: f32 },
    Glitch { amount: f32 },
    BeatPulse { beat_energy: f32, pulse_strength: f32 },
    TextOverlay {
        text: String,
        font_size: f32,
        x: f32,
        y: f32,
        color: [u8; 4],
    },
}

impl VideoEffect {
    /// Parse from a canonical text description string.
    pub fn parse_canonical(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        let name = parts[0].trim().to_lowercase();
        let val = parts.get(1).map(|v| v.trim()).unwrap_or("");

        match name.as_str() {
            "brightness" => val.parse::<f32>().ok().map(VideoEffect::Brightness),
            "contrast" => val.parse::<f32>().ok().map(VideoEffect::Contrast),
            "saturation" => val.parse::<f32>().ok().map(VideoEffect::Saturation),
            "blur" => val.parse::<u32>().ok().map(VideoEffect::BoxBlur),
            "glitch" => val.parse::<f32>().ok().map(|amount| VideoEffect::Glitch { amount }),
            "strobe" => val.parse::<f32>().ok().map(|hz| VideoEffect::Strobe {
                frequency_hz: hz,
                duty_cycle: 0.5,
            }),
            "beat_pulse" => val.parse::<f32>().ok().map(|strength| VideoEffect::BeatPulse {
                beat_energy: 1.0,
                pulse_strength: strength,
            }),
            "text" => Some(VideoEffect::TextOverlay {
                text: val.to_string(),
                font_size: 24.0,
                x: 0.5,
                y: 0.5,
                color: [255, 255, 255, 255],
            }),
            _ => None,
        }
    }
}

/// Applies a list of effects sequentially to a RenderFrame in pure software (Charter Rule 5).
pub fn apply_effects_to_frame(frame: &mut RenderFrame, effects: &[VideoEffect], time_sec: f64) {
    for effect in effects {
        match effect {
            VideoEffect::Brightness(b) => apply_brightness(frame, *b),
            VideoEffect::Contrast(c) => apply_contrast(frame, *c),
            VideoEffect::Saturation(s) => apply_saturation(frame, *s),
            VideoEffect::BoxBlur(radius) => apply_box_blur(frame, *radius),
            VideoEffect::Glitch { amount } => apply_glitch(frame, *amount),
            VideoEffect::Strobe { frequency_hz, duty_cycle } => {
                apply_strobe(frame, *frequency_hz, *duty_cycle, time_sec);
            }
            VideoEffect::BeatPulse { beat_energy, pulse_strength } => {
                let scale = 1.0 + beat_energy * pulse_strength * 0.1;
                apply_contrast(frame, scale);
            }
            VideoEffect::Shake { intensity, speed } => {
                apply_shake(frame, *intensity, *speed, time_sec);
            }
            VideoEffect::TextOverlay { text, font_size, x, y, color } => {
                apply_simple_text_overlay(frame, text, *font_size, *x, *y, *color);
            }
        }
    }
}

fn apply_brightness(frame: &mut RenderFrame, amount: f32) {
    let delta = (amount * 255.0) as i32;
    for chunk in frame.pixels.chunks_exact_mut(4) {
        chunk[0] = (chunk[0] as i32 + delta).clamp(0, 255) as u8;
        chunk[1] = (chunk[1] as i32 + delta).clamp(0, 255) as u8;
        chunk[2] = (chunk[2] as i32 + delta).clamp(0, 255) as u8;
    }
}

fn apply_contrast(frame: &mut RenderFrame, factor: f32) {
    for chunk in frame.pixels.chunks_exact_mut(4) {
        for c in 0..3 {
            let val = ((chunk[c] as f32 - 128.0) * factor + 128.0).round();
            chunk[c] = val.clamp(0.0, 255.0) as u8;
        }
    }
}

fn apply_saturation(frame: &mut RenderFrame, factor: f32) {
    for chunk in frame.pixels.chunks_exact_mut(4) {
        let r = chunk[0] as f32;
        let g = chunk[1] as f32;
        let b = chunk[2] as f32;
        let gray = 0.299 * r + 0.587 * g + 0.114 * b;

        chunk[0] = ((r - gray) * factor + gray).clamp(0.0, 255.0) as u8;
        chunk[1] = ((g - gray) * factor + gray).clamp(0.0, 255.0) as u8;
        chunk[2] = ((b - gray) * factor + gray).clamp(0.0, 255.0) as u8;
    }
}

fn apply_box_blur(frame: &mut RenderFrame, radius: u32) {
    if radius == 0 || frame.width == 0 || frame.height == 0 {
        return;
    }
    let r = radius.min(8) as usize;
    let w = frame.width as usize;
    let h = frame.height as usize;
    let mut temp = frame.pixels.clone();

    // Horizontal pass
    for y in 0..h {
        for x in 0..w {
            let start_x = x.saturating_sub(r);
            let end_x = (x + r + 1).min(w);
            let count = (end_x - start_x) as u32;

            let mut sum_r = 0u32;
            let mut sum_g = 0u32;
            let mut sum_b = 0u32;

            for sx in start_x..end_x {
                let idx = (y * w + sx) * 4;
                sum_r += frame.pixels[idx] as u32;
                sum_g += frame.pixels[idx + 1] as u32;
                sum_b += frame.pixels[idx + 2] as u32;
            }

            let out_idx = (y * w + x) * 4;
            temp[out_idx] = (sum_r / count) as u8;
            temp[out_idx + 1] = (sum_g / count) as u8;
            temp[out_idx + 2] = (sum_b / count) as u8;
        }
    }

    frame.pixels = temp;
}

fn apply_glitch(frame: &mut RenderFrame, amount: f32) {
    if amount <= 0.0 || frame.width == 0 || frame.height == 0 {
        return;
    }
    let shift = (amount * 20.0).round() as usize;
    let w = frame.width as usize;
    let h = frame.height as usize;

    // Displace red channel by shift pixels horizontally on odd scanlines
    for y in (0..h).step_by(2) {
        for x in 0..w.saturating_sub(shift) {
            let src_idx = (y * w + x + shift) * 4;
            let dst_idx = (y * w + x) * 4;
            frame.pixels[dst_idx] = frame.pixels[src_idx]; // shift Red
        }
    }
}

fn apply_strobe(frame: &mut RenderFrame, frequency_hz: f32, duty_cycle: f32, time_sec: f64) {
    if frequency_hz <= 0.0 {
        return;
    }
    let phase = (time_sec * frequency_hz as f64).fract();
    if phase > duty_cycle as f64 {
        // Flash to solid white
        for chunk in frame.pixels.chunks_exact_mut(4) {
            chunk[0] = 255;
            chunk[1] = 255;
            chunk[2] = 255;
        }
    }
}

fn apply_shake(frame: &mut RenderFrame, intensity: f32, speed: f32, time_sec: f64) {
    if intensity <= 0.0 || frame.width == 0 || frame.height == 0 {
        return;
    }
    let shift_x = ((time_sec * speed as f64 * 10.0).sin() * intensity as f64 * 10.0) as i32;
    let shift_y = ((time_sec * speed as f64 * 13.0).cos() * intensity as f64 * 10.0) as i32;

    if shift_x == 0 && shift_y == 0 {
        return;
    }

    let w = frame.width as i32;
    let h = frame.height as i32;
    let orig = frame.pixels.clone();

    for y in 0..h {
        let src_y = (y + shift_y).clamp(0, h - 1);
        for x in 0..w {
            let src_x = (x + shift_x).clamp(0, w - 1);
            let dst_idx = ((y * w + x) * 4) as usize;
            let src_idx = ((src_y * w + src_x) * 4) as usize;
            frame.pixels[dst_idx..dst_idx + 4].copy_from_slice(&orig[src_idx..src_idx + 4]);
        }
    }
}

fn apply_simple_text_overlay(
    frame: &mut RenderFrame,
    _text: &str,
    _font_size: f32,
    norm_x: f32,
    norm_y: f32,
    color: [u8; 4],
) {
    // Draws a small indicator badge on the frame
    let cx = (norm_x * frame.width as f32) as i32;
    let cy = (norm_y * frame.height as f32) as i32;
    let box_size = 12;

    for dy in -box_size..=box_size {
        let y = cy + dy;
        if y < 0 || y >= frame.height as i32 {
            continue;
        }
        for dx in -box_size..=box_size {
            let x = cx + dx;
            if x < 0 || x >= frame.width as i32 {
                continue;
            }
            let idx = ((y * frame.width as i32 + x) * 4) as usize;
            frame.pixels[idx..idx + 4].copy_from_slice(&color);
        }
    }
}
