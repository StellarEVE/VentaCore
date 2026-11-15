//! Variable Frame Rate (VFR) timecode indexing and timestamp normalization.
//!
//! Maps whole-number Tick timeline positions to exact frame indices for video
//! streams with fluctuating frame durations (e.g. phone footage, screen recordings).

use serde::{Deserialize, Serialize};
use ventacore_core::Tick;

/// Index of frame presentation timestamps enabling O(log N) frame lookups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VfrTimecodeIndex {
    /// Monotonically increasing presentation timestamps for each frame.
    pub frame_pts: Vec<Tick>,
    /// Precomputed individual frame durations.
    pub frame_durations: Vec<Tick>,
    /// Whether jitter exceeded 5% of nominal frame duration.
    pub is_vfr: bool,
}

impl VfrTimecodeIndex {
    /// Constructs a timecode index from raw frame timestamps.
    /// Clamps any non-monotonic timestamps to ensure strict monotonicity.
    pub fn build(pts_list: &[Tick], default_frame_duration: Tick) -> Self {
        if pts_list.is_empty() {
            return Self::default();
        }

        let mut monotonic_pts = Vec::with_capacity(pts_list.len());
        let mut last_pts = Tick::ZERO;

        for (i, &raw) in pts_list.iter().enumerate() {
            let pts = if i == 0 {
                raw.max(Tick::ZERO)
            } else {
                raw.max(last_pts + Tick::from_raw(1))
            };
            monotonic_pts.push(pts);
            last_pts = pts;
        }

        // Calculate durations between consecutive frames
        let mut durations = Vec::with_capacity(monotonic_pts.len());
        for i in 0..monotonic_pts.len() {
            let dur = if i + 1 < monotonic_pts.len() {
                monotonic_pts[i + 1] - monotonic_pts[i]
            } else {
                // Last frame takes previous duration or default
                durations.last().copied().unwrap_or(default_frame_duration)
            };
            durations.push(dur);
        }

        // Detect VFR variance (> 5% deviation from average duration)
        let is_vfr = detect_vfr_variance(&durations);

        Self {
            frame_pts: monotonic_pts,
            frame_durations: durations,
            is_vfr,
        }
    }

    /// Finds the frame index that is visible at the given timeline tick.
    pub fn frame_index_at_tick(&self, tick: Tick) -> usize {
        if self.frame_pts.is_empty() {
            return 0;
        }

        match self.frame_pts.binary_search(&tick) {
            Ok(exact) => exact,
            Err(0) => 0,
            Err(insertion_point) => {
                let candidate = insertion_point - 1;
                let end = self.frame_pts[candidate] + self.frame_durations[candidate];
                if tick < end {
                    candidate
                } else if insertion_point < self.frame_pts.len() {
                    insertion_point
                } else {
                    self.frame_pts.len() - 1
                }
            }
        }
    }

    /// Returns presentation timestamp for frame index.
    pub fn pts_at(&self, index: usize) -> Option<Tick> {
        self.frame_pts.get(index).copied()
    }

    /// Total count of indexed frames.
    pub fn frame_count(&self) -> usize {
        self.frame_pts.len()
    }

    /// Total duration covered by all indexed frames.
    pub fn total_duration(&self) -> Tick {
        match (self.frame_pts.last(), self.frame_durations.last()) {
            (Some(&last_pts), Some(&last_dur)) => last_pts + last_dur,
            _ => Tick::ZERO,
        }
    }
}

/// Detects whether frame durations vary by more than 5% across the stream.
fn detect_vfr_variance(durations: &[Tick]) -> bool {
    if durations.len() < 2 {
        return false;
    }

    let total: i128 = durations.iter().map(|d| d.raw() as i128).sum();
    let avg = total / durations.len() as i128;
    if avg == 0 {
        return false;
    }

    let threshold = avg / 20; // 5%

    durations
        .iter()
        .any(|d| ((d.raw() as i128) - avg).abs() > threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vfr_lookup_and_monotonicity() {
        let frame_step = Tick::from_seconds_f64(1.0 / 30.0);
        // Introduce jitter: 0, 33ms, 80ms, 110ms, 140ms
        let pts = vec![
            Tick::ZERO,
            frame_step,
            frame_step * 2 + Tick::from_seconds_f64(0.015),
            frame_step * 3 + Tick::from_seconds_f64(0.010),
            frame_step * 4,
        ];

        let index = VfrTimecodeIndex::build(&pts, frame_step);
        assert_eq!(index.frame_count(), 5);
        assert!(index.is_vfr);

        // Lookup exact point
        assert_eq!(index.frame_index_at_tick(Tick::ZERO), 0);
        assert_eq!(index.frame_index_at_tick(frame_step), 1);
    }
}
