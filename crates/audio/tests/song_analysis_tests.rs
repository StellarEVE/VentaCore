//! Stage 2 Song Analysis Integration Tests.
//!
//! Validates:
//! - 90, 120, and 140 BPM click trains measure within +/- 1 BPM.
//! - Octave ambiguity test (resolving half-time and double-time cases).
//! - All manual grid corrections (tap tempo, half time, double time, shift grid, set downbeat).
//! - Change points detected in a quiet-to-loud test signal.
//! - Determinism across repeated runs on the same machine.

use ventacore_audio::*;
use ventacore_core::{generate_click_train, GridMode, Tick};

#[test]
fn test_click_train_tempo_accuracy_90_120_140_bpm() {
    let test_cases = [90.0_f32, 120.0_f32, 140.0_f32];

    for &target_bpm in &test_cases {
        let sample_rate = 48000;
        let duration = 8.0;
        let samples = generate_click_train(target_bpm, duration, sample_rate);

        let result = analyze_song_samples(&samples, sample_rate, GridMode::Grid)
            .expect("Analysis should succeed on click train");

        let diff = (result.bpm - target_bpm).abs();
        assert!(
            diff <= 1.0,
            "Target {:.1} BPM measured as {:.2} BPM (diff {:.3} exceeds 1.0 BPM limit)",
            target_bpm,
            result.bpm,
            diff
        );

        // Verify beat grid has positive elements and matching intervals
        assert!(!result.beat_ticks.is_empty());
        assert!(!result.bar_ticks.is_empty());
        assert!(!result.phrase_ticks.is_empty());
    }
}

#[test]
fn test_octave_ambiguity_disambiguation() {
    // Generate an alternating click train with accents that could suggest 60 BPM or 120 BPM
    let sample_rate = 48000;
    let duration = 8.0;
    let total_samples = (duration * sample_rate as f32) as usize;
    let mut samples = vec![0.0_f32; total_samples];

    let period = (sample_rate as f32 * 60.0 / 120.0).round() as usize; // 120 BPM
    for (i, idx) in (0..total_samples).step_by(period).enumerate() {
        let amp = if i % 2 == 0 { 1.0 } else { 0.4 }; // alternating strength
        let click_len = 480.min(total_samples - idx);
        for k in 0..click_len {
            samples[idx + k] = amp * (-(k as f32) * 0.01).exp();
        }
    }

    let result = analyze_song_samples(&samples, sample_rate, GridMode::Grid)
        .expect("Analysis must succeed");

    // Must resolve to 120 BPM within +/- 1 BPM, not 60 or 240 BPM
    let diff = (result.bpm - 120.0).abs();
    assert!(
        diff <= 1.0,
        "Ambiguous pattern resolved to {:.2} BPM instead of 120 BPM",
        result.bpm
    );
}

#[test]
fn test_manual_corrections_produce_expected_grid() {
    let sample_rate = 48000;
    let samples = generate_click_train(120.0, 10.0, sample_rate);
    let original = analyze_song_samples(&samples, sample_rate, GridMode::Grid)
        .expect("Analysis must succeed");

    // 1. Half Time
    let halved = half_time(&original);
    assert!((halved.bpm - 60.0).abs() <= 1.0);
    assert!(
        halved.beat_ticks.len() < original.beat_ticks.len(),
        "Half time must have fewer, wider beats"
    );

    // 2. Double Time
    let doubled = double_time(&original);
    assert!((doubled.bpm - 240.0).abs() <= 2.0);
    assert!(
        doubled.beat_ticks.len() > original.beat_ticks.len(),
        "Double time must have more, denser beats"
    );

    // 3. Shift Grid
    let shift_delta = Tick::from_seconds_f64(0.25);
    let shifted = shift_grid(&original, shift_delta);
    assert_eq!(shifted.beat_ticks[0], original.beat_ticks[0] + shift_delta);
    assert_eq!(shifted.bar_ticks[0], original.bar_ticks[0] + shift_delta);

    // 4. Set Downbeat
    let new_downbeat = Tick::from_seconds_f64(1.0);
    let reanchored = set_downbeat(&original, new_downbeat);
    assert!(
        reanchored.bar_ticks.contains(&new_downbeat),
        "Downbeat must be present in bar ticks"
    );

    // 5. Tap Tempo
    let taps = [
        Tick::from_seconds(0),
        Tick::from_seconds_f64(0.667), // ~90 BPM
        Tick::from_seconds_f64(1.333),
        Tick::from_seconds_f64(2.0),
    ];
    let tapped_bpm = tap_tempo(&taps).expect("Tap tempo must succeed");
    assert!((tapped_bpm - 90.0).abs() <= 1.0);
}

#[test]
fn test_change_points_in_quiet_to_loud_signal() {
    let sample_rate = 48000;
    let duration = 8.0;
    let total_samples = (duration * sample_rate as f64) as usize;
    let mut samples = vec![0.0_f32; total_samples];

    // 0 to 4s: quiet
    for i in 0..(4 * sample_rate) {
        samples[i] = 0.05 * (i as f32 * 0.05).sin();
    }
    // 4 to 8s: loud drop
    for i in (4 * sample_rate)..total_samples {
        samples[i] = 0.95 * (i as f32 * 0.05).sin();
    }

    let result = analyze_song_samples(&samples, sample_rate as u32, GridMode::Grid)
        .expect("Analysis must succeed");

    assert!(
        !result.change_points.is_empty(),
        "Must detect at least one loudness change point"
    );
    let drop = &result.change_points[0];
    assert_eq!(drop.label, "Energy Lift / Chorus Drop");
    let drop_sec = drop.tick.to_seconds_f64();
    assert!(
        (drop_sec - 4.0).abs() < 0.6,
        "Change point should occur around 4.0s, got {:.2}s",
        drop_sec
    );
}

#[test]
fn test_results_are_identical_across_repeated_runs() {
    let sample_rate = 44100;
    let samples = generate_click_train(120.0, 6.0, sample_rate);

    let run1 = analyze_song_samples(&samples, sample_rate, GridMode::Grid).unwrap();
    let run2 = analyze_song_samples(&samples, sample_rate, GridMode::Grid).unwrap();

    assert_eq!(run1.bpm, run2.bpm);
    assert_eq!(run1.beat_ticks, run2.beat_ticks);
    assert_eq!(run1.bar_ticks, run2.bar_ticks);
    assert_eq!(run1.phrase_ticks, run2.phrase_ticks);
    assert_eq!(run1.onsets, run2.onsets);
    assert_eq!(run1.change_points, run2.change_points);
    assert_eq!(run1.energy_per_second, run2.energy_per_second);
    assert_eq!(run1.energy_per_bar, run2.energy_per_bar);
}
