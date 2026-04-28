//! GPUI-specific command registry — builds on rele-server command metadata.

use std::collections::HashMap;
use std::rc::Rc;

// Re-export shared metadata types
pub use rele_server::{CommandArgs, CommandCategory, InteractiveSpec};

use crate::state::MdAppState;

/// A command handler closure bound to MdAppState.
pub type CommandHandler = Rc<dyn Fn(&mut MdAppState, CommandArgs)>;

/// A named command with GPUI-specific handler.
#[derive(Clone)]
pub struct Command {
    pub name: String,
    pub description: String,
    pub category: CommandCategory,
    pub handler: CommandHandler,
    pub interactive: InteractiveSpec,
}

/// Registry of all available commands for this editor.
pub struct CommandRegistry {
    commands: HashMap<String, Command>,
    /// Insertion order for deterministic listing.
    order: Vec<String>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Register a command. Replaces any existing command with the same name.
    pub fn register(&mut self, cmd: Command) {
        if !self.commands.contains_key(&cmd.name) {
            self.order.push(cmd.name.clone());
        }
        self.commands.insert(cmd.name.clone(), cmd);
    }

    /// Convenience helper: register a Rust command with a closure.
    pub fn register_fn(
        &mut self,
        name: &str,
        description: &str,
        category: CommandCategory,
        interactive: InteractiveSpec,
        handler: impl Fn(&mut MdAppState, CommandArgs) + 'static,
    ) {
        self.register(Command {
            name: name.to_string(),
            description: description.to_string(),
            category,
            handler: Rc::new(handler),
            interactive,
        });
    }

    pub fn get(&self, name: &str) -> Option<&Command> {
        self.commands.get(name)
    }

    /// All command names in registration order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.order.iter().map(|s| s.as_str())
    }

    /// Number of registered commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Filter command names by substring match.
    pub fn filter(&self, query: &str) -> Vec<String> {
        let q = query.to_lowercase();
        self.order
            .iter()
            .filter(|name| q.is_empty() || name.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }
}

