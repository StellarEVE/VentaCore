//! Layer 1: Core crate for VentaCore.
//!
//! Stores project state, timeline model, tick time arithmetic, edit commands,
//! undo/redo snapshots, atomic storage, and schema migrations.
//!
//! Knows nothing of video files, codecs, graphics, or windowing.

pub mod command;
pub mod fixtures;
pub mod history;
pub mod logger;
pub mod model;
pub mod storage;
pub mod time;
pub mod validation;

pub use command::{Command, CommandError};
pub use fixtures::generate_click_train;
pub use history::History;
pub use logger::append_log_line;
pub use model::{
    Asset, AssetId, AssetRole, GenerationSettings, GridMode, Item, ItemId, Marker, MarkerId, PlacementMode,
    ProbeData, Project, ReplaceSongPolicy, SongEntry, SongId, SongSlot, Track, TrackId, TrackKind, Transition,
    TransitionKind, CURRENT_SCHEMA_VERSION,
};
pub use storage::{
    load_project, migrate_project, save_autosave, save_project_atomic, StorageError,
};
pub use time::{Tick, TimeRange, TICKS_PER_SECOND};
pub use validation::ValidationError;
