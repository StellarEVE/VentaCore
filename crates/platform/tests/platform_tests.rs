//! Stage 4 Integration Tests: Platform Video Codecs and Hardware Probing.
//!
//! Validates:
//! 1. Hardware capability probing and trial execution (Charter Rule 6).
//! 2. Active decoder and encoder names exposed honestly (Charter Rule 6 & 11).
//! 3. Software fallbacks are always proven working (Charter Rule 5).
//! 4. Decoder trait: seek, frame decoding, metadata inspection, and EOF handling.
//! 5. Encoder trait: frame encoding, stream finalization, file persistence.
//! 6. Variable Frame Rate (VFR) detection and binary search timecode lookup.
//! 7. Container rotation transforms (0, 90, 180, 270 degrees).
//! 8. 10-bit source handling (YUV420P10LE -> RGBA8).
//! 9. Environment configuration probing for Windows build setup.

use std::fs;
use ventacore_core::Tick;
use ventacore_platform::*;

#[test]
fn test_hardware_capabilities_and_trial_verification() {
    let report = probe_hardware_capabilities();

    // 1. Graphics adapters discovered
    assert!(!report.adapters.is_empty(), "Must discover at least 1 graphics adapter");
    for adapter in &report.adapters {
        assert!(!adapter.name.is_empty());
        assert!(!adapter.backend.is_empty());
    }

    // 2. Candidate decoders and encoders listed
    assert!(!report.decoders.is_empty(), "Decoders must not be empty");
    assert!(!report.encoders.is_empty(), "Encoders must not be empty");

    // 3. Charter Rule 5: Software fallback is always proven working
    let proven_h264_enc = report
        .best_proven_encoder(VideoCodec::H264)
        .expect("Must have proven H.264 encoder");
    assert!(proven_h264_enc.is_proven());

    let proven_h264_dec = report
        .best_proven_decoder(VideoCodec::H264)
        .expect("Must have proven H.264 decoder");
    assert!(proven_h264_dec.is_proven());

    // 4. Charter Rule 6: Name of encoder and decoder actually used is always exposed
    let enc_impl = proven_h264_enc.status.implementation_name();
    assert!(enc_impl.is_some(), "Proven encoder must have an implementation name");
    assert!(!enc_impl.unwrap().is_empty());

    let dec_impl = proven_h264_dec.status.implementation_name();
    assert!(dec_impl.is_some(), "Proven decoder must have an implementation name");
    assert!(!dec_impl.unwrap().is_empty());
}

#[test]
fn test_video_decoder_trait_and_seeking() {
    let duration = Tick::from_seconds(10);
    let fps_num = 30;
    let fps_den = 1;
    let mut decoder = SyntheticVideoDecoder::new(
        duration,
        1920,
        1080,
        fps_num,
        fps_den,
        Rotation::None,
        ColorDepth::Bits8,
    );

    // Metadata checks
    assert_eq!(decoder.metadata().width, 1920);
    assert_eq!(decoder.metadata().height, 1080);
    assert_eq!(decoder.metadata().duration, duration);
    assert_eq!(decoder.active_decoder_name(), "h264 (pure software fallback)");
    assert!(!decoder.is_hardware_accelerated());

    // Sequential frame decoding
    let frame0 = decoder.decode_next_frame().unwrap().expect("Frame 0 must exist");
    assert_eq!(frame0.pts, Tick::ZERO);
    assert_eq!(frame0.width, 1920);
    assert_eq!(frame0.height, 1080);
    assert_eq!(frame0.format, PixelFormat::Rgba8);

    let frame1 = decoder.decode_next_frame().unwrap().expect("Frame 1 must exist");
    let frame_dur = Tick::from_frames(1, fps_num, fps_den);
    assert_eq!(frame1.pts, frame_dur);

    // Seek to 5 seconds
    let target = Tick::from_seconds(5);
    decoder.seek(target).expect("Seek must succeed");
    let frame_at_5s = decoder.decode_next_frame().unwrap().expect("Frame at 5s must exist");
    assert_eq!(frame_at_5s.pts, target);

    // Seek to end
    decoder.seek(duration).expect("Seek to end must succeed");
    let at_end = decoder.decode_next_frame().unwrap();
    assert!(at_end.is_none(), "End of stream must return None");
}

