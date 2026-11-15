//! Timecode formatting and conversion helpers
//!
//! Formats integer Tick time into standard timecodes without rounding errors.

use ventacore_core::time::{Tick, TICKS_PER_SECOND};

/// Format Tick into standard MM:SS.mmm
pub fn format_time_minutes_seconds(tick: Tick) -> String {
    let total_secs = tick.0 as f64 / TICKS_PER_SECOND as f64;
    let mins = (total_secs / 60.0).floor() as u64;
    let secs = total_secs % 60.0;
    format!("{:02}:{:05.2}", mins, secs)
}

/// Format Tick into SMPTE timecode HH:MM:SS:FF at given frame rate
pub fn format_smpte_timecode(tick: Tick, fps: f64) -> String {
    let total_secs = (tick.0 as f64) / (TICKS_PER_SECOND as f64);
    let hours = (total_secs / 3600.0).floor() as u64;
    let mins = ((total_secs % 3600.0) / 60.0).floor() as u64;
    let secs = (total_secs % 60.0).floor() as u64;
    let frame = ((total_secs % 1.0) * fps).floor() as u64;

    format!("{:02}:{:02}:{:02}:{:02}", hours, mins, secs, frame)
}
