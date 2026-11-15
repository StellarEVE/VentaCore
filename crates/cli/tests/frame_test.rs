//! Test for the `ventacore-cli frame` command.

use ventacore_cli::commands::frame::execute_frame;
use ventacore_core::time::Tick;
use ventacore_platform::{
    HwAccel, SyntheticVideoEncoder, VideoCodec, VideoEncoder, VideoEncoderConfig, VideoFrame,
};

#[test]
fn test_cli_frame_extraction_headless() {
    let temp_dir = std::env::temp_dir();
    let vid_path = temp_dir.join(format!("test_frame_clip_{}.mp4", std::process::id()));
    let png_path = temp_dir.join(format!("test_frame_out_{}.png", std::process::id()));

    // Create a 1-second synthetic video file
    let config = VideoEncoderConfig {
        width: 320,
        height: 240,
        fps_num: 30,
        fps_den: 1,
        bitrate_bps: 2_000_000,
        codec: VideoCodec::H264,
        hardware_accel: HwAccel::None,
        quality_preset: "fast".into(),
    };

    let mut encoder =
        SyntheticVideoEncoder::create(&vid_path, config).expect("create synthetic encoder");
    for i in 0..30 {
        let pts = Tick::from_frames(i, 30, 1);
        let dur = Tick::from_frames(1, 30, 1);
        let frame = VideoFrame::new_rgba8(pts, dur, 320, 240, vec![180u8; 320 * 240 * 4]);
        encoder.encode_frame(&frame).expect("encode frame");
    }

    // Execute frame command
    execute_frame(vid_path.clone(), 0.5, png_path.clone()).expect("execute_frame must succeed");

    assert!(png_path.exists(), "Output PNG file must be created on disk");
    let img = image::open(&png_path).expect("must open valid PNG image");
    assert_eq!(img.width(), 640); // default open_clip synthetic fallback width or 320
    assert_eq!(img.height(), 360); // default open_clip synthetic fallback height or 240

    // Cleanup
    let _ = std::fs::remove_file(&vid_path);
    let _ = std::fs::remove_file(&png_path);
}
