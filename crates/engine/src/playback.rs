//! Playback Engine for VentaCore.
//!
//! Conforms to Part 4b & Stage R3:
//! - Powers every monitor in the application (Source Monitor, Program Monitor, Export Preview).
//! - Supports single Clip auditioning or full multitrack composed Timeline playback.
//! - Driven by `PlatformAudioClock` (audio device DAC callbacks or steady high-res timer fallback).
//! - Frame-accurate seeking, stepping (+/- 1 frame), play/pause, loop toggle.
//! - Resolution tier scaling (Full, Half, Quarter).
//! - Returns real RGBA frame bytes ready for GPU texture upload.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use ventacore_core::model::{AssetId, Project, TrackKind};
use ventacore_core::time::Tick;
use ventacore_platform::{
    rotate_rgba8, ColorDepth, PlatformAudioClock, PlatformVideoDecoder, Rotation,
    SyntheticVideoDecoder, VideoDecoder, VideoMetadata,
};
use ventacore_render::types::ResolutionTier;

/// A decoded video frame ready for texture upload in the UI.
#[derive(Debug, Clone)]
pub struct PlayerFrame {
    pub tick: Tick,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The media source loaded into the player.
enum PlayerTarget {
    Empty {
        width: u32,
        height: u32,
    },
    Clip {
        path: Option<PathBuf>,
        decoder: Box<dyn VideoDecoder>,
        metadata: VideoMetadata,
    },
    Timeline {
        project: Project,
        decoders: HashMap<AssetId, Box<dyn VideoDecoder>>,
        canvas_width: u32,
        canvas_height: u32,
        fps_num: u32,
        fps_den: u32,
    },
}

/// Shared video playback engine for monitors.
pub struct PlaybackEngine {
    target: PlayerTarget,
    clock: PlatformAudioClock,
    looping: bool,
    resolution_tier: ResolutionTier,
    current_frame: Option<PlayerFrame>,
    last_rendered_tick: Option<Tick>,
}

impl Default for PlaybackEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackEngine {
    /// Creates an empty playback engine.
    pub fn new() -> Self {
        let mut engine = Self {
            target: PlayerTarget::Empty {
                width: 1920,
                height: 1080,
            },
            clock: PlatformAudioClock::new(48000),
            looping: false,
            resolution_tier: ResolutionTier::Full,
            current_frame: None,
            last_rendered_tick: None,
        };
        engine.render_current_frame();
        engine
    }

    /// Opens a video file for source clip auditioning.
    pub fn open_clip(&mut self, path: impl AsRef<Path>) -> Result<(), String> {
        let path = path.as_ref();
        let decoder = PlatformVideoDecoder::open(path)
            .map_err(|e| format!("Failed to open video decoder for {}: {}", path.display(), e))?;
        let metadata = decoder.metadata().clone();

        self.target = PlayerTarget::Clip {
            path: Some(path.to_path_buf()),
            decoder: Box::new(decoder),
            metadata,
        };
        self.clock.stop();
        self.clock.seek(Tick::ZERO);
        self.last_rendered_tick = None;
        self.render_current_frame();
        Ok(())
    }

    /// Opens a synthetic test clip for automated testing.
    pub fn open_synthetic_clip(
        &mut self,
        duration: Tick,
        width: u32,
        height: u32,
        fps_num: u32,
        fps_den: u32,
    ) -> Result<(), String> {
        let decoder = SyntheticVideoDecoder::new(
            duration,
            width,
            height,
            fps_num,
            fps_den,
            Rotation::None,
            ColorDepth::Bits8,
        );
        let metadata = decoder.metadata().clone();

        self.target = PlayerTarget::Clip {
            path: None,
            decoder: Box::new(decoder),
            metadata,
        };
        self.clock.stop();
        self.clock.seek(Tick::ZERO);
        self.last_rendered_tick = None;
        self.render_current_frame();
        Ok(())
    }

    /// Opens an arbitrary custom `VideoDecoder` directly into the player.
    pub fn open_custom_decoder(&mut self, decoder: Box<dyn VideoDecoder>) -> Result<(), String> {
        let metadata = decoder.metadata().clone();
        self.target = PlayerTarget::Clip {
            path: None,
            decoder,
            metadata,
        };
        self.clock.stop();
        self.clock.seek(Tick::ZERO);
        self.last_rendered_tick = None;
        self.render_current_frame();
        Ok(())
    }

