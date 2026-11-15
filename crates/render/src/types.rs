//! Core rendering types: resolution tiers, fit modes, 2D transforms, transitions, and frame buffers.

use crate::error::RenderError;
use serde::{Deserialize, Serialize};

/// Resolution tiers for preview performance scaling and export generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ResolutionTier {
    #[default]
    Full,
    Half,
    Quarter,
    Eighth,
}

impl ResolutionTier {
    /// Fractional scale factor of this tier relative to full resolution.
    #[inline]
    pub fn scale_factor(&self) -> f32 {
        match self {
            ResolutionTier::Full => 1.0,
            ResolutionTier::Half => 0.5,
            ResolutionTier::Quarter => 0.25,
            ResolutionTier::Eighth => 0.125,
        }
    }

    /// Computes scaled (width, height) guaranteed to be at least 1x1
    /// and even-aligned for video codec compatibility.
    pub fn scale_dimensions(&self, width: u32, height: u32) -> (u32, u32) {
        let scale = self.scale_factor();
        let mut w = ((width as f32 * scale).round() as u32).max(1);
        let mut h = ((height as f32 * scale).round() as u32).max(1);
        if w > 1 && w % 2 != 0 {
            w += 1;
        }
        if h > 1 && h % 2 != 0 {
            h += 1;
        }
        (w, h)
    }
}

/// Fit mode determining how source video footage is mapped to canvas aspect ratios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum FitMode {
    /// Scale source to completely cover canvas; crop excess overflow symmetrically.
    #[default]
    Crop,
    /// Preserve source aspect ratio; fit within canvas with solid black letterbox/pillarbox bars.
    Letterbox,
    /// Preserve source aspect ratio; fill padding bars with blurred, stretched source image.
    BlurPadding,
}

/// 2D affine transformation parameters applied to a video layer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform2D {
    /// Normalized translation offset [x, y], where [0.0, 0.0] is centered.
    pub position: [f32; 2],
    /// Scale multiplier [sx, sy], where [1.0, 1.0] is nominal 100% scale.
    pub scale: [f32; 2],
    /// Rotation angle in radians clockwise.
    pub rotation_radians: f32,
    /// Opacity factor between 0.0 (transparent) and 1.0 (fully opaque).
    pub opacity: f32,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            scale: [1.0, 1.0],
            rotation_radians: 0.0,
            opacity: 1.0,
        }
    }
}

/// Type of transition between two video frames.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum TransitionKind {
    #[default]
    None,
    /// Linear cross-dissolve blend: (1 - t) * A + t * B.
    CrossDissolve,
    /// Fade down to color (e.g. black) and then fade up into incoming frame.
    DipToColor([u8; 4]),
    /// Horizontal wipe from left to right.
    WipeLeft,
    /// Horizontal wipe from right to left.
    WipeRight,
    /// Slide/push transition from left to right.
    PushLeft,
    /// Slide/push transition from right to left.
    PushRight,
}

/// Parameters driving a frame transition.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct TransitionParams {
    /// Progress normalized from 0.0 (frame A) to 1.0 (frame B).
    pub progress: f32,
    /// The kind of transition to apply.
    pub kind: TransitionKind,
}

impl TransitionParams {
    pub fn new(kind: TransitionKind, progress: f32) -> Self {
        Self {
            kind,
            progress: progress.clamp(0.0, 1.0),
        }
    }

    pub fn cross_dissolve(progress: f32) -> Self {
        Self::new(TransitionKind::CrossDissolve, progress)
    }

    pub fn dip_to_black(progress: f32) -> Self {
        Self::new(TransitionKind::DipToColor([0, 0, 0, 255]), progress)
    }
}

/// An uncompressed 8-bit RGBA video frame buffer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl RenderFrame {
    /// Creates a zeroed (black transparent) frame buffer.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0u8; (width * height * 4) as usize],
        }
    }

    /// Creates a frame buffer filled with a solid RGBA color.
    pub fn from_solid_color(width: u32, height: u32, color: [u8; 4]) -> Self {
        let count = (width * height) as usize;
        let mut pixels = Vec::with_capacity(count * 4);
        for _ in 0..count {
            pixels.extend_from_slice(&color);
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Wraps an existing RGBA8 pixel buffer with size validation.
    pub fn from_rgba8(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, RenderError> {
        let expected = (width * height * 4) as usize;
        if pixels.len() != expected {
            return Err(RenderError::BufferSizeMismatch {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Retrieves the RGBA pixel value at (x, y). Returns [0, 0, 0, 0] if out of bounds.
    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 0];
        }
        let idx = ((y * self.width + x) * 4) as usize;
        [
            self.pixels[idx],
            self.pixels[idx + 1],
            self.pixels[idx + 2],
            self.pixels[idx + 3],
        ]
    }

    /// Sets the RGBA pixel value at (x, y). No-op if out of bounds.
    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 4) as usize;
            self.pixels[idx] = rgba[0];
            self.pixels[idx + 1] = rgba[1];
            self.pixels[idx + 2] = rgba[2];
            self.pixels[idx + 3] = rgba[3];
        }
    }

    /// Computes the maximum absolute channel difference against another frame.
    /// Used for testing GPU output against CPU reference oracle (Charter Rule 5: max diff <= 1).
    pub fn max_channel_difference(&self, other: &RenderFrame) -> u8 {
        assert_eq!(self.width, other.width);
        assert_eq!(self.height, other.height);
        assert_eq!(self.pixels.len(), other.pixels.len());

        let mut max_diff: u8 = 0;
        for (a, b) in self.pixels.iter().zip(other.pixels.iter()) {
            let diff = (*a as i16 - *b as i16).abs() as u8;
            if diff > max_diff {
                max_diff = diff;
            }
        }
        max_diff
    }
}
