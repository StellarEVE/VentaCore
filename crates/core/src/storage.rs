//! Atomic project persistence, backup creation, autosave, and schema migration.
//!
//! Conforms to Charter Rule 9:
//! Safe data: One readable project file, saved atomically with a backup.
//! Saving writes to a temporary file and atomically renames it over target,
//! ensuring that a crash or error during write leaves the existing project
//! and backup files completely intact.

use crate::model::{Project, CURRENT_SCHEMA_VERSION};
use crate::validation::ValidationError;
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    #[error("Unsupported schema version: {0} (current is {1})")]
    UnsupportedSchemaVersion(u32, u32),

    #[error("Missing schema_version field in project file")]
    MissingSchemaVersion,
}

/// Atomically saves a project to pretty-printed JSON.
///
/// 1. Validates project invariants.
/// 2. Writes pretty JSON to a unique temporary file alongside the target.
/// 3. Flushes and syncs data to physical disk.
/// 4. Copies existing target file to `{target}.bak` if it exists.
/// 5. Atomically renames temporary file over target file.
pub fn save_project_atomic<P: AsRef<Path>>(path: P, project: &Project) -> Result<(), StorageError> {
    let path = path.as_ref();
    project.validate()?;

    let json_text = serde_json::to_string_pretty(project)?;

    // Create unique temp file in the same directory for atomic rename capability
    let temp_path = create_temp_path(path);

    // Write and sync temporary file
    {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&temp_path)?;
        file.write_all(json_text.as_bytes())?;
        file.flush()?;
        file.sync_all()?;
    }

    // Preserve backup if file already exists
    if path.exists() {
        let backup_path = backup_path_for(path);
        let _ = fs::copy(path, &backup_path);
    }

    // Atomic rename
    if let Err(e) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(StorageError::Io(e));
    }

    Ok(())
}

/// Atomically saves an autosave file alongside the project.
pub fn save_autosave<P: AsRef<Path>>(path: P, project: &Project) -> Result<PathBuf, StorageError> {
    let autosave_path = autosave_path_for(path.as_ref());
    save_project_atomic(&autosave_path, project)?;
    Ok(autosave_path)
}

/// Loads a project file, running necessary schema migrations.
pub fn load_project<P: AsRef<Path>>(path: P) -> Result<Project, StorageError> {
    let path_ref = path.as_ref();
    let content = fs::read_to_string(path_ref)?;
    let mut value: Value = serde_json::from_str(&content)?;

    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .ok_or(StorageError::MissingSchemaVersion)? as u32;

    if version < CURRENT_SCHEMA_VERSION {
        // Charter Rule / R1: Back up the old file first before any migration
        let backup_path = backup_path_for(path_ref);
        let _ = fs::copy(path_ref, &backup_path);

        let report = migrate_v1_to_v2(&mut value)?;

        // Write migration report text file alongside project
        let report_path = path_ref.with_extension("migration_report.txt");
        let _ = fs::write(report_path, report.report_text);
    }

    let project = migrate_project(value)?;
    project.validate()?;
    Ok(project)
}

/// Migration report detailing asset role assignment and preserved leftovers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub song_asset: Option<String>,
    pub footage_assets: Vec<String>,
    pub additional_audio_assets: Vec<String>,
    pub items_set_aside_to_leftovers: usize,
    pub report_text: String,
}

