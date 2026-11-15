//! Project validation and invariant checking for VentaCore.
//!
//! Enforces:
//! - Unique item IDs, track IDs, and asset IDs across the project.
//! - Positive item durations (`timeline_duration > 0`).
//! - Every item points to an existing asset.
//! - Every item points to an existing track.
//! - Items on the same track never overlap.
//! - Non-negative intro and outro trims.
//! - Valid transition durations (must not exceed item duration).

use crate::model::*;
use crate::time::Tick;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Clone)]
pub enum ValidationError {
    #[error("Duplicate asset ID: {0:?}")]
    DuplicateAssetId(AssetId),

    #[error("Duplicate track ID: {0:?}")]
    DuplicateTrackId(TrackId),

    #[error("Duplicate item ID: {0:?}")]
    DuplicateItemId(ItemId),

    #[error("Item {item_id:?} has non-positive duration: {duration:?}")]
    NonPositiveItemDuration { item_id: ItemId, duration: Tick },

    #[error("Item {item_id:?} references unknown asset {asset_id:?}")]
    UnknownAsset { item_id: ItemId, asset_id: AssetId },

    #[error("Item {item_id:?} references unknown track {track_id:?}")]
    UnknownTrack { item_id: ItemId, track_id: TrackId },

    #[error(
        "Overlap detected on track {track_id:?} between item {item1:?} (ends {end1}) and item {item2:?} (starts {start2})"
    )]
    TrackItemOverlap {
        track_id: TrackId,
        item1: ItemId,
        end1: Tick,
        item2: ItemId,
        start2: Tick,
    },

    #[error("Negative intro or outro trim on asset {asset_id:?}")]
    NegativeAssetTrim { asset_id: AssetId },

    #[error(
        "Transition on item {item_id:?} has duration {trans_dur} exceeding item duration {item_dur}"
    )]
    TransitionTooLong {
        item_id: ItemId,
        trans_dur: Tick,
        item_dur: Tick,
    },

    #[error("Active song {0:?} does not exist in songs pool")]
    UnknownActiveSong(SongId),

    // R1: Roles and dedicated song track validation errors
    #[error("Song track {track_id:?} has invalid item count: expected {expected}, found {actual}")]
    SongTrackItemCountMismatch {
        track_id: TrackId,
        expected: usize,
        actual: usize,
    },

    #[error("Item {item_id:?} on footage track {track_id:?} references song asset {asset_id:?}")]
    FootageTrackReferencesSongAsset {
        item_id: ItemId,
        track_id: TrackId,
        asset_id: AssetId,
    },

    #[error("Song item {item_id:?} placed on non-song track {track_id:?}")]
    SongItemOnNonSongTrack {
        item_id: ItemId,
        track_id: TrackId,
    },

    #[error("Item {item_id:?} ends at {end}, which is past the song duration {song_duration}")]
    ItemEndsPastSongRange {
        item_id: ItemId,
        end: Tick,
        song_duration: Tick,
    },

    #[error("Active song asset {0:?} does not exist or is not marked as Song")]
    InvalidSongAsset(AssetId),

    #[error("Non-positive song range: start {start} >= end {end}")]
    NonPositiveSongRange { start: Tick, end: Tick },
}