#[test]
fn test_video_encoder_trait_and_file_generation() {
    let temp_dir = std::env::temp_dir();
    let out_file = temp_dir.join(format!("test_enc_{}.bin", Tick::from_seconds(1).raw()));

    let config = VideoEncoderConfig {
        width: 1280,
        height: 720,
        fps_num: 30,
        fps_den: 1,
        bitrate_bps: 8_000_000,
        codec: VideoCodec::H264,
        hardware_accel: HwAccel::None,
        quality_preset: "medium".into(),
    };

    let mut encoder = SyntheticVideoEncoder::create(&out_file, config).expect("Encoder create must succeed");

    // Active encoder name must be exposed
    assert!(!encoder.active_encoder_name().is_empty());

    // Encode 10 test frames
    for i in 0..10 {
        let pts = Tick::from_frames(i, 30, 1);
        let dur = Tick::from_frames(1, 30, 1);
        let frame = VideoFrame::new_rgba8(pts, dur, 1280, 720, vec![128u8; 1280 * 720 * 4]);
        encoder.encode_frame(&frame).expect("Encode frame must succeed");
    }

    assert_eq!(encoder.frames_written(), 10);

    // Finalize
    Box::new(encoder).finish().expect("Finish must succeed");

    // Verify file exists on disk and is non-empty
    assert!(out_file.exists());
    let metadata = fs::metadata(&out_file).expect("File metadata must exist");
    assert!(metadata.len() > 0, "Encoded file must be non-empty");

    // Clean up
    let _ = fs::remove_file(&out_file);
}

#[test]
fn test_variable_frame_rate_detection_and_timecode_indexing() {
    let frame_step = Tick::from_frames(1, 30, 1);
    // Simulating smartphone jitter: 0, 33ms, 85ms (dropped frame), 118ms, 151ms
    let raw_pts = vec![
        Tick::ZERO,
        frame_step,
        frame_step * 2 + Tick::from_seconds_f64(0.020),
        frame_step * 3 + Tick::from_seconds_f64(0.015),
        frame_step * 4,
    ];

    let index = VfrTimecodeIndex::build(&raw_pts, frame_step);
    assert_eq!(index.frame_count(), 5);
    assert!(index.is_vfr, "Jitter exceeding 5% must be classified as VFR");

    // Binary search lookup
    let query_pts = frame_step * 2;
    let found_idx = index.frame_index_at_tick(query_pts);
    assert_eq!(found_idx, 1); // Falls inside interval [frame_step, frame_step*2 + 20ms)

    let query_pts2 = frame_step * 3;
    let found_idx2 = index.frame_index_at_tick(query_pts2);
    assert_eq!(found_idx2, 2);
}

#[test]
fn test_rotation_transforms_and_oriented_dimensions() {
    let width = 1920u32;
    let height = 1080u32;

    let meta_normal = VideoMetadata {
        duration: Tick::from_seconds(10),
        width,
        height,
        fps_num: 30,
        fps_den: 1,
        is_vfr: false,
        rotation: Rotation::None,
        bit_depth: 8,
        pixel_format: PixelFormat::Rgba8,
        codec_name: "h264".into(),
        has_audio: false,
        audio_sample_rate: 0,
        audio_channels: 0,
    };
    assert_eq!(meta_normal.oriented_dimensions(), (1920, 1080));

    let mut meta_90 = meta_normal.clone();
    meta_90.rotation = Rotation::Rotate90;
    assert_eq!(meta_90.oriented_dimensions(), (1080, 1920));

    let mut meta_180 = meta_normal.clone();
    meta_180.rotation = Rotation::Rotate180;
    assert_eq!(meta_180.oriented_dimensions(), (1920, 1080));

    let mut meta_270 = meta_normal.clone();
    meta_270.rotation = Rotation::Rotate270;
    assert_eq!(meta_270.oriented_dimensions(), (1080, 1920));

    // Pixel rotation verification
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    // Put red pixel at (0, 0)
    pixels[0] = 255;
    pixels[1] = 0;
    pixels[2] = 0;
    pixels[3] = 255;

    let (r90, rw, rh) = rotate_rgba8(&pixels, width, height, Rotation::Rotate90);
    assert_eq!((rw, rh), (1080, 1920));
    // In 90 deg clockwise: (0, 0) -> (h - 1, 0) = (1079, 0)
    let expected_dst_idx = 1079 * 4;
    assert_eq!(&r90[expected_dst_idx..expected_dst_idx + 4], &[255, 0, 0, 255]);
}

