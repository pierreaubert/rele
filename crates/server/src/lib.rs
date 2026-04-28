//! rele-server — Shared editor core
//!
//! Core functionality shared between TUI and GPUI clients:
//! - Document model (buffer, cursor, history, kill ring)
//! - Command metadata types
//! - Keyboard macros
//! - Markdown parsing
//! - Import/export

pub mod cancel;
pub mod commands;
pub mod document;
pub mod emacs;
pub mod error;
pub mod export;
pub mod import;
pub mod isearch;
pub mod lisp_host;
pub mod lsp;
pub mod macros;
pub mod markdown;
pub mod minibuffer;
pub mod syntax;

pub use cancel::CancellationFlag;
pub use commands::{CommandArgs, CommandCategory, CommandPlan, InteractiveSpec};

/// Built-in elisp command definitions shared by every client.
///
/// These `defun`s (`backward-char`, `next-line`, `newline`, `undo`, …)
/// are the canonical implementations: each client's `run_command`
/// dispatch looks up the function cell before falling back to a Rust
/// handler, so keeping one source file keeps TUI and GPUI in lockstep.
/// Embedded at compile time via `include_str!`.
pub const BUILTIN_COMMANDS_EL: &str = include_str!("../lisp/commands.el");
/// Default startup forms shared by every frontend.
pub const DEFAULT_INIT_EL: &str = include_str!("../lisp/default-init.el");
/// Minimal built-in major mode for Rust source files.
pub const RUST_MODE_EL: &str = include_str!("../lisp/modes/rust-mode.el");
pub use document::kill_ring::KillRing;
pub use document::{BufferId, BufferKind, DocumentBuffer, EditHistory, EditorCursor, StoredBuffer};
pub use error::{ServerError, ServerResult};
pub use lisp_host::{EditorCore, LispCommandDispatch, LispHost, LoadMode, LoadReport};
pub use macros::{KeyboardMacro, MacroState, RecordedAction};
