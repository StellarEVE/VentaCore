//! Milestone A End-to-End Integration Test (Stage 10 Exit Gate).
//!
//! Validates the entire pipeline from audio song and video footage to a finished,
//! beat-synchronized exported MP4 file with atomic finalization and cache cleanup.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use ventacore_cli::commands::purge_cache::execute_purge_cache;
use ventacore_cli::commands::run::execute_run;
use ventacore_core::generate_click_train;

/// Writes floating-point PCM audio samples to a valid 16-bit mono WAV file.
fn write_wav_file(path: impl AsRef<Path>, samples: &[f32], sample_rate: u32) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    let num_samples = samples.len() as u32;
    let byte_rate = sample_rate * 2; // 16-bit mono = 2 bytes per sample
    let data_len = num_samples * 2;

    // RIFF chunk descriptor
    file.write_all(b"RIFF")?;
    file.write_all(&(data_len + 36).to_le_bytes())?;
    file.write_all(b"WAVE")?;

    // "fmt " sub-chunk
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?; // Subchunk1Size (16 for PCM)
    file.write_all(&1u16.to_le_bytes())?;  // AudioFormat (1 = PCM)
    file.write_all(&1u16.to_le_bytes())?;  // NumChannels (1 = Mono)
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;  // BlockAlign (NumChannels * BitsPerSample / 8)
    file.write_all(&16u16.to_le_bytes())?; // BitsPerSample
    file.write_all(b"data")?;
    file.write_all(&data_len.to_le_bytes())?;

    for &s in samples {
        let pcm = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        file.write_all(&pcm.to_le_bytes())?;
    }
    file.flush()?;
    Ok(())
}

#[test]
fn test_milestone_a_end_to_end_beat_synced_pipeline() {
    let temp_dir = std::env::temp_dir().join("ventacore_m1_e2e_test");
    let _ = fs::create_dir_all(&temp_dir);

    let song_path = temp_dir.join("test_song_120bpm.wav");
    let video1_path = temp_dir.join("footage_1.mp4");
    let video2_path = temp_dir.join("footage_2.mp4");
    let output_path = temp_dir.join("exported_final_cut.mp4");
    let part_path = temp_dir.join("exported_final_cut.mp4.part");

    // Clean up if existed from previous run
    let _ = fs::remove_file(&song_path);
    let _ = fs::remove_file(&video1_path);
    let _ = fs::remove_file(&video2_path);
    let _ = fs::remove_file(&output_path);
    let _ = fs::remove_file(&part_path);

    // 1. Generate 6-second 120 BPM click-train audio file
    let sample_rate = 48000;
    let duration_secs = 6.0;
    let click_samples = generate_click_train(120.0, duration_secs, sample_rate);
    write_wav_file(&song_path, &click_samples, sample_rate).expect("Must write WAV fixture");

    // 2. Generate video footage placeholder files
    File::create(&video1_path).expect("Must create video 1");
    File::create(&video2_path).expect("Must create video 2");

    // 3. Execute flagship run command
    let run_result = execute_run(
        &song_path,
        &[video1_path.clone(), video2_path.clone()],
        &output_path,
        Some(320),
        Some(180),
        Some(42),
        Some("Balanced"),
    );

    assert!(
        run_result.is_ok(),
        "End-to-end execution failed: {:?}",
        run_result.err()
    );

    // 4. Verify outputs and Charter constraints
    assert!(output_path.exists(), "Final exported cut must exist on disk");
    let meta = fs::metadata(&output_path).expect("Must read output file metadata");
    assert!(meta.len() > 0, "Exported file must have non-zero size");

    // Charter Rule 9: Atomic rename guarantees no leftover .part file
    assert!(
        !part_path.exists(),
        "Partial .part file must not remain on disk after successful completion"
    );

    // 5. Verify cache purge command
    let purge_res = execute_purge_cache(Some(temp_dir.join("cache")));
    assert!(purge_res.is_ok(), "Cache purge must succeed cleanly");

    // 6. Cleanup test directory
    let _ = fs::remove_dir_all(temp_dir);
}
