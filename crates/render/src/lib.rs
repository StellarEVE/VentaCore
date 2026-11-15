//! Layer 3: Render crate for VentaCore.
//!
//! wgpu-backed graphics compositor, WGSL shader transitions and effects,
//! resolution tiers, CPU reference oracle, preview frame cache, master clock,
//! content-addressed render cache, and export frame generation.

pub mod clock;
pub mod compositor;
pub mod error;
pub mod oracle;
pub mod preview;
pub mod render_cache;
pub mod shaders;
pub mod types;

pub use clock::{MasterClock, PlaybackState, SyncAction};
pub use compositor::{BlitUniforms, GpuCompositor, TransitionUniforms};
pub use error::RenderError;
pub use oracle::CpuReferenceOracle;
pub use preview::{FrameCache, FrameCacheKey, PerformanceGovernor};
pub use render_cache::RenderCache;
pub use types::{
    FitMode, RenderFrame, ResolutionTier, Transform2D, TransitionKind, TransitionParams,
};
