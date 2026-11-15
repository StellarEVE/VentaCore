//! Tests for Stage 8 preview components: FrameCache, PerformanceGovernor, and MasterClock.

use std::time::Duration;
use ventacore_core::Tick;
use ventacore_render::{
    FrameCache, FrameCacheKey, MasterClock, PerformanceGovernor, PlaybackState, RenderFrame,
    ResolutionTier, SyncAction,
};

#[test]
fn test_frame_cache_insertion_retrieval_and_lru_eviction() {
    // 3 small frames of 10x10 RGBA8 (400 bytes each)
    let frame_bytes = 10 * 10 * 4;
    // Capacity for exactly 2 frames (800 bytes)
    let mut cache = FrameCache::new(frame_bytes * 2);

    let frame1 = RenderFrame::from_solid_color(10, 10, [255, 0, 0, 255]);
    let frame2 = RenderFrame::from_solid_color(10, 10, [0, 255, 0, 255]);
    let frame3 = RenderFrame::from_solid_color(10, 10, [0, 0, 255, 255]);

    let key1 = FrameCacheKey::new("asset_1", Tick::from_seconds(1), ResolutionTier::Full);
    let key2 = FrameCacheKey::new("asset_1", Tick::from_seconds(2), ResolutionTier::Full);
    let key3 = FrameCacheKey::new("asset_1", Tick::from_seconds(3), ResolutionTier::Full);

    cache.insert(key1.clone(), frame1);
    cache.insert(key2.clone(), frame2);
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.current_bytes(), frame_bytes * 2);

    // Access key1 to make it more recently used than key2
    assert!(cache.get(&key1).is_some());

    // Insert frame3 -> key2 (oldest) should be evicted
    cache.insert(key3.clone(), frame3);
    assert_eq!(cache.len(), 2);
    assert!(cache.get(&key1).is_some());
    assert!(cache.get(&key3).is_some());
    assert!(cache.get(&key2).is_none());

    // Clear
    cache.clear();
    assert_eq!(cache.len(), 0);
    assert_eq!(cache.current_bytes(), 0);
}

#[test]
fn test_performance_governor_auto_viewport_selection() {
    // Viewport fills full 1080p canvas
    let tier_full = PerformanceGovernor::select_tier_for_viewport(1920, 1080, 1920, 1080);
    assert_eq!(tier_full, ResolutionTier::Full);

    // Half size preview pane (960x540)
    let tier_half = PerformanceGovernor::select_tier_for_viewport(1920, 1080, 960, 540);
    assert_eq!(tier_half, ResolutionTier::Half);

    // Small quarter size preview pane (480x270)
    let tier_quarter = PerformanceGovernor::select_tier_for_viewport(1920, 1080, 480, 270);
    assert_eq!(tier_quarter, ResolutionTier::Quarter);

    // Tiny preview pane (240x135)
    let tier_eighth = PerformanceGovernor::select_tier_for_viewport(1920, 1080, 240, 135);
    assert_eq!(tier_eighth, ResolutionTier::Eighth);
}

#[test]
fn test_performance_governor_dynamic_stepdown_on_frame_drops() {
    let mut gov = PerformanceGovernor::new(60.0); // 16.6ms budget
    assert_eq!(gov.active_tier(), ResolutionTier::Full);

    // 2 slow frames (40ms) -> should not step down yet
    gov.record_frame_render_time(Duration::from_millis(40));
    gov.record_frame_render_time(Duration::from_millis(40));
    assert_eq!(gov.active_tier(), ResolutionTier::Full);

    // 3rd consecutive slow frame -> steps down to Half
    gov.record_frame_render_time(Duration::from_millis(40));
    assert_eq!(gov.active_tier(), ResolutionTier::Half);

    // 3 more slow frames -> steps down to Quarter
    gov.record_frame_render_time(Duration::from_millis(40));
    gov.record_frame_render_time(Duration::from_millis(40));
    gov.record_frame_render_time(Duration::from_millis(40));
    assert_eq!(gov.active_tier(), ResolutionTier::Quarter);
}

#[test]
fn test_master_clock_sample_timebase_and_av_sync() {
    let mut clock = MasterClock::new(48000);
    assert_eq!(clock.state(), PlaybackState::Stopped);
    assert_eq!(clock.current_tick(), Tick::ZERO);

    clock.play();
    assert_eq!(clock.state(), PlaybackState::Playing);

    // Advance 48,000 samples at 48 kHz -> exactly 1 second in ticks
    clock.advance_audio_samples(48000);
    assert_eq!(clock.current_tick(), Tick::from_seconds(1));

    // Seek to 5 seconds
    clock.seek(Tick::from_seconds(5));
    assert_eq!(clock.current_tick(), Tick::from_seconds(5));

    // Test A/V sync actions:
    // Frame duration at 30fps is ~8,467,200,000 ticks
    let frame_dur = Tick::from_frames(1, 30, 1);

    // In sync (at exactly 5s):
    assert_eq!(clock.check_sync(Tick::from_seconds(5), frame_dur), SyncAction::Display);

    // Video lagging behind audio (e.g. video is at 4s while audio is at 5s):
    assert_eq!(clock.check_sync(Tick::from_seconds(4), frame_dur), SyncAction::Drop);

    // Video ahead of audio (e.g. video is at 6s while audio is at 5s):
    assert_eq!(clock.check_sync(Tick::from_seconds(6), frame_dur), SyncAction::Repeat);
}
