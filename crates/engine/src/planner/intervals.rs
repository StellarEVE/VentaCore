//! Whole-number interval algebra for timeline and footage constraints.
//!
//! Enforces:
//! - Pinned and touched timeline regions are never overwritten.
//! - Candidate footage windows never cross intro trims, outro trims,
//!   exclude ranges, or black ranges.

use ventacore_core::{Asset, Item, Tick, TimeRange};
use ventacore_media::FootageAnalysis;

/// Computes open (unoccupied) timeline intervals within `[0, total_duration)`
/// that are not occupied by fixed (pinned or touched) items.
pub fn compute_open_timeline_intervals(
    total_duration: Tick,
    fixed_items: &[&Item],
) -> Vec<TimeRange> {
    if total_duration <= Tick::ZERO {
        return Vec::new();
    }

    let mut sorted_fixed: Vec<&Item> = fixed_items.to_vec();
    sorted_fixed.sort_by_key(|item| item.timeline_start);

    let mut open = Vec::new();
    let mut cursor = Tick::ZERO;

    for item in sorted_fixed {
        let item_start = item.timeline_start.min(total_duration);
        let item_end = item.timeline_end().min(total_duration);

        if item_start > cursor {
            open.push(TimeRange::from_start_end(cursor, item_start));
        }
        if item_end > cursor {
            cursor = item_end;
        }
    }

    if cursor < total_duration {
        open.push(TimeRange::from_start_end(cursor, total_duration));
    }

    open
}

/// Computes disjoint valid source intervals for an asset by:
/// 1. Applying intro and outro trims to source duration.
/// 2. Intersecting with include ranges (if any are specified).
/// 3. Subtracting exclude ranges.
/// 4. Subtracting black / dark ranges from footage analysis.
pub fn compute_valid_source_intervals(
    asset: &Asset,
    analysis: Option<&FootageAnalysis>,
) -> Vec<TimeRange> {
    let source_duration = asset
        .probe_data
        .as_ref()
        .map(|p| p.duration)
        .or_else(|| analysis.map(|a| a.duration))
        .unwrap_or(Tick::ZERO);

    if source_duration <= Tick::ZERO {
        return Vec::new();
    }

    let intro = asset.intro_trim.max(Tick::ZERO);
    let outro = asset.outro_trim.max(Tick::ZERO);
    let usable_end = source_duration - outro;

    if usable_end <= intro {
        return Vec::new();
    }

    let base_interval = TimeRange::from_start_end(intro, usable_end);
    let mut intervals = vec![base_interval];

    // 1. Intersect with include ranges if specified
    if !asset.include_ranges.is_empty() {
        intervals = intersect_intervals(&intervals, &asset.include_ranges);
    }

    // 2. Subtract exclude ranges
    if !asset.exclude_ranges.is_empty() {
        intervals = subtract_intervals(&intervals, &asset.exclude_ranges);
    }

    // 3. Subtract black ranges from footage analysis
    if let Some(ana) = analysis {
        if !ana.black_ranges.is_empty() {
            intervals = subtract_intervals(&intervals, &ana.black_ranges);
        }
    }

    intervals.retain(|r| r.duration > Tick::ZERO);
    intervals
}

/// Returns true if candidate window `[start, start + duration)` is entirely
/// contained within at least one valid source interval.
pub fn is_candidate_window_valid(
    valid_intervals: &[TimeRange],
    start: Tick,
    duration: Tick,
) -> bool {
    if duration <= Tick::ZERO {
        return false;
    }
    let end = start + duration;
    valid_intervals
        .iter()
        .any(|interval| interval.start <= start && end <= interval.end())
}

/// Intersects a set of intervals with a set of include ranges (union of includes).
fn intersect_intervals(intervals: &[TimeRange], includes: &[TimeRange]) -> Vec<TimeRange> {
    let mut result = Vec::new();
    for interval in intervals {
        for inc in includes {
            let start = interval.start.max(inc.start);
            let end = interval.end().min(inc.end());
            if end > start {
                result.push(TimeRange::from_start_end(start, end));
            }
        }
    }
    merge_overlapping_intervals(&mut result);
    result
}

