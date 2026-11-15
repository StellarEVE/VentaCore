//! Data types and error reporting for the montage cut planner.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ventacore_core::{AssetId, CommandError, Item, Tick, TrackId};

#[derive(Debug, Error, PartialEq, Clone)]
pub enum PlannerError {
    #[error("No eligible video assets available for planning")]
    NoEligibleAssets,

    #[error("No video tracks found in project")]
    NoVideoTracks,

    #[error("Target track {0:?} was not found")]
    TargetTrackNotFound(TrackId),

    #[error("Target track {0:?} is locked")]
    TargetTrackLocked(TrackId),

    #[error("Target track {0:?} is not a video track")]
    TargetTrackNotVideo(TrackId),

    #[error("Song structure has zero duration or empty beat grid")]
    EmptySong,

    #[error("Invalid generator settings: {0}")]
    InvalidSettings(String),

    #[error("Project command execution failed during plan application: {0}")]
    Command(#[from] CommandError),
}

/// A candidate source footage segment evaluated for a cut slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateClip {
    pub asset_id: AssetId,
    pub source_in: Tick,
    pub duration: Tick,
    pub score: f32,
    pub provisional: bool,
    pub reason: String,
}

/// A cut slot on the timeline with its assigned item and ranked alternatives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanSlot {
    pub slot_index: usize,
    pub timeline_start: Tick,
    pub timeline_duration: Tick,
    pub assigned_item: Item,
    pub alternatives: Vec<CandidateClip>,
    pub bar_energy: f32,
}

/// The complete deterministic montage plan emitted by the planner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MontagePlan {
    pub target_track_id: TrackId,
    pub slots: Vec<PlanSlot>,
    pub generated_items: Vec<Item>,
    pub pinned_items_count: usize,
    pub unpinned_replaced_count: usize,
    pub seed: u64,
}

impl MontagePlan {
    /// Helper to find a slot by its assigned item ID.
    pub fn find_slot_by_item_id(&self, item_id: ventacore_core::ItemId) -> Option<&PlanSlot> {
        self.slots.iter().find(|s| s.assigned_item.id == item_id)
    }

    /// Total timeline duration covered by generated cuts.
    pub fn total_generated_duration(&self) -> Tick {
        self.generated_items
            .iter()
            .map(|item| item.timeline_duration)
            .fold(Tick::ZERO, |acc, d| acc + d)
    }
}
