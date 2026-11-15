//! Breadth-before-depth pipeline conductor for live cut construction.
//!
//! Orchestrates:
//! 1. Song analysis (detects tempo, beats, bars, energy curves).
//! 2. Quick pass over all videos (emits thumbnails and coarse scenes/motion).
//! 3. Provisional live cut assembly (clips drop onto timeline live).
//! 4. Full pass decode in priority order (assets on timeline analyzed first).
//! 5. Cut refinement (exact scene snapping, dark edge elimination).
//! 6. Cooperative cancellation via CancellationToken.

use crate::pipeline::cancellation::CancellationToken;
use crate::pipeline::events::{ConductorUpdate, PipelineStage, ProgressEvent};
use crate::planner::{apply_plan_to_project, plan_montage};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use ventacore_audio::SongStructure;
use ventacore_core::{AssetId, Project, TrackId};
use ventacore_media::{analyze_media_file, AnalysisLevel, FootageAnalysis, MediaCache};

/// Input configuration for the Conductor pipeline.
pub struct ConductorConfig {
    pub project: Project,
    pub song: SongStructure,
    pub asset_files: Vec<(AssetId, PathBuf)>,
    pub cache: MediaCache,
    pub target_track_id: Option<TrackId>,
}

/// Conductor handle managing background execution.
pub struct ConductorHandle {
    pub thread_handle: JoinHandle<Result<Project, String>>,
    pub receiver: Receiver<ConductorUpdate>,
    pub cancellation: CancellationToken,
}

/// Spawns the breadth-before-depth pipeline in a dedicated background worker thread.
pub fn spawn_conductor(config: ConductorConfig) -> ConductorHandle {
    let cancellation = CancellationToken::new();
    let cancellation_clone = cancellation.clone();
    let (sender, receiver) = mpsc::channel();

    let thread_handle = thread::spawn(move || {
        run_conductor_pipeline(config, sender, cancellation_clone)
    });

    ConductorHandle {
        thread_handle,
        receiver,
        cancellation,
    }
}