    /// Opens a composed project timeline for program monitor playback.
    pub fn open_timeline(&mut self, project: Project) -> Result<(), String> {
        let mut decoders: HashMap<AssetId, Box<dyn VideoDecoder>> = HashMap::new();

        for asset in &project.assets {
            if asset.is_footage() {
                // Initialize decoder for asset
                if let Ok(dec) = PlatformVideoDecoder::open(&asset.path_hint) {
                    decoders.insert(asset.id, Box::new(dec));
                } else {
                    let probe = asset.probe_data.as_ref();
                    let dur = probe.map(|p| p.duration).unwrap_or(Tick::from_seconds(10));
                    let w = probe.map(|p| p.width).unwrap_or(1920);
                    let h = probe.map(|p| p.height).unwrap_or(1080);
                    let fps_n = probe.map(|p| p.fps_num).unwrap_or(30);
                    let fps_d = probe.map(|p| p.fps_den).unwrap_or(1);

                    let syn = SyntheticVideoDecoder::new(
                        dur,
                        w,
                        h,
                        fps_n,
                        fps_d,
                        Rotation::None,
                        ColorDepth::Bits8,
                    );
                    decoders.insert(asset.id, Box::new(syn));
                }
            }
        }

        self.target = PlayerTarget::Timeline {
            project,
            decoders,
            canvas_width: 1920,
            canvas_height: 1080,
            fps_num: 30,
            fps_den: 1,
        };
        self.clock.stop();
        self.clock.seek(Tick::ZERO);
        self.last_rendered_tick = None;
        self.render_current_frame();
        Ok(())
    }

    /// Updates the active project on a timeline target without resetting playback position.
    pub fn update_timeline_project(&mut self, project: Project) {
        if let PlayerTarget::Timeline {
            project: ref mut existing_proj,
            ref mut decoders,
            ..
        } = self.target
        {
            *existing_proj = project;
            // Ensure any new footage assets have decoders
            for asset in &existing_proj.assets {
                if asset.is_footage() && !decoders.contains_key(&asset.id) {
                    let probe = asset.probe_data.as_ref();
                    let dur = probe.map(|p| p.duration).unwrap_or(Tick::from_seconds(10));
                    let w = probe.map(|p| p.width).unwrap_or(1920);
                    let h = probe.map(|p| p.height).unwrap_or(1080);
                    let fps_n = probe.map(|p| p.fps_num).unwrap_or(30);
                    let fps_d = probe.map(|p| p.fps_den).unwrap_or(1);

                    let syn = SyntheticVideoDecoder::new(
                        dur,
                        w,
                        h,
                        fps_n,
                        fps_d,
                        Rotation::None,
                        ColorDepth::Bits8,
                    );
                    decoders.insert(asset.id, Box::new(syn));
                }
            }
            self.last_rendered_tick = None;
            self.render_current_frame();
        }
    }

    /// Starts playback.
    pub fn play(&mut self) {
        if self.current_tick() >= self.duration() {
            self.seek(Tick::ZERO);
        }
        self.clock.start();
    }

    /// Pauses playback.
    pub fn pause(&mut self) {
        self.clock.pause();
    }

    /// Toggles play / pause.
    pub fn toggle_play(&mut self) {
        if self.is_playing() {
            self.pause();
        } else {
            self.play();
        }
    }

    /// Returns true if currently playing.
    pub fn is_playing(&self) -> bool {
        self.clock.is_running()
    }

    /// Seeks playback to an exact Tick.
    pub fn seek(&mut self, tick: Tick) {
        let dur = self.duration();
        let clamped = if dur > Tick::ZERO {
            tick.min(dur)
        } else {
            Tick::ZERO
        };
        self.clock.seek(clamped);
        self.last_rendered_tick = None;
        self.render_current_frame();
    }

    /// Steps forward by 1 frame duration.
    pub fn step_forward(&mut self) {
        self.pause();
        let (fps_n, fps_d) = self.fps();
        let frame_step = Tick::from_frames(1, fps_n, fps_d);
        self.seek(self.current_tick() + frame_step);
    }

    /// Steps backward by 1 frame duration.
    pub fn step_backward(&mut self) {
        self.pause();
        let (fps_n, fps_d) = self.fps();
        let frame_step = Tick::from_frames(1, fps_n, fps_d);
        self.seek(self.current_tick().saturating_sub(frame_step));
    }

