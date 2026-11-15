//! Whole-number time module for VentaCore.
//!
//! Conforms to Charter Rule 2:
//! All positions and durations are integer ticks at 254,016,000,000 ticks per second.
//! Zero decimal seconds in edit mathematics.
//!
//! 254,016,000,000 divides cleanly by:
//! - Standard frame rates: 24, 25, 30, 50, 60, 120 fps.
//! - NTSC fractional rates: 23.976 (24000/1001), 29.97 (30000/1001), 59.94 (60000/1001) fps.
//! - Audio sample rates: 44,100 Hz and 48,000 Hz.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// The fundamental tick frequency: 254,016,000,000 ticks per second.
pub const TICKS_PER_SECOND: i64 = 254_016_000_000;

/// Exact integer tick representing timeline position or duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct Tick(pub i64);

impl Tick {
    pub const ZERO: Tick = Tick(0);
    pub const SECOND: Tick = Tick(TICKS_PER_SECOND);

    /// Construct a Tick from raw integer ticks.
    #[inline]
    pub const fn from_raw(raw: i64) -> Self {
        Tick(raw)
    }

    /// Return the raw integer tick count.
    #[inline]
    pub const fn raw(self) -> i64 {
        self.0
    }

    /// Construct a Tick from integer seconds.
    #[inline]
    pub fn from_seconds(secs: i64) -> Self {
        Tick(secs.saturating_mul(TICKS_PER_SECOND))
    }

    /// Convert from rational frame rate (num / den) and frame count.
    ///
    /// For integer frame rates, den = 1 (e.g. 24/1, 25/1, 30/1, 50/1, 60/1).
    /// For NTSC drop/fractional frame rates, num and den are exact:
    /// - 23.976 fps -> 24_000 / 1_001
    /// - 29.97 fps  -> 30_000 / 1_001
    /// - 59.94 fps  -> 60_000 / 1_001
    pub fn from_frames(frames: i64, rate_num: u32, rate_den: u32) -> Self {
        assert!(rate_num > 0, "Frame rate numerator must be positive");
        assert!(rate_den > 0, "Frame rate denominator must be positive");

        // Duration of 1 second is TICKS_PER_SECOND.
        // Duration of 1 frame is (TICKS_PER_SECOND * rate_den) / rate_num.
        // With TICKS_PER_SECOND = 254,016,000,000:
        // For 24_000/1_001: (254,016,000,000 * 1001) / 24_000 = 10,594,584,000 (exact integer)
        // For 30_000/1_001: (254,016,000,000 * 1001) / 30_000 = 8,475,667,200 (exact integer)
        // For 60_000/1_001: (254,016,000,000 * 1001) / 60_000 = 4,237,833,600 (exact integer)
        let ticks_per_frame = (TICKS_PER_SECOND as i128 * rate_den as i128) / rate_num as i128;
        let total_ticks = (frames as i128) * ticks_per_frame;
        Tick(total_ticks as i64)
    }

    /// Calculate exact frame count at a rational frame rate (rounding down).
    pub fn to_frames(self, rate_num: u32, rate_den: u32) -> i64 {
        assert!(rate_num > 0, "Frame rate numerator must be positive");
        assert!(rate_den > 0, "Frame rate denominator must be positive");
        let ticks_per_frame = (TICKS_PER_SECOND as i128 * rate_den as i128) / rate_num as i128;
        ((self.0 as i128) / ticks_per_frame) as i64
    }

    /// Convert from audio sample count and sample rate (in Hz).
    ///
    /// For 44,100 Hz: 254,016,000,000 / 44,100 = 5,760,000 ticks/sample (exact integer).
    /// For 48,000 Hz: 254,016,000,000 / 48,000 = 5,292,000 ticks/sample (exact integer).
    pub fn from_samples(samples: i64, sample_rate_hz: u32) -> Self {
        assert!(sample_rate_hz > 0, "Sample rate must be positive");
        let ticks_per_sample = (TICKS_PER_SECOND as i128) / (sample_rate_hz as i128);
        Tick(((samples as i128) * ticks_per_sample) as i64)
    }

    /// Convert to audio sample count at a given sample rate (in Hz).
    pub fn to_samples(self, sample_rate_hz: u32) -> i64 {
        assert!(sample_rate_hz > 0, "Sample rate must be positive");
        let ticks_per_sample = (TICKS_PER_SECOND as i128) / (sample_rate_hz as i128);
        ((self.0 as i128) / ticks_per_sample) as i64
    }

    /// Convert to floating-point seconds.
    ///
    /// CAUTION: Only use for UI display boundaries or external interfaces.
    /// Never use in edit mathematics.
    pub fn to_seconds_f64(self) -> f64 {
        self.0 as f64 / TICKS_PER_SECOND as f64
    }

    #[inline]
    pub fn saturating_sub(self, rhs: Tick) -> Self {
        Tick(self.0.saturating_sub(rhs.0))
    }

