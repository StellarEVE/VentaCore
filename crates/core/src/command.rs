//! Command pattern implementation for VentaCore.
//!
//! Conforms to Charter Rule 3:
//! Every change is a command value applied by one path:
//! 1. Clone state.
//! 2. Apply change.
//! 3. Validate against all project invariants.
//! 4. Commit candidate if valid; reject and discard if invalid.

use crate::model::*;
use crate::time::{Tick, TimeRange};
use crate::validation::ValidationError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Clone)]
pub enum CommandError {
    #[error("Validation failed: {0}")]
    Validation(#[from] ValidationError),

    #[error("Asset {0:?} not found")]
    AssetNotFound(AssetId),

    #[error("Track {0:?} not found")]
    TrackNotFound(TrackId),

    #[error("Item {0:?} not found")]
    ItemNotFound(ItemId),

    #[error("Track {0:?} is locked")]
    TrackLocked(TrackId),

    #[error("Split point {split_point} is not strictly inside item {item_id:?} [{start}, {end})")]
    InvalidSplitPoint {
        item_id: ItemId,
        split_point: Tick,
        start: Tick,
        end: Tick,
    },

    #[error("Marker {0:?} not found")]
    MarkerNotFound(MarkerId),

    #[error("Song {0:?} not found")]
    SongNotFound(SongId),
}

/// All discrete user and engine editing operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    AddAsset(Asset),
    RemoveAsset(AssetId),
    SetAssetTrims {
        asset_id: AssetId,
        intro: Tick,
        outro: Tick,
    },
    AddIncludeRange {
        asset_id: AssetId,
        range: TimeRange,
    },
    AddExcludeRange {
        asset_id: AssetId,
        range: TimeRange,
    },
    AddFavouriteMoment {
        asset_id: AssetId,
        range: TimeRange,
    },
    AddTrack(Track),
    RemoveTrack(TrackId),
    SetTrackMute {
        track_id: TrackId,
        muted: bool,
    },
    SetTrackSolo {
        track_id: TrackId,
        solo: bool,
    },
    SetTrackLock {
        track_id: TrackId,
        locked: bool,
    },
    InsertItem(Item),
    RemoveItem(ItemId),
    MoveItem {
        item_id: ItemId,
        new_start: Tick,
    },
    TrimItem {
        item_id: ItemId,
        new_source_in: Tick,
        new_duration: Tick,
    },
    SplitItem {
        item_id: ItemId,
        split_point: Tick,
        new_item_id: ItemId,
    },
    SetItemPin {
        item_id: ItemId,
        pinned: bool,
    },
    SetItemTransition {
        item_id: ItemId,
        transition: Option<Transition>,
    },
    SetItemEffects {
        item_id: ItemId,
        effects: Vec<String>,
    },
    AddMarker(Marker),
    RemoveMarker(MarkerId),
    AddSong(SongEntry),
    RemoveSong(SongId),
    SetActiveSong(Option<SongId>),
    UpdateSettings(GenerationSettings),

    // R1: Roles, dedicated song slot and footage management commands
    SetSong {
        asset_id: AssetId,
        start: Tick,
        end: Tick,
        policy: ReplaceSongPolicy,
    },
    ClearSong,
    SetSongRange {
        start: Tick,
        end: Tick,
    },
    AddFootage(Asset),
    RemoveFootage(AssetId),
}

