//! Command metadata types — shared between TUI and GPUI clients.
//!
//! Defines command arguments and interactive specifications.
//! Each app implements its own CommandRegistry with app-specific state types.

/// Arguments passed to a command handler.
#[derive(Clone, Debug, Default)]
pub struct CommandArgs {
    /// Universal prefix argument (C-u N).
    pub prefix_arg: Option<usize>,
    /// Mini-buffer string input, if the command was interactive.
    pub string: Option<String>,
}

impl CommandArgs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_prefix(mut self, n: usize) -> Self {
        self.prefix_arg = Some(n);
        self
    }

    pub fn with_string(mut self, s: String) -> Self {
        self.string = Some(s);
        self
    }

    /// Return the prefix argument, or 1 if unset.
    pub fn count(&self) -> usize {
        self.prefix_arg.unwrap_or(1)
    }
}

/// What kind of interactive input (if any) a command needs before running.
#[derive(Clone, Debug)]
pub enum InteractiveSpec {
    /// No arguments needed — just run.
    None,
    /// Prompts for a file path via the mini-buffer's FindFile prompt.
    File { prompt: String },
    /// Prompts for a buffer name via the mini-buffer's SwitchBuffer prompt.
    Buffer { prompt: String },
    /// Prompts for a free-text string via the mini-buffer's FreeText prompt.
    String { prompt: String },
    /// Prompts for a line number.
    Line,
    /// Prompts for yes/no confirmation.
    Confirm { prompt: String },
}

/// Command category for M-x filtering and organisation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandCategory {
    Navigation,
    Editing,
    Buffer,
    File,
    View,
    Search,
    Macro,
    Custom,
}
