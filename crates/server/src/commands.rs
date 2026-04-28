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
    /// Multi-prompt minibuffer input, in prompt order.
    pub strings: Vec<String>,
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

    pub fn with_strings(mut self, strings: Vec<String>) -> Self {
        self.strings = strings;
        self
    }

    pub fn string_args(&self) -> Vec<String> {
        if !self.strings.is_empty() {
            return self.strings.clone();
        }
        self.string.clone().into_iter().collect()
    }

    /// Return the prefix argument, or 1 if unset.
    pub fn count(&self) -> usize {
        self.prefix_arg.unwrap_or(1)
    }
}

/// What kind of interactive input (if any) a command needs before running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InteractiveSpec {
    /// No arguments needed — just run.
    None,
    /// Prompts for a file path via the mini-buffer's FindFile prompt.
    File { prompt: String },
    /// Prompts for a buffer name via the mini-buffer's SwitchBuffer prompt.
    Buffer { prompt: String },
    /// Prompts for a free-text string via the mini-buffer's FreeText prompt.
    String { prompt: String },
    /// Prompts for multiple free-text strings in sequence.
    StringList { prompts: Vec<String> },
    /// Prompts for a line number.
    Line,
    /// Prompts for yes/no confirmation.
    Confirm { prompt: String },
}

impl InteractiveSpec {
    /// Translate an Emacs `(interactive "CODE...")` spec string into
    /// the shared prompt enum used by frontends.
    ///
    /// Returns `None` for unsupported codes so callers can fall back to
    /// registry metadata or direct command execution.
    pub fn from_elisp_spec(spec: &str) -> Option<Self> {
        let specs: Vec<Self> = spec
            .split('\n')
            .filter(|part| !part.is_empty())
            .filter_map(Self::from_elisp_spec_line)
            .collect();
        if specs.len() > 1 && specs.iter().all(|spec| matches!(spec, Self::String { .. })) {
            let prompts = specs
                .into_iter()
                .filter_map(|spec| match spec {
                    Self::String { prompt } => Some(prompt),
                    _ => None,
                })
                .collect();
            return Some(Self::StringList { prompts });
        }
        specs.into_iter().next()
    }

    fn from_elisp_spec_line(spec: &str) -> Option<Self> {
        let mut chars = spec.chars();
        let code = chars.next()?;
        let prompt: String = chars.collect();
        match code {
            'p' | 'P' => Some(Self::None),
            's' => Some(Self::String { prompt }),
            'f' | 'F' => Some(Self::File { prompt }),
            'b' | 'B' => Some(Self::Buffer { prompt }),
            'n' => Some(Self::String { prompt }),
            _ => None,
        }
    }
}

/// Frontend-neutral command planning result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandPlan {
    /// The command can be run immediately with the caller's arguments.
    Run,
    /// The command needs minibuffer input before it can run.
    Prompt(InteractiveSpec),
    /// Neither the Rust registry nor the Lisp host knows this command.
    Missing,
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
