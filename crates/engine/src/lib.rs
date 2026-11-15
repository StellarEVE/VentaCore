//! Layer 4: Engine crate for VentaCore.
//!
//! Conductor for background analysis pipelines, live cut planning,
//! and job orchestration.

pub mod export;
pub mod import;
pub mod pipeline;
pub mod planner;

pub use export::{export_timeline, ExportConfig, ExportError, ExportProgress, ExportSummary};
pub use import::{
    apply_import_plan, plan_import, ImportAction, ImportChoicePrompt, ImportDestination, ImportPlan,
};
pub use pipeline::{
    freeze_timeline_cuts, run_conductor_pipeline, spawn_conductor, unfreeze_timeline_cuts,
    CancellationToken, ConductorConfig, ConductorHandle, ConductorUpdate, PipelineStage,
    PipelineStatus, ProgressEvent,
};
pub use planner::{
    apply_plan_to_project, compute_open_timeline_intervals, compute_valid_source_intervals,
    get_preset_settings, plan_montage, CandidateClip, DeterministicRng, MontagePlan, PlanSlot,
    PlannerError, PlannerState,
};
