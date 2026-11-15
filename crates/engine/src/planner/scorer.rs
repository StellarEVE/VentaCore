//! Candidate clip generation, scoring, and ranking engine.

use crate::planner::intervals::is_candidate_window_valid;
use crate::planner::prng::DeterministicRng;
use crate::planner::types::{CandidateClip, PlanSlot};
use std::collections::HashMap;
use ventacore_core::{
    Asset, AssetId, GenerationSettings, Item, ItemId, PlacementMode, Tick, TimeRange, TrackId,
};
use ventacore_media::{AnalysisLevel, FootageAnalysis};

/// State carried across consecutive slots during planning to track footage walk heads and reuse.
#[derive(Debug, Clone, Default)]
pub struct PlannerState {
    pub walk_heads: HashMap<AssetId, Tick>,
    pub asset_usage_counts: HashMap<AssetId, usize>,
    pub previous_asset_id: Option<AssetId>,
    pub next_item_id_counter: u64,
}

impl PlannerState {
    pub fn new(initial_item_id: u64) -> Self {
        Self {
            walk_heads: HashMap::new(),
            asset_usage_counts: HashMap::new(),
            previous_asset_id: None,
            next_item_id_counter: initial_item_id,
        }
    }
}

/// Evaluates, scores, and selects the best candidate clip for a cut slot,
/// emitting the assigned item and ranked alternatives.
pub fn score_and_select_slot(
    slot_index: usize,
    slot_start: Tick,
    slot_duration: Tick,
    bar_energy: f32,
    target_track_id: TrackId,
    assets: &[Asset],
    valid_intervals_map: &HashMap<AssetId, Vec<TimeRange>>,
    footage_map: &HashMap<AssetId, FootageAnalysis>,
    settings: &GenerationSettings,
    state: &mut PlannerState,
    prng: &mut DeterministicRng,
) -> Option<PlanSlot> {
    let mut candidates: Vec<CandidateClip> = Vec::new();

    // 1. Generate candidates from all eligible assets
    for asset in assets {
        let valid_intervals = match valid_intervals_map.get(&asset.id) {
            Some(intervals) if !intervals.is_empty() => intervals,
            _ => continue,
        };

        // Check if any interval is at least slot_duration
        let max_interval_dur = valid_intervals
            .iter()
            .map(|r| r.duration)
            .max()
            .unwrap_or(Tick::ZERO);

        if max_interval_dur < slot_duration {
            continue;
        }

        let footage = footage_map.get(&asset.id);

        let candidate_start_points = generate_candidate_start_points(
            asset.id,
            slot_duration,
            valid_intervals,
            footage,
            settings,
            state,
            prng,
        );

        for (source_in, snap_reason) in candidate_start_points {
            if !is_candidate_window_valid(valid_intervals, source_in, slot_duration) {
                continue;
            }

            let (score, provisional, reason) = evaluate_candidate(
                asset,
                source_in,
                slot_duration,
                bar_energy,
                snap_reason,
                footage,
                settings,
                state,
                prng,
            );

            candidates.push(CandidateClip {
                asset_id: asset.id,
                source_in,
                duration: slot_duration,
                score,
                provisional,
                reason,
            });
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // Sort descending by score. In case of tie, use asset_id and source_in
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.asset_id.0.cmp(&b.asset_id.0))
            .then_with(|| a.source_in.cmp(&b.source_in))
    });

    // Best candidate is assigned to slot
    let winner = candidates[0].clone();

    state.next_item_id_counter += 1;
    let item_id = ItemId(state.next_item_id_counter);

    let assigned_item = Item::new(
        item_id,
        target_track_id,
        winner.asset_id,
        slot_start,
        slot_duration,
        winner.source_in,
    );

    // Update state tracking
    *state.asset_usage_counts.entry(winner.asset_id).or_insert(0) += 1;
    state.previous_asset_id = Some(winner.asset_id);

    // Update walk head if continue_start_points is enabled
    if settings.continue_start_points || settings.placement_mode == PlacementMode::WindowedWalk {
        let spread_step = calculate_spread_step(slot_duration, settings.spread, prng);
        let next_head = winner.source_in + slot_duration + spread_step;
        state.walk_heads.insert(winner.asset_id, next_head);
    }

    // Form ranked alternatives (excluding the winner clip, no self-inclusion)
    let mut alternatives = Vec::new();
    let mut seen_keys = std::collections::HashSet::new();
    seen_keys.insert((winner.asset_id, winner.source_in));

    for cand in candidates.into_iter().skip(1) {
        if seen_keys.insert((cand.asset_id, cand.source_in)) {
            alternatives.push(cand);
            if alternatives.len() >= 5 {
                break;
            }
        }
    }

    Some(PlanSlot {
        slot_index,
        timeline_start: slot_start,
        timeline_duration: slot_duration,
        assigned_item,
        alternatives,
        bar_energy,
    })
}

