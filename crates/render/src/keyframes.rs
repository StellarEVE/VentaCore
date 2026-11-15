//! Parameter Keyframing and Curve Interpolation for VentaCore
//!
//! Conforms to Stage 16c requirements:
//! - Exact Tick-accurate keyframe timeline.
//! - Linear, EaseIn, EaseOut, and Bezier interpolation curves.
//! - Used for animating transform, opacity, volume, and effect parameters.

use serde::{Deserialize, Serialize};
use ventacore_core::time::Tick;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum KeyframeInterpolation {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyframe<T> {
    pub time: Tick,
    pub value: T,
    pub interpolation: KeyframeInterpolation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct KeyframeTrack<T> {
    pub keyframes: Vec<Keyframe<T>>,
}

impl<T: Clone> KeyframeTrack<T> {
    pub fn new() -> Self {
        Self {
            keyframes: Vec::new(),
        }
    }

    pub fn insert(&mut self, time: Tick, value: T, interp: KeyframeInterpolation) {
        if let Some(pos) = self.keyframes.iter().position(|k| k.time == time) {
            self.keyframes[pos].value = value;
            self.keyframes[pos].interpolation = interp;
        } else {
            self.keyframes.push(Keyframe {
                time,
                value,
                interpolation: interp,
            });
            self.keyframes.sort_by_key(|k| k.time);
        }
    }
}

impl KeyframeTrack<f32> {
    /// Evaluates the scalar float track at a given time tick.
    pub fn evaluate(&self, time: Tick, default_val: f32) -> f32 {
        if self.keyframes.is_empty() {
            return default_val;
        }
        if time <= self.keyframes[0].time {
            return self.keyframes[0].value;
        }
        if time >= self.keyframes.last().unwrap().time {
            return self.keyframes.last().unwrap().value;
        }

        // Find surrounding keyframe window
        for window in self.keyframes.windows(2) {
            let k0 = &window[0];
            let k1 = &window[1];
            if time >= k0.time && time <= k1.time {
                let range = (k1.time.0 - k0.time.0) as f32;
                if range <= 0.0 {
                    return k0.value;
                }
                let mut t = (time.0 - k0.time.0) as f32 / range;
                t = match k0.interpolation {
                    KeyframeInterpolation::Linear => t,
                    KeyframeInterpolation::EaseIn => t * t,
                    KeyframeInterpolation::EaseOut => t * (2.0 - t),
                    KeyframeInterpolation::EaseInOut => {
                        if t < 0.5 {
                            2.0 * t * t
                        } else {
                            -1.0 + (4.0 - 2.0 * t) * t
                        }
                    }
                };
                return k0.value + t * (k1.value - k0.value);
            }
        }

        default_val
    }
}
