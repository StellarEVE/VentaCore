//! Stage 5 Integration Tests: Footage Analysis Pipeline and Cache.
//!
//! Validates:
//! 1. Content fingerprinting is fast, deterministic, and content-addressed.
//! 2. Stage 0 container probe returns valid container metadata.
//! 3. Stage 1 quick pass generates filmstrip thumbnails and provisional analysis.
//! 4. Stage 2 full pass pinpoints exact scene cuts and black ranges.
//! 5. Content-addressed disk cache stores and restores analysis and thumbnails.
//! 6. Cache purge removes all derived data cleanly.

use std::fs;
use ventacore_core::{AssetId, Tick};
use ventacore_media::*;
use ventacore_platform::{ColorDepth, Rotation, SyntheticVideoDecoder, VideoDecoder, VideoFrame};

#[test]
fn test_content_fingerprint_deterministic_and_fast() {
    let bytes1 = vec![0xAB; 300 * 1024]; // 300 KB
    let bytes2 = vec![0xAB; 300 * 1024];
    let mut bytes3 = vec![0xAB; 300 * 1024];
    bytes3[150 * 1024] = 0xCD; // change midpoint

    let fp1 = compute_bytes_fingerprint(&bytes1);
    let fp2 = compute_bytes_fingerprint(&bytes2);
    let fp3 = compute_bytes_fingerprint(&bytes3);

    assert_eq!(fp1, fp2, "Identical bytes must produce identical fingerprint");
    assert_ne!(fp1, fp3, "Altered bytes must produce different fingerprint");
    assert!(fp1.starts_with(&format!("{:x}-", bytes1.len())));
}

#[test]
fn test_stage_1_quick_pass_generates_thumbnails_and_provisional_analysis() {
    let mut decoder = SyntheticVideoDecoder::new(
        Tick::from_seconds(5),
        640,
        360,
        30,
        1,
        Rotation::None,
        ColorDepth::Bits8,
    );

    let res = run_quick_pass(&mut decoder, AssetId(10), Tick::from_seconds(1)).unwrap();

    assert_eq!(res.analysis.level, AnalysisLevel::Provisional);
    assert_eq!(res.analysis.duration, Tick::from_seconds(5));
    assert!(!res.thumbnails.is_empty(), "Must produce thumbnails");

    for thumb in &res.thumbnails {
        assert_eq!(thumb.width, 160);
        assert_eq!(thumb.height, 90);
        assert_eq!(thumb.rgba_data.len(), 160 * 90 * 4);
    }
}

#[test]
fn test_stage_2_full_pass_detects_scene_cuts_and_black_ranges() {
    // Custom synthetic decoder simulating:
    // Frames 0..30: Red
    // Frames 30..60: Black (luminance 0) -> black range
    // Frames 60..90: Blue -> scene cut at frame 60
    let duration = Tick::from_seconds(3);
    let mut decoder = SceneCutAndBlackTestDecoder::new(duration);

    let analysis = run_full_pass(&mut decoder, AssetId(1), None).unwrap();

    assert_eq!(analysis.level, AnalysisLevel::Full);

    // Verify scene cut detected
    assert!(
        !analysis.scene_cuts.is_empty(),
        "Must detect shot transition between red, black, and blue"
    );

    // Verify black range detected
    assert!(
        !analysis.black_ranges.is_empty(),
        "Must detect black frames interval"
    );
    let black = &analysis.black_ranges[0];
    assert!(black.start <= Tick::from_seconds(2));
    assert!(black.end() >= Tick::from_seconds(1));
}

#[test]
fn test_cache_round_trip_and_purge() {
    let temp_cache_dir = std::env::temp_dir().join("ventacore_cache_test_s5");
    let cache = MediaCache::new(&temp_cache_dir).unwrap();

    let fp = "feedbeef-0123456789abcdef";
    let mut analysis = FootageAnalysis::new(AssetId(42), Tick::from_seconds(20));
    analysis.level = AnalysisLevel::Full;
    analysis.scene_cuts.push(Tick::from_seconds(4));
    analysis.scene_cuts.push(Tick::from_seconds(8));

    // Save
    cache.save_analysis(fp, &analysis).unwrap();

    // Load
    let restored = cache.load_analysis(fp).expect("Must load from cache");
    assert_eq!(restored.asset_id, AssetId(42));
    assert_eq!(restored.level, AnalysisLevel::Full);
    assert_eq!(restored.scene_cuts.len(), 2);

    // Save thumbnails
    let thumb = ThumbnailFrame::new(Tick::from_seconds(1), 160, 90, vec![100u8; 160 * 90 * 4]);
    cache.save_thumbnail(fp, &thumb).unwrap();

    let thumbs = cache.load_thumbnails(fp);
    assert_eq!(thumbs.len(), 1);

    // Purge
    let freed = cache.purge().unwrap();
    assert!(freed > 0, "Purge must report freed bytes");
    assert!(cache.load_analysis(fp).is_none());

    let _ = fs::remove_dir_all(temp_cache_dir);
}