    #[inline]
    pub fn saturating_add(self, rhs: Tick) -> Self {
        Tick(self.0.saturating_add(rhs.0))
    }

    /// Construct from floating-point seconds (UI input boundary only).
    pub fn from_seconds_f64(secs: f64) -> Self {
        Tick((secs * TICKS_PER_SECOND as f64).round() as i64)
    }
}

impl fmt::Display for Tick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}t", self.0)
    }
}

impl Add for Tick {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Tick(self.0.saturating_add(rhs.0))
    }
}

impl AddAssign for Tick {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_add(rhs.0);
    }
}

impl Sub for Tick {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Tick(self.0.saturating_sub(rhs.0))
    }
}

impl SubAssign for Tick {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_sub(rhs.0);
    }
}

impl Mul<i64> for Tick {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: i64) -> Self {
        Tick(self.0.saturating_mul(rhs))
    }
}

impl Div<i64> for Tick {
    type Output = Self;
    #[inline]
    fn div(self, rhs: i64) -> Self {
        Tick(self.0 / rhs)
    }
}

impl Neg for Tick {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Tick(self.0.saturating_neg())
    }
}

/// A contiguous time range `[start, start + duration)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: Tick,
    pub duration: Tick,
}

impl TimeRange {
    pub fn new(start: Tick, duration: Tick) -> Self {
        assert!(duration >= Tick::ZERO, "TimeRange duration cannot be negative");
        Self { start, duration }
    }

    pub fn from_start_end(start: Tick, end: Tick) -> Self {
        assert!(end >= start, "TimeRange end must be >= start");
        Self {
            start,
            duration: end - start,
        }
    }

    pub fn end(self) -> Tick {
        self.start + self.duration
    }

    pub fn contains(self, tick: Tick) -> bool {
        tick >= self.start && tick < self.end()
    }

    pub fn overlaps(self, other: TimeRange) -> bool {
        self.start < other.end() && other.start < self.end()
    }

    pub fn intersection(self, other: TimeRange) -> Option<TimeRange> {
        let start = self.start.max(other.start);
        let end = self.end().min(other.end());
        if end > start {
            Some(TimeRange::from_start_end(start, end))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_integer_frame_rate_conversions() {
        let test_rates = [24, 25, 30, 50, 60];
        for fps in test_rates {
            let one_frame = Tick::from_frames(1, fps, 1);
            let total = one_frame * (fps as i64);
            assert_eq!(
                total,
                Tick::SECOND,
                "Integer frame rate {} fps must convert with exactly 0 rounding error",
                fps
            );
        }
    }

    #[test]
    fn test_exact_ntsc_fractional_frame_rate_conversions() {
        // 23.976 fps -> 24000 / 1001
        let tick_23_976 = Tick::from_frames(1, 24_000, 1_001);
        assert_eq!(tick_23_976.raw(), 10_594_584_000);
        let frames_24k = Tick::from_frames(24_000, 24_000, 1_001);
        assert_eq!(frames_24k.raw(), 1_001 * TICKS_PER_SECOND);

        // 29.97 fps -> 30000 / 1001
        let tick_29_97 = Tick::from_frames(1, 30_000, 1_001);
        assert_eq!(tick_29_97.raw(), 8_475_667_200);
        let frames_30k = Tick::from_frames(30_000, 30_000, 1_001);
        assert_eq!(frames_30k.raw(), 1_001 * TICKS_PER_SECOND);

        // 59.94 fps -> 60000 / 1001
        let tick_59_94 = Tick::from_frames(1, 60_000, 1_001);
        assert_eq!(tick_59_94.raw(), 4_237_833_600);
        let frames_60k = Tick::from_frames(60_000, 60_000, 1_001);
        assert_eq!(frames_60k.raw(), 1_001 * TICKS_PER_SECOND);
    }

    #[test]
    fn test_exact_audio_sample_rate_conversions() {
        // 44,100 Hz
        let tick_44100 = Tick::from_samples(1, 44_100);
        assert_eq!(tick_44100.raw(), 5_760_000);
        assert_eq!(tick_44100 * 44_100, Tick::SECOND);

        // 48,000 Hz
        let tick_48000 = Tick::from_samples(1, 48_000);
        assert_eq!(tick_48000.raw(), 5_292_000);
        assert_eq!(tick_48000 * 48_000, Tick::SECOND);
    }

    #[test]
    fn test_time_range_operations() {
        let r1 = TimeRange::new(Tick(100), Tick(200));
        assert_eq!(r1.end(), Tick(300));
        assert!(r1.contains(Tick(100)));
        assert!(r1.contains(Tick(299)));
        assert!(!r1.contains(Tick(300)));

        let r2 = TimeRange::new(Tick(250), Tick(100));
        assert!(r1.overlaps(r2));

        let r3 = TimeRange::new(Tick(300), Tick(100));
        assert!(!r1.overlaps(r3));
    }
}
