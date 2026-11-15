//! Footage analysis data structures and query helpers.
//!
//! Stores staged scene cut detection, black range detection, motion curves,
//! and highlight segments for imported video assets.

use serde::{Deserialize, Serialize};
use ventacore_core::{AssetId, Tick, TimeRange};

/// The depth of analysis completed for an asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum AnalysisLevel {
    #[default]
    None,
    /// Quick pass: provisional low-res scene cuts, basic black frames, rough motion.
    Provisional,
    /// Full pass: full-decode exact scene cuts, verified black ranges, dense motion curves.
    Full,
}

/// A point measurement along an asset's motion curve.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MotionPoint {
    pub time: Tick,
    /// Normalized motion intensity (0.0 = completely static, 1.0 = fast kinetic action).
    pub score: f32,
}

/// The analysis results for a video asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FootageAnalysis {
    pub asset_id: AssetId,
    pub level: AnalysisLevel,
    pub duration: Tick,
    /// Timestamp of detected scene cuts / shot boundaries within the asset.
    pub scene_cuts: Vec<Tick>,
    /// Intervals identified as black / dead screen that should never appear in a cut.
    pub black_ranges: Vec<TimeRange>,
    /// Motion energy sampled across the asset.
    pub motion_scores: Vec<MotionPoint>,
    /// Highlight or high-interest moments identified automatically.
    pub interesting_ranges: Vec<TimeRange>,
}

impl FootageAnalysis {
    pub fn new(asset_id: AssetId, duration: Tick) -> Self {
        Self {
            asset_id,
            level: AnalysisLevel::None,
            duration,
            scene_cuts: Vec::new(),
            black_ranges: Vec::new(),
            motion_scores: Vec::new(),
            interesting_ranges: Vec::new(),
        }
    }

    pub fn new_provisional(asset_id: AssetId, duration: Tick) -> Self {
        Self {
            asset_id,
            level: AnalysisLevel::Provisional,
            duration,
            scene_cuts: Vec::new(),
            black_ranges: Vec::new(),
            motion_scores: Vec::new(),
            interesting_ranges: Vec::new(),
        }
    }

    /// Returns true if the given range overlaps any black/dead range.
    pub fn overlaps_black_range(&self, range: TimeRange) -> bool {
        self.black_ranges.iter().any(|br| br.overlaps(range))
    }

    /// Computes the average motion score across the given time range.
    /// Returns a default of 0.5 if no motion data exists in the range.
    pub fn average_motion_in_range(&self, range: TimeRange) -> f32 {
        if self.motion_scores.is_empty() {
            return 0.5;
        }

        let matching: Vec<f32> = self
            .motion_scores
            .iter()
            .filter(|mp| mp.time >= range.start && mp.time < range.end())
            .map(|mp| mp.score)
            .collect();

        if matching.is_empty() {
            // Sample the closest motion point before or after
            self.motion_scores
                .iter()
                .min_by_key(|mp| (mp.time.0 - range.start.0).abs())
                .map(|mp| mp.score)
                .unwrap_or(0.5)
        } else {
            matching.iter().copied().sum::<f32>() / (matching.len() as f32)
        }
    }

    /// Finds the nearest scene cut within `threshold` ticks of `target`.
    pub fn nearest_scene_cut(&self, target: Tick, threshold: Tick) -> Option<Tick> {
        self.scene_cuts
            .iter()
            .copied()
            .filter(|&cut| Tick((cut.0 - target.0).abs()) <= threshold)
            .min_by_key(|&cut| (cut.0 - target.0).abs())
    }
}