/// Executes the breadth-before-depth pipeline synchronously.
pub fn run_conductor_pipeline(
    mut config: ConductorConfig,
    sender: Sender<ConductorUpdate>,
    cancellation: CancellationToken,
) -> Result<Project, String> {
    let total_assets = config.asset_files.len();

    // 1. Song analysis is already provided in config
    let _ = sender.send(ConductorUpdate::Progress(ProgressEvent {
        stage: PipelineStage::AnalyzingSong,
        asset_id: None,
        progress: 0.10,
        message: format!("Song analysis ready: {:.1} BPM", config.song.bpm),
    }));

    if cancellation.is_cancelled() {
        let _ = sender.send(ConductorUpdate::Progress(ProgressEvent {
            stage: PipelineStage::Cancelled,
            asset_id: None,
            progress: 0.0,
            message: "Pipeline cancelled".into(),
        }));
        return Err("Pipeline cancelled".into());
    }

    // 2. Stage 1: Quick Pass across ALL imported assets
    let mut footage_map: HashMap<AssetId, FootageAnalysis> = HashMap::new();

    for (idx, (asset_id, path)) in config.asset_files.iter().enumerate() {
        if cancellation.is_cancelled() {
            let _ = sender.send(ConductorUpdate::Progress(ProgressEvent {
                stage: PipelineStage::Cancelled,
                asset_id: Some(*asset_id),
                progress: 0.0,
                message: "Pipeline cancelled during quick pass".into(),
            }));
            return Err("Pipeline cancelled".into());
        }

        let progress = 0.10 + (0.30 * (idx as f32 / total_assets.max(1) as f32));
        let _ = sender.send(ConductorUpdate::Progress(ProgressEvent {
            stage: PipelineStage::QuickPassingFootage {
                completed: idx,
                total: total_assets,
            },
            asset_id: Some(*asset_id),
            progress,
            message: format!("Quick pass on video {} of {}", idx + 1, total_assets),
        }));

        match analyze_media_file(path, *asset_id, &config.cache, AnalysisLevel::Provisional, None) {
            Ok(result) => {
                footage_map.insert(*asset_id, result.analysis);
            }
            Err(e) => {
                // Non-fatal for individual asset: log and continue
                eprintln!("Warning: quick pass failed for {:?}: {}", asset_id, e);
            }
        }
    }

    let _ = sender.send(ConductorUpdate::Progress(ProgressEvent {
        stage: PipelineStage::QuickPassingFootage {
            completed: total_assets,
            total: total_assets,
        },
        asset_id: None,
        progress: 0.40,
        message: "Quick passes complete on all videos".into(),
    }));

    if cancellation.is_cancelled() {
        return Err("Pipeline cancelled".into());
    }

    // 3. Emit Provisional Live Cut
    let provisional_plan = match plan_montage(
        &config.project,
        &config.song,
        &footage_map,
        None,
        config.target_track_id,
    ) {
        Ok(plan) => plan,
        Err(e) => {
            let msg = format!("Failed to generate provisional plan: {}", e);
            let _ = sender.send(ConductorUpdate::Progress(ProgressEvent {
                stage: PipelineStage::Failed(msg.clone()),
                asset_id: None,
                progress: 0.40,
                message: msg.clone(),
            }));
            return Err(msg);
        }
    };

    // Apply provisional plan to project
    let _ = apply_plan_to_project(&mut config.project, &provisional_plan);

    let _ = sender.send(ConductorUpdate::ProvisionalCut(provisional_plan.clone()));
    let _ = sender.send(ConductorUpdate::Progress(ProgressEvent {
        stage: PipelineStage::ProvisionalCutReady,
        asset_id: None,
        progress: 0.45,
        message: format!(
            "Provisional cut emitted with {} clips on timeline",
            provisional_plan.slots.len()
        ),
    }));

    if cancellation.is_cancelled() {
        return Err("Pipeline cancelled".into());
    }

    // 4. Prioritize Full Passes: Assets placed on timeline are analyzed FIRST
    let mut prioritized_assets = config.asset_files.clone();
    let placed_asset_ids: Vec<AssetId> = provisional_plan
        .generated_items
        .iter()
        .map(|item| item.asset_id)
        .collect();

    prioritized_assets.sort_by_key(|(aid, _)| {
        if placed_asset_ids.contains(aid) { 0 } else { 1 }
    });

    // 5. Stage 2: Full Passes in priority order
    for (idx, (asset_id, path)) in prioritized_assets.iter().enumerate() {
        if cancellation.is_cancelled() {
            let _ = sender.send(ConductorUpdate::Progress(ProgressEvent {
                stage: PipelineStage::Cancelled,
                asset_id: Some(*asset_id),
                progress: 0.0,
                message: "Pipeline cancelled during full pass".into(),
            }));
            return Err("Pipeline cancelled".into());
        }

        let progress = 0.45 + (0.45 * (idx as f32 / total_assets.max(1) as f32));
        let _ = sender.send(ConductorUpdate::Progress(ProgressEvent {
            stage: PipelineStage::FullPassingFootage {
                completed: idx,
                total: total_assets,
            },
            asset_id: Some(*asset_id),
            progress,
            message: format!("Full analysis on video {} of {}", idx + 1, total_assets),
        }));

        if let Ok(result) = analyze_media_file(
            path,
            *asset_id,
            &config.cache,
            AnalysisLevel::Full,
            None,
        ) {
            footage_map.insert(*asset_id, result.analysis);
        }
    }

    if cancellation.is_cancelled() {
        return Err("Pipeline cancelled".into());
    }

    // 6. Refine Cut with verified full analyses
    let refined_plan = match plan_montage(
        &config.project,
        &config.song,
        &footage_map,
        None,
        config.target_track_id,
    ) {
        Ok(plan) => plan,
        Err(e) => {
            let msg = format!("Failed to generate refined plan: {}", e);
            let _ = sender.send(ConductorUpdate::Progress(ProgressEvent {
                stage: PipelineStage::Failed(msg.clone()),
                asset_id: None,
                progress: 0.90,
                message: msg.clone(),
            }));
            return Err(msg);
        }
    };

    let _ = apply_plan_to_project(&mut config.project, &refined_plan);

    let _ = sender.send(ConductorUpdate::RefinedCut(refined_plan));
    let _ = sender.send(ConductorUpdate::Progress(ProgressEvent {
        stage: PipelineStage::RefinedCutReady,
        asset_id: None,
        progress: 0.95,
        message: "Cut refined with verified exact scenes and motion".into(),
    }));

    let _ = sender.send(ConductorUpdate::Progress(ProgressEvent {
        stage: PipelineStage::Complete,
        asset_id: None,
        progress: 1.0,
        message: "Live build complete".into(),
    }));

    let _ = sender.send(ConductorUpdate::Finished(config.project.clone()));

    Ok(config.project)
}
