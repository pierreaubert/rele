//! Rust implementations of Emacs C primitives.
//!
//! Each sub-module mirrors an Emacs C source file (`data.c`, `fns.c`, etc.)
//! and contains pure-Rust functions with clean Rust signatures.
//!
//! These implementations are independent of the `rele-elisp` crate's
//! `LispObject` type. The elisp crate will wrap them as Lisp primitives
//! once the stub layer stabilizes.

pub mod casefiddle;
pub mod data;
pub mod editfns;
pub mod fns;
pub mod search;