impl Project {
    /// Applies an edit command by copying state, applying, validating, and committing.
    /// If validation fails, `self` is guaranteed to be untouched.
    pub fn apply_command(&mut self, cmd: &Command) -> Result<(), CommandError> {
        let mut candidate = self.clone();
        candidate.mutate(cmd)?;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    fn mutate(&mut self, cmd: &Command) -> Result<(), CommandError> {
        match cmd {
            Command::AddAsset(asset) => {
                self.assets.push(asset.clone());
            }
            Command::RemoveAsset(asset_id) => {
                self.assets.retain(|a| a.id != *asset_id);
                // Also remove items and songs referencing this asset
                self.items.retain(|i| i.asset_id != *asset_id);
                self.songs.retain(|s| s.asset_id != *asset_id);
            }
            Command::SetAssetTrims {
                asset_id,
                intro,
                outro,
            } => {
                let asset = self
                    .assets
                    .iter_mut()
                    .find(|a| a.id == *asset_id)
                    .ok_or(CommandError::AssetNotFound(*asset_id))?;
                asset.intro_trim = *intro;
                asset.outro_trim = *outro;
            }
            Command::AddIncludeRange { asset_id, range } => {
                let asset = self
                    .assets
                    .iter_mut()
                    .find(|a| a.id == *asset_id)
                    .ok_or(CommandError::AssetNotFound(*asset_id))?;
                asset.include_ranges.push(*range);
            }
            Command::AddExcludeRange { asset_id, range } => {
                let asset = self
                    .assets
                    .iter_mut()
                    .find(|a| a.id == *asset_id)
                    .ok_or(CommandError::AssetNotFound(*asset_id))?;
                asset.exclude_ranges.push(*range);
            }
            Command::AddFavouriteMoment { asset_id, range } => {
                let asset = self
                    .assets
                    .iter_mut()
                    .find(|a| a.id == *asset_id)
                    .ok_or(CommandError::AssetNotFound(*asset_id))?;
                asset.favourite_moments.push(*range);
            }
            Command::AddTrack(track) => {
                self.tracks.push(track.clone());
            }
            Command::RemoveTrack(track_id) => {
                self.tracks.retain(|t| t.id != *track_id);
                self.items.retain(|i| i.track_id != *track_id);
            }
            Command::SetTrackMute { track_id, muted } => {
                let track = self
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == *track_id)
                    .ok_or(CommandError::TrackNotFound(*track_id))?;
                track.muted = *muted;
            }
            Command::SetTrackSolo { track_id, solo } => {
                let track = self
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == *track_id)
                    .ok_or(CommandError::TrackNotFound(*track_id))?;
                track.solo = *solo;
            }
            Command::SetTrackLock { track_id, locked } => {
                let track = self
                    .tracks
                    .iter_mut()
                    .find(|t| t.id == *track_id)
                    .ok_or(CommandError::TrackNotFound(*track_id))?;
                track.locked = *locked;
            }
            Command::InsertItem(item) => {
                let track = self
                    .tracks
                    .iter()
                    .find(|t| t.id == item.track_id)
                    .ok_or(CommandError::TrackNotFound(item.track_id))?;
                if track.locked {
                    return Err(CommandError::TrackLocked(track.id));
                }
                self.items.push(item.clone());
            }
            Command::RemoveItem(item_id) => {
                let item = self
                    .items
                    .iter()
                    .find(|i| i.id == *item_id)
                    .ok_or(CommandError::ItemNotFound(*item_id))?;
                let track = self
                    .tracks
                    .iter()
                    .find(|t| t.id == item.track_id)
                    .ok_or(CommandError::TrackNotFound(item.track_id))?;
                if track.locked {
                    return Err(CommandError::TrackLocked(track.id));
                }
                self.items.retain(|i| i.id != *item_id);
            }
            Command::MoveItem { item_id, new_start } => {
                let item = self
                    .items
                    .iter_mut()
                    .find(|i| i.id == *item_id)
                    .ok_or(CommandError::ItemNotFound(*item_id))?;
                let track_id = item.track_id;
                let track = self
                    .tracks
                    .iter()
                    .find(|t| t.id == track_id)
                    .ok_or(CommandError::TrackNotFound(track_id))?;
                if track.locked {
                    return Err(CommandError::TrackLocked(track.id));
                }
                item.timeline_start = *new_start;
                item.touched = true;
            }
            Command::TrimItem {
                item_id,
                new_source_in,
                new_duration,
            } => {
                let item = self
                    .items
                    .iter_mut()
                    .find(|i| i.id == *item_id)
                    .ok_or(CommandError::ItemNotFound(*item_id))?;
                let track_id = item.track_id;
                let track = self
                    .tracks
                    .iter()
                    .find(|t| t.id == track_id)
                    .ok_or(CommandError::TrackNotFound(track_id))?;
                if track.locked {
                    return Err(CommandError::TrackLocked(track.id));
                }
                item.source_in = *new_source_in;
                item.timeline_duration = *new_duration;
                item.touched = true;
            }
            Command::SplitItem {
                item_id,
                split_point,
                new_item_id,
            } => {
                let (idx, track_id) = {
                    let (idx, item) = self
                        .items
                        .iter()
                        .enumerate()
                        .find(|(_, i)| i.id == *item_id)
                        .ok_or(CommandError::ItemNotFound(*item_id))?;
                    if *split_point <= item.timeline_start || *split_point >= item.timeline_end() {
                        return Err(CommandError::InvalidSplitPoint {
                            item_id: *item_id,
                            split_point: *split_point,
                            start: item.timeline_start,
                            end: item.timeline_end(),
                        });
                    }
                    (idx, item.track_id)
                };

                let track = self
                    .tracks
                    .iter()
                    .find(|t| t.id == track_id)
                    .ok_or(CommandError::TrackNotFound(track_id))?;
                if track.locked {
                    return Err(CommandError::TrackLocked(track.id));
                }

                let left = &mut self.items[idx];
                let orig_start = left.timeline_start;
                let orig_duration = left.timeline_duration;
                let orig_source_in = left.source_in;
                let orig_speed = left.speed;
                let orig_asset_id = left.asset_id;

                let left_duration = *split_point - orig_start;
                let right_duration = orig_duration - left_duration;
                let right_source_in = orig_source_in + left_duration;

                left.timeline_duration = left_duration;
                left.touched = true;

                let mut right = Item::new(
                    *new_item_id,
                    track_id,
                    orig_asset_id,
                    *split_point,
                    right_duration,
                    right_source_in,
                );
                right.speed = orig_speed;
                right.touched = true;

                self.items.push(right);
            }
            Command::SetItemPin { item_id, pinned } => {
                let item = self
                    .items
                    .iter_mut()
                    .find(|i| i.id == *item_id)
                    .ok_or(CommandError::ItemNotFound(*item_id))?;
                item.pinned = *pinned;
            }
            Command::SetItemTransition {
                item_id,
                transition,
            } => {
                let item = self
                    .items
                    .iter_mut()
                    .find(|i| i.id == *item_id)
                    .ok_or(CommandError::ItemNotFound(*item_id))?;
                item.transition = transition.clone();
                item.touched = true;
            }
            Command::SetItemEffects { item_id, effects } => {
                let item = self
                    .items
                    .iter_mut()
                    .find(|i| i.id == *item_id)
                    .ok_or(CommandError::ItemNotFound(*item_id))?;
                item.effects = effects.clone();
                item.touched = true;
            }
            Command::AddMarker(marker) => {
                self.markers.push(marker.clone());
            }
            Command::RemoveMarker(marker_id) => {
                self.markers.retain(|m| m.id != *marker_id);
            }
            Command::AddSong(song) => {
                self.songs.push(song.clone());
            }
            Command::RemoveSong(song_id) => {
                self.songs.retain(|s| s.id != *song_id);
                if self.active_song_id == Some(*song_id) {
                    self.active_song_id = None;
                }
            }
            Command::SetActiveSong(song_id) => {
                self.active_song_id = *song_id;
            }
            Command::UpdateSettings(settings) => {
                self.settings = settings.clone();
            }

            // R1: Roles, dedicated song slot and footage management commands
            Command::SetSong {
                asset_id,
                start,
                end,
                policy,
            } => {
                let asset = self
                    .assets
                    .iter_mut()
                    .find(|a| a.id == *asset_id)
                    .ok_or(CommandError::AssetNotFound(*asset_id))?;
                asset.role = AssetRole::Song;

                // Ensure fixed Song track exists
                let song_track_id = if let Some(t) = self.song_track() {
                    t.id
                } else {
                    let tid = TrackId(0);
                    self.tracks.insert(0, Track::new(tid, "Song", TrackKind::Song));
                    tid
                };
                self.song_track_id = Some(song_track_id);

                let song_dur = (*end).saturating_sub(*start);

                // Handle replacement policy for existing items
                match policy {
                    ReplaceSongPolicy::StartOver => {
                        // Clear all non-song items
                        self.items.retain(|i| i.track_id == song_track_id);
                    }
                    ReplaceSongPolicy::KeepPinnedAndTouched => {
                        // Remove unpinned and untouched items; keep pinned or touched items
                        let mut leftovers = Vec::new();
                        self.items.retain(|i| {
                            if i.track_id == song_track_id {
                                return true;
                            }
                            if i.pinned || i.touched {
                                if i.timeline_end() > song_dur {
                                    leftovers.push(i.clone());
                                    false
                                } else {
                                    true
                                }
                            } else {
                                false
                            }
                        });
                        self.migration_leftovers.extend(leftovers);
                    }
                }

                // Remove existing song item on Song track
                self.items.retain(|i| i.track_id != song_track_id);

                // Insert the new single song item on the Song track
                let next_item_id = ItemId(self.items.iter().map(|i| i.id.0).max().unwrap_or(0) + 1);
                let song_item = Item::new(
                    next_item_id,
                    song_track_id,
                    *asset_id,
                    Tick::ZERO,
                    song_dur,
                    *start,
                );
                self.items.push(song_item);
                self.song = Some(SongSlot::new(*asset_id, *start, *end));
            }
            Command::ClearSong => {
                let song_track_id = self.song_track().map(|t| t.id);
                if let Some(stid) = song_track_id {
                    self.items.retain(|i| i.track_id != stid);
                }
                self.song = None;
            }
            Command::SetSongRange { start, end } => {
                if let Some(song) = &mut self.song {
                    song.start = *start;
                    song.end = *end;
                    let song_dur = (*end).saturating_sub(*start);
                    let song_track_id = self.song_track().map(|t| t.id);
                    if let Some(stid) = song_track_id {
                        if let Some(item) = self.items.iter_mut().find(|i| i.track_id == stid) {
                            item.source_in = *start;
                            item.timeline_duration = song_dur;
                        }

                        // Protect against items extending past song duration
                        let mut leftovers = Vec::new();
                        self.items.retain(|i| {
                            if i.track_id == stid {
                                true
                            } else if i.timeline_end() > song_dur {
                                leftovers.push(i.clone());
                                false
                            } else {
                                true
                            }
                        });
                        self.migration_leftovers.extend(leftovers);
                    }
                }
            }
            Command::AddFootage(asset) => {
                let mut a = asset.clone();
                a.role = AssetRole::Footage;
                self.assets.push(a);
            }
            Command::RemoveFootage(asset_id) => {
                self.assets.retain(|a| a.id != *asset_id);
                self.items.retain(|i| i.asset_id != *asset_id);
            }
        }
        Ok(())
    }
}
