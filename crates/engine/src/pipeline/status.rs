//! Real-time status reporting and metrics for the background pipeline.

use crate::pipeline::events::PipelineStage;
use serde::{Deserialize, Serialize};

/// High-level summary of the active pipeline execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineStatus {
    pub stage: PipelineStage,
    pub overall_progress: f32,
    pub song_analyzed: bool,
    pub quick_pass_completed: usize,
    pub full_pass_completed: usize,
    pub total_assets: usize,
    pub is_provisional_cut_ready: bool,
    pub is_refined_cut_ready: bool,
    pub is_frozen: bool,
    pub message: String,
}

impl Default for PipelineStatus {
    fn default() -> Self {
        Self {
            stage: PipelineStage::Idle,
            overall_progress: 0.0,
            song_analyzed: false,
            quick_pass_completed: 0,
            full_pass_completed: 0,
            total_assets: 0,
            is_provisional_cut_ready: false,
            is_refined_cut_ready: false,
            is_frozen: false,
            message: "Idle".into(),
        }
    }
}

impl PipelineStatus {
    /// Formatted status summary for UI status bars and CLI progress displays.
    pub fn summary(&self) -> String {
        match &self.stage {
            PipelineStage::Idle => "Idle - Ready to build".into(),
            PipelineStage::AnalyzingSong => "Listening to song (detecting tempo & beats)...".into(),
            PipelineStage::QuickPassingFootage { completed, total } => {
                format!("Quick pass on footage: {} of {} videos ready", completed, total)
            }
            PipelineStage::ProvisionalCutReady => "Provisional cut assembled live on timeline".into(),
            PipelineStage::FullPassingFootage { completed, total } => {
                format!("Full deep analysis: {} of {} videos refined", completed, total)
            }
            PipelineStage::RefinedCutReady => "Cut refined with exact scene cuts and motion".into(),
            PipelineStage::Complete => "Build complete".into(),
            PipelineStage::Cancelled => "Pipeline cancelled by user".into(),
            PipelineStage::Failed(reason) => format!("Pipeline failed: {}", reason),
        }
    }
}
