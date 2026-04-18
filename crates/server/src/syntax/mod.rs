//! Syntax highlighting infrastructure.
//!
//! This module defines the abstraction the editor view uses to colourise
//! buffer content. Today the only implementation is a thin wrapper around
//! the existing regex-based markdown highlighter in
//! `crates/app-gpui/src/markdown/syntax_highlight.rs`; the plan
//! (`PERFORMANCE.md` Rule 4, and the tree-sitter follow-up project) is to
//! add a `TreeSitter` impl that:
//!
//! 1. Holds one `tree_sitter::Tree` per buffer.
//! 2. On edit, consumes `DocumentBuffer::drain_changes()` to build
//!    `InputEdit`s and re-parses incrementally.
//! 3. Runs highlight queries over the tree, scoped to the visible range.
//!
//! Landing the trait first means the editor view already talks to an
//! abstraction — the tree-sitter switch becomes additive, not a rewrite
//! of every call site.
//!
//! The tree-sitter work is descoped from the initial perf PR because it
//! adds C-toolchain build dependencies (`cc`, per-language grammar
//! crates) that deserve their own review.

pub mod highlight;
pub mod line_adapter;
pub mod tree_sitter_impl;

pub use highlight::{Highlight, HighlightRange, Highlighter, LineHighlighter};
pub use line_adapter::LineHighlighterAdapter;
pub use tree_sitter_impl::{TreeSitterHighlighter, TsLanguage, language_for_extension};
