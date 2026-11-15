//! Layer 4: Deterministic montage planner for VentaCore.
//!
//! Orchestrates:
//! - Beat-synchronised cut point slicing scaled by bar energy.
//! - Hard constraint enforcement (intro/outro trims, exclusions, black ranges never crossed).
//! - Absolute preservation of pinned and touched user clips.
//! - Candidate scoring with placement modes, source reuse penalties, and scene snapping.
//! - Emits winning clip timeline plus ranked alternative suggestions per slot.

pub mod intervals;
pub mod presets;
pub mod prng;
pub mod scorer;
pub mod slicer;
pub mod types;

pub use intervals::{
    compute_open_timeline_intervals, compute_valid_source_intervals, is_candidate_window_valid,
};
pub use presets::{get_preset_settings, PRESET_NAMES};
pub use prng::DeterministicRng;
pub use scorer::{score_and_select_slot, PlannerState};
pub use slicer::{get_energy_at_tick, slice_open_region, RawSlot};
pub use types::{CandidateClip, MontagePlan, PlanSlot, PlannerError};

use std::collections::HashMap;
use ventacore_audio::SongStructure;
use ventacore_core::{AssetId, Command, CommandError, GenerationSettings, Project, Tick, TrackId, TrackKind};
use ventacore_media::FootageAnalysis;

/// Generates a deterministic cut montage plan for the given project and song.
pub fn plan_montage(
    project: &Project,
    song: &SongStructure,
    footage_map: &HashMap<AssetId, FootageAnalysis>,
    settings_override: Option<&GenerationSettings>,
    target_track_id: Option<TrackId>,
) -> Result<MontagePlan, PlannerError> {
    // 1. Validate song structure
    if song.beat_ticks.is_empty() {
        return Err(PlannerError::EmptySong);
    }

    let song_duration = *song.beat_ticks.last().unwrap();
    if song_duration <= Tick::ZERO {
        return Err(PlannerError::EmptySong);
    }

    // 2. Resolve settings
    let settings = settings_override.unwrap_or(&project.settings);
    if settings.min_clip_duration <= Tick::ZERO {
        return Err(PlannerError::InvalidSettings(
            "min_clip_duration must be positive".into(),
        ));
    }
    if settings.max_clip_duration < settings.min_clip_duration {
        return Err(PlannerError::InvalidSettings(
            "max_clip_duration must be >= min_clip_duration".into(),
        ));
    }

    // 3. Resolve target video track
    let track = match target_track_id {
        Some(tid) => project
            .find_track(tid)
            .ok_or(PlannerError::TargetTrackNotFound(tid))?,
        None => project
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Video && !t.locked)
            .ok_or(PlannerError::NoVideoTracks)?,
    };

    if track.locked {
        return Err(PlannerError::TargetTrackLocked(track.id));
    }
    if track.kind != TrackKind::Video {
        return Err(PlannerError::TargetTrackNotVideo(track.id));
    }

    // 4. Precompute valid source intervals for all project assets
    let mut valid_intervals_map: HashMap<AssetId, Vec<ventacore_core::TimeRange>> = HashMap::new();
    for asset in &project.assets {
        let valid = compute_valid_source_intervals(asset, footage_map.get(&asset.id));
        if !valid.is_empty() {
            valid_intervals_map.insert(asset.id, valid);
        }
    }

    if valid_intervals_map.is_empty() {
        return Err(PlannerError::NoEligibleAssets);
    }

    // 5. Identify fixed (pinned or touched) items on target track
    let existing_track_items = project.track_items(track.id);
    let fixed_items: Vec<&ventacore_core::Item> = existing_track_items
        .iter()
        .copied()
        .filter(|item| item.pinned || item.touched)
        .collect();

    let pinned_count = fixed_items.len();
    let unpinned_replaced_count = existing_track_items.len() - pinned_count;

    // 6. Partition timeline into open regions (excluding pinned/touched clips)
    let open_regions = compute_open_timeline_intervals(song_duration, &fixed_items);

    // 7. Initialize deterministic PRNG and state tracking
    let mut prng = DeterministicRng::new(settings.seed);
    let max_item_id = project.items.iter().map(|i| i.id.0).max().unwrap_or(0);
    let mut state = PlannerState::new(max_item_id);

    // 8. Slicing and candidate selection across open regions
    let mut slots = Vec::new();
    let mut generated_items = Vec::new();
    let mut slot_index = 0;

    for region in open_regions {
        let raw_slots = slice_open_region(region, song, settings, &mut prng);

        for raw in raw_slots {
            if let Some(plan_slot) = score_and_select_slot(
                slot_index,
                raw.start,
                raw.duration,
                raw.bar_energy,
                track.id,
                &project.assets,
                &valid_intervals_map,
                footage_map,
                settings,
                &mut state,
                &mut prng,
            ) {
                generated_items.push(plan_slot.assigned_item.clone());
                slots.push(plan_slot);
                slot_index += 1;
            }
        }
    }

    Ok(MontagePlan {
        target_track_id: track.id,
        slots,
        generated_items,
        pinned_items_count: pinned_count,
        unpinned_replaced_count,
        seed: settings.seed,
    })
}

/// Applies a montage plan to the project model strictly through Project commands.
///
/// Removes unpinned items on the target track and inserts the newly generated items.
/// Pinned and touched items are untouched.
pub fn apply_plan_to_project(
    project: &mut Project,
    plan: &MontagePlan,
) -> Result<(), CommandError> {
    // Collect unpinned items on target track to remove
    let unpinned_ids: Vec<ventacore_core::ItemId> = project
        .items
        .iter()
        .filter(|item| item.track_id == plan.target_track_id && !item.pinned && !item.touched)
        .map(|item| item.id)
        .collect();

    // 1. Remove unpinned items
    for id in unpinned_ids {
        project.apply_command(&Command::RemoveItem(id))?;
    }

    // 2. Insert new generated items
    for item in &plan.generated_items {
        project.apply_command(&Command::InsertItem(item.clone()))?;
    }

    Ok(())
}
