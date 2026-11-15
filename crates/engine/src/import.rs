//! Layer 4: Import pipeline and routing coordinator for VentaCore.
//!
//! Conforms to Part 4b & Stage R2:
//! - Classifies every incoming file using `ventacore_platform::classify`.
//! - Routes audio to Song and video to Footage according to Part 4b rules.
//! - Checks for duplicates and refuses unsupported or empty files.
//! - Enforces: dropped on Music panel vs Footage panel vs Anywhere else.
//! - Generates an `ImportPlan` with actions, choices, or refusals.
//! - Applies the import plan as atomic, undoable Project commands.

use std::path::PathBuf;
use ventacore_core::command::{Command, CommandError};
use ventacore_core::model::{Asset, AssetId, ProbeData, Project, ReplaceSongPolicy};
use ventacore_core::time::Tick;
use ventacore_platform::{classify, MediaClassification};

/// Where the user dropped or navigated the files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportDestination {
    MusicPanel,
    FootagePanel,
    AnywhereElse,
}

/// Disclose questions / choices required from the user before finalizing import.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportChoicePrompt {
    /// Dropped a video on the Music panel: ask whether to use sound as song or add to footage.
    VideoOnMusicPanel {
        path: PathBuf,
        classification: MediaClassification,
    },
    /// Multiple audio files arrived at once: ask which is the song.
    MultipleAudioFiles {
        candidates: Vec<PathBuf>,
    },
    /// A song is already active: ask whether to keep pinned clips or start over.
    ReplaceExistingSong {
        new_song_path: PathBuf,
    },
}

/// Action to be taken for an imported file.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportAction {
    ImportSong {
        path: PathBuf,
        classification: MediaClassification,
    },
    ImportFootage {
        path: PathBuf,
        classification: MediaClassification,
    },
    Refused {
        path: PathBuf,
        reason: String,
        suggest_use_as_song: bool,
    },
    NeedsChoice {
        path: PathBuf,
        choice: ImportChoicePrompt,
    },
}

/// Complete plan for an import transaction.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportPlan {
    pub actions: Vec<ImportAction>,
}

impl ImportPlan {
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn songs_count(&self) -> usize {
        self.actions.iter().filter(|a| matches!(a, ImportAction::ImportSong { .. })).count()
    }

    pub fn footage_count(&self) -> usize {
        self.actions.iter().filter(|a| matches!(a, ImportAction::ImportFootage { .. })).count()
    }

    pub fn refused_count(&self) -> usize {
        self.actions.iter().filter(|a| matches!(a, ImportAction::Refused { .. })).count()
    }

    pub fn choices_count(&self) -> usize {
        self.actions.iter().filter(|a| matches!(a, ImportAction::NeedsChoice { .. })).count()
    }
}

/// Evaluates a list of file paths against the current project and target drop location,
/// returning an `ImportPlan` conforming strictly to Part 4b.
pub fn plan_import(
    project: &Project,
    paths: &[PathBuf],
    destination: ImportDestination,
) -> ImportPlan {
    let mut actions = Vec::new();
    let mut pending_audio_candidates = Vec::new();

    // 1. First pass: Check for duplicates, classify, and bucket
    for path in paths {
        let path_str = path.to_string_lossy();

        // Duplicate check
        if project.assets.iter().any(|a| a.path_hint == path_str) {
            actions.push(ImportAction::Refused {
                path: path.clone(),
                reason: "Duplicate file: already imported in project".into(),
                suggest_use_as_song: false,
            });
            continue;
        }

        let classification = classify(path);

        match classification {
            MediaClassification::Unsupported { reason } => {
                actions.push(ImportAction::Refused {
                    path: path.clone(),
                    reason,
                    suggest_use_as_song: false,
                });
            }
            MediaClassification::AudioOnly { .. } => {
                pending_audio_candidates.push((path.clone(), classification));
            }
            MediaClassification::Video { .. } => {
                match destination {
                    ImportDestination::MusicPanel => {
                        actions.push(ImportAction::NeedsChoice {
                            path: path.clone(),
                            choice: ImportChoicePrompt::VideoOnMusicPanel {
                                path: path.clone(),
                                classification: classification.clone(),
                            },
                        });
                    }
                    ImportDestination::FootagePanel | ImportDestination::AnywhereElse => {
                        actions.push(ImportAction::ImportFootage {
                            path: path.clone(),
                            classification,
                        });
                    }
                }
            }
        }
    }

    // 2. Second pass: Process Audio candidates according to Part 4b rules
    if pending_audio_candidates.len() > 1 {
        // Multiple audio files at once: ask which is the song
        let candidate_paths: Vec<PathBuf> = pending_audio_candidates
            .iter()
            .map(|(p, _)| p.clone())
            .collect();
        for (path, _) in pending_audio_candidates {
            actions.push(ImportAction::NeedsChoice {
                path,
                choice: ImportChoicePrompt::MultipleAudioFiles {
                    candidates: candidate_paths.clone(),
                },
            });
        }
    } else if let Some((path, classification)) = pending_audio_candidates.into_iter().next() {
        match destination {
            ImportDestination::FootagePanel => {
                // Audio dropped on Footage panel: Refused with one-click Use as song button
                actions.push(ImportAction::Refused {
                    path,
                    reason: "Audio-only file dropped on Footage panel. Music belongs in the Song panel.".into(),
                    suggest_use_as_song: true,
                });
            }
            ImportDestination::MusicPanel | ImportDestination::AnywhereElse => {
                if project.song.is_some() {
                    actions.push(ImportAction::NeedsChoice {
                        path: path.clone(),
                        choice: ImportChoicePrompt::ReplaceExistingSong {
                            new_song_path: path,
                        },
                    });
                } else {
                    actions.push(ImportAction::ImportSong {
                        path,
                        classification,
                    });
                }
            }
        }
    }

    ImportPlan { actions }
}

