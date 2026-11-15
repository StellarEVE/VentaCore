//! Content-addressed disposable disk cache for analysis and thumbnails.
//!
//! Conforms to Charter Rule 9: All derived data lives in one disposable cache
//! that the user can purge. Original media is never modified.

use crate::analysis::FootageAnalysis;
use crate::error::MediaError;
use crate::thumbnail::ThumbnailFrame;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use ventacore_core::Tick;

/// Content-addressed disposable disk cache manager.
#[derive(Debug, Clone)]
pub struct MediaCache {
    root_dir: PathBuf,
}

impl MediaCache {
    /// Creates a cache manager anchored at the specified root directory.
    pub fn new(root_dir: impl AsRef<Path>) -> Result<Self, MediaError> {
        let root_dir = root_dir.as_ref().to_path_buf();
        fs::create_dir_all(&root_dir)?;
        Ok(Self { root_dir })
    }

    /// Default system cache directory for VentaCore footage analysis.
    pub fn default_dir() -> PathBuf {
        let base = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        base.join("VentaCore").join("cache").join("footage")
    }

    /// Path to the directory reserved for a specific content fingerprint.
    pub fn fingerprint_dir(&self, fingerprint: &str) -> PathBuf {
        self.root_dir.join(fingerprint)
    }

    /// Loads cached `FootageAnalysis` for a fingerprint if present and valid.
    pub fn load_analysis(&self, fingerprint: &str) -> Option<FootageAnalysis> {
        let path = self.fingerprint_dir(fingerprint).join("analysis.json");
        if !path.exists() {
            return None;
        }

        let file = File::open(path).ok()?;
        serde_json::from_reader(file).ok()
    }

    /// Saves `FootageAnalysis` atomically to the content-addressed cache.
    pub fn save_analysis(
        &self,
        fingerprint: &str,
        analysis: &FootageAnalysis,
    ) -> Result<(), MediaError> {
        let dir = self.fingerprint_dir(fingerprint);
        fs::create_dir_all(&dir)?;

        let target_path = dir.join("analysis.json");
        let temp_path = dir.join("analysis.json.tmp");

        let json_data = serde_json::to_vec_pretty(analysis)?;
        {
            let mut file = File::create(&temp_path)?;
            file.write_all(&json_data)?;
            file.flush()?;
            file.sync_all()?;
        }

        fs::rename(temp_path, target_path)?;
        Ok(())
    }

    /// Saves a thumbnail frame to `<root>/<fingerprint>/thumbnails/<time>.raw`.
    pub fn save_thumbnail(
        &self,
        fingerprint: &str,
        thumbnail: &ThumbnailFrame,
    ) -> Result<(), MediaError> {
        let dir = self.fingerprint_dir(fingerprint).join("thumbnails");
        fs::create_dir_all(&dir)?;

        let file_path = dir.join(format!("{}.raw", thumbnail.time.raw()));
        let mut file = File::create(file_path)?;

        // Header: width (4 bytes), height (4 bytes), time (8 bytes)
        file.write_all(&thumbnail.width.to_le_bytes())?;
        file.write_all(&thumbnail.height.to_le_bytes())?;
        file.write_all(&thumbnail.time.raw().to_le_bytes())?;
        file.write_all(&thumbnail.rgba_data)?;
        file.flush()?;

        Ok(())
    }

    /// Loads all cached thumbnails for a fingerprint, sorted by timestamp.
    pub fn load_thumbnails(&self, fingerprint: &str) -> Vec<ThumbnailFrame> {
        let dir = self.fingerprint_dir(fingerprint).join("thumbnails");
        if !dir.exists() {
            return Vec::new();
        }

        let mut thumbnails = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) == Some("raw") {
                    if let Ok(mut file) = File::open(&path) {
                        let mut header = [0u8; 16];
                        if file.read_exact(&mut header).is_ok() {
                            let width = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
                            let height = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
                            let time_raw = i64::from_le_bytes([
                                header[8], header[9], header[10], header[11],
                                header[12], header[13], header[14], header[15],
                            ]);

                            let mut rgba_data = Vec::new();
                            if file.read_to_end(&mut rgba_data).is_ok() {
                                thumbnails.push(ThumbnailFrame {
                                    time: Tick::from_raw(time_raw),
                                    width,
                                    height,
                                    rgba_data,
                                });
                            }
                        }
                    }
                }
            }
        }

        thumbnails.sort_by_key(|t| t.time);
        thumbnails
    }

    /// Purges all derived data in the cache directory, returning the number of bytes deleted.
    pub fn purge(&self) -> Result<u64, MediaError> {
        let mut total_bytes = 0u64;
        if self.root_dir.exists() {
            total_bytes = calculate_dir_size(&self.root_dir);
            fs::remove_dir_all(&self.root_dir)?;
            fs::create_dir_all(&self.root_dir)?;
        }
        Ok(total_bytes)
    }
}

fn calculate_dir_size(path: &Path) -> u64 {
    let mut size = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    size += calculate_dir_size(&entry.path());
                } else {
                    size += meta.len();
                }
            }
        }
    }
    size
}

#[cfg(test)]
mod tests {
    use super::*;
    use ventacore_core::AssetId;

    #[test]
    fn test_cache_analysis_and_thumbnails_round_trip() {
        let temp_dir = std::env::temp_dir().join("ventacore_test_cache");
        let cache = MediaCache::new(&temp_dir).unwrap();
        let fp = "test_fp_123456";

        let mut analysis = FootageAnalysis::new(AssetId(1), Tick::from_seconds(30));
        analysis.scene_cuts.push(Tick::from_seconds(5));
        analysis.black_ranges.push(ventacore_core::TimeRange::from_start_end(
            Tick::from_seconds(10),
            Tick::from_seconds(12),
        ));

        // Save and verify load
        cache.save_analysis(fp, &analysis).unwrap();
        let loaded = cache.load_analysis(fp).expect("Must load cached analysis");
        assert_eq!(loaded.asset_id, analysis.asset_id);
        assert_eq!(loaded.scene_cuts, analysis.scene_cuts);
        assert_eq!(loaded.black_ranges, analysis.black_ranges);

        // Save thumbnail and verify load
        let thumb = ThumbnailFrame::new(Tick::from_seconds(2), 160, 90, vec![200u8; 160 * 90 * 4]);
        cache.save_thumbnail(fp, &thumb).unwrap();

        let loaded_thumbs = cache.load_thumbnails(fp);
        assert_eq!(loaded_thumbs.len(), 1);
        assert_eq!(loaded_thumbs[0].time, Tick::from_seconds(2));

        // Purge
        let freed = cache.purge().unwrap();
        assert!(freed > 0);
        assert!(cache.load_analysis(fp).is_none());
        assert!(cache.load_thumbnails(fp).is_empty());

        let _ = fs::remove_dir_all(temp_dir);
    }
}
