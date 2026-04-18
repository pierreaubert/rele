//! Cancellation token used by long-running commands (grep, large search,
//! whole-buffer format). Backed by an `Arc<AtomicBool>` so it can be
//! cloned cheaply into spawned tasks and checked from any thread without
//! locking.
//!
//! See `PERFORMANCE.md` Rule 7.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Shared cancellation flag. Cheap to clone; calling `cancel()` on any
/// clone flips the flag for all observers.
#[derive(Clone, Debug, Default)]
pub struct CancellationFlag {
    flag: Arc<AtomicBool>,
}

impl CancellationFlag {
    /// Create a new flag in the "not cancelled" state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Request cancellation. Idempotent — calling again after the flag
    /// is already set is a no-op.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    /// Has cancellation been requested?
    ///
    /// Long-running loops should call this at iteration boundaries
    /// (per-line, per-file, per-chunk) and break out when it returns
    /// true.
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    /// Reset the flag back to "not cancelled". Called by the UI when a
    /// new long-running operation starts, so the previous `C-g` doesn't
    /// immediately cancel it.
    pub fn reset(&self) {
        self.flag.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_not_cancelled() {
        let f = CancellationFlag::new();
        assert!(!f.is_cancelled());
    }

    #[test]
    fn cancel_propagates_across_clones() {
        let f1 = CancellationFlag::new();
        let f2 = f1.clone();
        assert!(!f2.is_cancelled());
        f1.cancel();
        assert!(f2.is_cancelled());
    }

    #[test]
    fn reset_clears_flag() {
        let f = CancellationFlag::new();
        f.cancel();
        assert!(f.is_cancelled());
        f.reset();
        assert!(!f.is_cancelled());
    }
}
