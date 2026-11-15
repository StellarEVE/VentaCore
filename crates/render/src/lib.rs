//! Layer 3: Render crate for VentaCore.
//!
//! wgpu-backed graphics compositor, WGSL shader transitions and effects,
//! resolution tiers, CPU reference oracle, and export frame generation.

pub mod compositor;
pub mod error;
pub mod oracle;
pub mod shaders;
pub mod types;

pub use compositor::{BlitUniforms, GpuCompositor, TransitionUniforms};
pub use error::RenderError;
pub use oracle::CpuReferenceOracle;
pub use types::{
    FitMode, RenderFrame, ResolutionTier, Transform2D, TransitionKind, TransitionParams,
};
