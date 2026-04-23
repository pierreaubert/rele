//! A `Send + Sync` wrapper around `RefCell` for single-threaded
//! interior mutability that panics instead of deadlocking.
//!
//! The elisp evaluator is single-threaded: only one thread calls
//! `eval()` at a time. `parking_lot::RwLock` was used only to satisfy
//! Rust's borrow checker, but it deadlocks on re-entrant access (e.g.
//! bytecode VM holding a read lock, then calling eval which tries to
//! write-lock the same environment). This wrapper replaces it:
//!
//! - Zero lock overhead (no OS futex, no atomic CAS)
//! - **Panics** on re-entrant borrow (debuggable stack trace)
//! - `Send + Sync` so `InterpreterState` can cross thread boundaries
//!   (only `AtomicU64` fields are touched from the watchdog thread)

use std::cell::{Ref, RefCell, RefMut};

/// A `RefCell` that is `Send + Sync`. Only safe when access is
/// single-threaded (which it is for our eval engine).
///
/// Unlike `parking_lot::RwLock`, this **panics** on re-entrant access
/// instead of deadlocking — giving a clear stack trace for debugging.
/// For the eval hot path, `try_read()`/`try_write()` return `None`
/// on contention so the caller can handle it gracefully.
pub struct SyncRefCell<T>(RefCell<T>);

// SAFETY: The evaluator is single-threaded. The SyncRefCell is only
// accessed from the eval thread. Other threads (watchdog) only touch
// AtomicU64 fields on InterpreterState, never the SyncRefCell fields.
unsafe impl<T: Send> Send for SyncRefCell<T> {}
unsafe impl<T: Send> Sync for SyncRefCell<T> {}

impl<T> SyncRefCell<T> {
    pub fn new(value: T) -> Self {
        SyncRefCell(RefCell::new(value))
    }

    /// Immutable borrow. Panics if already mutably borrowed.
    #[inline]
    pub fn read(&self) -> Ref<'_, T> {
        self.0.borrow()
    }

    /// Mutable borrow. Panics if already borrowed (read or write).
    #[inline]
    pub fn write(&self) -> RefMut<'_, T> {
        self.0.borrow_mut()
    }

    /// Alias for `write()` — drop-in replacement for `Mutex::lock()`.
    #[inline]
    pub fn lock(&self) -> RefMut<'_, T> {
        self.0.borrow_mut()
    }

    /// Non-panicking read. Returns `None` if already mutably borrowed.
    #[inline]
    pub fn try_read(&self) -> Option<Ref<'_, T>> {
        self.0.try_borrow().ok()
    }

    /// Non-panicking write. Returns `None` if already borrowed.
    #[inline]
    pub fn try_write(&self) -> Option<RefMut<'_, T>> {
        self.0.try_borrow_mut().ok()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for SyncRefCell<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0.try_borrow() {
            Ok(v) => write!(f, "SyncRefCell({:?})", &*v),
            Err(_) => write!(f, "SyncRefCell(<borrowed>)"),
        }
    }
}

impl<T: Clone> Clone for SyncRefCell<T> {
    fn clone(&self) -> Self {
        SyncRefCell(RefCell::new(self.0.borrow().clone()))
    }
}
