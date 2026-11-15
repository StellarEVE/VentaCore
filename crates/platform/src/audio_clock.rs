//! High-resolution master audio clock with steady timer fallback.
//!
//! Follows Part 4b & Charter Rule 2:
//! - When audio device is streaming, advances based on actual audio sample progression.
//! - When running headless or audio output is muted/unavailable, falls back to steady high-resolution monotonic timer.
//! - AV sync target: within 1 frame (<= 16.7ms at 60 fps, <= 33.3ms at 30 fps).

use std::time::Instant;
use ventacore_core::Tick;

/// Identifies whether time is driven by hardware audio DAC callbacks or monotonic timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockSource {
    AudioDevice,
    HighResolutionTimer,
}

/// Thread-safe or local master playback clock for audio/video synchronization.
#[derive(Debug, Clone)]
pub struct PlatformAudioClock {
    base_tick: Tick,
    start_instant: Option<Instant>,
    sample_rate: u32,
    audio_samples_played: i64,
    source: ClockSource,
    is_running: bool,
    speed: f64,
}

impl Default for PlatformAudioClock {
    fn default() -> Self {
        Self::new(48000)
    }
}

impl PlatformAudioClock {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            base_tick: Tick::ZERO,
            start_instant: None,
            sample_rate: sample_rate.max(1),
            audio_samples_played: 0,
            source: ClockSource::HighResolutionTimer,
            is_running: false,
            speed: 1.0,
        }
    }

    pub fn start(&mut self) {
        if !self.is_running {
            self.start_instant = Some(Instant::now());
            self.is_running = true;
        }
    }

    pub fn pause(&mut self) {
        if self.is_running {
            self.base_tick = self.now_tick();
            self.start_instant = None;
            self.is_running = false;
        }
    }

    pub fn stop(&mut self) {
        self.is_running = false;
        self.start_instant = None;
        self.base_tick = Tick::ZERO;
        self.audio_samples_played = 0;
    }

    pub fn seek(&mut self, target: Tick) {
        self.base_tick = target;
        self.audio_samples_played = target.to_samples(self.sample_rate).max(0);
        if self.is_running {
            self.start_instant = Some(Instant::now());
        }
    }

    pub fn set_speed(&mut self, speed: f64) {
        if speed > 0.0 {
            if self.is_running {
                self.base_tick = self.now_tick();
                self.start_instant = Some(Instant::now());
            }
            self.speed = speed;
        }
    }

    pub fn speed(&self) -> f64 {
        self.speed
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn clock_source(&self) -> ClockSource {
        self.source
    }

    /// Registers audio DAC sample advancement from an audio callback thread.
    pub fn on_audio_samples_written(&mut self, samples: usize) {
        self.source = ClockSource::AudioDevice;
        self.audio_samples_played += samples as i64;
    }

    /// Computes the exact current playback Tick.
    pub fn now_tick(&self) -> Tick {
        if !self.is_running {
            return self.base_tick;
        }

        match self.source {
            ClockSource::AudioDevice => {
                Tick::from_samples(self.audio_samples_played, self.sample_rate)
            }
            ClockSource::HighResolutionTimer => {
                if let Some(start) = self.start_instant {
                    let elapsed_sec = start.elapsed().as_secs_f64() * self.speed;
                    self.base_tick + Tick::from_seconds_f64(elapsed_sec)
                } else {
                    self.base_tick
                }
            }
        }
    }

    /// Evaluates if video frame timestamp is within sync tolerance of the master clock.
    /// Returns true if within +/- 1 frame duration (<= 16.7ms at 60fps, <= 33.3ms at 30fps).
    pub fn is_in_sync(&self, video_tick: Tick, frame_duration: Tick) -> bool {
        let current = self.now_tick();
        let diff = (video_tick.raw() - current.raw()).abs();
        diff <= frame_duration.raw()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_clock_timer_fallback_monotonicity() {
        let mut clock = PlatformAudioClock::new(48000);
        assert!(!clock.is_running());
        assert_eq!(clock.now_tick(), Tick::ZERO);

        clock.start();
        assert!(clock.is_running());

        std::thread::sleep(std::time::Duration::from_millis(20));
        let tick1 = clock.now_tick();
        assert!(tick1 > Tick::ZERO, "Monotonic timer must advance ticks");

        std::thread::sleep(std::time::Duration::from_millis(20));
        let tick2 = clock.now_tick();
        assert!(tick2 > tick1, "Ticks must advance monotonically");

        clock.pause();
        assert!(!clock.is_running());
        let paused_tick = clock.now_tick();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(clock.now_tick(), paused_tick, "Paused clock must remain steady");
    }

    #[test]
    fn test_audio_clock_hardware_samples_mode() {
        let mut clock = PlatformAudioClock::new(48000);
        clock.start();

        // Feed 48,000 samples = 1 second
        clock.on_audio_samples_written(48000);
        assert_eq!(clock.clock_source(), ClockSource::AudioDevice);
        assert_eq!(clock.now_tick(), Tick::from_seconds(1));

        // Seek to 5 seconds
        clock.seek(Tick::from_seconds(5));
        assert_eq!(clock.now_tick(), Tick::from_seconds(5));
    }
}
