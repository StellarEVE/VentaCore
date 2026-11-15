//! Freeze timeline cuts into immutable pinned clips.
//!
//! Conforms to Charter Rule 8: Never change what the user touched.
//! Freezing permanently sets `pinned = true` on all placed clips on a track,
//! protecting them from any subsequent automated re-planning or generation.

use ventacore_core::{Command, CommandError, Project, TrackId};

/// Freezes all currently placed items on the specified track by setting `pinned = true`.
/// Returns the number of items newly pinned.
pub fn freeze_timeline_cuts(
    project: &mut Project,
    track_id: TrackId,
) -> Result<usize, CommandError> {
    let unpinned_ids: Vec<ventacore_core::ItemId> = project
        .items
        .iter()
        .filter(|item| item.track_id == track_id && !item.pinned)
        .map(|item| item.id)
        .collect();

    let count = unpinned_ids.len();

    for id in unpinned_ids {
        project.apply_command(&Command::SetItemPin {
            item_id: id,
            pinned: true,
        })?;
    }

    Ok(count)
}

/// Unfreezes all pinned items on the specified track by setting `pinned = false`.
/// Returns the number of items unpinned.
pub fn unfreeze_timeline_cuts(
    project: &mut Project,
    track_id: TrackId,
) -> Result<usize, CommandError> {
    let pinned_ids: Vec<ventacore_core::ItemId> = project
        .items
        .iter()
        .filter(|item| item.track_id == track_id && item.pinned)
        .map(|item| item.id)
        .collect();

    let count = pinned_ids.len();

    for id in pinned_ids {
        project.apply_command(&Command::SetItemPin {
            item_id: id,
            pinned: false,
        })?;
    }

    Ok(count)
}
