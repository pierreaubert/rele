//! Primitives module - organized by domain
//!
//! Contains all built-in Lisp primitives organized by domain:
//! - buffer: buffer operations
//! - cl: Common Lisp extensions
//! - core: core primitives (math, list, string, predicates, etc.)
//! - eieio: EIEIO (CLOS-like) object system
//! - file: file and filesystem operations
//! - modules: module/foreign function interface
//! - value: value/hash table operations
//! - window: window/frame operations

pub mod buffer;
pub mod cl;
pub mod core;
pub mod eieio;
pub mod file;
pub mod modules;
pub mod value;
pub mod window;

pub use core::{add_primitives, call_primitive};

// Re-export ERT functions that are used elsewhere
pub use core::{make_ert_test_obj, set_current_ert_test};

// Re-export submodules with legacy names for compatibility
pub use buffer as primitives_buffer;
pub use cl as primitives_cl;
pub use eieio as primitives_eieio;
pub use file as primitives_file;
pub use modules as primitives_modules;
pub use value as primitives_value;
pub use window as primitives_window;
