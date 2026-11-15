//! Application State Container for VentaCore Shell
//!
//! Stores the active Project, History, background Conductor state,
//! playback transport, hardware disclosures, and UI toasts.
//!
//! Enforces Charter Rule 3: Every change is a validated command applied
//! through History, with rollback on failure.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

use ventacore_audio::SongStructure;
use ventacore_core::command::Command;
use ventacore_core::history::History;
use ventacore_core::model::{Asset, AssetId, Item, ItemId, Project};
use ventacore_core::storage::{save_autosave, save_project_atomic};
use ventacore_core::time::Tick;
use ventacore_engine::pipeline::{CancellationToken, ConductorUpdate};
use ventacore_media::cache::MediaCache;
use ventacore_platform::capabilities::HardwareCapabilityReport;
use ventacore_render::clock::MasterClock;
use ventacore_render::preview::FrameCache;
use ventacore_render::render_cache::RenderCache;
use ventacore_render::types::FitMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPage {
    Media,
    Cut,
    Edit,
    Export,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct ToastMessage {
    pub id: u64,
    pub level: ToastLevel,
    pub message: String,
    pub created_at: Instant,
}

#[derive(Debug)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub current_tick: Tick,
    pub playback_speed: f64,
    pub looping: bool,
    pub clock: MasterClock,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            is_playing: false,
            current_tick: Tick(0),
            playback_speed: 1.0,
            looping: false,
            clock: MasterClock::new(48_000),
        }
    }
}

pub struct ConductorState {
    pub is_running: bool,
    pub progress: f32,
    pub status: String,
    pub cancel_token: Option<CancellationToken>,
    pub receiver: Option<Receiver<ConductorUpdate>>,
}

impl Default for ConductorState {
    fn default() -> Self {
        Self {
            is_running: false,
            progress: 0.0,
            status: "Idle".to_string(),
            cancel_token: None,
            receiver: None,
        }
    }
}

pub struct ExportUiState {
    pub is_exporting: bool,
    pub progress: f32,
    pub status: String,
    pub output_path: PathBuf,
    pub selected_encoder: String,
    pub resolution: (u32, u32),
    pub framerate: u32,
    pub fit_mode: FitMode,
    pub cancel_sender: Option<Sender<()>>,
    pub progress_receiver: Option<Receiver<(f32, String)>>,
}

impl Default for ExportUiState {
    fn default() -> Self {
        Self {
            is_exporting: false,
            progress: 0.0,
            status: "Ready".to_string(),
            output_path: PathBuf::from("export.mp4"),
            selected_encoder: "Default".to_string(),
            resolution: (1920, 1080),
            framerate: 30,
            fit_mode: FitMode::Letterbox,
            cancel_sender: None,
            progress_receiver: None,
        }
    }
}

pub struct AppState {
    pub project: Project,
    pub history: History,
    pub project_path: Option<PathBuf>,
    pub active_page: AppPage,
    pub toasts: Vec<ToastMessage>,
    pub next_toast_id: u64,
    pub playback: PlaybackState,
    pub conductor: ConductorState,
    pub export: ExportUiState,
    pub hardware_report: Option<HardwareCapabilityReport>,
    pub selected_asset_id: Option<AssetId>,
    pub selected_timeline_item_id: Option<ItemId>,
    pub song_structure: Option<SongStructure>,
    pub media_cache: MediaCache,
    pub render_cache: RenderCache,
    pub frame_cache: FrameCache,
    pub is_dirty: bool,
    pub last_autosave: Instant,
}

impl AppState {
    pub fn new(cache_root: PathBuf) -> Self {
        let initial_project = Project::new("Untitled Project");
        let media_cache = MediaCache::new(cache_root.join("media_cache"))
            .unwrap_or_else(|_| MediaCache::new(std::env::temp_dir().join("ventacore_media")).unwrap());
        let render_cache = RenderCache::new(cache_root.join("render_cache"))
            .unwrap_or_else(|_| RenderCache::new(std::env::temp_dir().join("ventacore_render")).unwrap());
        let frame_cache = FrameCache::new(128 * 1024 * 1024); // 128 MB LRU budget

        Self {
            history: History::with_max_depth(initial_project.clone(), 50),
            project: initial_project,
            project_path: None,
            active_page: AppPage::Media,
            toasts: Vec::new(),
            next_toast_id: 1,
            playback: PlaybackState::default(),
            conductor: ConductorState::default(),
            export: ExportUiState::default(),
            hardware_report: None,
            selected_asset_id: None,
            selected_timeline_item_id: None,
            song_structure: None,
            media_cache,
            render_cache,
            frame_cache,
            is_dirty: false,
            last_autosave: Instant::now(),
        }
    }

    /// Dispatch a command to history and project (Charter Rule 3).
    pub fn apply_command(&mut self, cmd: Command) -> Result<(), String> {
        self.history
            .apply_command(&cmd)
            .map_err(|e| format!("Command error: {e}"))?;
        self.project = self.history.present().clone();
        self.is_dirty = true;
        Ok(())
    }

    /// Undo last command.
    pub fn undo(&mut self) -> Result<(), String> {
        if self.history.undo() {
            self.project = self.history.present().clone();
            self.is_dirty = true;
            self.add_toast(ToastLevel::Info, "Undone");
            Ok(())
        } else {
            Err("Nothing to undo".to_string())
        }
    }

    /// Redo previous command.
    pub fn redo(&mut self) -> Result<(), String> {
        if self.history.redo() {
            self.project = self.history.present().clone();
            self.is_dirty = true;
            self.add_toast(ToastLevel::Info, "Redone");
            Ok(())
        } else {
            Err("Nothing to redo".to_string())
        }
    }

    /// Add a notification toast banner.
    pub fn add_toast(&mut self, level: ToastLevel, msg: impl Into<String>) {
        let id = self.next_toast_id;
        self.next_toast_id += 1;
        self.toasts.push(ToastMessage {
            id,
            level,
            message: msg.into(),
            created_at: Instant::now(),
        });
    }

    /// Prune old toasts (older than 4 seconds).
    pub fn prune_toasts(&mut self) {
        let now = Instant::now();
        self.toasts
            .retain(|t| now.duration_since(t.created_at).as_secs_f32() < 4.0);
    }

    /// Run autosave if modified and interval elapsed (> 30s) (Charter Rule 9).
    pub fn autosave_heartbeat(&mut self) {
        if self.is_dirty && self.last_autosave.elapsed().as_secs() >= 30 {
            if let Some(path) = &self.project_path {
                let _ = save_autosave(path, &self.project);
            }
            self.last_autosave = Instant::now();
        }
    }

    /// Save project atomically.
    pub fn save_project(&mut self, path: PathBuf) -> Result<(), String> {
        save_project_atomic(&path, &self.project).map_err(|e| format!("Save error: {e}"))?;
        self.project_path = Some(path);
        self.is_dirty = false;
        self.add_toast(ToastLevel::Success, "Project saved successfully");
        Ok(())
    }

    /// Get selected asset if valid.
    pub fn selected_asset(&self) -> Option<&Asset> {
        let id = self.selected_asset_id?;
        self.project.assets.iter().find(|a| a.id == id)
    }

    /// Get selected timeline item if valid.
    pub fn selected_item(&self) -> Option<&Item> {
        let id = self.selected_timeline_item_id?;
        self.project.items.iter().find(|i| i.id == id)
    }
}
