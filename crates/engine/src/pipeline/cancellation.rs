//! Thread-safe cooperative cancellation token for long-running pipeline tasks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A thread-safe cancellation handle that can be passed across background worker threads.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Signals cancellation to all holders of this token.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Returns true if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Resets the cancellation state back to active.
    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }
}
