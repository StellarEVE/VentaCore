//! Content-addressed disposable disk cache for rendered composite frames.
//!
//! Conforms to Charter Rule 9: All derived data lives in one disposable cache
//! that the user can purge. Original media is never modified.

use crate::error::RenderError;
use crate::types::{FitMode, RenderFrame, ResolutionTier, Transform2D};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use ventacore_core::Tick;

/// Content-addressed disk cache for rendered video frames and transition segments.
#[derive(Debug, Clone)]
pub struct RenderCache {
    root_dir: PathBuf,
}

impl RenderCache {
    pub fn new(root_dir: impl AsRef<Path>) -> Result<Self, RenderError> {
        let root_dir = root_dir.as_ref().to_path_buf();
        fs::create_dir_all(&root_dir)
            .map_err(|e| RenderError::BufferMappingFailed(e.to_string()))?;
        Ok(Self { root_dir })
    }

    /// Path to the root cache directory.
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Default system cache directory for VentaCore render cache.
    pub fn default_dir() -> PathBuf {
        let base = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        base.join("VentaCore").join("cache").join("rendered")
    }

    /// Computes a unique deterministic hash key for a rendered cut frame.
    pub fn compute_cut_key(
        fingerprint: &str,
        source_tick: Tick,
        fit: FitMode,
        transform: &Transform2D,
        tier: ResolutionTier,
        width: u32,
        height: u32,
    ) -> String {
        // Fast deterministic FNV-1a hash over parameters
        let mut hasher = 0xcbf29ce484222325u64;
        let mut update = |bytes: &[u8]| {
            for b in bytes {
                hasher ^= *b as u64;
                hasher = hasher.wrapping_mul(0x100000001b3);
            }
        };

        update(fingerprint.as_bytes());
        update(&source_tick.raw().to_le_bytes());
        update(&[fit as u8, tier as u8]);
        update(&width.to_le_bytes());
        update(&height.to_le_bytes());
        update(&transform.position[0].to_le_bytes());
        update(&transform.position[1].to_le_bytes());
        update(&transform.scale[0].to_le_bytes());
        update(&transform.scale[1].to_le_bytes());
        update(&transform.rotation_radians.to_le_bytes());
        update(&transform.opacity.to_le_bytes());

        format!("{:016x}", hasher)
    }

    /// Loads a cached rendered frame from disk if present.
    pub fn get(&self, key: &str) -> Option<RenderFrame> {
        let path = self.root_dir.join(format!("{}.raw", key));
        if !path.exists() {
            return None;
        }

        let mut file = File::open(path).ok()?;
        let mut header = [0u8; 8];
        file.read_exact(&mut header).ok()?;

        let width = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let height = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);

        let mut pixels = Vec::new();
        file.read_to_end(&mut pixels).ok()?;

        RenderFrame::from_rgba8(width, height, pixels).ok()
    }

    /// Stores a rendered frame to the content-addressed disk cache.
    pub fn put(&self, key: &str, frame: &RenderFrame) -> Result<(), RenderError> {
        let temp_path = self.root_dir.join(format!("{}.tmp", key));
        let target_path = self.root_dir.join(format!("{}.raw", key));

        {
            let mut file = File::create(&temp_path)
                .map_err(|e| RenderError::BufferMappingFailed(e.to_string()))?;
            file.write_all(&frame.width.to_le_bytes())
                .map_err(|e| RenderError::BufferMappingFailed(e.to_string()))?;
            file.write_all(&frame.height.to_le_bytes())
                .map_err(|e| RenderError::BufferMappingFailed(e.to_string()))?;
            file.write_all(&frame.pixels)
                .map_err(|e| RenderError::BufferMappingFailed(e.to_string()))?;
            file.flush()
                .map_err(|e| RenderError::BufferMappingFailed(e.to_string()))?;
        }

        fs::rename(temp_path, target_path)
            .map_err(|e| RenderError::BufferMappingFailed(e.to_string()))?;
        Ok(())
    }

    /// Purges all cached frames, returning the number of files deleted.
    pub fn purge(&self) -> Result<usize, RenderError> {
        let mut count = 0;
        if self.root_dir.exists() {
            if let Ok(entries) = fs::read_dir(&self.root_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if fs::remove_file(path).is_ok() {
                            count += 1;
                        }
                    }
                }
            }
        }
        Ok(count)
    }
}
