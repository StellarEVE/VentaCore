//! Pure functions for manual grid correction operations.
//!
//! Conforms to Stage 2 requirements:
//! - Tap tempo estimation from consecutive tap ticks.
//! - Half time (halves BPM, doubles beat period).
//! - Double time (doubles BPM, halves beat period).
//! - Shift grid (offsets all grid ticks by a delta).
//! - Set downbeat (re-anchors the grid so bar 1 beat 1 lands on downbeat).
//!
//! Free of any UI framework dependencies; directly callable by commands in Stage 12.

use crate::analysis::SongStructure;
use crate::grid::build_beat_grid;
use ventacore_core::Tick;

/// Estimates BPM from a slice of user tap timestamps.
pub fn tap_tempo(tap_ticks: &[Tick]) -> Option<f32> {
    if tap_ticks.len() < 2 {
        return None;
    }

    let mut total_interval_sec = 0.0_f64;
    let mut count = 0usize;

    for window in tap_ticks.windows(2) {
        let diff = window[1] - window[0];
        if diff > Tick::ZERO {
            total_interval_sec += diff.to_seconds_f64();
            count += 1;
        }
    }

    if count == 0 {
        return None;
    }

    let avg_interval_sec = total_interval_sec / count as f64;
    if avg_interval_sec <= 0.0 {
        return None;
    }

    let bpm = (60.0 / avg_interval_sec) as f32;
    Some(bpm.clamp(40.0, 240.0))
}

/// Halves tempo (e.g. 140 BPM -> 70 BPM), doubling beat spacing.
pub fn half_time(song: &SongStructure) -> SongStructure {
    let new_bpm = (song.bpm / 2.0).max(30.0);
    recompute_grid_with_tempo(song, new_bpm)
}

/// Doubles tempo (e.g. 70 BPM -> 140 BPM), halving beat spacing.
pub fn double_time(song: &SongStructure) -> SongStructure {
    let new_bpm = (song.bpm * 2.0).min(300.0);
    recompute_grid_with_tempo(song, new_bpm)
}

/// Shifts all beat, bar, and phrase ticks by `delta`.
pub fn shift_grid(song: &SongStructure, delta: Tick) -> SongStructure {
    let mut modified = song.clone();
    for b in &mut modified.beat_ticks {
        *b += delta;
    }
    for b in &mut modified.bar_ticks {
        *b += delta;
    }
    for p in &mut modified.phrase_ticks {
        *p += delta;
    }
    modified
}

/// Re-anchors the grid so that bar 1 beat 1 falls exactly at `downbeat`.
pub fn set_downbeat(song: &SongStructure, downbeat: Tick) -> SongStructure {
    if song.bpm <= 0.0 {
        return song.clone();
    }

    let period_sec = 60.0 / song.bpm as f64;
    let period_ticks = Tick::from_seconds_f64(period_sec);
    if period_ticks <= Tick::ZERO {
        return song.clone();
    }

    let total_duration = song
        .beat_ticks
        .last()
        .copied()
        .unwrap_or(Tick::from_seconds(300));

    let mut beat_ticks = Vec::new();

    // Step backwards to 0
    let mut t = downbeat;
    let mut prefix = Vec::new();
    while t >= period_ticks {
        t -= period_ticks;
        prefix.push(t);
    }
    prefix.reverse();
    beat_ticks.extend(prefix);

    // Step forward from downbeat to total_duration
    let mut t = downbeat;
    while t < total_duration {
        beat_ticks.push(t);
        t += period_ticks;
    }

    let bpb = song.beats_per_bar.max(1) as usize;
    let bpp = (song.beats_per_bar.max(1) * song.bars_per_phrase.max(1)) as usize;

    let downbeat_idx = beat_ticks
        .iter()
        .position(|&b| b == downbeat)
        .unwrap_or(0);

    let mut bar_ticks = Vec::new();
    for (i, &beat) in beat_ticks.iter().enumerate() {
        if i >= downbeat_idx && (i - downbeat_idx) % bpb == 0 {
            bar_ticks.push(beat);
        } else if i < downbeat_idx && (downbeat_idx - i) % bpb == 0 {
            bar_ticks.push(beat);
        }
    }
    bar_ticks.sort();

    let mut phrase_ticks = Vec::new();
    for (i, &beat) in beat_ticks.iter().enumerate() {
        if i >= downbeat_idx && (i - downbeat_idx) % bpp == 0 {
            phrase_ticks.push(beat);
        } else if i < downbeat_idx && (downbeat_idx - i) % bpp == 0 {
            phrase_ticks.push(beat);
        }
    }
    phrase_ticks.sort();

    let mut modified = song.clone();
    modified.beat_ticks = beat_ticks;
    modified.bar_ticks = bar_ticks;
    modified.phrase_ticks = phrase_ticks;
    modified
}

fn recompute_grid_with_tempo(song: &SongStructure, new_bpm: f32) -> SongStructure {
    let total_duration = song
        .beat_ticks
        .last()
        .copied()
        .unwrap_or(Tick::from_seconds(300));

    let (beats, bars, phrases) = build_beat_grid(
        new_bpm,
        total_duration,
        &song.onsets,
        song.grid_mode,
        song.beats_per_bar,
        song.bars_per_phrase,
    );

    let mut modified = song.clone();
    modified.bpm = new_bpm;
    modified.beat_ticks = beats;
    modified.bar_ticks = bars;
    modified.phrase_ticks = phrases;
    modified
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tap_tempo_computes_exact_bpm() {
        // Taps at 500ms intervals = 120 BPM
        let taps = vec![
            Tick::from_seconds_f64(0.0),
            Tick::from_seconds_f64(0.5),
            Tick::from_seconds_f64(1.0),
            Tick::from_seconds_f64(1.5),
        ];
        let measured = tap_tempo(&taps).expect("Must return BPM");
        assert!((measured - 120.0).abs() < 0.1);
    }
}