#[test]
fn test_10bit_source_conversion_to_rgba8() {
    let width = 64;
    let height = 64;
    let y_plane = vec![1023u16; width * height]; // peak white in 10-bit
    let u_plane = vec![512u16; (width / 2) * (height / 2)]; // neutral chrome
    let v_plane = vec![512u16; (width / 2) * (height / 2)]; // neutral chrome

    let rgba = yuv420p10le_to_rgba8(
        &y_plane,
        &u_plane,
        &v_plane,
        width,
        height,
        width,
        width / 2,
        width / 2,
    );

    assert_eq!(rgba.len(), width * height * 4);
    // Peak white 1023 in 10-bit converts to ~255 in 8-bit
    for pixel in rgba.chunks(4) {
        assert_eq!(pixel[0], 255);
        assert_eq!(pixel[1], 255);
        assert_eq!(pixel[2], 255);
        assert_eq!(pixel[3], 255);
    }
}

#[test]
fn test_platform_environment_config_probing() {
    let config = PlatformEnvironmentConfig::probe_from_env();
    // Probing should never panic and should report any discovered DLLs or paths
    println!("Platform environment config: {:?}", config);
}

#[test]
fn test_media_classifier_with_mp3_cover_art_mp4_and_text() {
    let temp_dir = std::env::temp_dir();
    let mp3_path = temp_dir.join(format!("test_art_{}.mp3", std::process::id()));
    let mp4_sound_path = temp_dir.join(format!("test_sound_{}.mp4", std::process::id()));
    let mp4_no_sound_path = temp_dir.join(format!("test_nosound_{}.mp4", std::process::id()));
    let txt_path = temp_dir.join(format!("test_doc_{}.txt", std::process::id()));

    // 1. Synthetic MP3 with ID3v2 APIC Cover Art frame
    let mut mp3_bytes = Vec::new();
    // ID3 header: "ID3", ver 3.0, flags 0, synchsafe size 128 bytes
    mp3_bytes.extend_from_slice(b"ID3\x03\x00\x00\x00\x00\x01\x00");
    // APIC frame
    mp3_bytes.extend_from_slice(b"APIC\x00\x00\x00\x20\x00\x00");
    mp3_bytes.extend_from_slice(&[0u8; 32]); // Image payload placeholder
    while mp3_bytes.len() < 138 {
        mp3_bytes.push(0);
    }
    // MPEG Frame Sync (MPEG1 Layer3 44.1kHz 128kbps stereo)
    mp3_bytes.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x64]);
    mp3_bytes.extend_from_slice(&[0u8; 1000]);
    std::fs::write(&mp3_path, &mp3_bytes).unwrap();

    // 2. Synthetic MP4 with Video and Audio
    let mut mp4_sound_bytes = Vec::new();
    mp4_sound_bytes.extend_from_slice(b"\x00\x00\x00\x18ftypisom\x00\x00\x02\x00isomiso2mp41");
    mp4_sound_bytes.extend_from_slice(b"\x00\x00\x00\x40moov");
    mp4_sound_bytes.extend_from_slice(b"hdlrvide");
    mp4_sound_bytes.extend_from_slice(&[0u8; 16]);
    mp4_sound_bytes.extend_from_slice(b"hdlrsoun");
    mp4_sound_bytes.extend_from_slice(&[0u8; 64]);
    std::fs::write(&mp4_sound_path, &mp4_sound_bytes).unwrap();

    // 3. Synthetic MP4 with Video ONLY (no audio)
    let mut mp4_nosound_bytes = Vec::new();
    mp4_nosound_bytes.extend_from_slice(b"\x00\x00\x00\x18ftypisom\x00\x00\x02\x00isomiso2mp41");
    mp4_nosound_bytes.extend_from_slice(b"\x00\x00\x00\x30moov");
    mp4_nosound_bytes.extend_from_slice(b"hdlrvide");
    mp4_nosound_bytes.extend_from_slice(&[0u8; 64]);
    std::fs::write(&mp4_no_sound_path, &mp4_nosound_bytes).unwrap();

    // 4. Plain Text File
    std::fs::write(&txt_path, "VentaCore Editor: Plain Text Readme File\nNot video or audio.\n").unwrap();

    // Classify and Assert
    let c_mp3 = classify(&mp3_path);
    assert!(c_mp3.is_audio_only(), "MP3 with cover art must be classified as AudioOnly!");
    if let MediaClassification::AudioOnly { has_cover_art, .. } = c_mp3 {
        assert!(has_cover_art, "MP3 must detect APIC cover art");
    }

    let c_sound = classify(&mp4_sound_path);
    assert!(c_sound.is_video(), "MP4 with video+audio must be classified as Video");
    if let MediaClassification::Video { has_audio, .. } = c_sound {
        assert!(has_audio, "MP4 must detect audio track");
    }

    let c_nosound = classify(&mp4_no_sound_path);
    assert!(c_nosound.is_video(), "MP4 without sound must be classified as Video");
    if let MediaClassification::Video { has_audio, .. } = c_nosound {
        assert!(!has_audio, "MP4 must report no audio track");
    }

    let c_txt = classify(&txt_path);
    assert!(c_txt.is_unsupported(), "Text file must be classified as Unsupported");

    // Cleanup
    let _ = std::fs::remove_file(&mp3_path);
    let _ = std::fs::remove_file(&mp4_sound_path);
    let _ = std::fs::remove_file(&mp4_no_sound_path);
    let _ = std::fs::remove_file(&txt_path);
}

