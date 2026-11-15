//! Audio Track Mixing, Envelope Fades, Volume Scaling, and Soft Limiting.
//!
//! Follows Charter Rule 2: Integer tick alignment at 254,016,000,000 ticks/sec.
//! Accurately mixes song audio and video clip audio into stereo output.

use ventacore_core::{Tick, TimeRange};

/// An individual audio track/clip feeding into the mixer.
#[derive(Debug, Clone)]
pub struct AudioMixTrack {
    pub start_tick: Tick,
    pub duration_ticks: Tick,
    pub volume: f32,
    pub fade_in_ticks: Tick,
    pub fade_out_ticks: Tick,
    pub mute: bool,
    pub solo: bool,
    /// Interleaved stereo f32 samples: [L0, R0, L1, R1, ...]
    pub samples: Vec<f32>,
}

impl AudioMixTrack {
    pub fn new(start_tick: Tick, duration_ticks: Tick, samples: Vec<f32>) -> Self {
        Self {
            start_tick,
            duration_ticks,
            volume: 1.0,
            fade_in_ticks: Tick::ZERO,
            fade_out_ticks: Tick::ZERO,
            mute: false,
            solo: false,
            samples,
        }
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume.max(0.0);
        self
    }

    pub fn with_fades(mut self, fade_in: Tick, fade_out: Tick) -> Self {
        self.fade_in_ticks = fade_in;
        self.fade_out_ticks = fade_out;
        self
    }
}

/// Multitrack stereo audio mixer with envelope fades and soft clipping.
#[derive(Debug, Clone)]
pub struct AudioMixer {
    sample_rate: u32,
}

impl AudioMixer {
    pub fn new(sample_rate: u32) -> Self {
        assert!(sample_rate > 0, "Sample rate must be positive");
        Self { sample_rate }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Mixes multiple audio tracks over a specified timeline range into interleaved stereo f32 samples.
    pub fn mix_range(
        &self,
        range: TimeRange,
        master_volume: f32,
        tracks: &[AudioMixTrack],
    ) -> Vec<f32> {
        let frame_count = range.duration.to_samples(self.sample_rate).max(0) as usize;
        let mut mixed = vec![0.0f32; frame_count * 2];

        if frame_count == 0 || tracks.is_empty() {
            return mixed;
        }

        let any_solo = tracks.iter().any(|t| t.solo);

        for track in tracks {
            if track.mute {
                continue;
            }
            if any_solo && !track.solo {
                continue;
            }
            if track.volume <= 0.0 || track.samples.is_empty() {
                continue;
            }

            let track_range = TimeRange::new(track.start_tick, track.duration_ticks);
            let overlap = match range.intersection(track_range) {
                Some(ov) => ov,
                None => continue,
            };

            let overlap_start_sample = (overlap.start.raw() - range.start.raw())
                .max(0)
                .min(range.duration.raw()) as f64
                / ventacore_core::TICKS_PER_SECOND as f64
                * self.sample_rate as f64;
            let dst_offset = (overlap_start_sample.round() as usize).min(frame_count);

            let track_offset_ticks = overlap.start.raw() - track.start_tick.raw();
            let track_start_sample = (track_offset_ticks.max(0) as f64
                / ventacore_core::TICKS_PER_SECOND as f64
                * self.sample_rate as f64)
                .round() as usize;

            let overlap_frames = (overlap.duration.to_samples(self.sample_rate) as usize)
                .min(frame_count.saturating_sub(dst_offset));

            let track_total_frames = track.samples.len() / 2;
            let gain = track.volume * master_volume.max(0.0);

            let fade_in_frames = track.fade_in_ticks.to_samples(self.sample_rate) as f32;
            let fade_out_frames = track.fade_out_ticks.to_samples(self.sample_rate) as f32;
            let total_dur_frames = track.duration_ticks.to_samples(self.sample_rate) as f32;

            for f in 0..overlap_frames {
                let src_f = track_start_sample + f;
                if src_f >= track_total_frames {
                    break;
                }

                let dst_f = dst_offset + f;
                if dst_f >= frame_count {
                    break;
                }

                // Envelope computation
                let mut env = 1.0f32;
                let cur_track_frame = src_f as f32;

                if fade_in_frames > 0.0 && cur_track_frame < fade_in_frames {
                    env *= (cur_track_frame / fade_in_frames).clamp(0.0, 1.0);
                }
                if fade_out_frames > 0.0 && cur_track_frame > total_dur_frames - fade_out_frames {
                    let remaining = (total_dur_frames - cur_track_frame).max(0.0);
                    env *= (remaining / fade_out_frames).clamp(0.0, 1.0);
                }

                let final_vol = gain * env;
                let src_idx = src_f * 2;
                let dst_idx = dst_f * 2;

                mixed[dst_idx] += track.samples[src_idx] * final_vol;
                mixed[dst_idx + 1] += track.samples[src_idx + 1] * final_vol;
            }
        }

        // Apply smooth soft limiting (tanh-like soft saturation)
        for s in mixed.iter_mut() {
            if s.abs() > 0.95 {
                *s = soft_limit(*s);
            }
        }

        mixed
    }

    /// Converts interleaved f32 audio samples (-1.0 to 1.0) to 16-bit PCM integer samples.
    pub fn to_interleaved_i16(samples: &[f32]) -> Vec<i16> {
        samples
            .iter()
            .map(|&s| {
                let clamped = s.clamp(-1.0, 1.0);
                (clamped * 32767.0).round() as i16
            })
            .collect()
    }
}

#[inline]
fn soft_limit(x: f32) -> f32 {
    if x > 0.0 {
        1.0 - (-x).exp()
    } else {
        (-x).exp() - 1.0
    }
}
