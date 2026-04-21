//! rele-gpui — GPUI client for rele markdown editor.
//!
//! A full-featured markdown editor demonstrating the gpui-toolkit ecosystem:
//! - GitHub Flavored Markdown (GFM) via comrak
//! - Split-pane editor + live preview
//! - Ropey-backed document buffer with undo/redo
//! - Source map for click-to-locate (preview → editor)
//! - Platform-specific keybindings via gpui-keybinding
//! - Theme support via gpui-themes
//! - Import/export: Word (.docx), PDF

// Re-export shared editor core from server
pub use rele_server::{
    CommandArgs, CommandCategory, DocumentBuffer, EditHistory, EditorCursor, InteractiveSpec,
    KeyboardMacro, KillRing, MacroState, RecordedAction, StoredBuffer,
};
pub use rele_server::{document, export, import, macros};

pub mod actions;
pub mod commands;
pub mod dired;
pub mod keybindings;
pub mod markdown;
pub mod minibuffer;
pub mod state;
pub mod views;

pub use keybindings::MdKeybindingProvider;
pub use state::MdAppState;
pub use views::main_view::MainView;
