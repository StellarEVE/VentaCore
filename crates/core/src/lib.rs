//! Layer 1: Core crate for VentaCore.
//!
//! Stores project state, timeline model, tick time arithmetic, edit commands,
//! undo/redo snapshots, and JSON persistence. Knows nothing of video files,
//! codecs, graphics, or windowing.

pub mod fixtures;
pub mod logger;

pub use fixtures::generate_click_train;
pub use logger::append_log_line;