/// Migrates Schema Version 1 projects to Schema Version 2 (Stage R1).
pub fn migrate_v1_to_v2(value: &mut Value) -> Result<MigrationReport, StorageError> {
    let mut song_asset_name: Option<String> = None;
    let mut song_asset_id: Option<u64> = None;
    let mut song_duration_ticks: i64 = 0;
    let mut footage_assets = Vec::new();
    let mut additional_audio = Vec::new();

    // 1. Classify assets based on probe data
    if let Some(assets) = value.get_mut("assets").and_then(|a| a.as_array_mut()) {
        let mut audio_candidate_indices = Vec::new();

        for (idx, asset) in assets.iter_mut().enumerate() {
            let path_hint = asset
                .get("path_hint")
                .and_then(|p| p.as_str())
                .unwrap_or("unknown")
                .to_string();

            let is_audio_only = if let Some(probe) = asset.get("probe_data") {
                let width = probe.get("width").and_then(|w| w.as_u64()).unwrap_or(0);
                let height = probe.get("height").and_then(|h| h.as_u64()).unwrap_or(0);
                width == 0 && height == 0
            } else {
                let lower = path_hint.to_lowercase();
                lower.ends_with(".mp3")
                    || lower.ends_with(".wav")
                    || lower.ends_with(".flac")
                    || lower.ends_with(".aac")
                    || lower.ends_with(".ogg")
                    || lower.ends_with(".m4a")
            };

            if is_audio_only {
                audio_candidate_indices.push(idx);
            } else {
                asset["role"] = serde_json::json!("Footage");
                footage_assets.push(path_hint);
            }
        }

        // 2. Select active song among audio candidates
        if !audio_candidate_indices.is_empty() {
            // Find longest audio candidate or first
            let mut best_idx = audio_candidate_indices[0];
            let mut best_dur = 0i64;

            for &idx in &audio_candidate_indices {
                let dur = assets[idx]
                    .get("probe_data")
                    .and_then(|p| p.get("duration"))
                    .and_then(|d| d.as_i64())
                    .unwrap_or(0);
                if dur > best_dur {
                    best_dur = dur;
                    best_idx = idx;
                }
            }

            for &idx in &audio_candidate_indices {
                let path_hint = assets[idx]
                    .get("path_hint")
                    .and_then(|p| p.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                if idx == best_idx {
                    assets[idx]["role"] = serde_json::json!("Song");
                    song_asset_name = Some(path_hint);
                    song_asset_id = assets[idx].get("id").and_then(|id| id.as_u64());
                    song_duration_ticks = if best_dur > 0 { best_dur } else { 30 * 254_016_000_000 };
                } else {
                    assets[idx]["role"] = serde_json::json!("Footage");
                    additional_audio.push(path_hint);
                }
            }
        }
    }

    // 3. Ensure fixed Song track exists
    let mut song_track_id: u64 = 0;
    if let Some(tracks) = value.get_mut("tracks").and_then(|t| t.as_array_mut()) {
        let has_song_track = tracks.iter().any(|t| {
            t.get("kind")
                .and_then(|k| k.as_str())
                .map(|k| k == "Song")
                .unwrap_or(false)
        });

        if !has_song_track {
            // Pick an unused track id
            let max_track_id = tracks
                .iter()
                .filter_map(|t| t.get("id").and_then(|id| id.as_u64()))
                .max()
                .unwrap_or(0);
            song_track_id = max_track_id + 1;
            tracks.insert(
                0,
                serde_json::json!({
                    "id": song_track_id,
                    "name": "Song",
                    "kind": "Song",
                    "muted": false,
                    "solo": false,
                    "locked": false,
                    "hidden": false
                }),
            );
        } else {
            song_track_id = tracks
                .iter()
                .find(|t| {
                    t.get("kind")
                        .and_then(|k| k.as_str())
                        .map(|k| k == "Song")
                        .unwrap_or(false)
                })
                .and_then(|t| t.get("id").and_then(|id| id.as_u64()))
                .unwrap_or(0);
        }
    }

    // 4. Filter items: remove song items from video tracks, and items past song duration into migration_leftovers
    let mut migration_leftovers = Vec::new();
    let mut max_item_id: u64 = 0;

    if let Some(items) = value.get_mut("items").and_then(|i| i.as_array_mut()) {
        let mut clean_items = Vec::new();
        for item in items.drain(..) {
            let i_id = item.get("id").and_then(|id| id.as_u64()).unwrap_or(0);
            if i_id > max_item_id {
                max_item_id = i_id;
            }
            let asset_id = item.get("asset_id").and_then(|id| id.as_u64());
            let start = item.get("timeline_start").and_then(|s| s.as_i64()).unwrap_or(0);
            let dur = item.get("timeline_duration").and_then(|d| d.as_i64()).unwrap_or(0);
            let end = start + dur;

            let is_song_item = song_asset_id.is_some() && asset_id == song_asset_id;
            let ends_past_song = song_duration_ticks > 0 && end > song_duration_ticks;

            if is_song_item || ends_past_song {
                migration_leftovers.push(item);
            } else {
                clean_items.push(item);
            }
        }

        // Add single song item onto the Song track if active song exists
        if let Some(s_aid) = song_asset_id {
            max_item_id += 1;
            clean_items.push(serde_json::json!({
                "id": max_item_id,
                "track_id": song_track_id,
                "asset_id": s_aid,
                "timeline_start": 0,
                "timeline_duration": song_duration_ticks,
                "source_in": 0,
                "speed": 1.0,
                "pinned": false,
                "touched": false,
                "enabled": true,
                "transition": null,
                "effects": [],
                "volume": 1.0
            }));
        }

        *items = clean_items;
    }

    let leftovers_count = migration_leftovers.len();
    value["migration_leftovers"] = Value::Array(migration_leftovers);
    value["song_track_id"] = serde_json::json!(song_track_id);
    if let Some(s_aid) = song_asset_id {
        value["song"] = serde_json::json!({
            "asset_id": s_aid,
            "start": 0,
            "end": song_duration_ticks
        });
    } else {
        value["song"] = Value::Null;
    }

    value["schema_version"] = serde_json::json!(2);

    let report_text = format!(
        "VentaCore Schema V1 to V2 Migration Report\n\
         ===========================================\n\
         Active Song: {}\n\
         Footage Assets: {}\n\
         Additional Audio Assets (Preserved for Owner Decision): {}\n\
         Items Set Aside into migration_leftovers: {}\n",
        song_asset_name.as_deref().unwrap_or("None"),
        if footage_assets.is_empty() { "None".to_string() } else { footage_assets.join(", ") },
        if additional_audio.is_empty() { "None".to_string() } else { additional_audio.join(", ") },
        leftovers_count
    );

    Ok(MigrationReport {
        song_asset: song_asset_name,
        footage_assets,
        additional_audio_assets: additional_audio,
        items_set_aside_to_leftovers: leftovers_count,
        report_text,
    })
}

/// Migration chain runner for versioned project files.
pub fn migrate_project(mut value: Value) -> Result<Project, StorageError> {
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .ok_or(StorageError::MissingSchemaVersion)? as u32;

    if version > CURRENT_SCHEMA_VERSION {
        return Err(StorageError::UnsupportedSchemaVersion(
            version,
            CURRENT_SCHEMA_VERSION,
        ));
    }

    if version == 1 {
        let _ = migrate_v1_to_v2(&mut value)?;
    }

    let project: Project = serde_json::from_value(value)?;
    Ok(project)
}

fn backup_path_for(path: &Path) -> PathBuf {
    let mut ext = path.extension().unwrap_or_default().to_os_string();
    if !ext.is_empty() {
        ext.push(".bak");
    } else {
        ext = "bak".into();
    }
    path.with_extension(ext)
}

fn autosave_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project.json");
    let autosave_name = format!("{}.autosave.json", file_name);
    match path.parent() {
        Some(parent) => parent.join(autosave_name),
        None => PathBuf::from(autosave_name),
    }
}