/// Register all built-in Rust commands on a fresh registry.
/// Called once from `MdAppState::new()`.
pub fn register_builtin_commands(registry: &mut CommandRegistry) {
    use CommandCategory::*;

    // ---- Navigation ----
    registry.register_fn(
        "forward-char",
        "Move cursor one character right",
        Navigation,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.move_right(false);
            }
        },
    );
    registry.register_fn(
        "backward-char",
        "Move cursor one character left",
        Navigation,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.move_left(false);
            }
        },
    );
    registry.register_fn(
        "next-line",
        "Move cursor down one line",
        Navigation,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.move_down(false);
            }
        },
    );
    registry.register_fn(
        "previous-line",
        "Move cursor up one line",
        Navigation,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.move_up(false);
            }
        },
    );
    registry.register_fn(
        "forward-word",
        "Move forward by one word",
        Navigation,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.move_word_right(false);
            }
        },
    );
    registry.register_fn(
        "backward-word",
        "Move backward by one word",
        Navigation,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.move_word_left(false);
            }
        },
    );
    registry.register_fn(
        "beginning-of-line",
        "Move to the beginning of the current line",
        Navigation,
        InteractiveSpec::None,
        |s, _| s.move_to_line_start(false),
    );
    registry.register_fn(
        "end-of-line",
        "Move to the end of the current line",
        Navigation,
        InteractiveSpec::None,
        |s, _| s.move_to_line_end(false),
    );
    registry.register_fn(
        "beginning-of-buffer",
        "Move to the beginning of the document",
        Navigation,
        InteractiveSpec::None,
        |s, _| s.move_to_doc_start(false),
    );
    registry.register_fn(
        "end-of-buffer",
        "Move to the end of the document",
        Navigation,
        InteractiveSpec::None,
        |s, _| s.move_to_doc_end(false),
    );
    registry.register_fn(
        "goto-line",
        "Jump to a given line number",
        Navigation,
        InteractiveSpec::Line,
        |s, args| {
            if let Some(line) = args.string.as_ref().and_then(|x| x.parse::<usize>().ok()) {
                s.jump_to_line(line);
            }
        },
    );

    // ---- Editing ----
    registry.register_fn(
        "delete-char",
        "Delete the character at point",
        Editing,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.delete_forward();
            }
        },
    );
    registry.register_fn(
        "backward-delete-char",
        "Delete the character before point",
        Editing,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.backspace();
            }
        },
    );
    registry.register_fn(
        "undo",
        "Undo the last edit",
        Editing,
        InteractiveSpec::None,
        |s, _| s.undo(),
    );
    registry.register_fn(
        "redo",
        "Redo the last undone edit",
        Editing,
        InteractiveSpec::None,
        |s, _| s.redo(),
    );
    // kill-line, kill-word, backward-kill-word, yank, yank-pop,
    // upcase-word, downcase-word, transpose-chars, transpose-words,
    // set-mark, exchange-point-and-mark are now defined as elisp
    // defuns in `crates/server/lisp/commands.el` and dispatched
    // through `editor--*` primitives that hit the EditorCallbacks
    // trait.

    // ---- Buffer ----
    registry.register_fn(
        "switch-to-buffer",
        "Switch to another buffer",
        Buffer,
        InteractiveSpec::Buffer {
            prompt: "Switch to buffer".to_string(),
        },
        |s, args| {
            if let Some(name) = args.string.as_ref() {
                s.switch_to_buffer_by_name(name);
            }
        },
    );
    registry.register_fn(
        "kill-buffer",
        "Kill a buffer",
        Buffer,
        InteractiveSpec::Buffer {
            prompt: "Kill buffer".to_string(),
        },
        |s, args| {
            if let Some(name) = args.string.as_ref() {
                if name.is_empty() {
                    s.kill_current_buffer();
                } else {
                    s.kill_buffer_by_name(name);
                }
            }
        },
    );
    // next-buffer / previous-buffer are now elisp defuns.

    // ---- File ----
    registry.register_fn(
        "find-file",
        "Open a file in a new buffer",
        File,
        InteractiveSpec::File {
            prompt: "Find file".to_string(),
        },
        |s, args| {
            if let Some(path_str) = args.string.as_ref() {
                let path = std::path::PathBuf::from(path_str);
                #[allow(clippy::disallowed_methods)] // TODO(perf): make command handler async
                if let Ok(content) = std::fs::read_to_string(&path) {
                    s.open_file_as_buffer(path, &content);
                }
            }
        },
    );
    // `dired` is now the elisp `dired-cmd` defun in
    // `crates/server/lisp/commands.el` — it lazy-loads upstream
    // dired.el and runs the real `(dired path)`. We keep the
    // legacy Rust dired UI behind `dired-rust` for the transition
    // period; once Phase 6 lands, this and the supporting
    // `dired_open` machinery go away entirely.
    registry.register_fn(
        "dired-rust",
        "Open a directory browser (Rust DiredState — legacy)",
        File,
        InteractiveSpec::None,
        |s, _| {
            let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
            s.dired_open(dir);
        },
    );

    // ---- View ----
    // toggle-preview, toggle-line-numbers, toggle-preview-line-numbers
    // are now elisp defuns.

    // ---- Search ----
    registry.register_fn(
        "isearch-forward",
        "Incremental search forward",
        Search,
        InteractiveSpec::None,
        |s, _| s.isearch_start(crate::state::IsearchDirection::Forward),
    );
    registry.register_fn(
        "isearch-backward",
        "Incremental search backward",
        Search,
        InteractiveSpec::None,
        |s, _| s.isearch_start(crate::state::IsearchDirection::Backward),
    );
    registry.register_fn(
        "find",
        "Open the find bar",
        Search,
        InteractiveSpec::None,
        |s, _| s.toggle_find(),
    );
    registry.register_fn(
        "replace",
        "Open the find-and-replace bar",
        Search,
        InteractiveSpec::None,
        |s, _| s.toggle_find_replace(),
    );

    // ---- Macros ----
    registry.register_fn(
        "start-kbd-macro",
        "Start recording a keyboard macro",
        Macro,
        InteractiveSpec::None,
        |s, _| s.macro_start_recording(),
    );
    registry.register_fn(
        "end-kbd-macro",
        "Stop recording the keyboard macro",
        Macro,
        InteractiveSpec::None,
        |s, _| s.macro_stop_recording(),
    );
    registry.register_fn(
        "call-last-kbd-macro",
        "Execute the last recorded keyboard macro",
        Macro,
        InteractiveSpec::None,
        |s, args| s.macro_execute_last(args.count()),
    );
    registry.register_fn(
        "eval-expression",
        "Evaluate an elisp expression",
        Custom,
        InteractiveSpec::String {
            prompt: "Elisp expression:".into(),
        },
        |s, args| {
            if let Some(expr) = &args.string
                && let Ok(parsed) = rele_elisp::read(expr)
            {
                match s.elisp.eval(parsed) {
                    Ok(result) => {
                        let result_str = format!("{:?}", result);
                        if !result.is_nil() {
                            s.insert_text(&result_str);
                        }
                        s.minibuffer_open(
                            crate::minibuffer::MiniBufferPrompt::FreeText {
                                label: format!("Result: {}", result_str),
                            },
                            vec![],
                            crate::state::PendingMinibufferAction::RunCommandWithInput {
                                name: "eval-expression".into(),
                            },
                        );
                    }
                    Err(e) => {
                        log::error!("Elisp error: {:?}", e);
                    }
                }
            }
        },
    );

    // ---- LSP ----
    registry.register_fn(
        "lsp-completion",
        "Request LSP completion at point",
        Custom,
        InteractiveSpec::None,
        |s, _args| s.lsp_request_completion(),
    );
    registry.register_fn(
        "lsp-hover",
        "Show LSP hover info at point",
        Custom,
        InteractiveSpec::None,
        |s, _args| s.lsp_request_hover(),
    );
    registry.register_fn(
        "lsp-goto-definition",
        "Jump to definition at point (M-.)",
        Custom,
        InteractiveSpec::None,
        |s, _args| s.lsp_goto_definition(),
    );
    registry.register_fn(
        "lsp-find-references",
        "Find all references at point",
        Custom,
        InteractiveSpec::None,
        |s, _args| s.lsp_find_references(),
    );
    registry.register_fn(
        "lsp-format-buffer",
        "Format the current buffer via LSP",
        Custom,
        InteractiveSpec::None,
        |s, _args| s.lsp_format_buffer(),
    );
    registry.register_fn(
        "lsp-restart-server",
        "Restart the LSP server for the current language",
        Custom,
        InteractiveSpec::None,
        |s, _args| s.lsp_restart_server(),
    );
}