    /// Toggles looping.
    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    pub fn looping(&self) -> bool {
        self.looping
    }

    /// Returns the source path of the currently loaded clip, if any.
    pub fn clip_path(&self) -> Option<&Path> {
        match &self.target {
            PlayerTarget::Clip { path, .. } => path.as_deref(),
            _ => None,
        }
    }

    /// Sets the preview resolution tier.
    pub fn set_resolution_tier(&mut self, tier: ResolutionTier) {
        if self.resolution_tier != tier {
            self.resolution_tier = tier;
            self.last_rendered_tick = None;
            self.render_current_frame();
        }
    }

    pub fn resolution_tier(&self) -> ResolutionTier {
        self.resolution_tier
    }

    /// Total media or timeline duration.
    pub fn duration(&self) -> Tick {
        match &self.target {
            PlayerTarget::Empty { .. } => Tick::ZERO,
            PlayerTarget::Clip { metadata, .. } => metadata.duration,
            PlayerTarget::Timeline { project, .. } => project.timeline_duration(),
        }
    }

    /// Current playback position.
    pub fn current_tick(&self) -> Tick {
        self.clock.now_tick()
    }

    /// FPS (numerator, denominator) of the current target.
    pub fn fps(&self) -> (u32, u32) {
        match &self.target {
            PlayerTarget::Empty { .. } => (30, 1),
            PlayerTarget::Clip { metadata, .. } => (metadata.fps_num.max(1), metadata.fps_den.max(1)),
            PlayerTarget::Timeline {
                fps_num, fps_den, ..
            } => (*fps_num, *fps_den),
        }
    }

    /// Native pixel dimensions of the current media or canvas.
    pub fn native_dimensions(&self) -> (u32, u32) {
        match &self.target {
            PlayerTarget::Empty { width, height } => (*width, *height),
            PlayerTarget::Clip { metadata, .. } => (metadata.width, metadata.height),
            PlayerTarget::Timeline {
                canvas_width,
                canvas_height,
                ..
            } => (*canvas_width, *canvas_height),
        }
    }

    /// Returns the currently decoded/rendered frame ready for display.
    pub fn current_frame(&self) -> Option<&PlayerFrame> {
        self.current_frame.as_ref()
    }

    /// Updates playback state on every UI loop / animation tick.
    ///
    /// Handles clock advancement, loop wrap-around, and frame decoding.
    /// Returns `true` if a new frame was rendered or repaint is needed.
    pub fn update(&mut self) -> bool {
        let dur = self.duration();
        let now = self.current_tick();

        if self.is_playing() {
            if dur > Tick::ZERO && now >= dur {
                if self.looping {
                    self.seek(Tick::ZERO);
                } else {
                    self.pause();
                    self.seek(dur);
                }
            }
        }

        let current = self.current_tick();
        if self.last_rendered_tick != Some(current) {
            self.render_current_frame();
            true
        } else {
            false
        }
    }

