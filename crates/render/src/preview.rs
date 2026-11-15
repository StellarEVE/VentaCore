//! In-memory frame cache and dynamic auto-resolution governor for real-time preview.

use crate::types::{RenderFrame, ResolutionTier};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use ventacore_core::Tick;

/// Cache key uniquely identifying a frame at a specific timestamp and resolution tier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FrameCacheKey {
    pub source_id: String,
    pub time: Tick,
    pub tier: ResolutionTier,
}

impl FrameCacheKey {
    pub fn new(source_id: impl Into<String>, time: Tick, tier: ResolutionTier) -> Self {
        Self {
            source_id: source_id.into(),
            time,
            tier,
        }
    }
}

/// LRU-like in-memory frame cache bounded by byte capacity.
pub struct FrameCache {
    entries: HashMap<FrameCacheKey, (RenderFrame, Instant)>,
    capacity_bytes: usize,
    current_bytes: usize,
}

impl FrameCache {
    pub const DEFAULT_CAPACITY: usize = 128 * 1024 * 1024; // 128 MB

    pub fn new(capacity_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            capacity_bytes,
            current_bytes: 0,
        }
    }

    pub fn with_default_capacity() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }

    pub fn get(&mut self, key: &FrameCacheKey) -> Option<RenderFrame> {
        if let Some((frame, last_used)) = self.entries.get_mut(key) {
            *last_used = Instant::now();
            Some(frame.clone())
        } else {
            None
        }
    }

    pub fn insert(&mut self, key: FrameCacheKey, frame: RenderFrame) {
        let frame_bytes = frame.pixels.len();
        if frame_bytes > self.capacity_bytes {
            return;
        }

        // Evict oldest entries until within capacity
        while self.current_bytes + frame_bytes > self.capacity_bytes && !self.entries.is_empty() {
            if let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, time))| *time)
                .map(|(k, _)| k.clone())
            {
                if let Some((old_frame, _)) = self.entries.remove(&oldest_key) {
                    self.current_bytes = self.current_bytes.saturating_sub(old_frame.pixels.len());
                }
            } else {
                break;
            }
        }

        self.current_bytes += frame_bytes;
        self.entries.insert(key, (frame, Instant::now()));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_bytes = 0;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    pub fn capacity_bytes(&self) -> usize {
        self.capacity_bytes
    }
}

/// Dynamic resolution tier manager that adjusts preview resolution to keep playback real-time.
#[derive(Debug, Clone)]
pub struct PerformanceGovernor {
    target_tier: ResolutionTier,
    active_tier: ResolutionTier,
    frame_budget: Duration,
    consecutive_drops: u32,
    consecutive_smooth: u32,
}

impl PerformanceGovernor {
    /// Creates a new governor for a given target playback frame rate (e.g. 60.0 or 30.0 fps).
    pub fn new(fps: f64) -> Self {
        let frame_ms = if fps > 0.0 { 1000.0 / fps } else { 33.33 };
        Self {
            target_tier: ResolutionTier::Full,
            active_tier: ResolutionTier::Full,
            frame_budget: Duration::from_micros((frame_ms * 1000.0) as u64),
            consecutive_drops: 0,
            consecutive_smooth: 0,
        }
    }

    /// Selects an initial tier automatically based on viewport dimensions.
    pub fn select_tier_for_viewport(
        canvas_width: u32,
        canvas_height: u32,
        viewport_width: u32,
        viewport_height: u32,
    ) -> ResolutionTier {
        if canvas_width == 0 || canvas_height == 0 || viewport_width == 0 || viewport_height == 0 {
            return ResolutionTier::Full;
        }

        let scale = (viewport_width as f32 / canvas_width as f32)
            .max(viewport_height as f32 / canvas_height as f32);

        if scale >= 0.75 {
            ResolutionTier::Full
        } else if scale >= 0.35 {
            ResolutionTier::Half
        } else if scale >= 0.18 {
            ResolutionTier::Quarter
        } else {
            ResolutionTier::Eighth
        }
    }

    pub fn active_tier(&self) -> ResolutionTier {
        self.active_tier
    }

    pub fn set_target_tier(&mut self, tier: ResolutionTier) {
        self.target_tier = tier;
        self.active_tier = tier;
        self.consecutive_drops = 0;
        self.consecutive_smooth = 0;
    }

    /// Records the time spent rendering a frame and adapts the resolution tier if necessary.
    pub fn record_frame_render_time(&mut self, render_duration: Duration) {
        if render_duration > self.frame_budget {
            self.consecutive_drops += 1;
            self.consecutive_smooth = 0;

            // If 3 consecutive frames drop, step down tier
            if self.consecutive_drops >= 3 {
                self.step_down();
                self.consecutive_drops = 0;
            }
        } else {
            self.consecutive_smooth += 1;
            self.consecutive_drops = 0;

            // If 60 consecutive frames run comfortably under 70% budget, try stepping up towards target
            if self.consecutive_smooth >= 60 && render_duration < self.frame_budget.mul_f32(0.7) {
                self.step_up();
                self.consecutive_smooth = 0;
            }
        }
    }

    fn step_down(&mut self) {
        self.active_tier = match self.active_tier {
            ResolutionTier::Full => ResolutionTier::Half,
            ResolutionTier::Half => ResolutionTier::Quarter,
            ResolutionTier::Quarter => ResolutionTier::Eighth,
            ResolutionTier::Eighth => ResolutionTier::Eighth,
        };
    }

    fn step_up(&mut self) {
        if self.active_tier != self.target_tier {
            self.active_tier = match self.active_tier {
                ResolutionTier::Eighth => ResolutionTier::Quarter,
                ResolutionTier::Quarter => ResolutionTier::Half,
                ResolutionTier::Half => ResolutionTier::Full,
                ResolutionTier::Full => ResolutionTier::Full,
            };
        }
    }
}
