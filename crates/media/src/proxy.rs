//! Lightweight intra-frame video proxies for instant seeking and responsive scrubbing.
//!
//! Long GOP files (H.264/H.265) require decoding from the nearest keyframe when seeking,
//! causing stuttering on low-end hardware. Intra-frame proxies decode each frame once,
//! downscale to a fast preview tier, and store each frame independently for O(1) random access.

use crate::error::MediaError;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use ventacore_core::Tick;
use ventacore_platform::traits::VideoDecoder;

/// Proxy resolution tier for downscaling footage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ProxyTier {
    /// 50% scale (0.5x width, 0.5x height).
    #[default]
    Half,
    /// 25% scale (0.25x width, 0.25x height).
    Quarter,
    /// 12.5% scale (0.125x width, 0.125x height).
    Eighth,
}

impl ProxyTier {
    pub fn scale_factor(&self) -> f32 {
        match self {
            ProxyTier::Half => 0.5,
            ProxyTier::Quarter => 0.25,
            ProxyTier::Eighth => 0.125,
        }
    }

    pub fn folder_name(&self) -> &'static str {
        match self {
            ProxyTier::Half => "half",
            ProxyTier::Quarter => "quarter",
            ProxyTier::Eighth => "eighth",
        }
    }

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

/// A decoded intra-frame proxy buffer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyFrame {
    pub time: Tick,
    pub width: u32,
    pub height: u32,
    pub rgba_data: Vec<u8>,
}

impl ProxyFrame {
    pub fn new(time: Tick, width: u32, height: u32, rgba_data: Vec<u8>) -> Self {
        Self {
            time,
            width,
            height,
            rgba_data,
        }
    }
}

/// Metadata index for an intra-frame proxy stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyIndex {
    pub fingerprint: String,
    pub tier: ProxyTier,
    pub width: u32,
    pub height: u32,
    pub frame_count: usize,
    pub timecodes: Vec<Tick>,
}

/// Helper function to downscale an RGBA8 buffer using bilinear area sampling.
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
    if src_w == dst_w && src_h == dst_h {
        return src.to_vec();
    }

    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
    let x_ratio = src_w as f32 / dst_w as f32;
    let y_ratio = src_h as f32 / dst_h as f32;

    for dy in 0..dst_h {
        let sy = ((dy as f32 + 0.5) * y_ratio).floor() as u32;
        let clamped_sy = sy.min(src_h - 1);
        for dx in 0..dst_w {
            let sx = ((dx as f32 + 0.5) * x_ratio).floor() as u32;
            let clamped_sx = sx.min(src_w - 1);

            let src_idx = ((clamped_sy * src_w + clamped_sx) * 4) as usize;
            let dst_idx = ((dy * dst_w + dx) * 4) as usize;

            if src_idx + 3 < src.len() && dst_idx + 3 < dst.len() {
                dst[dst_idx] = src[src_idx];
                dst[dst_idx + 1] = src[src_idx + 1];
                dst[dst_idx + 2] = src[src_idx + 2];
                dst[dst_idx + 3] = src[src_idx + 3];
            }
        }
    }

    dst
}

/// Generates an intra-frame proxy on disk from a VideoDecoder.
pub fn generate_intra_proxy(
    decoder: &mut dyn VideoDecoder,
    cache_dir: &Path,
    fingerprint: &str,
    tier: ProxyTier,
) -> Result<ProxyIndex, MediaError> {
    let meta = decoder.metadata().clone();
    let (proxy_w, proxy_h) = tier.scale_dimensions(meta.width, meta.height);
    let proxy_dir = cache_dir.join(fingerprint).join("proxies").join(tier.folder_name());
    fs::create_dir_all(&proxy_dir)?;

    decoder.seek(Tick::ZERO)?;

    let mut timecodes = Vec::new();
    let mut frame_count = 0;

    while let Some(frame) = decoder.decode_next_frame()? {
        let (rgba, raw_w, raw_h) = frame.to_oriented_rgba8();

        let downscaled = downscale_rgba8(&rgba, raw_w, raw_h, proxy_w, proxy_h);
        let frame_path = proxy_dir.join(format!("{}.raw", frame.pts.raw()));

        let mut file = File::create(&frame_path)?;
        // Write header: width(4), height(4), pts(8)
        file.write_all(&proxy_w.to_le_bytes())?;
        file.write_all(&proxy_h.to_le_bytes())?;
        file.write_all(&frame.pts.raw().to_le_bytes())?;
        file.write_all(&downscaled)?;
        file.flush()?;

        timecodes.push(frame.pts);
        frame_count += 1;
    }

    let index = ProxyIndex {
        fingerprint: fingerprint.to_string(),
        tier,
        width: proxy_w,
        height: proxy_h,
        frame_count,
        timecodes,
    };

    let index_path = proxy_dir.join("index.json");
    let json_bytes = serde_json::to_vec_pretty(&index)?;
    fs::write(index_path, json_bytes)?;

    Ok(index)
}

/// Reads a single cached intra-frame proxy directly from disk.
pub fn load_proxy_frame(
    cache_dir: &Path,
    fingerprint: &str,
    tier: ProxyTier,
    time: Tick,
) -> Option<ProxyFrame> {
    let frame_path = cache_dir
        .join(fingerprint)
        .join("proxies")
        .join(tier.folder_name())
        .join(format!("{}.raw", time.raw()));

    if !frame_path.exists() {
        return None;
    }

    let mut file = File::open(&frame_path).ok()?;
    let mut header = [0u8; 16];
    file.read_exact(&mut header).ok()?;

    let width = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let height = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    let pts_raw = i64::from_le_bytes([
        header[8], header[9], header[10], header[11],
        header[12], header[13], header[14], header[15],
    ]);

    let mut rgba_data = Vec::new();
    file.read_to_end(&mut rgba_data).ok()?;

    Some(ProxyFrame::new(Tick::from_raw(pts_raw), width, height, rgba_data))
}