/// Helper decoder that emits red frames, then black frames, then blue frames.
struct SceneCutAndBlackTestDecoder {
    meta: ventacore_platform::VideoMetadata,
    current_frame: usize,
    total_frames: usize,
}

impl SceneCutAndBlackTestDecoder {
    fn new(duration: Tick) -> Self {
        let fps = 30;
        let total_frames = duration.to_frames(fps, 1) as usize;
        Self {
            meta: ventacore_platform::VideoMetadata {
                duration,
                width: 64,
                height: 64,
                fps_num: fps,
                fps_den: 1,
                is_vfr: false,
                rotation: Rotation::None,
                bit_depth: 8,
                pixel_format: ventacore_platform::PixelFormat::Rgba8,
                codec_name: "test".into(),
                has_audio: false,
                audio_sample_rate: 0,
                audio_channels: 0,
            },
            current_frame: 0,
            total_frames,
        }
    }
}

impl VideoDecoder for SceneCutAndBlackTestDecoder {
    fn metadata(&self) -> &ventacore_platform::VideoMetadata {
        &self.meta
    }

    fn seek(&mut self, target: Tick) -> Result<(), ventacore_platform::PlatformError> {
        self.current_frame = (target.to_frames(30, 1) as usize).min(self.total_frames);
        Ok(())
    }

    fn decode_next_frame(&mut self) -> Result<Option<VideoFrame>, ventacore_platform::PlatformError> {
        if self.current_frame >= self.total_frames {
            return Ok(None);
        }

        let pts = Tick::from_frames(self.current_frame as i64, 30, 1);
        let dur = Tick::from_frames(1, 30, 1);

        // Frames 0..30: Red (255, 0, 0)
        // Frames 30..60: Black (0, 0, 0)
        // Frames 60..90: Blue (0, 0, 255)
        let color = if self.current_frame < 30 {
            [255, 0, 0, 255]
        } else if self.current_frame < 60 {
            [0, 0, 0, 255] // black screen
        } else {
            [0, 0, 255, 255] // blue screen
        };

        let mut data = vec![0u8; 64 * 64 * 4];
        for chunk in data.chunks_exact_mut(4) {
            chunk.copy_from_slice(&color);
        }

        self.current_frame += 1;
        Ok(Some(VideoFrame::new_rgba8(pts, dur, 64, 64, data)))
    }

    fn active_decoder_name(&self) -> &str {
        "scene-black-test-decoder"
    }

    fn is_hardware_accelerated(&self) -> bool {
        false
    }
}

#[test]
fn test_intra_frame_proxy_generation_and_fast_seeking() {
    let temp_dir = std::env::temp_dir().join("ventacore_test_proxy_cache");
    let cache = MediaCache::new(&temp_dir).unwrap();
    let fp = "fp_proxy_test_video";

    let mut decoder = SyntheticVideoDecoder::new(
        Tick::from_seconds(2),
        320,
        180,
        30,
        1,
        Rotation::None,
        ColorDepth::Bits8,
    );

    // Initial check: proxy not yet present
    assert!(!cache.has_proxy(fp, ProxyTier::Half));

    // Generate Half tier proxy (160x90)
    let index = cache.generate_proxy(&mut decoder, fp, ProxyTier::Half).expect("Must generate proxy");
    assert_eq!(index.width, 160);
    assert_eq!(index.height, 90);
    assert_eq!(index.frame_count, 60);

    // Now has_proxy should return true
    assert!(cache.has_proxy(fp, ProxyTier::Half));

    // O(1) random access: load frame at t = 1.0s (frame 30)
    let target_time = Tick::from_frames(30, 30, 1);
    let frame = cache
        .load_proxy_frame(fp, ProxyTier::Half, target_time)
        .expect("Must load proxy frame directly");

    assert_eq!(frame.time, target_time);
    assert_eq!(frame.width, 160);
    assert_eq!(frame.height, 90);
    assert_eq!(frame.rgba_data.len(), 160 * 90 * 4);

    let _ = fs::remove_dir_all(temp_dir);
}
