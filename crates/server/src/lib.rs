//! rele-server — Shared editor core
//!
//! Core functionality shared between TUI and GPUI clients:
//! - Document model (buffer, cursor, history, kill ring)
//! - Command metadata types
//! - Keyboard macros
//! - Markdown parsing
//! - Import/export

pub mod commands;
pub mod document;
pub mod error;
pub mod export;
pub mod import;
pub mod macros;
pub mod markdown;

pub use commands::{CommandArgs, CommandCategory, InteractiveSpec};
pub use document::{BufferId, BufferKind, DocumentBuffer, EditHistory, EditorCursor, StoredBuffer};
pub use document::kill_ring::KillRing;
pub use error::{ServerError, ServerResult};
pub use macros::{KeyboardMacro, MacroState, RecordedAction};
