//! CLI command: Analyze an audio song file for tempo, beats, bars, energy, and change points.

use std::path::Path;
use ventacore_audio::analysis::analyze_song_file;
use ventacore_audio::decoder::SymphoniaDecoder;
use ventacore_core::GridMode;

pub fn execute_analyze_song(audio_path: impl AsRef<Path>, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let path = audio_path.as_ref();
    let decoder = SymphoniaDecoder::new();
    let structure = analyze_song_file(path, &decoder, GridMode::Transient)?;

    if json {
        let json_str = serde_json::to_string_pretty(&structure)?;
        println!("{}", json_str);
    } else {
        let total_beats = structure.beat_ticks.len();
        let duration = structure.beat_ticks.last().copied().unwrap_or_default();

        println!("============================================================");
        println!(" VentaCore Song Analysis: {}", path.display());
        println!("============================================================");
        println!("Estimated Tempo:    {:.2} BPM", structure.bpm);
        println!("Total Beats:        {}", total_beats);
        println!("Total Bars:         {}", structure.bar_ticks.len());
        println!("Total Phrases:      {}", structure.phrase_ticks.len());
        println!("Approx. Duration:   {:.2}s", duration.to_seconds_f64());
        println!("Change Points:      {}", structure.change_points.len());
        for (i, cp) in structure.change_points.iter().enumerate() {
            println!(
                "  [{}] at {:.2}s: {} ({:?})",
                i + 1,
                cp.tick.to_seconds_f64(),
                cp.label,
                cp.energy_regime
            );
        }
        println!("============================================================");
    }

    Ok(())
}