impl Project {
    /// Validates all project invariants. Returns `Ok(())` if fully valid.
    pub fn validate(&self) -> Result<(), ValidationError> {
        // 1. Unique Asset IDs and non-negative trims
        let mut seen_assets = HashSet::new();
        for asset in &self.assets {
            if !seen_assets.insert(asset.id) {
                return Err(ValidationError::DuplicateAssetId(asset.id));
            }
            if asset.intro_trim < Tick::ZERO || asset.outro_trim < Tick::ZERO {
                return Err(ValidationError::NegativeAssetTrim {
                    asset_id: asset.id,
                });
            }
        }

        // 2. Unique Track IDs
        let mut seen_tracks = HashSet::new();
        for track in &self.tracks {
            if !seen_tracks.insert(track.id) {
                return Err(ValidationError::DuplicateTrackId(track.id));
            }
        }

        // 3. Unique Item IDs, positive duration, asset/track existence
        let mut seen_items = HashSet::new();
        for item in &self.items {
            if !seen_items.insert(item.id) {
                return Err(ValidationError::DuplicateItemId(item.id));
            }
            if item.timeline_duration <= Tick::ZERO {
                return Err(ValidationError::NonPositiveItemDuration {
                    item_id: item.id,
                    duration: item.timeline_duration,
                });
            }
            if !seen_assets.contains(&item.asset_id) {
                return Err(ValidationError::UnknownAsset {
                    item_id: item.id,
                    asset_id: item.asset_id,
                });
            }
            if !seen_tracks.contains(&item.track_id) {
                return Err(ValidationError::UnknownTrack {
                    item_id: item.id,
                    track_id: item.track_id,
                });
            }
            if let Some(trans) = &item.transition {
                if trans.duration > item.timeline_duration {
                    return Err(ValidationError::TransitionTooLong {
                        item_id: item.id,
                        trans_dur: trans.duration,
                        item_dur: item.timeline_duration,
                    });
                }
            }
        }

        // 4. No overlapping items on any track
        for track in &self.tracks {
            let mut track_items: Vec<&Item> =
                self.items.iter().filter(|i| i.track_id == track.id).collect();
            track_items.sort_by_key(|i| i.timeline_start);

            for window in track_items.windows(2) {
                let first = window[0];
                let second = window[1];
                if first.timeline_end() > second.timeline_start {
                    return Err(ValidationError::TrackItemOverlap {
                        track_id: track.id,
                        item1: first.id,
                        end1: first.timeline_end(),
                        item2: second.id,
                        start2: second.timeline_start,
                    });
                }
            }
        }

        // 5. Active song references a valid song entry
        if let Some(song_id) = self.active_song_id {
            if !self.songs.iter().any(|s| s.id == song_id) {
                return Err(ValidationError::UnknownActiveSong(song_id));
            }
        }

        // 6. R1 Invariant: Dedicated Song Slot & Song Track Validation
        if let Some(song) = &self.song {
            if song.end <= song.start {
                return Err(ValidationError::NonPositiveSongRange {
                    start: song.start,
                    end: song.end,
                });
            }
            let song_asset = self.find_asset(song.asset_id).ok_or(ValidationError::InvalidSongAsset(song.asset_id))?;
            if song_asset.role != AssetRole::Song {
                return Err(ValidationError::InvalidSongAsset(song.asset_id));
            }

            let song_dur = song.duration();

            // Validate that the Song track holds only the active song as exactly one item
            for track in &self.tracks {
                if track.kind == TrackKind::Song {
                    let track_items = self.track_items(track.id);
                    if track_items.len() != 1 {
                        return Err(ValidationError::SongTrackItemCountMismatch {
                            track_id: track.id,
                            expected: 1,
                            actual: track_items.len(),
                        });
                    }
                    let song_item = track_items[0];
                    if song_item.asset_id != song.asset_id {
                        return Err(ValidationError::InvalidSongAsset(song_item.asset_id));
                    }
                } else {
                    // Non-song tracks: must never hold song asset or end past song duration
                    for item in self.track_items(track.id) {
                        let item_asset = self.find_asset(item.asset_id);
                        if let Some(a) = item_asset {
                            if a.role == AssetRole::Song || a.id == song.asset_id {
                                return Err(ValidationError::FootageTrackReferencesSongAsset {
                                    item_id: item.id,
                                    track_id: track.id,
                                    asset_id: a.id,
                                });
                            }
                        }
                        if item.timeline_end() > song_dur {
                            return Err(ValidationError::ItemEndsPastSongRange {
                                item_id: item.id,
                                end: item.timeline_end(),
                                song_duration: song_dur,
                            });
                        }
                    }
                }
            }
        } else {
            // When no song is active, any Song track must have 0 items
            for track in &self.tracks {
                if track.kind == TrackKind::Song {
                    let track_items = self.track_items(track.id);
                    if !track_items.is_empty() {
                        return Err(ValidationError::SongTrackItemCountMismatch {
                            track_id: track.id,
                            expected: 0,
                            actual: track_items.len(),
                        });
                    }
                } else {
                    // Non-song tracks must never hold song assets
                    for item in self.track_items(track.id) {
                        if let Some(a) = self.find_asset(item.asset_id) {
                            if a.role == AssetRole::Song {
                                return Err(ValidationError::FootageTrackReferencesSongAsset {
                                    item_id: item.id,
                                    track_id: track.id,
                                    asset_id: a.id,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_project_passes() {
        let mut proj = Project::new("Test");
        let asset = Asset::new(AssetId(1), "test.mp4".into());
        let track = Track::new(TrackId(1), "Video 1", TrackKind::Video);
        let item = Item::new(
            ItemId(1),
            TrackId(1),
            AssetId(1),
            Tick(0),
            Tick::from_seconds(2),
            Tick(0),
        );

        proj.assets.push(asset);
        proj.tracks.push(track);
        proj.items.push(item);

        assert!(proj.validate().is_ok());
    }

    #[test]
    fn test_overlap_rejected() {
        let mut proj = Project::new("Test Overlap");
        proj.assets.push(Asset::new(AssetId(1), "video.mp4".into()));
        proj.tracks.push(Track::new(TrackId(1), "V1", TrackKind::Video));

        proj.items.push(Item::new(
            ItemId(1),
            TrackId(1),
            AssetId(1),
            Tick(0),
            Tick::from_seconds(3),
            Tick(0),
        ));
        proj.items.push(Item::new(
            ItemId(2),
            TrackId(1),
            AssetId(1),
            Tick::from_seconds(2), // Starts before item 1 ends!
            Tick::from_seconds(2),
            Tick(0),
        ));

        let res = proj.validate();
        assert!(matches!(res, Err(ValidationError::TrackItemOverlap { .. })));
    }
}