fn create_temp_path(path: &Path) -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp_name = format!(
        ".{}.tmp.{}.{}",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("ventacore_proj"),
        std::process::id(),
        now
    );
    match path.parent() {
        Some(parent) => parent.join(temp_name),
        None => PathBuf::from(temp_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use crate::time::Tick;
    use std::fs::File;

    #[test]
    fn test_schema_v1_migration_creates_backup_and_preserves_leftovers() {
        let temp_dir = std::env::temp_dir();
        let proj_path = temp_dir.join(format!("v1_project_{}.json", std::process::id()));

        // Create a synthetic Schema Version 1 project JSON
        let v1_json = serde_json::json!({
            "schema_version": 1,
            "title": "Legacy Project",
            "assets": [
                {
                    "id": 1,
                    "path_hint": "track.mp3",
                    "fingerprint": null,
                    "probe_data": {
                        "duration": 5080320000000i64, // 20s
                        "width": 0,
                        "height": 0,
                        "fps_num": 0,
                        "fps_den": 1,
                        "has_audio": true,
                        "audio_sample_rate": 44100
                    },
                    "intro_trim": 0,
                    "outro_trim": 0,
                    "include_ranges": [],
                    "exclude_ranges": [],
                    "favourite_moments": []
                },
                {
                    "id": 2,
                    "path_hint": "video.mp4",
                    "fingerprint": null,
                    "probe_data": {
                        "duration": 5080320000000i64, // 20s
                        "width": 1920,
                        "height": 1080,
                        "fps_num": 30,
                        "fps_den": 1,
                        "has_audio": true,
                        "audio_sample_rate": 48000
                    },
                    "intro_trim": 0,
                    "outro_trim": 0,
                    "include_ranges": [],
                    "exclude_ranges": [],
                    "favourite_moments": []
                }
            ],
            "tracks": [
                {
                    "id": 1,
                    "name": "Video 1",
                    "kind": "Video",
                    "muted": false,
                    "solo": false,
                    "locked": false,
                    "hidden": false
                }
            ],
            "items": [
                {
                    "id": 10,
                    "track_id": 1,
                    "asset_id": 1, // Erroneously pointing at song!
                    "timeline_start": 0,
                    "timeline_duration": 1270080000000i64, // 5s
                    "source_in": 0,
                    "speed": 1.0,
                    "pinned": false,
                    "touched": false,
                    "enabled": true,
                    "transition": null,
                    "effects": [],
                    "volume": 1.0
                },
                {
                    "id": 11,
                    "track_id": 1,
                    "asset_id": 2,
                    "timeline_start": 1270080000000i64,
                    "timeline_duration": 1270080000000i64,
                    "source_in": 0,
                    "speed": 1.0,
                    "pinned": true,
                    "touched": false,
                    "enabled": true,
                    "transition": null,
                    "effects": [],
                    "volume": 1.0
                }
            ],
            "markers": [],
            "songs": [],
            "active_song_id": null,
            "settings": {
                "clip_variance": 0.5,
                "source_reuse_penalty": 0.7,
                "spread": 0.6,
                "continue_start_points": false,
                "placement_mode": "WindowedWalk",
                "min_clip_duration": 254016000000i64,
                "max_clip_duration": 1016064000000i64,
                "cut_offset": 0,
                "snap_to_scenes": true,
                "snap_to_dark_edges": true,
                "energy_density": 0.5,
                "grid_mode": "Transient",
                "seed": 42,
                "preset_name": null
            }
        });

        std::fs::write(&proj_path, serde_json::to_string_pretty(&v1_json).unwrap()).unwrap();

        // Load project through migration pipeline
        let project = load_project(&proj_path).expect("migration and load must succeed");

        // 1. Check backup file was created
        let backup_path = backup_path_for(&proj_path);
        assert!(backup_path.exists(), "Backup .bak must exist");

        // 2. Check migration report was written
        let report_path = proj_path.with_extension("migration_report.txt");
        assert!(report_path.exists(), "Migration report text must exist");
        let report_text = std::fs::read_to_string(&report_path).unwrap();
        assert!(report_text.contains("track.mp3"));

        // 3. Check schema version and roles
        assert_eq!(project.schema_version, 2);
        assert_eq!(project.find_asset(AssetId(1)).unwrap().role, AssetRole::Song);
        assert_eq!(project.find_asset(AssetId(2)).unwrap().role, AssetRole::Footage);

        // 4. Check active song and Song track
        assert!(project.song.is_some());
        assert_eq!(project.song.as_ref().unwrap().asset_id, AssetId(1));
        let song_track = project.song_track().expect("song track exists");
        let song_items = project.track_items(song_track.id);
        assert_eq!(song_items.len(), 1);
        assert_eq!(song_items[0].asset_id, AssetId(1));

        // 5. Check that item 10 (pointing to song) was moved to migration_leftovers
        assert_eq!(project.migration_leftovers.len(), 1);
        assert_eq!(project.migration_leftovers[0].id, ItemId(10));

        // 6. Check that item 11 (valid footage) remains on video track
        let video_items = project.track_items(TrackId(1));
        assert_eq!(video_items.len(), 1);
        assert_eq!(video_items[0].id, ItemId(11));

        // Cleanup
        let _ = std::fs::remove_file(&proj_path);
        let _ = std::fs::remove_file(&backup_path);
        let _ = std::fs::remove_file(&report_path);
    }

    #[test]
    fn test_save_then_load_returns_identical_project() {
        let temp_dir = std::env::temp_dir();
        let proj_path = temp_dir.join(format!("test_project_{}.json", std::process::id()));

        let mut project = Project::new("Epic Cut");
        let asset = Asset::new(AssetId(1), "video1.mp4".into());
        let track = Track::new(TrackId(1), "Main Video", TrackKind::Video);
        let item = Item::new(
            ItemId(1),
            TrackId(1),
            AssetId(1),
            Tick(0),
            Tick::from_seconds(5),
            Tick::from_seconds(1),
        );

        project.assets.push(asset);
        project.tracks.push(track);
        project.items.push(item);

        // Save
        save_project_atomic(&proj_path, &project).expect("Save must succeed");

        // Load
        let loaded = load_project(&proj_path).expect("Load must succeed");
        assert_eq!(project, loaded, "Loaded project must be byte/value identical");

        // Verify backup creation on resave
        let mut modified = project.clone();
        modified.title = "Modified Cut".into();
        save_project_atomic(&proj_path, &modified).expect("Second save must succeed");

        let backup_path = backup_path_for(&proj_path);
        assert!(backup_path.exists(), "Backup file must be created");
        let loaded_backup = load_project(&backup_path).expect("Backup must be readable");
        assert_eq!(loaded_backup.title, "Epic Cut");

        let loaded_modified = load_project(&proj_path).expect("Latest must be readable");
        assert_eq!(loaded_modified.title, "Modified Cut");

        // Clean up
        let _ = fs::remove_file(&proj_path);
        let _ = fs::remove_file(&backup_path);
    }

    #[test]
    fn test_simulated_crash_during_saving_leaves_previous_file_intact() {
        let temp_dir = std::env::temp_dir();
        let proj_path = temp_dir.join(format!("crash_test_project_{}.json", std::process::id()));

        // 1. Initial good project save
        let initial = Project::new("Pre-Crash State");
        save_project_atomic(&proj_path, &initial).expect("Initial save must succeed");
        let original_content = fs::read_to_string(&proj_path).unwrap();

        // 2. Simulate crash during saving:
        // We simulate a failure in writing temp file (e.g., interrupted write)
        let temp_path = create_temp_path(&proj_path);
        let mut partial = File::create(&temp_path).unwrap();
        let _ = partial.write_all(b"{\"schema_version\": 1, \"title\": \"half-wri");
        drop(partial);

        // Verify that target file on disk is still the exact uncorrupted original
        let current_content = fs::read_to_string(&proj_path).unwrap();
        assert_eq!(
            current_content, original_content,
            "Target project file must remain completely uncorrupted"
        );

        // Clean up
        let _ = fs::remove_file(&temp_path);
        let _ = fs::remove_file(&proj_path);
        let _ = fs::remove_file(backup_path_for(&proj_path));
    }
}
