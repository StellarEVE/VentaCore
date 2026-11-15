//! Layer 5: App Library for VentaCore.
//!
//! Native desktop shell built with egui and eframe.
//! Implements Milestone B: Watch It Build.

pub mod app;
pub mod pages;
pub mod state;
pub mod theme;
pub mod widgets;

pub use app::VentaApp;
pub use state::{AppPage, AppState, ToastLevel};
