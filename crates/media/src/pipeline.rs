//! Staged media analysis pipeline coordinator.
//!
//! Orchestrates fingerprinting, cache checks, Stage 0 probe, Stage 1 quick pass,
//! and Stage 2 full pass with persistent disk caching.

use crate::analysis::{AnalysisLevel, FootageAnalysis};
use crate::cache::MediaCache;
use crate::error::MediaError;
use crate::fingerprint::compute_content_fingerprint;
use crate::full_pass::run_full_pass;
use crate::probe::{probe_video_container, Stage0ProbeResult};
use crate::quick_pass::run_quick_pass;
use std::path::Path;
use ventacore_core::{AssetId, Tick};
use ventacore_platform::PlatformVideoDecoder;

/// Complete analysis result for an asset file.
#[derive(Debug, Clone)]
pub struct AssetAnalysisResult {
    pub fingerprint: String,
    pub probe: Stage0ProbeResult,
    pub analysis: FootageAnalysis,
}

/// Analyzes an imported media file using the staged pipeline with caching.
pub fn analyze_media_file(
    path: impl AsRef<Path>,
    asset_id: AssetId,
    cache: &MediaCache,
    target_level: AnalysisLevel,
    progress_callback: Option<&dyn Fn(f32)>,
) -> Result<AssetAnalysisResult, MediaError> {
    let path = path.as_ref();
    let fingerprint = compute_content_fingerprint(path)?;

    // 1. Stage 0: Instant Probe
    let probe = probe_video_container(path)?;

    // 2. Check if cached analysis satisfies requested level
    if let Some(cached) = cache.load_analysis(&fingerprint) {
        if cached.level >= target_level {
            if let Some(cb) = progress_callback {
                cb(1.0);
            }
            return Ok(AssetAnalysisResult {
                fingerprint,
                probe,
                analysis: cached,
            });
        }
    }

    // 3. Open decoder for analysis
    let mut decoder = PlatformVideoDecoder::open(path)?;

    // 4. Staged execution
    let analysis = match target_level {
        AnalysisLevel::None => FootageAnalysis::new(asset_id, probe.core_probe.duration),

        AnalysisLevel::Provisional => {
            let sample_interval = Tick::from_seconds(1);
            let quick_result = run_quick_pass(&mut decoder, asset_id, sample_interval)?;

            // Store thumbnails in cache
            for thumb in &quick_result.thumbnails {
                let _ = cache.save_thumbnail(&fingerprint, thumb);
            }
            cache.save_analysis(&fingerprint, &quick_result.analysis)?;
            quick_result.analysis
        }

        AnalysisLevel::Full => {
            // Ensure thumbnails exist from quick pass
            if cache.load_thumbnails(&fingerprint).is_empty() {
                let sample_interval = Tick::from_seconds(1);
                if let Ok(quick_result) = run_quick_pass(&mut decoder, asset_id, sample_interval) {
                    for thumb in &quick_result.thumbnails {
                        let _ = cache.save_thumbnail(&fingerprint, thumb);
                    }
                }
            }

            // Run full decode pass
            let full_analysis = run_full_pass(&mut decoder, asset_id, progress_callback)?;
            cache.save_analysis(&fingerprint, &full_analysis)?;
            full_analysis
        }
    };

    Ok(AssetAnalysisResult {
        fingerprint,
        probe,
        analysis,
    })
}
