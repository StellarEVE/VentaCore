//! Tests for Stage 9 content-addressed render cache.

use ventacore_core::Tick;
use ventacore_render::{
    FitMode, RenderCache, RenderFrame, ResolutionTier, Transform2D,
};

#[test]
fn test_render_cache_put_get_and_purge() {
    let temp_dir = std::env::temp_dir().join("ventacore_test_render_cache");
    let cache = RenderCache::new(&temp_dir).unwrap();

    let fp = "asset_fingerprint_abc";
    let in_tick = Tick::from_seconds(10);
    let transform = Transform2D::default();

    let key = RenderCache::compute_cut_key(
        fp,
        in_tick,
        FitMode::Crop,
        &transform,
        ResolutionTier::Full,
        1920,
        1080,
    );

    // Initial lookup should miss
    assert!(cache.get(&key).is_none());

    // Put rendered frame
    let frame = RenderFrame::from_solid_color(1920, 1080, [100, 150, 200, 255]);
    cache.put(&key, &frame).unwrap();

    // Lookup should now hit
    let cached = cache.get(&key).expect("Cached frame must be returned");
    assert_eq!(cached.width, 1920);
    assert_eq!(cached.height, 1080);
    assert_eq!(cached.get_pixel(100, 100), [100, 150, 200, 255]);

    // Purge
    let deleted = cache.purge().unwrap();
    assert_eq!(deleted, 1);
    assert!(cache.get(&key).is_none());

    let _ = std::fs::remove_dir_all(temp_dir);
}
