//! Timeline interval partitioner based on beat grids, energy scaling, and offsets.

use crate::planner::prng::DeterministicRng;
use ventacore_audio::SongStructure;
use ventacore_core::{GenerationSettings, Tick, TimeRange};

#[derive(Debug, Clone, PartialEq)]
pub struct RawSlot {
    pub start: Tick,
    pub duration: Tick,
    pub bar_energy: f32,
}

impl RawSlot {
    pub fn end(&self) -> Tick {
        self.start + self.duration
    }
}

/// Partitions an open timeline interval into cut slots according to:
/// - Song beat grid and bar boundaries
/// - Energy density scaling (higher energy -> shorter cuts)
/// - Clip variance (jitter around target beat multiple)
/// - Min/max duration limits
/// - Cut offset (landing ahead of beats)
pub fn slice_open_region(
    region: TimeRange,
    song: &SongStructure,
    settings: &GenerationSettings,
    prng: &mut DeterministicRng,
) -> Vec<RawSlot> {
    if region.duration <= Tick::ZERO {
        return Vec::new();
    }

    // If region duration is already very short, emit single slot
    if region.duration <= settings.min_clip_duration {
        let energy = get_energy_at_tick(region.start, song);
        return vec![RawSlot {
            start: region.start,
            duration: region.duration,
            bar_energy: energy,
        }];
    }

    let mut slots: Vec<RawSlot> = Vec::new();
    let mut cursor = region.start;
    let end_limit = region.end();

    while cursor < end_limit {
        let remaining = end_limit - cursor;

        // If what remains is less than or close to min_clip_duration, merge into last slot or emit final slot
        if remaining <= settings.min_clip_duration {
            if let Some(last) = slots.last_mut() {
                last.duration += remaining;
            } else {
                slots.push(RawSlot {
                    start: cursor,
                    duration: remaining,
                    bar_energy: get_energy_at_tick(cursor, song),
                });
            }
            break;
        }

        let bar_energy = get_energy_at_tick(cursor, song);
        let effective_energy = bar_energy * settings.energy_density.clamp(0.0, 1.0);

        // Determine base target beat count
        let mut target_beats: usize = if effective_energy >= 0.70 {
            1 // 1 beat for intense / fast passage
        } else if effective_energy >= 0.35 {
            2 // 2 beats for moderate passage
        } else {
            4 // 4 beats (1 bar) for calm / slow passage
        };

        // Apply clip variance
        if settings.clip_variance > 0.05 {
            let roll = prng.next_f32();
            if roll < settings.clip_variance * 0.4 {
                // Shorten cut if possible
                if target_beats > 1 {
                    target_beats -= 1;
                }
            } else if roll > 1.0 - (settings.clip_variance * 0.4) {
                // Lengthen cut
                target_beats += if prng.next_f32() > 0.5 { 2 } else { 1 };
            }
        }

        // Find candidate cut point using beat_ticks
        let candidate_cut = find_next_beat_cut(
            cursor,
            target_beats,
            song,
            settings.cut_offset,
            settings.min_clip_duration,
            settings.max_clip_duration,
            end_limit,
        );

        let mut slot_dur = candidate_cut - cursor;

        // Enforce min and max limits
        if slot_dur < settings.min_clip_duration && remaining > settings.min_clip_duration {
            slot_dur = settings.min_clip_duration.min(remaining);
        } else if slot_dur > settings.max_clip_duration {
            slot_dur = settings.max_clip_duration.min(remaining);
        }

        // Check trailing remainder: don't leave an orphan slot smaller than min_clip_duration
        let next_cursor = cursor + slot_dur;
        let trailing = end_limit - next_cursor;
        if trailing > Tick::ZERO && trailing < settings.min_clip_duration {
            // Extend this slot to cover the small trailing piece
            slot_dur = end_limit - cursor;
        }

        slots.push(RawSlot {
            start: cursor,
            duration: slot_dur,
            bar_energy,
        });

        cursor += slot_dur;
    }

    slots
}

/// Computes the song energy at a given timeline tick.
pub fn get_energy_at_tick(tick: Tick, song: &SongStructure) -> f32 {
    if !song.energy_per_bar.is_empty() && !song.bar_ticks.is_empty() {
        for (i, &bar_start) in song.bar_ticks.iter().enumerate() {
            let next_bar = song.bar_ticks.get(i + 1).copied();
            let bar_end = next_bar.unwrap_or(Tick::from_seconds(999999));
            if tick >= bar_start && tick < bar_end {
                return song.energy_per_bar.get(i).copied().unwrap_or(0.5);
            }
        }
    }

    if !song.energy_per_second.is_empty() {
        let sec = (tick.to_seconds_f64().floor() as usize).min(song.energy_per_second.len() - 1);
        return song.energy_per_second.get(sec).copied().unwrap_or(0.5);
    }

    0.5
}

