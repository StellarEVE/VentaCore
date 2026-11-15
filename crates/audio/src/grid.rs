//! Beat, bar, and phrase grid construction.
//!
//! Supports two grid modes:
//! - `Grid`: strictly regular grid based on measured tempo and optimal phase.
//! - `Transient`: regular baseline snapped to nearby strong onsets to follow organic timing.

use crate::spectral::OnsetPeak;
use ventacore_core::{GridMode, Tick};

/// Constructs beat, bar, and phrase ticks from tempo, onsets, and duration.
pub fn build_beat_grid(
    bpm: f32,
    total_duration: Tick,
    onsets: &[OnsetPeak],
    grid_mode: GridMode,
    beats_per_bar: u32,
    bars_per_phrase: u32,
) -> (Vec<Tick>, Vec<Tick>, Vec<Tick>) {
    if bpm <= 0.0 || total_duration <= Tick::ZERO {
        return (Vec::new(), Vec::new(), Vec::new());
    }

    let period_sec = 60.0 / bpm as f64;
    let period_ticks = Tick::from_seconds_f64(period_sec);
    if period_ticks <= Tick::ZERO {
        return (Vec::new(), Vec::new(), Vec::new());
    }

    // Step 1: Find best phase (t0 in [0, period_ticks)) aligned with onsets
    let best_phase = find_best_phase(period_ticks, total_duration, onsets);

    // Step 2: Build beats according to GridMode
    let mut beat_ticks = Vec::new();
    let mut current_tick = best_phase;
    let snap_tolerance = period_ticks / 5; // +/- 20% window

    while current_tick < total_duration {
        let placed_tick = match grid_mode {
            GridMode::Grid => current_tick,
            GridMode::Transient => {
                // Find strongest onset in [current_tick - snap_tolerance, current_tick + snap_tolerance]
                let window_start = current_tick - snap_tolerance;
                let window_end = current_tick + snap_tolerance;

                let candidate = onsets
                    .iter()
                    .filter(|o| o.tick >= window_start && o.tick <= window_end)
                    .max_by(|a, b| a.strength.total_cmp(&b.strength));

                if let Some(best_onset) = candidate {
                    if best_onset.strength > 0.15 {
                        best_onset.tick
                    } else {
                        current_tick
                    }
                } else {
                    current_tick
                }
            }
        };

        beat_ticks.push(placed_tick);
        current_tick += period_ticks;
    }

    // Step 3: Derive bars and phrases
    let bpb = beats_per_bar.max(1) as usize;
    let bpp = (beats_per_bar.max(1) * bars_per_phrase.max(1)) as usize;

    let mut bar_ticks = Vec::new();
    for (i, &beat) in beat_ticks.iter().enumerate() {
        if i % bpb == 0 {
            bar_ticks.push(beat);
        }
    }

    let mut phrase_ticks = Vec::new();
    for (i, &beat) in beat_ticks.iter().enumerate() {
        if i % bpp == 0 {
            phrase_ticks.push(beat);
        }
    }

    (beat_ticks, bar_ticks, phrase_ticks)
}

fn find_best_phase(period_ticks: Tick, total_duration: Tick, onsets: &[OnsetPeak]) -> Tick {
    if onsets.is_empty() {
        return Tick::ZERO;
    }

    let num_steps = 100usize;
    let step_ticks = period_ticks / (num_steps as i64);
    if step_ticks <= Tick::ZERO {
        return Tick::ZERO;
    }

    let tolerance = period_ticks / 10; // +/- 10%

    let mut best_phase = Tick::ZERO;
    let mut best_score = f32::MIN;

    for s in 0..num_steps {
        let phase = step_ticks * (s as i64);
        let mut score = 0.0_f32;

        let mut beat = phase;
        while beat < total_duration {
            for o in onsets {
                if o.tick >= beat - tolerance && o.tick <= beat + tolerance {
                    score += o.strength;
                }
            }
            beat += period_ticks;
        }

        if score > best_score {
            best_score = score;
            best_phase = phase;
        }
    }

    best_phase
}
