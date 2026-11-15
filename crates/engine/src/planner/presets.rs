//! Built-in montage planner presets.

use ventacore_core::{GenerationSettings, GridMode, PlacementMode, Tick};

/// Available preset names.
pub const PRESET_NAMES: &[&str] = &[
    "Balanced",
    "FastCuts",
    "Cinematic",
    "NarrativeWalk",
    "MontageShuffle",
];

/// Returns a copy of `GenerationSettings` configured for the specified preset.
pub fn get_preset_settings(name: &str) -> Option<GenerationSettings> {
    match name.to_ascii_lowercase().as_str() {
        "balanced" | "default" => Some(GenerationSettings {
            clip_variance: 0.5,
            source_reuse_penalty: 0.7,
            spread: 0.6,
            continue_start_points: false,
            placement_mode: PlacementMode::WindowedWalk,
            min_clip_duration: Tick::from_seconds(1),
            max_clip_duration: Tick::from_seconds(4),
            cut_offset: Tick::ZERO,
            snap_to_scenes: true,
            snap_to_dark_edges: true,
            energy_density: 0.5,
            grid_mode: GridMode::Transient,
            seed: 42,
            preset_name: Some("Balanced".into()),
        }),

        "fastcuts" | "highenergy" => Some(GenerationSettings {
            clip_variance: 0.8,
            source_reuse_penalty: 0.85,
            spread: 0.8,
            continue_start_points: false,
            placement_mode: PlacementMode::WindowedWalk,
            min_clip_duration: Tick::from_seconds_f64(0.4),
            max_clip_duration: Tick::from_seconds_f64(2.0),
            cut_offset: Tick::from_seconds_f64(-0.04), // 40ms lead
            snap_to_scenes: true,
            snap_to_dark_edges: true,
            energy_density: 0.9,
            grid_mode: GridMode::Transient,
            seed: 42,
            preset_name: Some("FastCuts".into()),
        }),

        "cinematic" | "slowpaced" => Some(GenerationSettings {
            clip_variance: 0.2,
            source_reuse_penalty: 0.5,
            spread: 0.4,
            continue_start_points: true,
            placement_mode: PlacementMode::WindowedWalk,
            min_clip_duration: Tick::from_seconds(2),
            max_clip_duration: Tick::from_seconds(8),
            cut_offset: Tick::ZERO,
            snap_to_scenes: true,
            snap_to_dark_edges: true,
            energy_density: 0.3,
            grid_mode: GridMode::Grid,
            seed: 42,
            preset_name: Some("Cinematic".into()),
        }),

        "narrativewalk" => Some(GenerationSettings {
            clip_variance: 0.3,
            source_reuse_penalty: 0.3,
            spread: 0.2,
            continue_start_points: true,
            placement_mode: PlacementMode::WindowedWalk,
            min_clip_duration: Tick::from_seconds(1),
            max_clip_duration: Tick::from_seconds(5),
            cut_offset: Tick::ZERO,
            snap_to_scenes: true,
            snap_to_dark_edges: true,
            energy_density: 0.5,
            grid_mode: GridMode::Transient,
            seed: 42,
            preset_name: Some("NarrativeWalk".into()),
        }),

        "montageshuffle" => Some(GenerationSettings {
            clip_variance: 0.7,
            source_reuse_penalty: 0.9,
            spread: 0.9,
            continue_start_points: false,
            placement_mode: PlacementMode::EvenWindows,
            min_clip_duration: Tick::from_seconds(1),
            max_clip_duration: Tick::from_seconds(4),
            cut_offset: Tick::ZERO,
            snap_to_scenes: true,
            snap_to_dark_edges: true,
            energy_density: 0.6,
            grid_mode: GridMode::Transient,
            seed: 42,
            preset_name: Some("MontageShuffle".into()),
        }),

        _ => None,
    }
}
