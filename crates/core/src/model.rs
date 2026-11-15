//! Project data model for VentaCore.
//!
//! Stores assets, tracks, items, transitions, effect stacks, markers,
//! generation controls, and multiple songs.
//!
//! Strictly separated from codecs, rendering, and UI frameworks.

use crate::time::{Tick, TimeRange};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Strongly typed identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AssetId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TrackId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ItemId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MarkerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SongId(pub u64);

/// Container metadata obtained from initial probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeData {
    pub duration: Tick,
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub has_audio: bool,
    pub audio_sample_rate: u32,
}

/// An imported media file reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub id: AssetId,
    pub path_hint: String,
    pub fingerprint: Option<String>,
    pub probe_data: Option<ProbeData>,
    pub intro_trim: Tick,
    pub outro_trim: Tick,
    pub include_ranges: Vec<TimeRange>,
    pub exclude_ranges: Vec<TimeRange>,
    pub favourite_moments: Vec<TimeRange>,
}

impl Asset {
    pub fn new(id: AssetId, path_hint: String) -> Self {
        Self {
            id,
            path_hint,
            fingerprint: None,
            probe_data: None,
            intro_trim: Tick::ZERO,
            outro_trim: Tick::ZERO,
            include_ranges: Vec::new(),
            exclude_ranges: Vec::new(),
            favourite_moments: Vec::new(),
        }
    }
}

/// Track kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackKind {
    Video,
    Audio,
}

/// A timeline track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub kind: TrackKind,
    pub muted: bool,
    pub solo: bool,
    pub locked: bool,
    pub hidden: bool,
}

impl Track {
    pub fn new(id: TrackId, name: impl Into<String>, kind: TrackKind) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            muted: false,
            solo: false,
            locked: false,
            hidden: false,
        }
    }
}

/// Transition kinds supported across stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransitionKind {
    CrossDissolve,
    DipToColor,
    Push,
    Slide,
    Split,
    CrashZoom,
    DirectionalZoom,
    TwistWheel,
    RadialTwist,
    FilmDissolve,
    DipToDissolve,
}

/// A transition between adjacent clips.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    pub kind: TransitionKind,
    pub duration: Tick,
    pub centred_on_cut: bool,
    pub params: HashMap<String, String>,
}

impl Transition {
    pub fn new(kind: TransitionKind, duration: Tick) -> Self {
        Self {
            kind,
            duration,
            centred_on_cut: true,
            params: HashMap::new(),
        }
    }
}

/// A placed clip on the timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    pub track_id: TrackId,
    pub asset_id: AssetId,
    pub timeline_start: Tick,
    pub timeline_duration: Tick,
    pub source_in: Tick,
    pub speed: f32,
    pub pinned: bool,
    pub touched: bool,
    pub enabled: bool,
    pub transition: Option<Transition>,
    /// Effect stacks stored as canonical text descriptions.
    pub effects: Vec<String>,
    pub volume: f32,
}

impl Item {
    pub fn new(
        id: ItemId,
        track_id: TrackId,
        asset_id: AssetId,
        timeline_start: Tick,
        timeline_duration: Tick,
        source_in: Tick,
    ) -> Self {
        Self {
            id,
            track_id,
            asset_id,
            timeline_start,
            timeline_duration,
            source_in,
            speed: 1.0,
            pinned: false,
            touched: false,
            enabled: true,
            transition: None,
            effects: Vec::new(),
            volume: 1.0,
        }
    }

    pub fn timeline_end(&self) -> Tick {
        self.timeline_start + self.timeline_duration
    }
}

/// A timeline marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    pub id: MarkerId,
    pub position: Tick,
    pub label: String,
    pub color: String,
}

/// An entry in the multi-song pool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SongEntry {
    pub id: SongId,
    pub asset_id: AssetId,
    pub title: String,
    pub tempo_bpm: Option<f32>,
    pub downbeat_offset: Tick,
    pub gain: f32,
}

impl SongEntry {
    pub fn new(id: SongId, asset_id: AssetId, title: impl Into<String>) -> Self {
        Self {
            id,
            asset_id,
            title: title.into(),
            tempo_bpm: None,
            downbeat_offset: Tick::ZERO,
            gain: 1.0,
        }
    }
}

/// Placement modes for the montage planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PlacementMode {
    #[default]
    WindowedWalk,
    EvenWindows,
}

/// Grid timing modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum GridMode {
    #[default]
    Transient,
    Grid,
}

/// Controls for the automatic montage generator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationSettings {
    pub clip_variance: f32,
    pub source_reuse_penalty: f32,
    pub spread: f32,
    pub continue_start_points: bool,
    pub placement_mode: PlacementMode,
    pub min_clip_duration: Tick,
    pub max_clip_duration: Tick,
    pub cut_offset: Tick,
    pub snap_to_scenes: bool,
    pub snap_to_dark_edges: bool,
    pub energy_density: f32,
    pub grid_mode: GridMode,
    pub seed: u64,
    pub preset_name: Option<String>,
}

impl Default for GenerationSettings {
    fn default() -> Self {
        Self {
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
            preset_name: None,
        }
    }
}

/// Complete VentaCore Project Model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: u32,
    pub title: String,
    pub assets: Vec<Asset>,
    pub tracks: Vec<Track>,
    pub items: Vec<Item>,
    pub markers: Vec<Marker>,
    pub songs: Vec<SongEntry>,
    pub active_song_id: Option<SongId>,
    pub settings: GenerationSettings,
}

impl Default for Project {
    fn default() -> Self {
        Self::new("Untitled Project")
    }
}

impl Project {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            title: title.into(),
            assets: Vec::new(),
            tracks: Vec::new(),
            items: Vec::new(),
            markers: Vec::new(),
            songs: Vec::new(),
            active_song_id: None,
            settings: GenerationSettings::default(),
        }
    }

    /// Helper to find asset by ID.
    pub fn find_asset(&self, id: AssetId) -> Option<&Asset> {
        self.assets.iter().find(|a| a.id == id)
    }

    /// Helper to find track by ID.
    pub fn find_track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    /// Helper to find item by ID.
    pub fn find_item(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Returns items belonging to a track, sorted by timeline_start.
    pub fn track_items(&self, track_id: TrackId) -> Vec<&Item> {
        let mut list: Vec<&Item> = self.items.iter().filter(|i| i.track_id == track_id).collect();
        list.sort_by_key(|i| i.timeline_start);
        list
    }
}
