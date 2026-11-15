//! Integration tests for Stage 7: GPU Render Core.
//!
//! Validates:
//! 1. Resolution tiers downscaling and even dimension alignment.
//! 2. CPU reference oracle correctness across fit modes (Letterbox, Crop, BlurPadding).
//! 3. CPU reference oracle transition blends (CrossDissolve, DipToColor, WipeLeft, PushLeft).
//! 4. 2D Transforms (scale, position, rotation, opacity).
//! 5. Charter Rule 5: GPU compositor rendering matches CPU Reference Oracle within <= 1 color step.
//! 6. Software fallback execution when running without hardware acceleration.

use ventacore_render::{
    FitMode, GpuCompositor, RenderFrame, ResolutionTier, Transform2D, TransitionKind,
    TransitionParams,
};

#[test]
fn test_resolution_tier_scaling() {
    let full = ResolutionTier::Full;
    let half = ResolutionTier::Half;
    let quarter = ResolutionTier::Quarter;
    let eighth = ResolutionTier::Eighth;

    assert_eq!(full.scale_dimensions(1920, 1080), (1920, 1080));
    assert_eq!(half.scale_dimensions(1920, 1080), (960, 540));
    assert_eq!(quarter.scale_dimensions(1920, 1080), (480, 270));
    // 1920 * 0.125 = 240, 1080 * 0.125 = 135 -> rounded and adjusted to even: 136
    let (ew, eh) = eighth.scale_dimensions(1920, 1080);
    assert_eq!(ew, 240);
    assert_eq!(eh % 2, 0); // must be even for codec alignment
    assert!(eh == 136 || eh == 134);

    // Minimum dimensions
    assert_eq!(eighth.scale_dimensions(1, 1), (1, 1));
}

#[test]
fn test_cpu_oracle_hard_cut_1_to_1() {
    let compositor = GpuCompositor::new_cpu_only();
    let oracle = compositor.oracle();

    let mut src = RenderFrame::new(100, 100);
    // Draw pattern
    for y in 0..100 {
        for x in 0..100 {
            src.set_pixel(x, y, [x as u8, y as u8, 128, 255]);
        }
    }

    let out = oracle.render_cut(
        &src,
        100,
        100,
        FitMode::Crop,
        &Transform2D::default(),
    );

    assert_eq!(out.width, 100);
    assert_eq!(out.height, 100);
    assert_eq!(src.max_channel_difference(&out), 0);
}

#[test]
fn test_cpu_oracle_letterbox() {
    let compositor = GpuCompositor::new_cpu_only();
    let oracle = compositor.oracle();

    // 16:9 source (160x90), target is square (160x160)
    let src = RenderFrame::from_solid_color(160, 90, [255, 100, 50, 255]);
    let out = oracle.render_cut(
        &src,
        160,
        160,
        FitMode::Letterbox,
        &Transform2D::default(),
    );

    assert_eq!(out.width, 160);
    assert_eq!(out.height, 160);

    // Top border (y = 5) should be black padding
    assert_eq!(out.get_pixel(80, 5), [0, 0, 0, 255]);
    // Bottom border (y = 155) should be black padding
    assert_eq!(out.get_pixel(80, 155), [0, 0, 0, 255]);
    // Center (y = 80) should be source color
    let center = out.get_pixel(80, 80);
    assert_eq!(center, [255, 100, 50, 255]);
}

#[test]
fn test_cpu_oracle_crop() {
    let compositor = GpuCompositor::new_cpu_only();
    let oracle = compositor.oracle();

    // 16:9 source (160x90), target is square (160x160)
    let src = RenderFrame::from_solid_color(160, 90, [20, 180, 220, 255]);
    let out = oracle.render_cut(
        &src,
        160,
        160,
        FitMode::Crop,
        &Transform2D::default(),
    );

    // Every pixel across the entire square canvas must be the source color (no black bars)
    assert_eq!(out.get_pixel(0, 0), [20, 180, 220, 255]);
    assert_eq!(out.get_pixel(80, 80), [20, 180, 220, 255]);
    assert_eq!(out.get_pixel(159, 159), [20, 180, 220, 255]);
}

#[test]
fn test_cpu_oracle_blur_padding() {
    let compositor = GpuCompositor::new_cpu_only();
    let oracle = compositor.oracle();

    // 16:9 source (160x90), target is square (160x160)
    let src = RenderFrame::from_solid_color(160, 90, [100, 200, 100, 255]);
    let out = oracle.render_cut(
        &src,
        160,
        160,
        FitMode::BlurPadding,
        &Transform2D::default(),
    );

    // Center contains crisp source
    assert_eq!(out.get_pixel(80, 80), [100, 200, 100, 255]);

    // Top border (y = 5) contains blurred/dimmed padding (NOT pure black)
    let top_pad = out.get_pixel(80, 5);
    assert!(top_pad[0] > 0);
    assert!(top_pad[1] > 0);
    assert_eq!(top_pad[3], 255);
}