    /// Internal: Decodes and scales frame at current playback position.
    fn render_current_frame(&mut self) {
        let now = self.current_tick();
        self.last_rendered_tick = Some(now);

        match &mut self.target {
            PlayerTarget::Empty { width, height } => {
                let (tw, th) = self.resolution_tier.scale_dimensions(*width, *height);
                let black_pixels = vec![0u8; (tw * th * 4) as usize];
                self.current_frame = Some(PlayerFrame {
                    tick: now,
                    width: tw,
                    height: th,
                    rgba: black_pixels,
                });
            }
            PlayerTarget::Clip {
                decoder, metadata, ..
            } => {
                let _ = decoder.seek(now);
                if let Ok(Some(frame)) = decoder.decode_next_frame() {
                    let (data_rgba, w, h) = if frame.rotation != Rotation::None {
                        let (rot, rw, rh) =
                            rotate_rgba8(&frame.data, frame.width, frame.height, frame.rotation);
                        (rot, rw, rh)
                    } else {
                        (frame.data, frame.width, frame.height)
                    };

                    let (tw, th) = self.resolution_tier.scale_dimensions(w, h);
                    let final_rgba = downscale_rgba(&data_rgba, w, h, tw, th);

                    self.current_frame = Some(PlayerFrame {
                        tick: now,
                        width: tw,
                        height: th,
                        rgba: final_rgba,
                    });
                } else {
                    let (tw, th) = self
                        .resolution_tier
                        .scale_dimensions(metadata.width, metadata.height);
                    let black_pixels = vec![0u8; (tw * th * 4) as usize];
                    self.current_frame = Some(PlayerFrame {
                        tick: now,
                        width: tw,
                        height: th,
                        rgba: black_pixels,
                    });
                }
            }
            PlayerTarget::Timeline {
                project,
                decoders,
                canvas_width,
                canvas_height,
                ..
            } => {
                // Find visible video item at `now` across video tracks
                let mut found_clip = None;
                for track in project.tracks.iter().rev() {
                    if track.kind == TrackKind::Video && !track.locked {
                        for item in project.track_items(track.id) {
                            if now >= item.timeline_start && now < item.timeline_end() {
                                found_clip = Some(item);
                                break;
                            }
                        }
                    }
                    if found_clip.is_some() {
                        break;
                    }
                }

                if let Some(item) = found_clip {
                    let source_tick = item.source_in + (now - item.timeline_start);
                    if let Some(dec) = decoders.get_mut(&item.asset_id) {
                        let _ = dec.seek(source_tick);
                        if let Ok(Some(frame)) = dec.decode_next_frame() {
                            let (data_rgba, w, h) = if frame.rotation != Rotation::None {
                                let (rot, rw, rh) = rotate_rgba8(
                                    &frame.data,
                                    frame.width,
                                    frame.height,
                                    frame.rotation,
                                );
                                (rot, rw, rh)
                            } else {
                                (frame.data, frame.width, frame.height)
                            };

                            let (tw, th) = self.resolution_tier.scale_dimensions(w, h);
                            let final_rgba = downscale_rgba(&data_rgba, w, h, tw, th);

                            self.current_frame = Some(PlayerFrame {
                                tick: now,
                                width: tw,
                                height: th,
                                rgba: final_rgba,
                            });
                            return;
                        }
                    }
                }

                // Gap or no clip at `now`: render solid black frame
                let (tw, th) = self
                    .resolution_tier
                    .scale_dimensions(*canvas_width, *canvas_height);
                let black_pixels = vec![0u8; (tw * th * 4) as usize];
                self.current_frame = Some(PlayerFrame {
                    tick: now,
                    width: tw,
                    height: th,
                    rgba: black_pixels,
                });
            }
        }
    }
}

/// Fast downscale RGBA image buffer to target dimensions.
pub fn downscale_rgba(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    if src_w == dst_w && src_h == dst_h {
        return src.to_vec();
    }
    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
    for y in 0..dst_h {
        let src_y = (y * src_h / dst_h).min(src_h - 1);
        for x in 0..dst_w {
            let src_x = (x * src_w / dst_w).min(src_w - 1);
            let src_idx = ((src_y * src_w + src_x) * 4) as usize;
            let dst_idx = ((y * dst_w + x) * 4) as usize;
            dst[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
        }
    }
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_synthetic_clip_playback_and_stepping() {
        let mut player = PlaybackEngine::new();
        let dur = Tick::from_seconds(5);
        player
            .open_synthetic_clip(dur, 640, 360, 30, 1)
            .expect("open synthetic clip");

        assert_eq!(player.duration(), dur);
        assert_eq!(player.current_tick(), Tick::ZERO);
        assert!(!player.is_playing());

        let frame0 = player.current_frame().expect("initial frame must exist");
        assert_eq!(frame0.width, 640);
        assert_eq!(frame0.height, 360);
        assert_eq!(frame0.rgba.len(), 640 * 360 * 4);

        // Step forward 1 frame
        player.step_forward();
        let frame_step = Tick::from_frames(1, 30, 1);
        assert_eq!(player.current_tick(), frame_step);

        // Step backward 1 frame
        player.step_backward();
        assert_eq!(player.current_tick(), Tick::ZERO);

        // Resolution tier half
        player.set_resolution_tier(ResolutionTier::Half);
        let frame_half = player.current_frame().expect("half frame");
        assert_eq!(frame_half.width, 320);
        assert_eq!(frame_half.height, 180);

        // Seek and verify frame update
        player.seek(Tick::from_seconds(2));
        assert_eq!(player.current_tick(), Tick::from_seconds(2));
    }
}
