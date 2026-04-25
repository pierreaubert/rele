//! # FallbackFrame - Trait Implementations
//!
//! This module contains trait implementations for `FallbackFrame`.
//!
//! ## Implemented Traits
//!
//! - `Drop`
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use super::FALLBACK_STACK;
use super::types::FallbackFrame;

impl Drop for FallbackFrame {
    fn drop(&mut self) {
        FALLBACK_STACK.with(|st| {
            let mut stack = st.borrow_mut();
            if let Some(pos) = stack.iter().rposition(|n| n == &self.name) {
                stack.remove(pos);
            }
        });
    }
}