/// Finds the next beat cut point after `cursor` given target beats.
fn find_next_beat_cut(
    cursor: Tick,
    target_beats: usize,
    song: &SongStructure,
    cut_offset: Tick,
    min_dur: Tick,
    max_dur: Tick,
    end_limit: Tick,
) -> Tick {
    // If we have beat ticks from song analysis, find the beats ahead of cursor
    if !song.beat_ticks.is_empty() {
        let first_beat_idx = song.beat_ticks.iter().position(|&b| b > cursor);

        if let Some(first_idx) = first_beat_idx {
            let target_idx = (first_idx + target_beats.saturating_sub(1)).min(song.beat_ticks.len() - 1);
            let raw_beat = song.beat_ticks[target_idx];
            let offset_cut = raw_beat + cut_offset;

            // Offset cut must be strictly greater than cursor
            if offset_cut > cursor && offset_cut <= end_limit {
                let dur = offset_cut - cursor;
                if dur >= min_dur && dur <= max_dur {
                    return offset_cut;
                }
            }
        }
    }

    // Fallback if no beat or offset beat out of bounds: estimate from BPM
    let bpm = if song.bpm > 30.0 { song.bpm } else { 120.0 };
    let beat_duration_sec = 60.0 / bpm as f64;
    let beat_ticks = Tick::from_seconds_f64(beat_duration_sec);
    let nominal_dur = beat_ticks * (target_beats as i64);

    let mut target = cursor + nominal_dur + cut_offset;
    if target <= cursor {
        target = cursor + min_dur;
    }
    target.min(end_limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ventacore_core::GridMode;

    fn make_test_song(bpm: f32, duration_sec: f64, energy_level: f32) -> SongStructure {
        let beat_duration_sec = 60.0 / bpm as f64;
        let beat_tick = Tick::from_seconds_f64(beat_duration_sec);
        let num_beats = (duration_sec / beat_duration_sec) as usize;

        let beat_ticks: Vec<Tick> = (1..=num_beats).map(|i| beat_tick * (i as i64)).collect();
        let bar_ticks: Vec<Tick> = beat_ticks.iter().step_by(4).copied().collect();
        let energy_per_bar = vec![energy_level; bar_ticks.len() + 1];

        SongStructure {
            bpm,
            beats_per_bar: 4,
            bars_per_phrase: 4,
            grid_mode: GridMode::Grid,
            beat_ticks,
            bar_ticks,
            phrase_ticks: Vec::new(),
            onsets: Vec::new(),
            change_points: Vec::new(),
            waveform_peaks: Vec::new(),
            loudness_envelope: Vec::new(),
            energy_per_second: vec![energy_level; duration_sec.ceil() as usize],
            energy_per_bar,
        }
    }

    #[test]
    fn test_higher_energy_produces_shorter_cuts() {
        let song_high = make_test_song(120.0, 30.0, 0.95);
        let song_low = make_test_song(120.0, 30.0, 0.10);

        let mut settings = GenerationSettings::default();
        settings.clip_variance = 0.0;
        settings.energy_density = 1.0;
        settings.min_clip_duration = Tick::from_seconds_f64(0.2);
        settings.max_clip_duration = Tick::from_seconds_f64(5.0);

        let region = TimeRange::from_start_end(Tick::ZERO, Tick::from_seconds(20));

        let mut rng1 = DeterministicRng::new(42);
        let slots_high = slice_open_region(region, &song_high, &settings, &mut rng1);

        let mut rng2 = DeterministicRng::new(42);
        let slots_low = slice_open_region(region, &song_low, &settings, &mut rng2);

        assert!(slots_high.len() > slots_low.len(), "High energy must produce more cuts ({}) than low energy ({})", slots_high.len(), slots_low.len());

        let avg_dur_high: f64 = slots_high.iter().map(|s| s.duration.to_seconds_f64()).sum::<f64>() / slots_high.len() as f64;
        let avg_dur_low: f64 = slots_low.iter().map(|s| s.duration.to_seconds_f64()).sum::<f64>() / slots_low.len() as f64;

        assert!(avg_dur_high < avg_dur_low, "High energy average duration ({:.2}s) must be shorter than low energy ({:.2}s)", avg_dur_high, avg_dur_low);
    }

    #[test]
    fn test_cuts_land_on_grid_with_offset() {
        let song = make_test_song(120.0, 20.0, 0.5);
        let mut settings = GenerationSettings::default();
        let offset = Tick::from_seconds_f64(-0.05); // -50ms
        settings.cut_offset = offset;
        settings.clip_variance = 0.0;
        settings.min_clip_duration = Tick::from_seconds_f64(0.4);

        let region = TimeRange::from_start_end(Tick::ZERO, Tick::from_seconds(10));
        let mut rng = DeterministicRng::new(100);
        let slots = slice_open_region(region, &song, &settings, &mut rng);

        // Every internal cut point should match beat + offset
        for slot in &slots[..slots.len() - 1] {
            let cut_end = slot.end();
            let matches_beat_offset = song.beat_ticks.iter().any(|&b| b + offset == cut_end);
            assert!(matches_beat_offset, "Cut point {} should land on a beat shifted by offset {}", cut_end, offset);
        }
    }
}
