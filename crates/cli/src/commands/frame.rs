//! Frame extraction command for headless verification of the decode pipeline.
//!
//! Conforms to Stage R3:
//! Extracts a single decoded video frame to a PNG image at a given timestamp in seconds.

use std::path::PathBuf;
use ventacore_core::time::Tick;
use ventacore_engine::playback::PlaybackEngine;

pub fn execute_frame(
    video: PathBuf,
    at_secs: f64,
    out: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    if !video.exists() {
        return Err(format!("Input video file not found: {}", video.display()).into());
    }

    let mut player = PlaybackEngine::new();
    player.open_clip(&video)?;

    let target_tick = Tick::from_seconds_f64(at_secs);
    player.seek(target_tick);

    let frame = player
        .current_frame()
        .ok_or_else(|| "Failed to decode frame at target timestamp".to_string())?;

    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    image::save_buffer(
        &out,
        &frame.rgba,
        frame.width,
        frame.height,
        image::ExtendedColorType::Rgba8,
    )?;

    println!(
        "Extracted frame at {:.3}s ({}x{}) to {}",
        at_secs,
        frame.width,
        frame.height,
        out.display()
    );

    Ok(())
}
