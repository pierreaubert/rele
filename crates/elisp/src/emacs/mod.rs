//! Rust equivalents of Emacs C primitives.
//!
//! Each sub-module mirrors an Emacs C source file (`data.c`, `fns.c`, etc.)
//! and contains pure-Rust functions with clean Rust signatures — no
//! dependencies on `LispObject`, `Value`, or editor types.
//!
//! These implementations are the single source of truth for Emacs
//! semantics. The elisp primitive dispatch layer (`primitives.rs`) wraps
//! them with `LispObject` arg/return conversion.
//!
//! Buffer-region variants (e.g. `upcase-region`, `string-match` against a
//! buffer) live in `rele-server`'s own `emacs/` module because they need
//! `DocumentBuffer` / `EditorCursor` which aren't available here.

pub mod casefiddle;
pub mod data;
pub mod fns;
pub mod search;
