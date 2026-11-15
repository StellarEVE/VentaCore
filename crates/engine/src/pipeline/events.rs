//! Pipeline progress events, stages, and update messages.

use crate::planner::MontagePlan;
use serde::{Deserialize, Serialize};
use ventacore_core::{AssetId, Project};

/// Discrete stages of the breadth-before-depth pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PipelineStage {
    Idle,
    AnalyzingSong,
    QuickPassingFootage { completed: usize, total: usize },
    ProvisionalCutReady,
    FullPassingFootage { completed: usize, total: usize },
    RefinedCutReady,
    Complete,
    Cancelled,
    Failed(String),
}

/// A progress notification emitted during pipeline execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub stage: PipelineStage,
    pub asset_id: Option<AssetId>,
    pub progress: f32,
    pub message: String,
}

/// Asynchronous update emitted by the Conductor across channels.
#[derive(Debug, Clone, PartialEq)]
pub enum ConductorUpdate {
    Progress(ProgressEvent),
    ProvisionalCut(MontagePlan),
    RefinedCut(MontagePlan),
    Finished(Project),
}
