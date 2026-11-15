//! Layer 3: Media crate for VentaCore.
//!
//! Studies footage: content fingerprinting, thumbnails, staged scene analysis,
//! black range detection, motion estimation, and cache management.

pub mod analysis;
pub mod cache;
pub mod error;
pub mod fingerprint;
pub mod full_pass;
pub mod pipeline;
pub mod probe;
pub mod quick_pass;
pub mod thumbnail;

pub use analysis::{AnalysisLevel, FootageAnalysis, MotionPoint};
pub use cache::MediaCache;
pub use error::MediaError;
pub use fingerprint::{compute_bytes_fingerprint, compute_content_fingerprint};
pub use full_pass::run_full_pass;
pub use pipeline::{analyze_media_file, AssetAnalysisResult};
pub use probe::{probe_video_container, Stage0ProbeResult};
pub use quick_pass::{compute_average_luminance, compute_frame_difference, run_quick_pass, QuickPassResult};
pub use thumbnail::{downscale_rgba8, ThumbnailFrame};
