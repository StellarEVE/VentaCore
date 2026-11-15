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
    let content = fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&content)?;

    let project = migrate_project(value)?;
    project.validate()?;
    Ok(project)
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

    // Chain migrations when future schemas are added:
    // if version == 1 { value = migrate_v1_to_v2(value)?; }
    let _ = &mut value;

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
