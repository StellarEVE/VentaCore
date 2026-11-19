//! Layer 5: App Library for VentaCore.
//!
//! Native desktop shell and state management for VentaCore Studio.

pub mod app;
pub mod pages;
pub mod state;
pub mod theme;
pub mod widgets;

pub use app::VentaApp;
pub use state::{AppPage, AppState, ToastLevel};
