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
    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn names(&self) -> &[String] {
        &self.order
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
    registry.register_fn(
        "kill-line",
        "Kill text from cursor to end of line",
        Editing,
        InteractiveSpec::None,
        |s, _| s.kill_line(),
    );
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
        |s, _| s.save_file(),
    );

    // ---- View ----

    registry.register_fn(
        "scroll-up-command",
        "Scroll up one page",
        View,
        InteractiveSpec::None,
        |s, _| {
            s.scroll_line = s.scroll_line.saturating_sub(20);
        },
    );
    registry.register_fn(
        "scroll-down-command",
        "Scroll down one page",
        View,
        InteractiveSpec::None,
        |s, _| {
            s.scroll_line = s.scroll_line.saturating_add(20);
        },
    );

    // ---- Buffer / quit ----

    registry.register_fn(
        "save-buffers-kill-emacs",
        "Save and quit",
        Buffer,
        InteractiveSpec::None,
        |s, _| {
            s.save_file();
            s.should_quit = true;
        },
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
