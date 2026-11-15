//! Tests for Stage 9 audio mixer and envelope fades.

use ventacore_audio::{AudioMixTrack, AudioMixer};
use ventacore_core::{Tick, TimeRange};

#[test]
fn test_audio_mixer_multitrack_gain_and_fades() {
    let mixer = AudioMixer::new(48000);

    // Track 1: 1 second of stereo tone (0.5 amplitude on L and R)
    let samples_t1 = vec![0.5f32; 48000 * 2];
    let track1 = AudioMixTrack::new(Tick::ZERO, Tick::from_seconds(1), samples_t1).with_volume(1.0);

    // Track 2: 1 second of stereo tone starting at 0.5s with volume 0.5
    let samples_t2 = vec![0.4f32; 48000 * 2];
    let track2 = AudioMixTrack::new(Tick::from_seconds_f64(0.5), Tick::from_seconds(1), samples_t2)
        .with_volume(0.5)
        .with_fades(Tick::from_seconds_f64(0.1), Tick::from_seconds_f64(0.1));

    // Mix first second: [0.0s to 1.0s]
    let mix_range = TimeRange::new(Tick::ZERO, Tick::from_seconds(1));
    let mixed = mixer.mix_range(mix_range, 1.0, &[track1, track2]);

    assert_eq!(mixed.len(), 48000 * 2);

    // At 0.2s: only track 1 is active -> 0.5
    let sample_0_2s = mixed[(48000.0 * 0.2) as usize * 2];
    assert!((sample_0_2s - 0.5).abs() < 1e-4);

    // At 0.75s: both tracks active -> track 1 (0.5) + track 2 (0.4 * 0.5 = 0.2) = 0.7
    let sample_0_75s = mixed[(48000.0 * 0.75) as usize * 2];
    assert!((sample_0_75s - 0.7).abs() < 1e-3);
}

#[test]
fn test_audio_mixer_mute_and_solo() {
    let mixer = AudioMixer::new(48000);

    let t1_samples = vec![0.8f32; 48000 * 2];
    let mut t1 = AudioMixTrack::new(Tick::ZERO, Tick::from_seconds(1), t1_samples);

    let t2_samples = vec![0.3f32; 48000 * 2];
    let mut t2 = AudioMixTrack::new(Tick::ZERO, Tick::from_seconds(1), t2_samples);

    // Case 1: t1 muted -> only t2 heard
    t1.mute = true;
    let mixed_muted = mixer.mix_range(
        TimeRange::new(Tick::ZERO, Tick::from_seconds(1)),
        1.0,
        &[t1.clone(), t2.clone()],
    );
    assert!((mixed_muted[0] - 0.3).abs() < 1e-4);

    // Case 2: t1 unmuted, t2 solo -> only t2 heard
    t1.mute = false;
    t2.solo = true;
    let mixed_solo = mixer.mix_range(
        TimeRange::new(Tick::ZERO, Tick::from_seconds(1)),
        1.0,
        &[t1, t2],
    );
    assert!((mixed_solo[0] - 0.3).abs() < 1e-4);
}

#[test]
fn test_audio_mixer_soft_limiting_prevents_clipping() {
    let mixer = AudioMixer::new(48000);

    // Two loud signals that sum to 1.6 (> 1.0)
    let t1_samples = vec![0.8f32; 48000 * 2];
    let t1 = AudioMixTrack::new(Tick::ZERO, Tick::from_seconds(1), t1_samples);

    let t2_samples = vec![0.8f32; 48000 * 2];
    let t2 = AudioMixTrack::new(Tick::ZERO, Tick::from_seconds(1), t2_samples);

    let mixed = mixer.mix_range(
        TimeRange::new(Tick::ZERO, Tick::from_seconds(1)),
        1.0,
        &[t1, t2],
    );

    // Maximum sample should be soft-limited to <= 1.0 (no integer wrapping or harsh clipping)
    for &s in &mixed {
        assert!(s.abs() <= 1.0);
    }
}
