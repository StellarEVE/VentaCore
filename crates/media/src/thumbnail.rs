//! Thumbnail generation and image downscaling for filmstrips.

use serde::{Deserialize, Serialize};
use ventacore_core::Tick;
use ventacore_platform::VideoFrame;

/// A downscaled thumbnail frame for timeline filmstrip display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThumbnailFrame {
    pub time: Tick,
    pub width: u32,
    pub height: u32,
    pub rgba_data: Vec<u8>,
}

impl ThumbnailFrame {
    pub fn new(time: Tick, width: u32, height: u32, rgba_data: Vec<u8>) -> Self {
        Self {
            time,
            width,
            height,
            rgba_data,
        }
    }

    /// Creates a downscaled thumbnail from a decoded `VideoFrame`.
    pub fn from_video_frame(frame: &VideoFrame, target_width: u32, target_height: u32) -> Self {
        let (src_rgba, src_w, src_h) = frame.to_oriented_rgba8();
        let downscaled = downscale_rgba8(&src_rgba, src_w, src_h, target_width, target_height);
        Self {
            time: frame.pts,
            width: target_width,
            height: target_height,
            rgba_data: downscaled,
        }
    }
}

/// Downscales an RGBA8 buffer using area averaging (box filter).
pub fn downscale_rgba8(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Vec<u8> {
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return Vec::new();
    }

    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
    let x_ratio = src_w as f64 / dst_w as f64;
    let y_ratio = src_h as f64 / dst_h as f64;

    for dy in 0..dst_h {
        let sy_start = (dy as f64 * y_ratio).floor() as u32;
        let sy_end = (((dy + 1) as f64 * y_ratio).ceil() as u32).min(src_h);

        for dx in 0..dst_w {
            let sx_start = (dx as f64 * x_ratio).floor() as u32;
            let sx_end = (((dx + 1) as f64 * x_ratio).ceil() as u32).min(src_w);

            let mut sum_r = 0u64;
            let mut sum_g = 0u64;
            let mut sum_b = 0u64;
            let mut sum_a = 0u64;
            let mut count = 0u64;

            for sy in sy_start..sy_end {
                for sx in sx_start..sx_end {
                    let idx = ((sy * src_w + sx) * 4) as usize;
                    if idx + 3 < src.len() {
                        sum_r += src[idx] as u64;
                        sum_g += src[idx + 1] as u64;
                        sum_b += src[idx + 2] as u64;
                        sum_a += src[idx + 3] as u64;
                        count += 1;
                    }
                }
            }

            let out_idx = ((dy * dst_w + dx) * 4) as usize;
            if count > 0 {
                dst[out_idx] = (sum_r / count) as u8;
                dst[out_idx + 1] = (sum_g / count) as u8;
                dst[out_idx + 2] = (sum_b / count) as u8;
                dst[out_idx + 3] = (sum_a / count) as u8;
            } else {
                dst[out_idx + 3] = 255;
            }
        }
    }

    dst
}
