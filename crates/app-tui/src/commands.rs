use std::collections::HashMap;
use std::rc::Rc;

use rele_server::{CommandArgs, CommandCategory, InteractiveSpec};

use crate::state::TuiAppState;

/// Handler function type for commands.
type Handler = Rc<dyn Fn(&mut TuiAppState, CommandArgs)>;

/// A single registered command.
#[allow(dead_code)]
pub struct Command {
    pub name: String,
    pub description: String,
    pub category: CommandCategory,
    pub interactive: InteractiveSpec,
    pub handler: Handler,
}

/// Registry of all available commands.
pub struct CommandRegistry {
    commands: HashMap<String, Command>,
    order: Vec<String>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn register(&mut self, cmd: Command) {
        let name = cmd.name.clone();
        self.commands.insert(name.clone(), cmd);
        if !self.order.contains(&name) {
            self.order.push(name);
        }
    }

    pub fn register_fn(
        &mut self,
        name: &str,
        description: &str,
        category: CommandCategory,
        interactive: InteractiveSpec,
        handler: impl Fn(&mut TuiAppState, CommandArgs) + 'static,
    ) {
        self.register(Command {
            name: name.to_string(),
            description: description.to_string(),
            category,
            interactive,
            handler: Rc::new(handler),
        });
    }

    pub fn get(&self, name: &str) -> Option<&Command> {
        self.commands.get(name)
    }

    /// All registered command names, in registration order.
    /// Used by `M-x` to populate the minibuffer's candidate list.
    pub fn names(&self) -> &[String] {
        &self.order
    }
}

/// Lines to jump per "page" for C-v / M-v / PageUp / PageDown.
/// Uses the viewport height from the last render, with a two-line
/// overlap (Emacs convention — keeps context visible across page
/// boundaries) and a conservative fallback when the height hasn't been
/// measured yet (e.g. the very first keystroke before any draw).
fn page_size(state: &TuiAppState) -> usize {
    let h = state.last_viewport_height;
    if h > 2 { h - 2 } else { 20 }
}

fn save_buffers_and_quit(state: &mut TuiAppState) {
    // Don't quit on save failure — the user would silently lose
    // file-backed edits otherwise. Non-file buffers such as
    // `*scratch*` are skipped by `save_dirty_file_buffers`.
    if state.save_dirty_file_buffers().is_ok() {
        state.should_quit = true;
    }
}

