//! # InterpreterState - Trait Implementations
//!
//! This module contains trait implementations for `InterpreterState`.
//!
//! ## Implemented Traits
//!
//! - `Clone`
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use super::types::InterpreterState;

impl Clone for InterpreterState {
    fn clone(&self) -> Self {
        InterpreterState {
            features: self.features.clone(),
            profiler: self.profiler.clone(),
            #[cfg(feature = "jit")]
            jit: self.jit.clone(),
            special_vars: self.special_vars.clone(),
            specpdl: self.specpdl.clone(),
            global_env: self.global_env.clone(),
            symbol_cells: self.symbol_cells.clone(),
            heap: self.heap.clone(),
            cons_count: self.cons_count.clone(),
            autoloads: self.autoloads.clone(),
            eval_ops: self.eval_ops.clone(),
            eval_ops_limit: self.eval_ops_limit.clone(),
            deadline: std::cell::Cell::new(self.deadline.get()),
        }
    }
}