/// Generates candidate start timestamps for an asset according to placement mode and snapping.
fn generate_candidate_start_points(
    asset_id: AssetId,
    duration: Tick,
    valid_intervals: &[TimeRange],
    footage: Option<&FootageAnalysis>,
    settings: &GenerationSettings,
    state: &PlannerState,
    prng: &mut DeterministicRng,
) -> Vec<(Tick, Option<&'static str>)> {
    let mut results = Vec::new();

    match settings.placement_mode {
        PlacementMode::WindowedWalk => {
            let walk_head = state.walk_heads.get(&asset_id).copied().unwrap_or_else(|| {
                valid_intervals.first().map(|r| r.start).unwrap_or(Tick::ZERO)
            });

            // Find interval containing or after walk head
            let active_interval = valid_intervals
                .iter()
                .find(|r| r.start <= walk_head && walk_head + duration <= r.end())
                .or_else(|| {
                    valid_intervals
                        .iter()
                        .find(|r| r.duration >= duration)
                });

            if let Some(intv) = active_interval {
                let mut start_pos = walk_head.max(intv.start);
                if start_pos + duration > intv.end() {
                    start_pos = intv.start;
                }

                // Check scene snapping
                let (snapped_start, reason) = apply_snapping(
                    start_pos,
                    duration,
                    intv,
                    footage,
                    settings.snap_to_scenes,
                    settings.snap_to_dark_edges,
                );
                results.push((snapped_start, reason));

                // If snap_to_scenes is enabled, also evaluate scene cuts within range of the current walk window
                if let Some(f) = footage {
                    if settings.snap_to_scenes {
                        for &cut in &f.scene_cuts {
                            if cut >= intv.start && cut + duration <= intv.end() {
                                if (cut.0 - start_pos.0).abs() <= Tick::from_seconds(10).0 {
                                    results.push((cut, Some("Scene Cut Snap")));
                                }
                            }
                        }
                    }
                }

                // Also generate an offset exploration candidate if interval allows
                if intv.duration >= duration * 2 {
                    let max_offset = (intv.duration - duration).raw();
                    let random_offset = Tick(prng.gen_range_i64(0, max_offset));
                    let alt_pos = intv.start + random_offset;
                    let (alt_snapped, alt_reason) = apply_snapping(
                        alt_pos,
                        duration,
                        intv,
                        footage,
                        settings.snap_to_scenes,
                        settings.snap_to_dark_edges,
                    );
                    results.push((alt_snapped, alt_reason));
                }
            }
        }
        PlacementMode::EvenWindows => {
            // Partition each valid interval of sufficient size into even sample points
            for intv in valid_intervals {
                if intv.duration < duration {
                    continue;
                }
                let available = intv.duration - duration;
                let num_samples = 3.min((intv.duration.0 / duration.0).max(1) as usize);

                for i in 0..num_samples {
                    let ratio = if num_samples > 1 {
                        i as f64 / (num_samples - 1) as f64
                    } else {
                        0.0
                    };
                    let offset = Tick((available.0 as f64 * ratio).round() as i64);
                    let pos = intv.start + offset;
                    let (snapped, reason) = apply_snapping(
                        pos,
                        duration,
                        intv,
                        footage,
                        settings.snap_to_scenes,
                        settings.snap_to_dark_edges,
                    );
                    results.push((snapped, reason));
                }
            }
        }
    }

    results
}