/// Register all builtin Emacs-style commands.
#[allow(clippy::too_many_lines)]
pub fn register_builtin_commands(registry: &mut CommandRegistry) {
    use CommandCategory::{Buffer, Editing, File, Navigation, View};

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
        "move-beginning-of-line",
        "Move cursor to beginning of line",
        Navigation,
        InteractiveSpec::None,
        |s, _| s.move_to_line_start(false),
    );
    registry.register_fn(
        "move-end-of-line",
        "Move cursor to end of line",
        Navigation,
        InteractiveSpec::None,
        |s, _| s.move_to_line_end(false),
    );
    registry.register_fn(
        "beginning-of-buffer",
        "Move cursor to beginning of buffer",
        Navigation,
        InteractiveSpec::None,
        |s, _| s.move_to_buffer_start(false),
    );
    registry.register_fn(
        "end-of-buffer",
        "Move cursor to end of buffer",
        Navigation,
        InteractiveSpec::None,
        |s, _| s.move_to_buffer_end(false),
    );

    // ---- Editing ----

    registry.register_fn(
        "delete-backward-char",
        "Delete character before cursor",
        Editing,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.backspace();
            }
        },
    );
    registry.register_fn(
        "delete-char",
        "Delete character at cursor",
        Editing,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.delete_forward();
            }
        },
    );
    registry.register_fn(
        "newline",
        "Insert newline",
        Editing,
        InteractiveSpec::None,
        |s, _| s.insert_text("\n"),
    );
    // kill-line is now an elisp defun in
    // `crates/server/lisp/commands.el`, dispatched through the
    // `editor--kill-line` primitive.
    registry.register_fn(
        "undo",
        "Undo last edit",
        Editing,
        InteractiveSpec::None,
        |s, _| s.undo(),
    );
    registry.register_fn(
        "redo",
        "Redo last undone edit",
        Editing,
        InteractiveSpec::None,
        |s, _| s.redo(),
    );

    // ---- File ----

    registry.register_fn(
        "save-buffer",
        "Save current buffer to file",
        File,
        InteractiveSpec::None,
        |s, _| {
            // Drop the Result — save_file already sets a message for
            // both success and failure; the interactive save command
            // has no caller to surface the error to.
            let _ = s.save_file();
        },
    );

    // ---- View ----

    // Emacs semantics:
    //   scroll-up-command   = display scrolls UP   = cursor moves DOWN a page  (C-v, PageDown)
    //   scroll-down-command = display scrolls DOWN = cursor moves UP   a page  (M-v, PageUp)
    //
    // We implement "page" as the last known viewport height minus two
    // lines of context overlap (the same convention Emacs uses) and
    // actually move the cursor — otherwise `ensure_cursor_visible`
    // immediately pulls the viewport back to wherever the cursor is,
    // defeating the scroll.
    registry.register_fn(
        "scroll-up-command",
        "Move forward one screenful (C-v)",
        View,
        InteractiveSpec::None,
        |s, args| {
            let page = page_size(s);
            let n = args.count() * page;
            for _ in 0..n {
                s.move_down(false);
            }
        },
    );
    registry.register_fn(
        "scroll-down-command",
        "Move backward one screenful (M-v)",
        View,
        InteractiveSpec::None,
        |s, args| {
            let page = page_size(s);
            let n = args.count() * page;
            for _ in 0..n {
                s.move_up(false);
            }
        },
    );

    // ---- Window management ----
    registry.register_fn(
        "split-window-below",
        "Split current window into top+bottom (C-x 2)",
        View,
        InteractiveSpec::None,
        |s, _| s.split_window_below(),
    );
    registry.register_fn(
        "split-window-right",
        "Split current window into left+right (C-x 3)",
        View,
        InteractiveSpec::None,
        |s, _| s.split_window_right(),
    );
    registry.register_fn(
        "delete-window",
        "Delete the current window (C-x 0)",
        View,
        InteractiveSpec::None,
        |s, _| s.delete_window(),
    );
    registry.register_fn(
        "delete-other-windows",
        "Make the current window fill the frame (C-x 1)",
        View,
        InteractiveSpec::None,
        |s, _| s.delete_other_windows(),
    );
    registry.register_fn(
        "other-window",
        "Move focus to the next window (C-x o)",
        View,
        InteractiveSpec::None,
        |s, _| s.focus_next_window(),
    );

    // ---- LSP diagnostic navigation ----
    //
    // Moves the cursor to the next / previous diagnostic in the
    // current buffer, wrapping. Echo-area shows the message
    // automatically (see `draw_message_line` priority order in ui.rs).
    registry.register_fn(
        "lsp-next-diagnostic",
        "Move cursor to next LSP diagnostic (M-g n)",
        View,
        InteractiveSpec::None,
        |s, _| {
            if !s.goto_next_diagnostic() {
                s.message = Some("No diagnostics".to_string());
            }
        },
    );
    registry.register_fn(
        "lsp-prev-diagnostic",
        "Move cursor to previous LSP diagnostic (M-g p)",
        View,
        InteractiveSpec::None,
        |s, _| {
            if !s.goto_prev_diagnostic() {
                s.message = Some("No diagnostics".to_string());
            }
        },
    );

    // ---- Buffer / quit ----

    registry.register_fn(
        "save-buffers-kill-terminal",
        "Save file buffers and quit",
        Buffer,
        InteractiveSpec::None,
        |s, _| save_buffers_and_quit(s),
    );
    registry.register_fn(
        "save-buffers-kill-emacs",
        "Save file buffers and quit",
        Buffer,
        InteractiveSpec::None,
        |s, _| save_buffers_and_quit(s),
    );
    registry.register_fn(
        "kill-emacs",
        "Quit without saving",
        Buffer,
        InteractiveSpec::None,
        |s, _| {
            s.should_quit = true;
        },
    );
}
