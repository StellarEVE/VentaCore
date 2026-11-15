//! Layer 4: Engine crate for VentaCore.
//!
//! Conductor for background analysis pipelines, live cut planning,
//! and job orchestration.

pub mod planner;

pub use planner::{
    apply_plan_to_project, compute_open_timeline_intervals, compute_valid_source_intervals,
    get_preset_settings, plan_montage, CandidateClip, DeterministicRng, MontagePlan, PlanSlot,
    PlannerError, PlannerState,
};