/// Subtracts multiple exclusion ranges from a set of intervals.
fn subtract_intervals(intervals: &[TimeRange], exclusions: &[TimeRange]) -> Vec<TimeRange> {
    let mut current = intervals.to_vec();

    for exc in exclusions {
        let mut next = Vec::new();
        for intv in current {
            let exc_start = exc.start;
            let exc_end = exc.end();

            if exc_end <= intv.start || exc_start >= intv.end() {
                // No overlap: keep interval intact
                next.push(intv);
            } else {
                // Overlap: split around exclusion
                if exc_start > intv.start {
                    next.push(TimeRange::from_start_end(intv.start, exc_start));
                }
                if exc_end < intv.end() {
                    next.push(TimeRange::from_start_end(exc_end, intv.end()));
                }
            }
        }
        current = next;
    }

    current.retain(|r| r.duration > Tick::ZERO);
    current
}

/// Merges adjacent or overlapping intervals.
fn merge_overlapping_intervals(intervals: &mut Vec<TimeRange>) {
    if intervals.is_empty() {
        return;
    }
    intervals.sort_by_key(|r| r.start);
    let mut merged = Vec::new();
    let mut current = intervals[0];

    for &next in intervals.iter().skip(1) {
        if next.start <= current.end() {
            let new_end = current.end().max(next.end());
            current = TimeRange::from_start_end(current.start, new_end);
        } else {
            merged.push(current);
            current = next;
        }
    }
    merged.push(current);
    *intervals = merged;
}

#[cfg(test)]
mod tests {
    use super::*;
    use ventacore_core::{AssetId, ItemId, TrackId};

    #[test]
    fn test_compute_open_timeline_intervals_with_fixed_items() {
        let total = Tick::from_seconds(10);
        let item1 = Item::new(
            ItemId(1),
            TrackId(1),
            AssetId(1),
            Tick::from_seconds(2),
            Tick::from_seconds(2),
            Tick::ZERO,
        );
        let item2 = Item::new(
            ItemId(2),
            TrackId(1),
            AssetId(2),
            Tick::from_seconds(7),
            Tick::from_seconds(1),
            Tick::ZERO,
        );

        let open = compute_open_timeline_intervals(total, &[&item1, &item2]);
        assert_eq!(open.len(), 3);
        assert_eq!(open[0], TimeRange::from_start_end(Tick::from_seconds(0), Tick::from_seconds(2)));
        assert_eq!(open[1], TimeRange::from_start_end(Tick::from_seconds(4), Tick::from_seconds(7)));
        assert_eq!(open[2], TimeRange::from_start_end(Tick::from_seconds(8), Tick::from_seconds(10)));
    }

    #[test]
    fn test_valid_source_intervals_subtractions() {
        let mut asset = Asset::new(AssetId(1), "test.mp4".into());
        asset.probe_data = Some(ventacore_core::ProbeData {
            duration: Tick::from_seconds(30),
            width: 1920,
            height: 1080,
            fps_num: 30,
            fps_den: 1,
            has_audio: false,
            audio_sample_rate: 0,
        });
        asset.intro_trim = Tick::from_seconds(2);
        asset.outro_trim = Tick::from_seconds(3);
        // Usable: [2, 27)

        // Exclude [10, 15)
        asset.exclude_ranges.push(TimeRange::from_start_end(Tick::from_seconds(10), Tick::from_seconds(15)));

        let mut analysis = FootageAnalysis::new(AssetId(1), Tick::from_seconds(30));
        // Black range: [20, 22)
        analysis.black_ranges.push(TimeRange::from_start_end(Tick::from_seconds(20), Tick::from_seconds(22)));

        let valid = compute_valid_source_intervals(&asset, Some(&analysis));
        assert_eq!(valid.len(), 3);
        assert_eq!(valid[0], TimeRange::from_start_end(Tick::from_seconds(2), Tick::from_seconds(10)));
        assert_eq!(valid[1], TimeRange::from_start_end(Tick::from_seconds(15), Tick::from_seconds(20)));
        assert_eq!(valid[2], TimeRange::from_start_end(Tick::from_seconds(22), Tick::from_seconds(27)));

        // Candidate [5, 8) is valid
        assert!(is_candidate_window_valid(&valid, Tick::from_seconds(5), Tick::from_seconds(3)));
        // Candidate [9, 12) crosses exclusion [10, 15) -> invalid
        assert!(!is_candidate_window_valid(&valid, Tick::from_seconds(9), Tick::from_seconds(3)));
        // Candidate [19, 21) crosses black range [20, 22) -> invalid
        assert!(!is_candidate_window_valid(&valid, Tick::from_seconds(19), Tick::from_seconds(2)));
        // Candidate [1, 4) crosses intro trim 2 -> invalid
        assert!(!is_candidate_window_valid(&valid, Tick::from_seconds(1), Tick::from_seconds(3)));
    }
}
