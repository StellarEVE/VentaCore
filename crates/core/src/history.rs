//! Snapshot-based undo and redo history for VentaCore.
//!
//! Stores exact state snapshots. Every accepted command pushes the current
//! state onto the undo stack and clears the redo stack.

use crate::command::{Command, CommandError};
use crate::model::Project;

pub struct History {
    past: Vec<Project>,
    present: Project,
    future: Vec<Project>,
    max_depth: usize,
}

impl History {
    pub fn new(initial: Project) -> Self {
        Self {
            past: Vec::new(),
            present: initial,
            future: Vec::new(),
            max_depth: 100,
        }
    }

    pub fn with_max_depth(initial: Project, max_depth: usize) -> Self {
        Self {
            past: Vec::new(),
            present: initial,
            future: Vec::new(),
            max_depth,
        }
    }

    pub fn present(&self) -> &Project {
        &self.present
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn undo_stack_depth(&self) -> usize {
        self.past.len()
    }

    pub fn redo_stack_depth(&self) -> usize {
        self.future.len()
    }

    /// Applies a command to the current project.
    ///
    /// If the command is valid, the current state is pushed to the undo stack,
    /// the redo stack is cleared, and the new state becomes current.
    /// If the command fails, the project state and history are untouched.
    pub fn apply_command(&mut self, cmd: &Command) -> Result<(), CommandError> {
        let mut candidate = self.present.clone();
        candidate.apply_command(cmd)?;

        self.past.push(std::mem::replace(&mut self.present, candidate));
        self.future.clear();

        if self.past.len() > self.max_depth {
            self.past.remove(0);
        }

        Ok(())
    }

    /// Pushes a new project state onto the undo stack and clears the redo stack.
    /// Used for atomic batch operations.
    pub fn record(&mut self, new_state: Project) {
        self.past.push(std::mem::replace(&mut self.present, new_state));
        self.future.clear();

        if self.past.len() > self.max_depth {
            self.past.remove(0);
        }
    }

    /// Reverts to the previous project state.
    /// Returns `true` if an undo was performed, `false` if undo stack was empty.
    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.past.pop() {
            let current = std::mem::replace(&mut self.present, prev);
            self.future.push(current);
            true
        } else {
            false
        }
    }

    /// Restores the next project state.
    /// Returns `true` if a redo was performed, `false` if redo stack was empty.
    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.future.pop() {
            let current = std::mem::replace(&mut self.present, next);
            self.past.push(current);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[test]
    fn test_undo_and_redo_restore_exact_earlier_states() {
        let initial = Project::new("Initial");
        let mut history = History::new(initial.clone());

        assert_eq!(history.present(), &initial);
        assert!(!history.can_undo());
        assert!(!history.can_redo());

        // Command 1: Add Asset
        let asset = Asset::new(AssetId(1), "clip.mp4".into());
        history
            .apply_command(&Command::AddAsset(asset.clone()))
            .unwrap();

        assert!(history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.present().assets.len(), 1);

        // Command 2: Add Track
        let track = Track::new(TrackId(1), "Video Track", TrackKind::Video);
        history
            .apply_command(&Command::AddTrack(track.clone()))
            .unwrap();

        assert_eq!(history.present().tracks.len(), 1);

        // Undo once -> Track should be gone, asset still present
        assert!(history.undo());
        assert_eq!(history.present().tracks.len(), 0);
        assert_eq!(history.present().assets.len(), 1);
        assert!(history.can_redo());

        // Undo again -> Back to exact initial state
        assert!(history.undo());
        assert_eq!(history.present(), &initial);
        assert!(!history.can_undo());

        // Redo once -> Asset back
        assert!(history.redo());
        assert_eq!(history.present().assets.len(), 1);
        assert_eq!(history.present().tracks.len(), 0);

        // Redo again -> Track back
        assert!(history.redo());
        assert_eq!(history.present().assets.len(), 1);
        assert_eq!(history.present().tracks.len(), 1);
        assert!(!history.can_redo());
    }

    #[test]
    fn test_invalid_command_does_not_pollute_history() {
        let initial = Project::new("Initial");
        let mut history = History::new(initial);

        // Try inserting an item that references nonexistent asset/track
        let bad_item = Item::new(
            ItemId(99),
            TrackId(99),
            AssetId(99),
            crate::time::Tick(0),
            crate::time::Tick::from_seconds(1),
            crate::time::Tick(0),
        );

        let res = history.apply_command(&Command::InsertItem(bad_item));
        assert!(res.is_err());
        assert!(!history.can_undo());
        assert_eq!(history.undo_stack_depth(), 0);
    }
}
