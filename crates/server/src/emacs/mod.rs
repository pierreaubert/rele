//! Rust implementations of Emacs C primitives that need editor state
//! (`DocumentBuffer`, `EditorCursor`). Pure-Rust counterparts (data.c,
//! fns.c, and the char/string parts of casefiddle.c and search.c) now
//! live in the `rele-elisp` crate as `rele_elisp::emacs::*` so the
//! interpreter can use them without a circular dependency.

pub mod casefiddle;
pub mod editfns;
pub mod search;

// Re-export the pure modules from rele-elisp so callers that reference
// `rele_server::emacs::data::*` or `rele_server::emacs::fns::*` keep
// working.
pub use rele_elisp::emacs::data;
pub use rele_elisp::emacs::fns;
