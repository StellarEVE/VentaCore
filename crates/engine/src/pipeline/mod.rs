//! Pipeline execution and background conductor for live cut construction.

pub mod cancellation;
pub mod conductor;
pub mod events;
pub mod freeze;
pub mod status;

pub use cancellation::CancellationToken;
pub use conductor::{run_conductor_pipeline, spawn_conductor, ConductorConfig, ConductorHandle};
pub use events::{ConductorUpdate, PipelineStage, ProgressEvent};
pub use freeze::{freeze_timeline_cuts, unfreeze_timeline_cuts};
pub use status::PipelineStatus;
