//! Audio Master Clock and audio-video playback synchronization.
//!
//! Follows Charter Rule 2: Integer tick arithmetic at 254,016,000,000 ticks/sec.
//! In VentaCore, audio output is the master clock; video rendering is slaved to it.

use ventacore_core::Tick;

/// Playback state of the master clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Action the video renderer must take to keep perfect audio/video synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncAction {
    /// Video is in sync: display this frame.
    Display,
    /// Video is lagging behind audio by more than 1 frame duration: drop frame to catch up.
    Drop,
    /// Video is ahead of audio by more than 1 frame duration: hold/repeat previous frame.
    Repeat,
}

/// Master playback clock driven by exact integer audio sample counts.
#[derive(Debug, Clone)]
pub struct MasterClock {
    sample_rate: u32,
    samples_played: i64,
    state: PlaybackState,
    speed: f32,
}

impl MasterClock {
    pub fn new(sample_rate: u32) -> Self {
        assert!(sample_rate > 0, "Sample rate must be positive");
        Self {
            sample_rate,
            samples_played: 0,
            state: PlaybackState::Stopped,
            speed: 1.0,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn state(&self) -> PlaybackState {
        self.state
    }

    pub fn is_playing(&self) -> bool {
        self.state == PlaybackState::Playing
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn set_speed(&mut self, speed: f32) {
        if speed > 0.0 {
            self.speed = speed;
        }
    }

    pub fn play(&mut self) {
        self.state = PlaybackState::Playing;
    }

    pub fn pause(&mut self) {
        self.state = PlaybackState::Paused;
    }

    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.samples_played = 0;
    }

    pub fn seek(&mut self, target_tick: Tick) {
        self.samples_played = target_tick.to_samples(self.sample_rate).max(0);
    }

    pub fn advance_audio_samples(&mut self, count: usize) {
        if self.state == PlaybackState::Playing {
            self.samples_played += count as i64;
        }
    }

    pub fn current_tick(&self) -> Tick {
        Tick::from_samples(self.samples_played, self.sample_rate)
    }

    pub fn samples_played(&self) -> i64 {
        self.samples_played
    }

    /// Evaluates synchronization between current video frame timestamp and master audio clock.
    pub fn check_sync(&self, video_tick: Tick, frame_duration: Tick) -> SyncAction {
        let current_tick = self.current_tick();
        let diff_ticks = video_tick.raw() - current_tick.raw();
        let half_frame = frame_duration.raw() / 2;

        if diff_ticks.abs() <= half_frame {
            SyncAction::Display
        } else if diff_ticks < -half_frame {
            // Video is lagging behind audio
            SyncAction::Drop
        } else {
            // Video is ahead of audio
            SyncAction::Repeat
        }
    }
}