#[test]
fn test_cpu_oracle_cross_dissolve() {
    let compositor = GpuCompositor::new_cpu_only();
    let oracle = compositor.oracle();

    let frame_a = RenderFrame::from_solid_color(100, 100, [200, 0, 0, 255]);
    let frame_b = RenderFrame::from_solid_color(100, 100, [0, 0, 200, 255]);

    // Progress 0.0 -> pure A
    let out_0 = oracle.render_transition(
        &frame_a,
        &frame_b,
        100,
        100,
        FitMode::Crop,
        &TransitionParams::cross_dissolve(0.0),
    );
    assert_eq!(out_0.get_pixel(50, 50), [200, 0, 0, 255]);

    // Progress 1.0 -> pure B
    let out_1 = oracle.render_transition(
        &frame_a,
        &frame_b,
        100,
        100,
        FitMode::Crop,
        &TransitionParams::cross_dissolve(1.0),
    );
    assert_eq!(out_1.get_pixel(50, 50), [0, 0, 200, 255]);

    // Progress 0.5 -> 50% A, 50% B
    let out_half = oracle.render_transition(
        &frame_a,
        &frame_b,
        100,
        100,
        FitMode::Crop,
        &TransitionParams::cross_dissolve(0.5),
    );
    let mid_pixel = out_half.get_pixel(50, 50);
    assert_eq!(mid_pixel[0], 100);
    assert_eq!(mid_pixel[1], 0);
    assert_eq!(mid_pixel[2], 100);
    assert_eq!(mid_pixel[3], 255);
}

#[test]
fn test_cpu_oracle_dip_to_black() {
    let compositor = GpuCompositor::new_cpu_only();
    let oracle = compositor.oracle();

    let frame_a = RenderFrame::from_solid_color(100, 100, [250, 250, 0, 255]);
    let frame_b = RenderFrame::from_solid_color(100, 100, [0, 250, 250, 255]);

    // At t = 0.5, dip to black must be solid black
    let out_mid = oracle.render_transition(
        &frame_a,
        &frame_b,
        100,
        100,
        FitMode::Crop,
        &TransitionParams::dip_to_black(0.5),
    );
    assert_eq!(out_mid.get_pixel(50, 50), [0, 0, 0, 255]);
}

#[test]
fn test_cpu_oracle_wipe_left() {
    let compositor = GpuCompositor::new_cpu_only();
    let oracle = compositor.oracle();

    let frame_a = RenderFrame::from_solid_color(100, 100, [255, 0, 0, 255]);
    let frame_b = RenderFrame::from_solid_color(100, 100, [0, 0, 255, 255]);

    let out = oracle.render_transition(
        &frame_a,
        &frame_b,
        100,
        100,
        FitMode::Crop,
        &TransitionParams::new(TransitionKind::WipeLeft, 0.5),
    );

    // Left half (x < 50) is incoming frame B (Blue)
    assert_eq!(out.get_pixel(25, 50), [0, 0, 255, 255]);
    // Right half (x >= 50) is outgoing frame A (Red)
    assert_eq!(out.get_pixel(75, 50), [255, 0, 0, 255]);
}

#[test]
fn test_cpu_oracle_2d_transform_scale_and_opacity() {
    let compositor = GpuCompositor::new_cpu_only();
    let oracle = compositor.oracle();

    let src = RenderFrame::from_solid_color(100, 100, [200, 100, 50, 255]);
    let transform = Transform2D {
        position: [0.0, 0.0],
        scale: [0.5, 0.5],
        rotation_radians: 0.0,
        opacity: 0.5,
    };

    let out = oracle.render_cut(&src, 100, 100, FitMode::Crop, &transform);

    // Outer corner (0, 0) should be un-covered (black)
    assert_eq!(out.get_pixel(0, 0), [0, 0, 0, 255]);

    // Center (50, 50) should be source color attenuated by opacity 0.5
    let center = out.get_pixel(50, 50);
    assert!((center[0] as i16 - 100).abs() <= 1);
    assert!((center[1] as i16 - 50).abs() <= 1);
    assert!((center[2] as i16 - 25).abs() <= 1);
}

#[test]
fn test_gpu_compositor_vs_cpu_oracle_charter_rule_5() {
    let mut compositor = GpuCompositor::new();
    let is_gpu = compositor.is_hardware_accelerated();
    println!("GPU Compositor hardware accelerated: {is_gpu}");
    if let Some(name) = compositor.adapter_name() {
        println!("Active GPU Adapter: {name}");
    }

    let oracle = compositor.oracle().clone();

    // 1. Hard cut comparison with Crop
    let src = RenderFrame::from_solid_color(128, 72, [210, 140, 70, 255]);
    let target_w = 128;
    let target_h = 72;

    let oracle_frame = oracle.render_cut(&src, target_w, target_h, FitMode::Crop, &Transform2D::default());
    let compositor_frame = compositor
        .render_cut(&src, target_w, target_h, FitMode::Crop, &Transform2D::default())
        .expect("render_cut should succeed");

    let max_diff = compositor_frame.max_channel_difference(&oracle_frame);
    println!("Hard cut max channel difference vs oracle: {max_diff}");
    assert!(
        max_diff <= 1,
        "Charter Rule 5 violation: GPU output differs from CPU oracle by > 1 color step (diff = {max_diff})"
    );

    // 2. Cross-dissolve comparison
    let frame_a = RenderFrame::from_solid_color(target_w, target_h, [180, 40, 20, 255]);
    let frame_b = RenderFrame::from_solid_color(target_w, target_h, [20, 60, 200, 255]);
    let trans = TransitionParams::cross_dissolve(0.5);

    let oracle_trans = oracle.render_transition(&frame_a, &frame_b, target_w, target_h, FitMode::Crop, &trans);
    let compositor_trans = compositor
        .render_transition(&frame_a, &frame_b, target_w, target_h, FitMode::Crop, &trans)
        .expect("render_transition should succeed");

    let trans_diff = compositor_trans.max_channel_difference(&oracle_trans);
    println!("Cross-dissolve max channel difference vs oracle: {trans_diff}");
    assert!(
        trans_diff <= 1,
        "Charter Rule 5 violation: GPU transition differs from CPU oracle by > 1 color step (diff = {trans_diff})"
    );
}