/// Snaps candidate start position to scene cut or dark edge if enabled.
fn apply_snapping(
    pos: Tick,
    duration: Tick,
    intv: &TimeRange,
    footage: Option<&FootageAnalysis>,
    snap_scenes: bool,
    snap_dark: bool,
) -> (Tick, Option<&'static str>) {
    let mut snapped = pos;
    let mut reason = None;

    if let Some(f) = footage {
        if snap_scenes {
            let threshold = Tick::from_seconds_f64(0.5); // 500ms snap window
            if let Some(cut) = f.nearest_scene_cut(pos, threshold) {
                if cut >= intv.start && cut + duration <= intv.end() {
                    snapped = cut;
                    reason = Some("Scene Cut Snap");
                }
            }
        }

        if snap_dark && reason.is_none() {
            // Snap away from black edges if near
            for black in &f.black_ranges {
                if black.end() >= intv.start
                    && black.end() + duration <= intv.end()
                    && (black.end().0 - pos.0).abs() < Tick::from_seconds_f64(0.5).0
                {
                    snapped = black.end();
                    reason = Some("Dark Edge Snap");
                    break;
                }
            }
        }
    }

    (snapped, reason)
}

/// Evaluates a candidate clip and returns (score, is_provisional, reason).
fn evaluate_candidate(
    asset: &Asset,
    source_in: Tick,
    duration: Tick,
    bar_energy: f32,
    snap_reason: Option<&'static str>,
    footage: Option<&FootageAnalysis>,
    settings: &GenerationSettings,
    state: &PlannerState,
    prng: &mut DeterministicRng,
) -> (f32, bool, String) {
    let mut score = 1.0f32;
    let mut reasons = Vec::new();
    let cand_range = TimeRange::new(source_in, duration);

    // 1. Snap bonus
    if let Some(snap) = snap_reason {
        score += 0.35;
        reasons.push(snap.to_string());
    }

    // 2. Favourite moments bonus
    let overlaps_favourite = asset.favourite_moments.iter().any(|fm| fm.overlaps(cand_range));
    if overlaps_favourite {
        score += 0.60;
        reasons.push("Favourite Moment".into());
    }

    // 3. Motion vs Song Energy alignment
    let mut provisional = true;
    if let Some(f) = footage {
        if f.level == AnalysisLevel::Full {
            provisional = false;
        }

        let motion = f.average_motion_in_range(cand_range);
        let energy_factor = settings.energy_density.clamp(0.0, 1.0);

        if bar_energy >= 0.6 {
            // High energy song: reward high motion
            let motion_bonus = (motion - 0.4).max(0.0) * bar_energy * energy_factor * 0.8;
            score += motion_bonus;
            if motion >= 0.7 {
                reasons.push("High Motion Match".into());
            }
        } else if bar_energy < 0.4 {
            // Low energy song: reward calm motion
            let calm_bonus = (0.6 - motion).max(0.0) * (1.0 - bar_energy) * energy_factor * 0.8;
            score += calm_bonus;
            if motion <= 0.3 {
                reasons.push("Calm Footage Match".into());
            }
        }
    }

    // 4. Source reuse penalty
    let reuse_penalty_weight = settings.source_reuse_penalty.clamp(0.0, 1.0);
    if let Some(prev) = state.previous_asset_id {
        if prev == asset.id {
            // Direct immediate repetition penalty
            score *= 1.0 - (reuse_penalty_weight * 0.70);
        }
    }

    let usage_count = state.asset_usage_counts.get(&asset.id).copied().unwrap_or(0);
    if usage_count > 0 {
        let general_penalty = 1.0 / (1.0 + (reuse_penalty_weight * (usage_count as f32) * 0.35));
        score *= general_penalty;
    }

    // 5. Deterministic jitter for variance and seeded uniqueness
    let jitter = prng.next_f32() * 0.08 * settings.clip_variance.clamp(0.0, 1.0);
    score += jitter;

    let reason_str = if reasons.is_empty() {
        "Standard Match".into()
    } else {
        reasons.join(", ")
    };

    (score, provisional, reason_str)
}

/// Calculates the walk jump step based on spread parameter.
fn calculate_spread_step(duration: Tick, spread: f32, prng: &mut DeterministicRng) -> Tick {
    let spread_clamped = spread.clamp(0.0, 1.0);
    let base_jump = (duration.0 as f64 * (spread_clamped as f64 * 3.0)) as i64;
    let jitter = prng.gen_range_i64(0, (duration.0 as f64 * 0.5) as i64 + 1);
    Tick(base_jump + jitter)
}