/// Applies an `ImportPlan` to the project through atomic R1 commands.
/// Returns the list of executed commands so they can be undone as one unified step.
pub fn apply_import_plan(
    project: &mut Project,
    plan: &ImportPlan,
    replace_policy: ReplaceSongPolicy,
) -> Result<Vec<Command>, CommandError> {
    let mut executed_commands = Vec::new();
    let mut next_asset_id_raw = project.assets.iter().map(|a| a.id.0).max().unwrap_or(0);

    for action in &plan.actions {
        match action {
            ImportAction::ImportSong {
                path,
                classification,
            } => {
                next_asset_id_raw += 1;
                let asset_id = AssetId(next_asset_id_raw);

                let duration = match classification {
                    MediaClassification::AudioOnly { duration, .. } => *duration,
                    _ => Tick::from_seconds(30),
                };

                let mut asset = Asset::song(asset_id, path.to_string_lossy().to_string());
                asset.probe_data = Some(ProbeData {
                    duration,
                    width: 0,
                    height: 0,
                    fps_num: 0,
                    fps_den: 1,
                    has_audio: true,
                    audio_sample_rate: match classification {
                        MediaClassification::AudioOnly { sample_rate, .. } => *sample_rate,
                        _ => 44100,
                    },
                });

                let add_asset_cmd = Command::AddAsset(asset);
                project.apply_command(&add_asset_cmd)?;
                executed_commands.push(add_asset_cmd);

                let set_song_cmd = Command::SetSong {
                    asset_id,
                    start: Tick::ZERO,
                    end: duration,
                    policy: replace_policy,
                };
                project.apply_command(&set_song_cmd)?;
                executed_commands.push(set_song_cmd);
            }
            ImportAction::ImportFootage {
                path,
                classification,
            } => {
                next_asset_id_raw += 1;
                let asset_id = AssetId(next_asset_id_raw);

                let mut asset = Asset::footage(asset_id, path.to_string_lossy().to_string());

                if let MediaClassification::Video {
                    width,
                    height,
                    fps_num,
                    fps_den,
                    duration,
                    has_audio,
                } = classification
                {
                    asset.probe_data = Some(ProbeData {
                        duration: *duration,
                        width: *width,
                        height: *height,
                        fps_num: *fps_num,
                        fps_den: *fps_den,
                        has_audio: *has_audio,
                        audio_sample_rate: if *has_audio { 48000 } else { 0 },
                    });
                }

                let add_footage_cmd = Command::AddFootage(asset);
                project.apply_command(&add_footage_cmd)?;
                executed_commands.push(add_footage_cmd);
            }
            ImportAction::Refused { .. } | ImportAction::NeedsChoice { .. } => {
                // Refused or pending choice actions do not alter the project model
            }
        }
    }

    Ok(executed_commands)
}
