//! File Dialog Helper using RFD on background threads.
//!
//! Conforms to Part 4b & Stage R2:
//! - Never freezes the UI thread by executing native dialogs in background threads.
//! - Configures filters for Audio files, Video files, and All files.
//! - Returns paths through a thread-safe channel.
//! - Supports stand-in test dialog injection for deterministic headless testing.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

#[derive(Debug, Clone)]
pub enum FileDialogResult {
    Song(PathBuf),
    Videos(Vec<PathBuf>),
}

pub struct FileDialogHelper {
    tx: Sender<FileDialogResult>,
    rx: Receiver<FileDialogResult>,
    stand_in: Option<FileDialogResult>,
}

impl Default for FileDialogHelper {
    fn default() -> Self {
        Self::new()
    }
}

impl FileDialogHelper {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            tx,
            rx,
            stand_in: None,
        }
    }

    /// Injects a stand-in result for automated testing without opening OS dialogs.
    pub fn set_stand_in(&mut self, result: FileDialogResult) {
        self.stand_in = Some(result);
    }

    /// Spawns a background file picker for choosing a single song file.
    pub fn pick_song(&mut self) {
        if let Some(result) = self.stand_in.take() {
            let _ = self.tx.send(result);
            return;
        }

        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let dialog = rfd::FileDialog::new()
                .set_title("Browse for Song Audio")
                .add_filter(
                    "Audio Files (*.mp3, *.wav, *.flac, *.aac, *.ogg, *.m4a)",
                    &["mp3", "wav", "flac", "aac", "ogg", "m4a"],
                )
                .add_filter("All Files (*.*)", &["*"]);

            if let Some(path) = dialog.pick_file() {
                let _ = tx.send(FileDialogResult::Song(path));
            }
        });
    }

    /// Spawns a background file picker for choosing one or more video footage files.
    pub fn pick_videos(&mut self) {
        if let Some(result) = self.stand_in.take() {
            let _ = self.tx.send(result);
            return;
        }

        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let dialog = rfd::FileDialog::new()
                .set_title("Add Video Footage Files")
                .add_filter(
                    "Video Files (*.mp4, *.mov, *.mkv, *.webm, *.avi)",
                    &["mp4", "mov", "mkv", "webm", "avi"],
                )
                .add_filter("All Files (*.*)", &["*"]);

            if let Some(paths) = dialog.pick_files() {
                let _ = tx.send(FileDialogResult::Videos(paths));
            }
        });
    }

    /// Polls for completed dialog results from background threads.
    pub fn poll(&self) -> Option<FileDialogResult> {
        self.rx.try_recv().ok()
    }
}