#[test]
fn test_real_video_file_decoding_with_ffmpeg() {
    use ventacore_platform::{find_ffmpeg_binary, PlatformVideoDecoder, VideoDecoder};

    let ffmpeg_exe = match find_ffmpeg_binary() {
        Some(exe) => exe,
        None => return, // Skip if ffmpeg binary not installed on this machine
    };

    let temp_dir = std::env::temp_dir();
    let vid_path = temp_dir.join(format!("real_vid_test_{}.mp4", std::process::id()));

    // Generate a 2-second 320x240 test video with ffmpeg
    let mut cmd = std::process::Command::new(&ffmpeg_exe);
    cmd.args(&[
        "-f", "lavfi",
        "-i", "testsrc=duration=2:size=320x240:rate=25",
        "-pix_fmt", "yuv420p",
        "-y",
    ]);
    cmd.arg(&vid_path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let status = cmd.status().expect("generate test video with ffmpeg");
    assert!(status.success(), "ffmpeg video generation must succeed");

    // Open video with PlatformVideoDecoder
    let mut decoder = PlatformVideoDecoder::open(&vid_path).expect("open real video with PlatformVideoDecoder");

    // 1. Check metadata
    let meta = decoder.metadata();
    assert_eq!(meta.width, 320, "Metadata width must match real video (320), not synthetic 640");
    assert_eq!(meta.height, 240, "Metadata height must match real video (240), not synthetic 360");
    let dur_secs = meta.duration.to_seconds_f64();
    assert!((dur_secs - 2.0).abs() < 0.2, "Metadata duration must match real video (~2.0s), not synthetic 10s");

    // 2. Decode next frame
    let frame1 = decoder.decode_next_frame().expect("decode frame 1").expect("frame 1 must exist");
    assert_eq!(frame1.width, 320);
    assert_eq!(frame1.height, 240);
    assert_eq!(frame1.data.len(), 320 * 240 * 4);

    // 3. Seek to 1.0s and decode
    decoder.seek(ventacore_core::Tick::from_seconds(1)).expect("seek to 1s");
    let frame_seek = decoder.decode_next_frame().expect("decode at 1s").expect("frame at 1s must exist");
    assert_eq!(frame_seek.width, 320);
    assert_eq!(frame_seek.height, 240);

    // Cleanup
    let _ = std::fs::remove_file(&vid_path);
}
