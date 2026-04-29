#![allow(clippy::disallowed_methods)]
use std::path::{Path, PathBuf};

use rele_server::CancellationFlag;
use rele_server::document::buffer_list::name_from_path;
use rele_server::emacs::search::MatchData;
use rele_server::lsp::{LspBufferState, LspConfig, LspEvent, LspRegistry, position::uri_from_path};
use rele_server::minibuffer::{
    MiniBufferPrompt, MiniBufferResult, MiniBufferState, PathCompletionKind,
    path_completion_candidates,
};
use rele_server::syntax::{Highlighter, TreeSitterHighlighter, language_for_extension};
use rele_server::{
    BufferId, BufferKind, CommandArgs, CommandPlan, DocumentBuffer, EditHistory, EditorCore,
    EditorCursor, InteractiveSpec, KillRing, LispCommandDispatch, LispHost, MacroState,
    StoredBuffer,
};
use tokio::sync::mpsc;

use crate::commands::{CommandRegistry, register_builtin_commands};
use crate::windows::{SplitSize, Window, WindowContent};

#[allow(clippy::disallowed_methods)] // explicit user save/quit action on the TUI thread
fn write_document_to_path(document: &mut DocumentBuffer, path: &Path) -> std::io::Result<()> {
    let content = document.text();
    std::fs::write(path, &content)?;
    document.mark_clean();
    Ok(())
}

#[allow(dead_code)]
fn special_buffer_attrs(name: &str) -> Option<(BufferKind, bool)> {
    if name == "*Buffer List*" {
        Some((BufferKind::BufferList, true))
    } else if name.starts_with("*Dired: ") {
        Some((BufferKind::Dired, true))
    } else {
        None
    }
}

#[allow(dead_code)]
fn major_mode_for_kind(kind: &BufferKind) -> &'static str {
    match kind {
        BufferKind::Dired => "dired-mode",
        BufferKind::BufferList => "Buffer-menu-mode",
        BufferKind::Scratch => "lisp-interaction-mode",
        BufferKind::File => "text-mode",
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// What to do with the mini-buffer's `Submitted(text)` / `Cancelled`
/// result. Stored alongside `MiniBufferState` — the state machine itself
/// is generic; the action binds it to a concrete command.
#[derive(Debug, Clone)]
pub enum PendingMiniBufferAction {
    /// Open the typed path as a file (C-x C-f).
    FindFile,
    /// Switch to the buffer whose name is typed (C-x b).
    SwitchBuffer,
    /// Kill the buffer whose name is typed (C-x k); empty = current buffer.
    KillBuffer,
    /// Open a directory in dired (C-x d).
    Dired,
    /// Run the command whose name is typed (M-x / Esc-x).
    RunCommand,
    /// Run a command with a string argument supplied by a prompt.
    RunCommandWithInput { name: String },
    /// Run a command after collecting several string prompts in sequence.
    RunCommandWithInputs {
        name: String,
        prompts: Vec<String>,
        values: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryReplaceKind {
    Literal,
    Regexp,
}

#[derive(Debug, Clone)]
pub struct QueryReplaceState {
    pub from: String,
    pub to: String,
    pub replacements: usize,
    pub kind: QueryReplaceKind,
}

/// Application state for the TUI client.
///
/// Mirrors the GPUI `MdAppState` pattern: the active buffer's data lives as
/// top-level fields; inactive buffers are stored in `stored_buffers`.
#[allow(dead_code)]
pub struct TuiAppState {
    // ---- Active buffer ----
    pub document: DocumentBuffer,
    pub cursor: EditorCursor,
    pub history: EditHistory,
    pub current_buffer_id: BufferId,
    pub current_buffer_name: String,
    pub current_buffer_kind: BufferKind,
    pub current_buffer_read_only: bool,
    pub current_major_mode: Option<String>,

    // ---- Inactive buffers ----
    pub stored_buffers: Vec<StoredBuffer>,
    next_buffer_id: u64,

    // ---- Editor subsystems ----
    pub kill_ring: KillRing,
    pub search_match_data: MatchData,
    pub macros: MacroState,
    pub commands: CommandRegistry,
    pub lisp_host: LispHost,

    // ---- Viewport ----
    pub scroll_line: usize,
    /// Last known viewport height in lines, written by the UI renderer
    /// in `draw_editor`. Commands like `scroll-up-command` /
    /// `scroll-down-command` use this as the page size. Zero until the
    /// first frame paints.
    pub last_viewport_height: usize,

    // ---- Editing flags ----
    pub last_edit_was_char_insert: bool,
    pub last_move_was_vertical: bool,

    // ---- Prefix keys ----
    pub universal_arg: Option<usize>,
    pub meta_pending: bool,
    pub c_x_pending: bool,
    pub c_x_r_pending: bool,
    /// `M-g` prefix: the next key is interpreted as the second of a
    /// goto chord (`M-g n` = next diagnostic, `M-g p` = previous, …).
    pub m_g_pending: bool,

    // ---- Status line message (auto-clears after one frame) ----
    pub message: Option<String>,

    // ---- Quit flag ----
    pub should_quit: bool,

    // ---- Windows (C-x 2 / 3 / 0 / 1 / o) ----
    /// Binary tree of windows. Default layout is a single leaf
    /// showing the active buffer, with a `*Diagnostics*` panel
    /// attached beneath via `HSplit` — see `TuiAppState::new`. All
    /// `Buffer` leaves mirror the same active buffer for now;
    /// per-window cursor/scroll is a follow-up.
    pub windows: Window,
    /// In-order leaf index of the currently focused window. 0 is the
    /// top-left leaf. `Self::focus_window(idx)` is the only place
    /// this should be mutated from outside this module.
    pub focused_window: usize,

    // ---- Mini-buffer (C-x C-f, C-x b, …) ----
    /// Modal prompt state. Active whenever the minibuffer is open.
    pub minibuffer: MiniBufferState,
    /// What command to dispatch on submit. `None` when the minibuffer
    /// is closed.
    pub pending_minibuffer_action: Option<PendingMiniBufferAction>,
    pub query_replace: Option<QueryReplaceState>,

    // ---- Syntax highlighting ----
    /// Tree-sitter highlighter for the active buffer, if its language
    /// has a grammar (rust, markdown, …). `None` for plain-text
    /// buffers; the UI falls back to a single uncolored style.
    /// Rebuilt on buffer switch.
    pub buffer_highlighter: Option<Box<dyn Highlighter>>,
    /// Document version the highlighter last parsed against. Used by
    /// `ensure_highlight_fresh` to avoid redundant reparses.
    pub buffer_highlighter_version: u64,

    // ---- Cancellation ----
    pub cancel_flag: CancellationFlag,

    // ---- LSP ----
    pub lsp_registry: Option<LspRegistry>,
    pub lsp_buffer_state: Option<LspBufferState>,
    pub lsp_event_rx: Option<mpsc::UnboundedReceiver<LspEvent>>,
    pub lsp_completion_items: Vec<lsp_types::CompletionItem>,
    pub lsp_completion_selected: usize,
    pub lsp_completion_visible: bool,
    pub lsp_hover_text: Option<String>,
    pub lsp_status: Option<String>,
}

#[allow(dead_code)]
impl TuiAppState {
    pub fn new() -> Self {
        let mut commands = CommandRegistry::new();
        register_builtin_commands(&mut commands);

        Self {
            document: DocumentBuffer::from_text(""),
            cursor: EditorCursor::new(),
            history: EditHistory::default(),
            current_buffer_id: BufferId(0),
            current_buffer_name: "*scratch*".to_string(),
            current_buffer_kind: BufferKind::Scratch,
            current_buffer_read_only: false,
            current_major_mode: Some("lisp-interaction-mode".to_string()),
            stored_buffers: Vec::new(),
            next_buffer_id: 1,
            kill_ring: KillRing::default(),
            search_match_data: MatchData::default(),
            macros: MacroState::default(),
            commands,
            lisp_host: LispHost::new(EditorCore::new()),
            scroll_line: 0,
            last_viewport_height: 0,
            last_edit_was_char_insert: false,
            last_move_was_vertical: false,
            universal_arg: None,
            meta_pending: false,
            c_x_pending: false,
            c_x_r_pending: false,
            m_g_pending: false,
            message: None,
            should_quit: false,
            minibuffer: MiniBufferState::default(),
            pending_minibuffer_action: None,
            query_replace: None,
            // Default layout: main buffer on top, *Diagnostics* panel
            // below fixed at 5 lines tall. Focus starts on the
            // buffer. Users can delete *Diagnostics* with `C-x 0`
            // from it, re-split with `C-x 2` / `C-x 3`, or resize
            // (not yet implemented — `C-x ^` / `C-x }` would go here).
            windows: Window::HSplit(
                Box::new(Window::Leaf(WindowContent::Buffer)),
                Box::new(Window::Leaf(WindowContent::Diagnostics)),
                SplitSize::Lines(5),
            ),
            focused_window: 0,
            buffer_highlighter: None,
            buffer_highlighter_version: u64::MAX,
            cancel_flag: CancellationFlag::new(),
            lsp_registry: None,
            lsp_buffer_state: None,
            lsp_event_rx: None,
            lsp_completion_items: Vec::new(),
            lsp_completion_selected: 0,
            lsp_completion_visible: false,
            lsp_hover_text: None,
            lsp_status: None,
        }
    }

    pub fn from_file(path: PathBuf, content: &str) -> Self {
        let mut state = Self::new();
        state.current_buffer_name = name_from_path(&path);
        state.current_buffer_kind = BufferKind::File;
        state.current_major_mode = None;
        state.document = DocumentBuffer::from_file(path, content);
        state.refresh_buffer_highlighter();
        state.sync_lisp_core_from_state();
        state.activate_mode_for_current_buffer_if_needed();
        state
    }

    fn editor_core_snapshot(&self) -> EditorCore {
        EditorCore {
            document: self.document.clone(),
            cursor: self.cursor,
            history: self.history.clone(),
            current_buffer_id: self.current_buffer_id,
            current_buffer_name: self.current_buffer_name.clone(),
            current_buffer_kind: self.current_buffer_kind.clone(),
            current_buffer_read_only: self.current_buffer_read_only,
            current_major_mode: self.current_major_mode.clone(),
            stored_buffers: self.stored_buffers.clone(),
            next_buffer_id: self.next_buffer_id,
            lsp_state: self.lsp_buffer_state.clone(),
            kill_ring: self.kill_ring.clone(),
            search_match_data: self.search_match_data.clone(),
            scroll_line: self.scroll_line,
            last_edit_was_char_insert: self.last_edit_was_char_insert,
            last_move_was_vertical: self.last_move_was_vertical,
            message: self.message.clone(),
            keyboard_quit_requested: false,
            preview_toggle_requested: false,
            line_numbers_toggle_requested: false,
            preview_line_numbers_toggle_requested: false,
        }
    }

    fn sync_lisp_core_from_state(&mut self) {
        self.lisp_host.replace_core(self.editor_core_snapshot());
    }

    fn sync_state_from_lisp_core(&mut self) {
        let before_buffer_id = self.current_buffer_id;
        let before_kind = self.current_buffer_kind.clone();
        let before_major_mode = self.current_major_mode.clone();
        let before_doc_version = self.document.version();
        let before_path = self.document.file_path().cloned();
        let mut core = self.lisp_host.core_snapshot();
        let keyboard_quit_requested = core.keyboard_quit_requested;
        core.keyboard_quit_requested = false;

        self.document = core.document;
        self.cursor = core.cursor;
        self.history = core.history;
        self.current_buffer_id = core.current_buffer_id;
        self.current_buffer_name = core.current_buffer_name;
        self.current_buffer_kind = core.current_buffer_kind;
        self.current_buffer_read_only = core.current_buffer_read_only;
        self.current_major_mode = core.current_major_mode;
        self.stored_buffers = core.stored_buffers;
        self.next_buffer_id = core.next_buffer_id;
        self.lsp_buffer_state = core.lsp_state;
        self.kill_ring = core.kill_ring;
        self.search_match_data = core.search_match_data;
        self.scroll_line = core.scroll_line;
        self.last_edit_was_char_insert = core.last_edit_was_char_insert;
        self.last_move_was_vertical = core.last_move_was_vertical;
        self.message = core.message;

        let buffer_changed =
            before_buffer_id != self.current_buffer_id || before_kind != self.current_buffer_kind;
        let mode_changed = before_major_mode != self.current_major_mode;
        let doc_changed = before_doc_version != self.document.version();
        let path_changed = before_path != self.document.file_path().cloned();
        let should_activate_mode = path_changed && self.current_buffer_kind == BufferKind::File;

        if buffer_changed || path_changed || mode_changed {
            self.refresh_buffer_highlighter();
        }
        if path_changed && self.current_buffer_kind == BufferKind::File {
            self.lsp_did_open();
        } else if doc_changed {
            self.notify_highlighter();
            self.lsp_did_change();
        }
        if should_activate_mode {
            self.activate_mode_for_current_buffer_if_needed();
        }

        if keyboard_quit_requested {
            self.keyboard_quit();
            self.sync_lisp_core_from_state();
        } else {
            self.sync_lisp_core_from_state();
        }
    }

    pub fn eval_lisp(
        &mut self,
        expr: rele_elisp::LispObject,
    ) -> rele_elisp::ElispResult<rele_elisp::LispObject> {
        self.sync_lisp_core_from_state();
        let result = self.lisp_host.eval(expr);
        self.sync_state_from_lisp_core();
        result
    }

    pub fn eval_lisp_source(
        &mut self,
        source: &str,
    ) -> Result<rele_elisp::LispObject, (usize, rele_elisp::ElispError)> {
        self.sync_lisp_core_from_state();
        let result = self.lisp_host.eval_source(source);
        self.sync_state_from_lisp_core();
        result
    }

    fn run_hook_logged(&mut self, name: &str) {
        self.sync_lisp_core_from_state();
        if let Err(e) = self.lisp_host.run_hooks(name) {
            log::error!("hook {name}: {e:?}");
        }
        self.sync_state_from_lisp_core();
    }

    fn activate_mode_for_current_buffer_if_needed(&mut self) {
        let path = self.document.file_path().cloned();
        if path.is_none() {
            return;
        }

        self.sync_lisp_core_from_state();
        let result = self.lisp_host.activate_mode_for_path(path.as_deref());
        self.sync_state_from_lisp_core();

        if let Err((idx, error)) = result {
            self.message = Some(format!("mode load failed at form {idx}: {error:?}"));
            self.sync_lisp_core_from_state();
        }
    }

    pub fn command_completion_candidates(&self) -> Vec<String> {
        self.lisp_host
            .command_completion_candidates(self.commands.names().iter().cloned())
    }

    // ---- Buffer list ----

    fn allocate_buffer_id(&mut self) -> BufferId {
        let id = BufferId(self.next_buffer_id);
        self.next_buffer_id += 1;
        id
    }

    /// How many buffers are open in total (active + stored).
    pub fn buffer_count(&self) -> usize {
        1 + self.stored_buffers.len()
    }

    /// Names of all buffers (active first), for minibuffer completion.
    pub fn buffer_names(&self) -> Vec<String> {
        let mut names = Vec::with_capacity(1 + self.stored_buffers.len());
        names.push(self.current_buffer_name.clone());
        for b in &self.stored_buffers {
            names.push(b.name.clone());
        }
        names
    }

    /// Find a buffer (active or stored) by file path. Returns its id.
    fn find_buffer_by_path(&self, path: &std::path::Path) -> Option<BufferId> {
        if self.document.file_path().map(|p| p.as_path()) == Some(path) {
            return Some(self.current_buffer_id);
        }
        for b in &self.stored_buffers {
            if b.document.file_path().map(|p| p.as_path()) == Some(path) {
                return Some(b.id);
            }
        }
        None
    }

    /// Find a buffer (active or stored) by display name.
    fn find_buffer_by_name(&self, name: &str) -> Option<BufferId> {
        if self.current_buffer_name == name {
            return Some(self.current_buffer_id);
        }
        self.stored_buffers
            .iter()
            .find(|b| b.name == name)
            .map(|b| b.id)
    }

    /// Swap the active buffer with the stored buffer at `stored_idx`.
    /// The previously-active buffer is moved to the end of `stored_buffers`.
    fn swap_active_with_stored(&mut self, stored_idx: usize) {
        let incoming = self.stored_buffers.remove(stored_idx);
        let outgoing = StoredBuffer {
            id: self.current_buffer_id,
            name: std::mem::replace(&mut self.current_buffer_name, incoming.name),
            document: std::mem::replace(&mut self.document, incoming.document),
            history: std::mem::replace(&mut self.history, incoming.history),
            cursor: std::mem::replace(&mut self.cursor, incoming.cursor),
            kind: std::mem::replace(&mut self.current_buffer_kind, incoming.kind),
            read_only: std::mem::replace(&mut self.current_buffer_read_only, incoming.read_only),
            major_mode: std::mem::replace(&mut self.current_major_mode, incoming.major_mode),
            scroll_line: std::mem::replace(&mut self.scroll_line, incoming.scroll_line),
            last_edit_was_char_insert: std::mem::replace(
                &mut self.last_edit_was_char_insert,
                incoming.last_edit_was_char_insert,
            ),
            last_move_was_vertical: std::mem::replace(
                &mut self.last_move_was_vertical,
                incoming.last_move_was_vertical,
            ),
            lsp_state: std::mem::replace(&mut self.lsp_buffer_state, incoming.lsp_state),
        };
        self.current_buffer_id = incoming.id;
        self.stored_buffers.push(outgoing);
    }

    /// Switch to the buffer with the given id. Returns true on success.
    pub fn switch_to_buffer_id(&mut self, id: BufferId) -> bool {
        if id == self.current_buffer_id {
            return true;
        }
        let Some(idx) = self.stored_buffers.iter().position(|b| b.id == id) else {
            return false;
        };
        self.swap_active_with_stored(idx);
        // The language may have changed; re-pick the grammar. Parse
        // runs lazily on the next render via `ensure_highlight_fresh`.
        self.refresh_buffer_highlighter();
        true
    }

    /// Switch to a buffer by name. Returns true on success.
    pub fn switch_to_buffer_by_name(&mut self, name: &str) -> bool {
        if let Some(id) = self.find_buffer_by_name(name) {
            self.switch_to_buffer_id(id)
        } else {
            false
        }
    }

    /// Create or fetch a named buffer. Used by elisp
    /// `(get-buffer-create NAME)` so generated buffers such as dired
    /// and `*Buffer List*` live in the TUI buffer list.
    pub fn get_or_create_named_buffer(&mut self, name: &str) -> BufferId {
        if let Some(id) = self.find_buffer_by_name(name) {
            if id == self.current_buffer_id {
                self.apply_special_attrs_for_current_name();
            } else if let Some(buffer) = self.stored_buffers.iter_mut().find(|b| b.id == id)
                && let Some((kind, read_only)) = special_buffer_attrs(&buffer.name)
            {
                buffer.kind = kind;
                buffer.read_only = read_only;
                buffer.major_mode = Some(major_mode_for_kind(&buffer.kind).to_string());
            }
            return id;
        }

        let id = self.allocate_buffer_id();
        let mut buffer = StoredBuffer::new_scratch(id);
        buffer.name = name.to_string();
        if let Some((kind, read_only)) = special_buffer_attrs(name) {
            buffer.kind = kind;
            buffer.read_only = read_only;
            buffer.major_mode = Some(major_mode_for_kind(&buffer.kind).to_string());
        }
        self.stored_buffers.push(buffer);
        id
    }

    fn apply_special_attrs_for_current_name(&mut self) {
        if let Some((kind, read_only)) = special_buffer_attrs(&self.current_buffer_name) {
            self.current_buffer_kind = kind;
            self.current_buffer_read_only = read_only;
            self.current_major_mode =
                Some(major_mode_for_kind(&self.current_buffer_kind).to_string());
        }
    }

    /// Open a file as a buffer. If a buffer for this path is already
    /// open, switches to it; otherwise loads the file into a new
    /// buffer.
    pub fn open_file_as_buffer(&mut self, path: PathBuf, content: &str) {
        #[allow(clippy::disallowed_methods)] // canonicalize here is a user action
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);
        if let Some(id) = self.find_buffer_by_path(&canonical) {
            // switch_to_buffer_id already refreshes the highlighter.
            self.switch_to_buffer_id(id);
            self.lsp_did_open();
            self.activate_mode_for_current_buffer_if_needed();
            self.run_hook_logged("find-file-hook");
            return;
        }

        // Push the current buffer into stored, load the new file into active.
        let name = name_from_path(&canonical);
        let current_id = self.current_buffer_id;
        let outgoing = StoredBuffer {
            id: current_id,
            name: std::mem::replace(&mut self.current_buffer_name, name),
            document: std::mem::replace(
                &mut self.document,
                DocumentBuffer::from_file(canonical, content),
            ),
            history: std::mem::take(&mut self.history),
            cursor: std::mem::replace(&mut self.cursor, EditorCursor::new()),
            kind: std::mem::replace(&mut self.current_buffer_kind, BufferKind::File),
            read_only: std::mem::replace(&mut self.current_buffer_read_only, false),
            major_mode: self.current_major_mode.take(),
            scroll_line: std::mem::replace(&mut self.scroll_line, 0),
            last_edit_was_char_insert: std::mem::replace(
                &mut self.last_edit_was_char_insert,
                false,
            ),
            last_move_was_vertical: std::mem::replace(&mut self.last_move_was_vertical, false),
            lsp_state: self.lsp_buffer_state.take(),
        };
        self.stored_buffers.push(outgoing);
        self.current_buffer_id = self.allocate_buffer_id();
        // New buffer, possibly new language — pick the grammar.
        self.refresh_buffer_highlighter();
        self.lsp_did_open();
        self.activate_mode_for_current_buffer_if_needed();
        self.run_hook_logged("find-file-hook");
    }

    /// Kill the current buffer. Refuses if it's the only one.
    pub fn kill_current_buffer(&mut self) -> bool {
        if self.buffer_count() <= 1 {
            self.message = Some("Cannot kill the only buffer".to_string());
            return false;
        }
        // Run kill-buffer-hook before tearing down LSP / removing the
        // buffer so user code can still inspect it.
        self.run_hook_logged("kill-buffer-hook");
        // Notify LSP that the buffer is going away before we lose the state.
        self.lsp_did_close();
        // Swap the last stored buffer in, then drop what's now at the end.
        let last_idx = self.stored_buffers.len() - 1;
        self.swap_active_with_stored(last_idx);
        // The previously-active buffer is now the last stored one.
        self.stored_buffers.pop();
        // Language of the now-active buffer may differ — refresh.
        self.refresh_buffer_highlighter();
        true
    }

    /// Kill a buffer by name. Returns true on success.
    pub fn kill_buffer_by_name(&mut self, name: &str) -> bool {
        if name.is_empty() || name == self.current_buffer_name {
            return self.kill_current_buffer();
        }
        if let Some(pos) = self.stored_buffers.iter().position(|b| b.name == name) {
            self.run_hook_logged("kill-buffer-hook");
            self.stored_buffers.remove(pos);
            true
        } else {
            self.message = Some(format!("No such buffer: {name}"));
            false
        }
    }

    // ---- Window management ----

    /// Split the focused window horizontally (new window below) —
    /// `C-x 2` / `split-window-below`. New leaf shows the same
    /// content as the current one.
    pub fn split_window_below(&mut self) {
        self.focused_window = self.windows.split_horizontal(self.focused_window);
    }

    /// Split the focused window vertically (new window to the right)
    /// — `C-x 3` / `split-window-right`.
    pub fn split_window_right(&mut self) {
        self.focused_window = self.windows.split_vertical(self.focused_window);
    }

    /// Delete the focused window — `C-x 0`. Refuses when only one
    /// window remains.
    pub fn delete_window(&mut self) {
        if let Some(f) = self.windows.delete_focused(self.focused_window) {
            self.focused_window = f;
        } else {
            self.message = Some("Cannot delete the only window".to_string());
        }
    }

    /// Delete all other windows — `C-x 1`. Current window fills the
    /// whole editor area.
    pub fn delete_other_windows(&mut self) {
        self.focused_window = self.windows.keep_only_focused(self.focused_window);
    }

    /// Move focus to the next window — `C-x o`. Wraps.
    pub fn focus_next_window(&mut self) {
        self.focused_window = self.windows.focus_next(self.focused_window);
    }

    // ---- LSP diagnostics (read-only accessors) ----

    /// Return the diagnostic message to display in the echo area for
    /// the current cursor line, if any. If several diagnostics overlap
    /// the line, the most severe one wins. Returns `None` when the
    /// cursor is on a clean line or the active buffer has no LSP
    /// state.
    pub fn diagnostic_at_cursor(&self) -> Option<String> {
        let lsp = self.lsp_buffer_state.as_ref()?;
        let line = self.cursor_line() as u32;
        lsp.diagnostics
            .iter()
            .filter(|d| d.range.start.line <= line && d.range.end.line >= line)
            .min_by_key(|d| {
                use lsp_types::DiagnosticSeverity as S;
                match d.severity {
                    Some(s) if s == S::ERROR => 0u8,
                    Some(s) if s == S::WARNING => 1,
                    Some(s) if s == S::INFORMATION => 2,
                    Some(s) if s == S::HINT => 3,
                    _ => 4,
                }
            })
            .map(|d| {
                // LSP messages often contain newlines (rust-analyzer's
                // do). Keep the first line for the single-row echo
                // area; the full message is available via hover.
                d.message.lines().next().unwrap_or("").to_string()
            })
    }

    /// Jump the cursor to the next diagnostic in the buffer. Wraps.
    /// Returns `true` if it moved.
    pub fn goto_next_diagnostic(&mut self) -> bool {
        self.goto_diagnostic(true)
    }

    /// Jump to the previous diagnostic (see `goto_next_diagnostic`).
    pub fn goto_prev_diagnostic(&mut self) -> bool {
        self.goto_diagnostic(false)
    }

    fn goto_diagnostic(&mut self, forward: bool) -> bool {
        let Some(lsp) = self.lsp_buffer_state.as_ref() else {
            return false;
        };
        if lsp.diagnostics.is_empty() {
            return false;
        }
        let cur_line = self.cursor_line() as u32;
        let cur_col = self.cursor_col() as u32;

        // Sort stable by position; the server may re-emit diagnostics
        // in an arbitrary order as edits land.
        let mut positions: Vec<(u32, u32)> = lsp
            .diagnostics
            .iter()
            .map(|d| (d.range.start.line, d.range.start.character))
            .collect();
        positions.sort();

        let target = if forward {
            positions
                .iter()
                .find(|(l, c)| *l > cur_line || (*l == cur_line && *c > cur_col))
                .copied()
                .or_else(|| positions.first().copied())
        } else {
            positions
                .iter()
                .rev()
                .find(|(l, c)| *l < cur_line || (*l == cur_line && *c < cur_col))
                .copied()
                .or_else(|| positions.last().copied())
        };
        let Some((line, character)) = target else {
            return false;
        };
        // (line, UTF-16 character col) → rope char offset.
        let pos = lsp_types::Position { line, character };
        let offset =
            rele_server::lsp::position::position_to_char_offset(self.document.rope(), &pos);
        self.cursor.position = offset.min(self.document.len_chars());
        self.cursor.clear_selection();
        true
    }

    // ---- Syntax highlighting ----

    /// Pick a tree-sitter grammar based on the active buffer's file
    /// extension. Called after `open_file_as_buffer` or on buffer
    /// switch. Languages we don't ship a grammar for get `None` and
    /// render uncoloured.
    pub fn refresh_buffer_highlighter(&mut self) {
        let lang = self
            .document
            .file_path()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .and_then(language_for_extension);
        self.buffer_highlighter = lang
            .and_then(|l| TreeSitterHighlighter::new(l).ok())
            .map(|h| Box::new(h) as Box<dyn Highlighter>);
        // Force the first render to trigger a full parse.
        self.buffer_highlighter_version = u64::MAX;
    }

    /// Ensure the highlighter has parsed the current document version.
    /// Called from the UI before querying highlight ranges. If the
    /// highlighter has already seen the latest version (thanks to
    /// `notify_highlighter` running on every edit), this is a no-op.
    pub fn ensure_highlight_fresh(&mut self) {
        if let Some(h) = self.buffer_highlighter.as_mut() {
            let doc_version = self.document.version();
            if self.buffer_highlighter_version != doc_version {
                h.on_edit(self.document.rope(), &[], true);
                self.buffer_highlighter_version = doc_version;
            }
        }
    }

    /// Incremental tree-sitter update. Called from the edit path after
    /// the rope has mutated and before `lsp_did_change` drains the
    /// journal — we peek the journal so both consumers see the same
    /// events. Uses real incremental parsing via
    /// `tree_sitter::InputEdit` thanks to the pre-edit byte offsets
    /// recorded on each `TextChange`.
    pub fn notify_highlighter(&mut self) {
        let Some(h) = self.buffer_highlighter.as_mut() else {
            return;
        };
        let (changes, full_sync) = self.document.peek_changes();
        if changes.is_empty() && !full_sync {
            return;
        }
        h.on_edit(self.document.rope(), changes, full_sync);
        self.buffer_highlighter_version = self.document.version();
    }

    // ---- Mini-buffer ----

    /// Open the mini-buffer with a prompt. `candidates` are used for
    /// completion — pass an empty Vec for free-text prompts.
    pub fn minibuffer_open(
        &mut self,
        prompt: MiniBufferPrompt,
        candidates: Vec<String>,
        action: PendingMiniBufferAction,
    ) {
        self.minibuffer.open(prompt, candidates);
        self.pending_minibuffer_action = Some(action);
        self.minibuffer_refresh_completions();
    }

    pub fn minibuffer_refresh_completions(&mut self) {
        let Some(prompt) = self.minibuffer.prompt.clone() else {
            return;
        };
        let input = self.minibuffer.input.clone();
        let candidates = match prompt {
            MiniBufferPrompt::Command => Some(self.command_completion_candidates()),
            MiniBufferPrompt::SwitchBuffer | MiniBufferPrompt::KillBuffer => {
                Some(self.buffer_names())
            }
            MiniBufferPrompt::FindFile { base_dir } => Some(path_completion_candidates(
                &base_dir,
                &input,
                PathCompletionKind::FilesAndDirectories,
            )),
            MiniBufferPrompt::FreeText { .. }
                if matches!(
                    self.pending_minibuffer_action,
                    Some(PendingMiniBufferAction::Dired)
                ) =>
            {
                #[allow(clippy::disallowed_methods)]
                let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                Some(path_completion_candidates(
                    &base_dir,
                    &input,
                    PathCompletionKind::DirectoriesOnly,
                ))
            }
            _ => None,
        };
        if let Some(candidates) = candidates {
            self.minibuffer.set_candidates(candidates);
        }
    }

    pub fn minibuffer_complete(&mut self) {
        self.minibuffer_refresh_completions();
        self.minibuffer.complete();
        self.minibuffer_refresh_completions();
    }

    /// Close without dispatching (C-g / Escape).
    pub fn minibuffer_cancel(&mut self) {
        self.minibuffer.close();
        self.pending_minibuffer_action = None;
        self.message = Some("Quit".to_string());
    }

    /// Submit the current input (Enter). Dispatches based on the
    /// pending action, then closes.
    pub fn minibuffer_submit(&mut self) {
        let Some(action) = self.pending_minibuffer_action.take() else {
            self.minibuffer.close();
            return;
        };
        let value = self.minibuffer.current_value();
        self.minibuffer.push_history(value.clone());
        self.minibuffer.close();
        self.dispatch_minibuffer(action, MiniBufferResult::Submitted(value));
    }

    fn dispatch_minibuffer(&mut self, action: PendingMiniBufferAction, result: MiniBufferResult) {
        let MiniBufferResult::Submitted(text) = result else {
            return;
        };
        match action {
            PendingMiniBufferAction::FindFile => {
                let raw = PathBuf::from(&text);
                let path = if raw.is_absolute() {
                    raw
                } else {
                    #[allow(clippy::disallowed_methods)] // user-initiated
                    std::env::current_dir()
                        .unwrap_or_else(|_| PathBuf::from("."))
                        .join(raw)
                };
                #[allow(clippy::disallowed_methods)] // user-initiated file open
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        self.open_file_as_buffer(path.clone(), &content);
                        self.message = Some(format!("Opened {}", path.display()));
                    }
                    Err(e) => {
                        self.message = Some(format!("Cannot open: {e}"));
                    }
                }
            }
            PendingMiniBufferAction::SwitchBuffer => {
                self.run_command_with_string("switch-to-buffer", text);
            }
            PendingMiniBufferAction::KillBuffer => {
                self.kill_buffer_by_name(&text);
            }
            PendingMiniBufferAction::Dired => {
                let path = if text.trim().is_empty() {
                    ".".to_string()
                } else {
                    text
                };
                self.run_command_with_string("dired", path);
            }
            PendingMiniBufferAction::RunCommand => {
                if text.is_empty() {
                    return;
                }
                self.dispatch_command_by_name(&text);
            }
            PendingMiniBufferAction::RunCommandWithInput { name } => {
                self.run_command_with_string(&name, text);
            }
            PendingMiniBufferAction::RunCommandWithInputs {
                name,
                mut prompts,
                mut values,
            } => {
                values.push(text);
                if prompts.is_empty() {
                    self.dispatch_multi_string_command(&name, values);
                } else {
                    let prompt = prompts.remove(0);
                    self.minibuffer_open(
                        MiniBufferPrompt::FreeText { label: prompt },
                        Vec::new(),
                        PendingMiniBufferAction::RunCommandWithInputs {
                            name,
                            prompts,
                            values,
                        },
                    );
                }
            }
        }
    }

    fn dispatch_multi_string_command(&mut self, name: &str, values: Vec<String>) {
        match name {
            "query-replace" if values.len() >= 2 => {
                self.query_replace_start(values[0].clone(), values[1].clone());
            }
            "query-replace-regexp" if values.len() >= 2 => {
                self.query_replace_regexp_start(values[0].clone(), values[1].clone());
            }
            _ => self.run_command_with_strings(name, values),
        }
    }

    fn dispatch_command_by_name(&mut self, name: &str) {
        let rust_interactive = self.commands.get(name).map(|cmd| cmd.interactive.clone());
        match self.lisp_host.plan_command(name, rust_interactive) {
            CommandPlan::Run => self.run_command(name, CommandArgs::default()),
            CommandPlan::Prompt(interactive) => {
                self.dispatch_interactive_command(name.to_string(), interactive);
            }
            CommandPlan::Missing => {
                self.message = Some(format!("No such command: {name}"));
            }
        }
    }

    fn dispatch_interactive_command(&mut self, name: String, interactive: InteractiveSpec) {
        match interactive {
            InteractiveSpec::None => {
                self.run_command(&name, CommandArgs::default());
            }
            InteractiveSpec::File { prompt: _ } => {
                let pending = PendingMiniBufferAction::RunCommandWithInput { name };
                #[allow(clippy::disallowed_methods)]
                let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let candidates = path_completion_candidates(
                    &base_dir,
                    "",
                    PathCompletionKind::FilesAndDirectories,
                );
                self.minibuffer_open(MiniBufferPrompt::FindFile { base_dir }, candidates, pending);
            }
            InteractiveSpec::Buffer { prompt: _ } => {
                let pending = PendingMiniBufferAction::RunCommandWithInput { name };
                self.minibuffer_open(MiniBufferPrompt::SwitchBuffer, self.buffer_names(), pending);
            }
            InteractiveSpec::String { prompt } => {
                let pending = PendingMiniBufferAction::RunCommandWithInput { name };
                self.minibuffer_open(
                    MiniBufferPrompt::FreeText { label: prompt },
                    Vec::new(),
                    pending,
                );
            }
            InteractiveSpec::StringList { mut prompts } => {
                if prompts.is_empty() {
                    self.run_command(&name, CommandArgs::default());
                    return;
                }
                let prompt = prompts.remove(0);
                let pending = PendingMiniBufferAction::RunCommandWithInputs {
                    name,
                    prompts,
                    values: Vec::new(),
                };
                self.minibuffer_open(
                    MiniBufferPrompt::FreeText { label: prompt },
                    Vec::new(),
                    pending,
                );
            }
            InteractiveSpec::Line => {
                let pending = PendingMiniBufferAction::RunCommandWithInput { name };
                self.minibuffer_open(MiniBufferPrompt::GotoLine, Vec::new(), pending);
            }
            InteractiveSpec::Confirm { prompt } => {
                let pending = PendingMiniBufferAction::RunCommandWithInput { name };
                self.minibuffer_open(
                    MiniBufferPrompt::YesNo { label: prompt },
                    Vec::new(),
                    pending,
                );
            }
        }
    }

    /// Synchronize the server-owned elisp host and load the built-in
    /// command defuns. Kept under the old name because tests and the
    /// event loop use it as the "elisp is ready" hook.
    pub fn install_elisp_editor_callbacks(&mut self) {
        self.sync_lisp_core_from_state();
        self.lisp_host.bootstrap_emacs_stdlib_for_dired();
        if let Err((idx, e)) = self.lisp_host.load_builtin_commands() {
            self.message = Some(format!("elisp init failed at form {idx}: {e:?}"));
        }
        self.sync_state_from_lisp_core();
    }

    // ---- Keyboard quit (C-g) ----

    /// Cancel any in-progress modal state — prefix arg, pending C-x
    /// or M-g chord, isearch (TODO when TUI grows it), the
    /// minibuffer (caller decides whether to close it). Sets a
    /// "Quit" message so the user gets feedback that C-g landed.
    /// Mirrors `MdAppState::abort` on the GPUI side.
    pub fn keyboard_quit(&mut self) {
        self.cancel_flag.cancel();
        self.universal_arg = None;
        self.c_x_pending = false;
        self.c_x_r_pending = false;
        self.meta_pending = false;
        self.m_g_pending = false;
        self.query_replace = None;
        self.cursor.clear_selection();
        if self.minibuffer.active {
            self.minibuffer_cancel();
        }
        self.message = Some("Quit".to_string());
    }

    // ---- Command dispatch ----

    pub fn run_command(&mut self, name: &str, args: CommandArgs) {
        self.sync_lisp_core_from_state();
        match self.lisp_host.run_command(name, &args) {
            Ok(LispCommandDispatch::Handled) => {
                self.sync_state_from_lisp_core();
                self.run_post_command_hook();
                return;
            }
            Ok(LispCommandDispatch::Unhandled) => {}
            Err(e) => {
                self.sync_state_from_lisp_core();
                self.message = Some(format!("[elisp error in {name}: {e:?}]"));
                self.run_post_command_hook();
                return;
            }
        }

        let handler = self.commands.get(name).map(|c| c.handler.clone());
        if let Some(h) = handler {
            h(self, args);
            self.sync_lisp_core_from_state();
            self.run_post_command_hook();
        } else {
            self.message = Some(format!("[Unknown command: {name}]"));
        }
    }

    pub fn run_command_with_string(&mut self, name: &str, s: String) {
        self.run_command(name, CommandArgs::default().with_string(s));
    }

    pub fn run_command_with_strings(&mut self, name: &str, strings: Vec<String>) {
        self.run_command(name, CommandArgs::default().with_strings(strings));
    }

    fn mutate_lisp_core<R>(&mut self, f: impl FnOnce(&mut EditorCore) -> R) -> R {
        self.sync_lisp_core_from_state();
        let core = self.lisp_host.core();
        let result = {
            let mut core = core.lock();
            f(&mut core)
        };
        self.sync_state_from_lisp_core();
        result
    }

    pub fn search_forward_literal(&mut self, needle: &str) -> bool {
        self.mutate_lisp_core(|core| core.search_forward(needle).is_some())
    }

    pub fn search_backward_literal(&mut self, needle: &str) -> bool {
        self.mutate_lisp_core(|core| core.search_backward(needle).is_some())
    }

    pub fn search_forward_regexp(&mut self, pattern: &str) -> bool {
        self.mutate_lisp_core(|core| core.re_search_forward(pattern).is_some())
    }

    pub fn replace_current_match(&mut self, replacement: &str) -> bool {
        self.mutate_lisp_core(|core| core.replace_match_literal(replacement))
    }

    pub fn replace_current_regexp_match(&mut self, replacement: &str) -> bool {
        self.mutate_lisp_core(|core| core.replace_match_regexp(replacement))
    }

    pub fn query_replace_start(&mut self, from: String, to: String) {
        self.query_replace_start_with_kind(from, to, QueryReplaceKind::Literal);
    }

    pub fn query_replace_regexp_start(&mut self, from: String, to: String) {
        self.query_replace_start_with_kind(from, to, QueryReplaceKind::Regexp);
    }

    fn query_replace_start_with_kind(&mut self, from: String, to: String, kind: QueryReplaceKind) {
        if from.is_empty() {
            self.message = Some("Cannot query replace empty string".to_string());
            return;
        }
        self.query_replace = Some(QueryReplaceState {
            from,
            to,
            replacements: 0,
            kind,
        });
        self.query_replace_find_next();
    }

    pub fn query_replace_accept_current(&mut self, finish: bool) {
        let Some(mut query) = self.query_replace.take() else {
            return;
        };
        if self.query_replace_current_match(&query) {
            query.replacements += 1;
        }
        if finish {
            self.message = Some(format!(
                "Replaced {} occurrence{}",
                query.replacements,
                plural(query.replacements)
            ));
        } else {
            self.query_replace = Some(query);
            self.query_replace_find_next();
        }
    }

    pub fn query_replace_skip_current(&mut self) {
        if self.query_replace.is_some() {
            self.query_replace_find_next();
        }
    }

    pub fn query_replace_replace_all_remaining(&mut self) {
        let Some(query) = self.query_replace.take() else {
            return;
        };
        let mut replacements = query.replacements;
        loop {
            if !self.query_replace_current_match(&query) {
                break;
            }
            replacements += 1;
            if !self.query_replace_search_forward(&query) {
                break;
            }
        }
        self.message = Some(format!(
            "Replaced {replacements} occurrence{}",
            plural(replacements)
        ));
    }

    pub fn query_replace_cancel(&mut self) {
        self.query_replace = None;
        self.message = Some("Quit".to_string());
    }

    fn query_replace_find_next(&mut self) {
        let Some(query) = self.query_replace.as_ref() else {
            return;
        };
        let query = query.clone();
        if self.query_replace_search_forward(&query) {
            let label = match query.kind {
                QueryReplaceKind::Literal => "Query replace",
                QueryReplaceKind::Regexp => "Query replace regexp",
            };
            self.message = Some(format!(
                "{label} {:?} with {:?}: y n ! . q",
                query.from, query.to
            ));
        } else {
            let invalid_regexp = query.kind == QueryReplaceKind::Regexp
                && self
                    .message
                    .as_deref()
                    .is_some_and(|message| message.starts_with("Invalid regexp:"));
            self.query_replace = None;
            if invalid_regexp {
                return;
            }
            self.message = Some(format!(
                "Replaced {} occurrence{}",
                query.replacements,
                plural(query.replacements)
            ));
        }
    }

    fn query_replace_search_forward(&mut self, query: &QueryReplaceState) -> bool {
        match query.kind {
            QueryReplaceKind::Literal => self.search_forward_literal(&query.from),
            QueryReplaceKind::Regexp => self.search_forward_regexp(&query.from),
        }
    }

    fn query_replace_current_match(&mut self, query: &QueryReplaceState) -> bool {
        match query.kind {
            QueryReplaceKind::Literal => self.replace_current_match(&query.to),
            QueryReplaceKind::Regexp => self.replace_current_regexp_match(&query.to),
        }
    }

    /// Run `post-command-hook`. Suppressed during macro replay so a
    /// hook bound to (e.g.) idle autosave doesn't fire once per
    /// recorded action when `C-x e` plays back the macro.
    fn run_post_command_hook(&mut self) {
        if self.macros.applying {
            return;
        }
        self.run_hook_logged("post-command-hook");
    }

    // ---- Helpers ----

    fn update_preferred_column(&mut self) {
        // Stale hover info doesn't apply to the new cursor position.
        self.lsp_hover_text = None;

        if self.document.len_chars() == 0 {
            self.cursor.preferred_column = 0;
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        self.cursor.preferred_column = pos - self.document.line_to_char(line);
    }

    fn clamp_cursor(&mut self) {
        self.cursor.position = self.cursor.position.min(self.document.len_chars());
    }

    /// Ensure scroll_line keeps the cursor visible within the viewport.
    pub fn ensure_cursor_visible(&mut self, viewport_height: usize) {
        if self.document.len_chars() == 0 {
            self.scroll_line = 0;
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let cursor_line = self.document.char_to_line(pos);
        if cursor_line < self.scroll_line {
            self.scroll_line = cursor_line;
        } else if cursor_line >= self.scroll_line + viewport_height {
            self.scroll_line = cursor_line.saturating_sub(viewport_height - 1);
        }
    }

    // ---- Text editing ----

    pub fn insert_text(&mut self, text: &str) {
        self.clamp_cursor();
        self.last_move_was_vertical = false;

        let is_single_char = text.chars().count() == 1;
        let has_selection = self.cursor.has_selection();
        let should_coalesce = is_single_char && self.last_edit_was_char_insert && !has_selection;
        if !should_coalesce {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
        }

        if let Some((start, end)) = self.cursor.selection() {
            self.document.remove(start, end);
            self.cursor.position = start;
            self.cursor.clear_selection();
        } else {
            self.cursor.clear_selection();
        }

        self.document.insert(self.cursor.position, text);
        self.cursor.position += text.chars().count();
        self.last_edit_was_char_insert = is_single_char;
        self.lsp_completion_visible = false;
        self.update_preferred_column();
        // Update tree-sitter incrementally before LSP drains the journal.
        self.notify_highlighter();
        self.lsp_did_change();
    }

    pub fn backspace(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        if let Some((start, end)) = self.cursor.selection() {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            self.document.remove(start, end);
            self.cursor.position = start;
            self.cursor.clear_selection();
        } else {
            self.cursor.clear_selection();
            if self.cursor.position > 0 {
                self.history
                    .push_undo(self.document.snapshot(), self.cursor.position);
                self.cursor.position -= 1;
                self.document
                    .remove(self.cursor.position, self.cursor.position + 1);
            }
        }
        self.lsp_completion_visible = false;
        self.update_preferred_column();
        self.notify_highlighter();
        self.lsp_did_change();
    }

    pub fn delete_forward(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        if let Some((start, end)) = self.cursor.selection() {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            self.document.remove(start, end);
            self.cursor.position = start;
            self.cursor.clear_selection();
        } else {
            self.cursor.clear_selection();
            if self.cursor.position < self.document.len_chars() {
                self.history
                    .push_undo(self.document.snapshot(), self.cursor.position);
                self.document
                    .remove(self.cursor.position, self.cursor.position + 1);
            }
        }
        self.lsp_completion_visible = false;
        self.update_preferred_column();
        self.notify_highlighter();
        self.lsp_did_change();
    }

    pub fn kill_line(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.clamp_cursor();
        if self.document.len_chars() == 0 {
            return;
        }
        let pos = self.cursor.position;
        let line = self.document.char_to_line(pos);
        let line_start = self.document.line_to_char(line);
        let line_len = self.document.line(line).len_chars();
        let line_end = line_start + line_len;

        self.history
            .push_undo(self.document.snapshot(), self.cursor.position);

        if pos == line_end.saturating_sub(1)
            && line < self.document.len_lines() - 1
            && self.document.line(line).to_string().ends_with('\n')
        {
            // At end of line with trailing newline — kill the newline
            let killed = "\n".to_string();
            self.document.remove(pos, pos + 1);
            self.kill_ring.push(killed);
        } else {
            // Kill to end of line (not including newline)
            let end = if line_end > 0 && self.document.line(line).to_string().ends_with('\n') {
                line_end - 1
            } else {
                line_end
            };
            if end > pos {
                let text = self.document.rope().slice(pos..end).to_string();
                self.document.remove(pos, end);
                self.kill_ring.push(text);
            }
        }
        // Keep tree-sitter + LSP in sync. `notify_highlighter` peeks
        // the change journal so it must run before `lsp_did_change`
        // drains it — same ordering as the other edit paths.
        self.notify_highlighter();
        self.lsp_did_change();
    }

    // ---- Undo / redo ----

    pub fn undo(&mut self) {
        self.last_edit_was_char_insert = false;
        if let Some((snapshot, cursor)) = self
            .history
            .undo(self.document.snapshot(), self.cursor.position)
        {
            self.document.restore(snapshot);
            self.cursor.position = cursor.min(self.document.len_chars());
            self.cursor.clear_selection();
            // Restore sets full_sync_needed on the change journal;
            // notify triggers a full reparse.
            self.notify_highlighter();
            self.lsp_did_change();
        }
    }

    pub fn redo(&mut self) {
        self.last_edit_was_char_insert = false;
        if let Some((snapshot, cursor)) = self
            .history
            .redo(self.document.snapshot(), self.cursor.position)
        {
            self.document.restore(snapshot);
            self.cursor.position = cursor.min(self.document.len_chars());
            self.cursor.clear_selection();
            // Restore sets full_sync_needed on the change journal;
            // notify triggers a full reparse.
            self.notify_highlighter();
            self.lsp_did_change();
        }
    }

    // ---- Cursor movement ----

    pub fn move_left(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let extend = extend || self.cursor.anchor.is_some();
        if !extend && let Some((start, _)) = self.cursor.selection() {
            self.cursor.position = start;
            self.cursor.clear_selection();
            self.update_preferred_column();
            return;
        }
        if self.cursor.position > 0 {
            self.cursor.move_to(self.cursor.position - 1, extend);
        }
        self.update_preferred_column();
    }

    pub fn move_right(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let extend = extend || self.cursor.anchor.is_some();
        if !extend && let Some((_, end)) = self.cursor.selection() {
            self.cursor.position = end;
            self.cursor.clear_selection();
            self.update_preferred_column();
            return;
        }
        if self.cursor.position < self.document.len_chars() {
            self.cursor.move_to(self.cursor.position + 1, extend);
        }
        self.update_preferred_column();
    }

    pub fn move_up(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        let extend = extend || self.cursor.anchor.is_some();
        if self.document.len_chars() == 0 {
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        if line == 0 {
            self.cursor.move_to(0, extend);
            if !self.last_move_was_vertical {
                self.cursor.preferred_column = 0;
            }
            self.last_move_was_vertical = true;
            return;
        }

        let current_col = pos - self.document.line_to_char(line);
        let target_col = if self.last_move_was_vertical {
            self.cursor.preferred_column
        } else {
            current_col
        };

        let prev_line = line - 1;
        let prev_line_start = self.document.line_to_char(prev_line);
        let prev_line_chars = self.document.line(prev_line).len_chars();
        let max_col = if prev_line < self.document.len_lines() - 1 {
            prev_line_chars.saturating_sub(1)
        } else {
            prev_line_chars
        };
        let actual_col = target_col.min(max_col);
        self.cursor.move_to(prev_line_start + actual_col, extend);
        self.cursor.preferred_column = target_col;
        self.last_move_was_vertical = true;
    }

    pub fn move_down(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        let extend = extend || self.cursor.anchor.is_some();
        if self.document.len_chars() == 0 {
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        if line >= self.document.len_lines() - 1 {
            self.cursor.move_to(self.document.len_chars(), extend);
            if !self.last_move_was_vertical {
                self.cursor.preferred_column = pos - self.document.line_to_char(line);
            }
            self.last_move_was_vertical = true;
            return;
        }

        let current_col = pos - self.document.line_to_char(line);
        let target_col = if self.last_move_was_vertical {
            self.cursor.preferred_column
        } else {
            current_col
        };

        let next_line = line + 1;
        let next_line_start = self.document.line_to_char(next_line);
        let next_line_chars = self.document.line(next_line).len_chars();
        let max_col = if next_line < self.document.len_lines() - 1 {
            next_line_chars.saturating_sub(1)
        } else {
            next_line_chars
        };
        let actual_col = target_col.min(max_col);
        self.cursor.move_to(next_line_start + actual_col, extend);
        self.cursor.preferred_column = target_col;
        self.last_move_was_vertical = true;
    }

    pub fn move_to_line_start(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let extend = extend || self.cursor.anchor.is_some();
        if self.document.len_chars() == 0 {
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        let line_start = self.document.line_to_char(line);
        self.cursor.move_to(line_start, extend);
        self.update_preferred_column();
    }

    pub fn move_to_line_end(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let extend = extend || self.cursor.anchor.is_some();
        if self.document.len_chars() == 0 {
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        let line_start = self.document.line_to_char(line);
        let line_len = self.document.line(line).len_chars();
        let line_end = line_start + line_len;
        // Don't go past newline
        let target = if line < self.document.len_lines() - 1 {
            line_end.saturating_sub(1)
        } else {
            line_end
        };
        self.cursor.move_to(target, extend);
        self.update_preferred_column();
    }

    pub fn move_to_buffer_start(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let extend = extend || self.cursor.anchor.is_some();
        self.cursor.move_to(0, extend);
        self.update_preferred_column();
    }

    pub fn move_to_buffer_end(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let extend = extend || self.cursor.anchor.is_some();
        self.cursor.move_to(self.document.len_chars(), extend);
        self.update_preferred_column();
    }

    // ---- File I/O ----

    /// Save the active buffer to its backing file.
    ///
    /// Returns `Ok(())` on success, `Err(io::Error)` when the write
    /// failed or the buffer has no file path yet. Sets `self.message`
    /// in both cases so the echo area still tells the user what
    /// happened — callers that need to gate destructive follow-up
    /// actions (e.g. `save-buffers-kill-emacs`) must check the
    /// `Result`, not the message.
    pub fn save_file(&mut self) -> std::io::Result<()> {
        let Some(path) = self.document.file_path().cloned() else {
            self.message = Some("No file name".to_string());
            return Err(std::io::Error::other("no file name"));
        };
        self.run_hook_logged("before-save-hook");
        match write_document_to_path(&mut self.document, &path) {
            Ok(()) => {
                self.message = Some(format!("Wrote {}", path.display()));
                self.lsp_did_save();
                self.run_hook_logged("after-save-hook");
                Ok(())
            }
            Err(e) => {
                self.message = Some(format!("Error saving: {e}"));
                Err(e)
            }
        }
    }

    /// Save every dirty buffer that has a backing file.
    ///
    /// Non-file buffers such as `*scratch*`, `*Diagnostics*`, and
    /// generated listings are intentionally skipped. This matches the
    /// quit path's job: protect file-backed edits from being lost,
    /// while still letting a fresh TUI session exit with `C-x C-c`.
    pub fn save_dirty_file_buffers(&mut self) -> std::io::Result<usize> {
        let mut saved = 0;

        if self.document.needs_file_save() {
            self.save_file()?;
            saved += 1;
        }

        for buffer in &mut self.stored_buffers {
            if !buffer.document.needs_file_save() {
                continue;
            }
            let path = buffer
                .document
                .file_path()
                .cloned()
                .expect("needs_file_save guarantees a file path");
            if let Err(e) = write_document_to_path(&mut buffer.document, &path) {
                self.message = Some(format!("Error saving {}: {e}", path.display()));
                return Err(e);
            }
            saved += 1;
        }

        self.message = Some(match saved {
            0 => "No file buffers need saving".to_string(),
            1 => "Wrote 1 buffer".to_string(),
            n => format!("Wrote {n} buffers"),
        });
        Ok(saved)
    }

    // ---- Buffer info helpers ----

    pub fn cursor_line(&self) -> usize {
        if self.document.len_chars() == 0 {
            return 0;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        self.document.char_to_line(pos)
    }

    pub fn cursor_col(&self) -> usize {
        if self.document.len_chars() == 0 {
            return 0;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        pos - self.document.line_to_char(line)
    }

    // ---- LSP integration ----

    /// Initialize the LSP registry and take the event receiver.
    pub fn init_lsp(&mut self) {
        let config = LspConfig::load_or_default();
        let mut registry = LspRegistry::new(config);
        self.lsp_event_rx = registry.take_event_receiver();
        self.lsp_registry = Some(registry);
    }

    /// Open LSP session for the active buffer.
    pub fn lsp_did_open(&mut self) {
        let Some(path) = self.document.file_path().cloned() else {
            return;
        };
        if self.lsp_buffer_state.is_some() {
            return;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        let Some(uri) = uri_from_path(&path) else {
            return;
        };
        let text = self.document.text();

        if self.lsp_registry.is_none() {
            self.init_lsp();
        }
        let registry = self.lsp_registry.as_mut().expect("just created");

        let Some(server_name) = registry.ensure_server_for_file(&path) else {
            return;
        };
        let Some(config) = registry.config_for_extension(&ext) else {
            return;
        };
        let language_id = config.language_id.clone();
        let mut buf_state =
            LspBufferState::new(uri.clone(), language_id.clone(), server_name.clone());
        let version = buf_state.next_version();

        // Send didOpen if the server is ready; otherwise defer until ServerStarted.
        if let Some(client) = registry.client(&server_name) {
            buf_state.did_open_sent = true;
            registry.runtime_handle().spawn(async move {
                if let Err(e) = client.did_open(uri, &language_id, version, &text).await {
                    log::error!("LSP didOpen error: {e}");
                }
            });
        }
        self.lsp_buffer_state = Some(buf_state);
    }

    /// Flush `didOpen` for the active buffer if its server just finished
    /// initializing and we haven't opened it yet. TUI doesn't currently
    /// track stored buffer LSP state separately.
    fn flush_pending_did_open(&mut self, server_name: &str) {
        let Some(ref mut lsp_state) = self.lsp_buffer_state else {
            return;
        };
        if lsp_state.did_open_sent || lsp_state.server_name != server_name {
            return;
        }
        let uri = lsp_state.uri.clone();
        let language_id = lsp_state.language_id.clone();
        let version = lsp_state.lsp_version;
        let text = self.document.text();
        lsp_state.did_open_sent = true;
        if let Some(ref registry) = self.lsp_registry
            && let Some(client) = registry.client(server_name)
        {
            registry.runtime_handle().spawn(async move {
                if let Err(e) = client.did_open(uri, &language_id, version, &text).await {
                    log::error!("LSP deferred didOpen error: {e}");
                }
            });
        }
    }

    /// Notify LSP of document changes.
    pub fn lsp_did_change(&mut self) {
        let Some(ref mut lsp_state) = self.lsp_buffer_state else {
            return;
        };
        let (changes, full_sync) = self.document.drain_changes();
        if changes.is_empty() && !full_sync {
            return;
        }
        let version = lsp_state.next_version();
        let uri = lsp_state.uri.clone();
        let server_name = lsp_state.server_name.clone();

        let content_changes = if full_sync {
            vec![lsp_types::TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: self.document.text(),
            }]
        } else {
            changes
                .into_iter()
                .map(|c| lsp_types::TextDocumentContentChangeEvent {
                    range: Some(lsp_types::Range {
                        start: lsp_types::Position {
                            line: c.start_line,
                            character: c.start_character,
                        },
                        end: lsp_types::Position {
                            line: c.end_line,
                            character: c.end_character,
                        },
                    }),
                    range_length: None,
                    text: c.new_text,
                })
                .collect()
        };

        if let Some(ref registry) = self.lsp_registry
            && let Some(client) = registry.client(&server_name)
        {
            registry.runtime_handle().spawn(async move {
                if let Err(e) = client.did_change(uri, version, content_changes).await {
                    log::error!("LSP didChange error: {e}");
                }
            });
        }
    }

    /// Notify the LSP server that the active buffer was saved.
    pub fn lsp_did_save(&mut self) {
        let Some(ref lsp_state) = self.lsp_buffer_state else {
            return;
        };
        let uri = lsp_state.uri.clone();
        let server_name = lsp_state.server_name.clone();
        let text = self.document.text();

        if let Some(ref registry) = self.lsp_registry
            && let Some(client) = registry.client(&server_name)
        {
            registry.runtime_handle().spawn(async move {
                if let Err(e) = client.did_save(uri, Some(&text)).await {
                    log::error!("LSP didSave error: {e}");
                }
            });
        }
    }

    /// Notify the LSP server that the active buffer was closed.
    pub fn lsp_did_close(&mut self) {
        let Some(lsp_state) = self.lsp_buffer_state.take() else {
            return;
        };
        let uri = lsp_state.uri.clone();
        let server_name = lsp_state.server_name.clone();

        if let Some(ref registry) = self.lsp_registry
            && let Some(client) = registry.client(&server_name)
        {
            registry.runtime_handle().spawn(async move {
                if let Err(e) = client.did_close(uri).await {
                    log::error!("LSP didClose error: {e}");
                }
            });
        }
    }

    /// Poll for pending LSP events (non-blocking). Call in event loop.
    pub fn poll_lsp_events(&mut self) {
        let mut events = Vec::new();
        if let Some(ref mut rx) = self.lsp_event_rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            self.handle_lsp_event(event);
        }
    }

    fn handle_lsp_event(&mut self, event: LspEvent) {
        match event {
            LspEvent::Diagnostics {
                uri, diagnostics, ..
            } => {
                if let Some(ref mut lsp_state) = self.lsp_buffer_state
                    && lsp_state.uri == uri
                {
                    lsp_state.diagnostics = diagnostics;
                }
            }
            LspEvent::CompletionResponse { items, .. } => {
                self.lsp_completion_items = items;
                self.lsp_completion_selected = 0;
                self.lsp_completion_visible = !self.lsp_completion_items.is_empty();
            }
            LspEvent::HoverResponse { contents, .. } => {
                self.lsp_hover_text = contents.map(rele_server::lsp::hover_contents_to_string);
                if let Some(ref text) = self.lsp_hover_text {
                    self.message = Some(text.lines().next().unwrap_or("").to_string());
                }
            }
            LspEvent::DefinitionResponse { locations, .. } => {
                if locations.len() == 1 {
                    let loc = &locations[0];
                    let uri_str = loc.uri.as_str();
                    if let Some(path_str) = uri_str.strip_prefix("file://") {
                        let path = PathBuf::from(path_str);
                        // Route through `open_file_as_buffer` so the
                        // old buffer is pushed to `stored_buffers`,
                        // LSP is properly handed off, and
                        // tree-sitter sees the language switch.
                        #[allow(clippy::disallowed_methods)] // TODO(perf): move off UI thread
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            self.open_file_as_buffer(path, &content);
                            let target = rele_server::lsp::position::position_to_char_offset(
                                self.document.rope(),
                                &loc.range.start,
                            );
                            self.cursor.position = target.min(self.document.len_chars());
                            self.cursor.clear_selection();
                        }
                    }
                } else if locations.is_empty() {
                    self.message = Some("Definition: no results".to_string());
                } else {
                    self.message = Some(format!("Definition: {} results", locations.len()));
                }
            }
            LspEvent::FormattingResponse { edits, .. } => {
                if !edits.is_empty() {
                    self.history
                        .push_undo(self.document.snapshot(), self.cursor.position);
                    let mut sorted = edits;
                    sorted.sort_by(|a, b| {
                        b.range
                            .start
                            .line
                            .cmp(&a.range.start.line)
                            .then(b.range.start.character.cmp(&a.range.start.character))
                    });
                    for edit in sorted {
                        let start = rele_server::lsp::position::position_to_char_offset(
                            self.document.rope(),
                            &edit.range.start,
                        );
                        let end = rele_server::lsp::position::position_to_char_offset(
                            self.document.rope(),
                            &edit.range.end,
                        );
                        if start < end {
                            self.document.remove(start, end);
                        }
                        if !edit.new_text.is_empty() {
                            self.document.insert(start, &edit.new_text);
                        }
                    }
                    // Each edit mutates the rope and journals a
                    // TextChange; drain them into tree-sitter and LSP
                    // so both stay in sync with the formatted buffer.
                    self.notify_highlighter();
                    self.lsp_did_change();
                    self.message = Some("Formatted".to_string());
                }
            }
            LspEvent::ServerStarted { server_name } => {
                self.lsp_status = Some(format!("LSP: {server_name}"));
                self.message = Some(format!("LSP: {server_name} started"));
                self.flush_pending_did_open(&server_name);
            }
            LspEvent::ServerExited { server_name, .. } => {
                self.lsp_status = None;
                self.message = Some(format!("LSP: {server_name} exited"));
            }
            LspEvent::Error { message } => {
                self.message = Some(format!("LSP error: {message}"));
            }
            LspEvent::ReferencesResponse { locations, .. } => {
                if locations.is_empty() {
                    self.message = Some("References: no results".to_string());
                } else {
                    self.message = Some(format!("References: {} results", locations.len()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rele_server::minibuffer::MiniBufferPrompt;
    use tempfile::tempdir;

    #[test]
    fn minibuffer_opens_find_file_prompt() {
        let mut s = TuiAppState::new();
        let base = PathBuf::from("/tmp");
        s.minibuffer_open(
            MiniBufferPrompt::FindFile { base_dir: base },
            Vec::new(),
            PendingMiniBufferAction::FindFile,
        );
        assert!(s.minibuffer.active);
        assert!(matches!(
            s.pending_minibuffer_action,
            Some(PendingMiniBufferAction::FindFile)
        ));
    }

    #[test]
    fn find_file_minibuffer_completes_paths() {
        let dir = tempdir().unwrap();
        let note = dir.path().join("notes.md");
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(note, "# notes\n").unwrap();

        let mut s = TuiAppState::new();
        s.minibuffer_open(
            MiniBufferPrompt::FindFile {
                base_dir: dir.path().to_path_buf(),
            },
            Vec::new(),
            PendingMiniBufferAction::FindFile,
        );

        s.minibuffer.add_char('n');
        s.minibuffer_refresh_completions();
        s.minibuffer_complete();

        assert_eq!(s.minibuffer.input, "notes.md");
        assert_eq!(s.minibuffer.current_value(), "notes.md");
    }

    #[test]
    fn dired_minibuffer_completes_directories_only() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("nested");
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::create_dir(&nested).unwrap();
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(dir.path().join("notes.md"), "# notes\n").unwrap();

        let candidates =
            path_completion_candidates(dir.path(), "n", PathCompletionKind::DirectoriesOnly);

        assert_eq!(
            candidates,
            vec![format!("nested{}", std::path::MAIN_SEPARATOR)]
        );
    }

    #[test]
    fn command_completion_candidates_include_elisp_defuns() {
        let mut s = TuiAppState::new();
        s.install_elisp_editor_callbacks();

        let candidates = s.command_completion_candidates();

        assert!(candidates.iter().any(|name| name == "next-line"));
        assert!(candidates.iter().any(|name| name == "save-buffer"));
        assert!(candidates.iter().any(|name| name == "rust-mode"));
    }

    #[test]
    fn minibuffer_cancel_closes_without_dispatch() {
        let mut s = TuiAppState::new();
        s.minibuffer_open(
            MiniBufferPrompt::SwitchBuffer,
            vec!["*scratch*".into()],
            PendingMiniBufferAction::SwitchBuffer,
        );
        s.minibuffer_cancel();
        assert!(!s.minibuffer.active);
        assert!(s.pending_minibuffer_action.is_none());
    }

    #[test]
    fn switch_to_buffer_command_uses_elisp_buffer_bridge() {
        let mut s = TuiAppState::new();
        s.get_or_create_named_buffer("notes.md");
        s.install_elisp_editor_callbacks();

        s.run_command_with_string("switch-to-buffer", "notes.md".to_string());

        assert_eq!(s.current_buffer_name, "notes.md");

        s.run_command_with_string("switch-to-buffer", "new-scratch".to_string());

        assert_eq!(s.current_buffer_name, "new-scratch");
    }

    #[test]
    fn list_buffers_command_builds_buffer_list_buffer() {
        let mut s = TuiAppState::new();
        s.get_or_create_named_buffer("notes.md");
        s.install_elisp_editor_callbacks();

        s.run_command("list-buffers", CommandArgs::default());

        assert_eq!(s.current_buffer_name, "*Buffer List*");
        assert_eq!(s.current_buffer_kind, BufferKind::BufferList);
        assert!(s.current_buffer_read_only);
        let text = s.document.text();
        assert!(text.contains("*scratch*"));
        assert!(text.contains("notes.md"));
    }

    #[test]
    fn dired_command_builds_dired_buffer_from_elisp() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("entry.txt");
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(&path, "hello").unwrap();

        let mut s = TuiAppState::new();
        s.install_elisp_editor_callbacks();

        s.run_command_with_string("dired", dir.path().display().to_string());

        assert!(s.current_buffer_name.starts_with("*Dired: "));
        assert_eq!(s.current_buffer_kind, BufferKind::Dired);
        assert!(s.current_buffer_read_only);
        assert!(s.document.text().contains("entry.txt"));
    }

    #[test]
    fn minibuffer_submit_find_file_opens_buffer() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("greeting.md");
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(&path, "# hello\n").unwrap();

        let mut s = TuiAppState::new();
        let starting_buffers = s.buffer_count();
        s.minibuffer_open(
            MiniBufferPrompt::FindFile {
                base_dir: dir.path().to_path_buf(),
            },
            Vec::new(),
            PendingMiniBufferAction::FindFile,
        );
        // Type the absolute path (simpler than exercising tab-completion).
        for ch in path.to_str().unwrap().chars() {
            s.minibuffer.add_char(ch);
        }
        s.minibuffer_submit();
        assert!(!s.minibuffer.active);
        // A new buffer was created (the scratch is now stored).
        assert_eq!(s.buffer_count(), starting_buffers + 1);
        assert_eq!(s.current_buffer_name, "greeting.md");
        assert_eq!(s.document.text(), "# hello\n");
    }

    #[test]
    fn switch_and_kill_buffer_round_trip() {
        // Build a state with two file buffers (no FS reads — we craft
        // `open_file_as_buffer` inputs directly).
        let mut s = TuiAppState::new();
        s.open_file_as_buffer(PathBuf::from("/tmp/a.md"), "content a");
        // Opening this replaces the scratch with "a.md". Scratch is now stored.
        assert_eq!(s.current_buffer_name, "a.md");
        assert_eq!(s.buffer_count(), 2);

        s.open_file_as_buffer(PathBuf::from("/tmp/b.md"), "content b");
        assert_eq!(s.current_buffer_name, "b.md");
        assert_eq!(s.buffer_count(), 3);

        // Switch back to a.md.
        assert!(s.switch_to_buffer_by_name("a.md"));
        assert_eq!(s.current_buffer_name, "a.md");
        assert_eq!(s.document.text(), "content a");

        // Switch to non-existent buffer — fails, state unchanged.
        assert!(!s.switch_to_buffer_by_name("does-not-exist"));
        assert_eq!(s.current_buffer_name, "a.md");

        // Kill the current (a.md) — falls back to whatever was stored last.
        assert!(s.kill_current_buffer());
        assert_eq!(s.buffer_count(), 2);
        assert_ne!(s.current_buffer_name, "a.md");
    }

    #[test]
    fn kill_refuses_when_only_one_buffer() {
        let mut s = TuiAppState::new();
        assert_eq!(s.buffer_count(), 1);
        assert!(!s.kill_current_buffer());
        assert_eq!(s.buffer_count(), 1);
        assert!(s.message.as_deref().unwrap_or("").contains("only buffer"));
    }

    #[test]
    fn m_x_submits_known_command() {
        let mut s = TuiAppState::new();
        // Pre-populate some buffer content so `next-line` has somewhere to go.
        s.insert_text("line1\nline2\nline3\n");
        s.cursor.position = 0;
        // `next-line` is a defun in commands.el now — install the elisp
        // bridge so run_command routes through it correctly. Without
        // this, the global obarray may still have the function cell
        // set by a previously-run test and forward-line would fall
        // back to the stub buffer (no-op).
        s.install_elisp_editor_callbacks();
        // Open M-x minibuffer — candidates are all registered command names.
        let candidates: Vec<String> = s.commands.names().to_vec();
        s.minibuffer_open(
            MiniBufferPrompt::Command,
            candidates,
            PendingMiniBufferAction::RunCommand,
        );
        // Type "next-line" and submit.
        for ch in "next-line".chars() {
            s.minibuffer.add_char(ch);
        }
        s.minibuffer_submit();
        // Should have moved the cursor to line 1.
        assert_eq!(s.cursor_line(), 1);
        // Minibuffer closed.
        assert!(!s.minibuffer.active);
    }

    #[test]
    fn m_x_runs_autoloaded_rust_mode() {
        let mut s = TuiAppState::new();
        s.minibuffer_open(
            MiniBufferPrompt::Command,
            s.command_completion_candidates(),
            PendingMiniBufferAction::RunCommand,
        );
        for ch in "rust-mode".chars() {
            s.minibuffer.add_char(ch);
        }

        s.minibuffer_submit();

        assert_eq!(s.current_major_mode.as_deref(), Some("rust-mode"));
        assert!(!s.minibuffer.active);
    }

    #[test]
    fn m_x_unknown_command_reports_message() {
        let mut s = TuiAppState::new();
        s.minibuffer_open(
            MiniBufferPrompt::Command,
            Vec::new(),
            PendingMiniBufferAction::RunCommand,
        );
        for ch in "no-such-command".chars() {
            s.minibuffer.add_char(ch);
        }
        s.minibuffer_submit();
        let msg = s.message.as_deref().unwrap_or("");
        assert!(
            msg.contains("No such command"),
            "expected not-found message, got {msg:?}"
        );
    }

    #[test]
    fn scroll_up_command_moves_cursor_forward() {
        let mut s = TuiAppState::new();
        // 50 lines of content, cursor at line 0.
        let text: String = (0..50).map(|i| format!("line {i}\n")).collect();
        s.insert_text(&text);
        s.cursor.position = 0;
        s.last_viewport_height = 20; // simulate a 20-line viewport
        // Invoke scroll-up-command (C-v): cursor should move forward
        // by page_size = viewport - 2 = 18 lines.
        s.run_command("scroll-up-command", CommandArgs::default());
        assert_eq!(s.cursor_line(), 18);
    }

    #[test]
    fn scroll_down_command_moves_cursor_backward() {
        let mut s = TuiAppState::new();
        let text: String = (0..50).map(|i| format!("line {i}\n")).collect();
        s.insert_text(&text);
        // Start near the end.
        s.cursor.position = s.document.len_chars();
        let start_line = s.cursor_line();
        s.last_viewport_height = 20;
        // M-v: cursor should move back 18 lines.
        s.run_command("scroll-down-command", CommandArgs::default());
        assert_eq!(s.cursor_line(), start_line - 18);
    }

    #[test]
    fn opening_rust_file_installs_tree_sitter_highlighter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("example.rs");
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(&path, "fn main() { println!(\"hi\"); }\n").unwrap();

        let mut s = TuiAppState::new();
        assert!(
            s.buffer_highlighter.is_none(),
            "scratch buffer has no grammar"
        );
        #[allow(clippy::disallowed_methods)] // test fixture
        let content = std::fs::read_to_string(&path).unwrap();
        s.open_file_as_buffer(path, &content);
        assert!(
            s.buffer_highlighter.is_some(),
            ".rs file should pick up the rust grammar"
        );
        assert_eq!(s.current_major_mode.as_deref(), Some("rust-mode"));
        assert_eq!(
            rele_elisp::lookup_mode_key(s.lisp_host.interpreter(), "rust-mode", "C-c C-c"),
            Some("rust-compile".to_string())
        );

        // Trigger a parse + query.
        s.ensure_highlight_fresh();
        let ranges =
            s.buffer_highlighter
                .as_ref()
                .unwrap()
                .highlight_range(s.document.rope(), 0, 1);
        // Line 0 should have at least one highlight (the `fn` keyword).
        assert!(
            ranges.iter().any(|(line, r)| *line == 0 && !r.is_empty()),
            "expected highlight ranges on line 0, got {ranges:?}"
        );
    }

    #[test]
    fn elisp_find_file_loads_rust_mode() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("via-elisp.rs");
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(&path, "fn main() {}\n").unwrap();
        let path = path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let expr = format!("(find-file \"{path}\")");

        let mut s = TuiAppState::new();
        s.eval_lisp(rele_elisp::read(&expr).unwrap())
            .expect("find-file should evaluate");

        assert_eq!(s.current_major_mode.as_deref(), Some("rust-mode"));
        assert_eq!(
            rele_elisp::lookup_mode_key(s.lisp_host.interpreter(), "rust-mode", "C-c C-c"),
            Some("rust-compile".to_string())
        );
    }

    #[test]
    fn opening_unknown_extension_leaves_highlighter_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("notes.xyz");
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(&path, "plain\n").unwrap();

        let mut s = TuiAppState::new();
        #[allow(clippy::disallowed_methods)] // test fixture
        let content = std::fs::read_to_string(&path).unwrap();
        s.open_file_as_buffer(path, &content);
        assert!(s.buffer_highlighter.is_none());
    }

    #[test]
    fn edit_updates_tree_sitter_incrementally() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("a.rs");
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(&path, "fn a() {}\n").unwrap();

        let mut s = TuiAppState::new();
        #[allow(clippy::disallowed_methods)] // test fixture
        let content = std::fs::read_to_string(&path).unwrap();
        s.open_file_as_buffer(path, &content);
        s.ensure_highlight_fresh();
        let v1 = s.buffer_highlighter_version;
        // Type at end of buffer.
        s.cursor.position = s.document.len_chars();
        s.insert_text("fn b() {}\n");
        // The edit path calls notify_highlighter, which bumps the
        // version to the document's current one.
        assert!(s.buffer_highlighter_version > v1);
        assert_eq!(s.buffer_highlighter_version, s.document.version());
    }

    /// Regression: elisp `save-buffer` must report nil when the active
    /// buffer has no file path. The scratch buffer has no file path, so
    /// saving it is a genuine failure.
    #[test]
    fn elisp_save_buffer_returns_false_when_no_file_name() {
        let mut s = TuiAppState::new();
        s.install_elisp_editor_callbacks();
        assert!(s.document.file_path().is_none());

        let result = s
            .eval_lisp(rele_elisp::read("(save-buffer)").unwrap())
            .expect("save-buffer eval");

        assert_eq!(result, rele_elisp::LispObject::nil());
    }

    /// Regression: M-x used to gate submission on the Rust command
    /// registry alone — defuns loaded from elisp with no Rust twin
    /// were reported as "No such command" and never dispatched. The
    /// minibuffer submit now also honours user-authored elisp
    /// functions.
    #[test]
    fn minibuffer_submit_runs_elisp_only_defuns() {
        let mut s = TuiAppState::new();
        s.install_elisp_editor_callbacks();
        s.insert_text("abc");
        s.cursor.position = 0;

        // Define a defun that isn't registered on the Rust side.
        s.eval_lisp_source("(defun my-elisp-only-jump (&optional _n) (goto-char 2))")
            .expect("defun eval");

        s.minibuffer_open(
            MiniBufferPrompt::Command,
            Vec::new(),
            PendingMiniBufferAction::RunCommand,
        );
        for ch in "my-elisp-only-jump".chars() {
            s.minibuffer.add_char(ch);
        }
        s.minibuffer_submit();

        assert_eq!(
            s.cursor.position, 2,
            "elisp-only defun should have moved the cursor"
        );
        let msg = s.message.as_deref().unwrap_or("");
        assert!(
            !msg.contains("No such command"),
            "minibuffer should not report unknown for elisp defuns, got {msg:?}"
        );
    }

    /// Regression: `save-buffers-kill-emacs` used to set
    /// `should_quit = true` even when the preceding `save_file` call
    /// failed, silently discarding edits. The handler now checks
    /// `save_file`'s `Result` and refuses to quit on failure.
    #[test]
    fn save_buffers_kill_emacs_refuses_to_quit_when_save_fails() {
        let dir = tempdir().unwrap();
        // Path whose parent directory doesn't exist — `fs::write`
        // will fail on this.
        let bogus_path = dir.path().join("missing_dir").join("file.md");

        let mut s = TuiAppState::new();
        s.document = DocumentBuffer::from_file(bogus_path, "contents");
        s.document.insert(s.document.len_chars(), "!");
        s.refresh_buffer_highlighter();
        s.current_buffer_kind = BufferKind::File;

        s.run_command("save-buffers-kill-emacs", CommandArgs::default());

        assert!(
            !s.should_quit,
            "save-buffers-kill-emacs must not quit when save_file fails"
        );
        // And the user should see an error.
        let msg = s.message.as_deref().unwrap_or("");
        assert!(
            msg.to_lowercase().contains("error") || msg.to_lowercase().contains("cannot"),
            "expected error message, got {msg:?}"
        );
    }

    #[test]
    fn save_buffers_kill_terminal_quits_from_scratch_buffer() {
        let mut s = TuiAppState::new();

        s.run_command("save-buffers-kill-terminal", CommandArgs::default());

        assert!(
            s.should_quit,
            "scratch buffers have no file name and should not block C-x C-c"
        );
    }

    #[test]
    fn save_buffers_kill_terminal_saves_dirty_file_before_quit() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("note.md");
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(&path, "hello").unwrap();

        let mut s = TuiAppState::from_file(path.clone(), "hello");
        s.cursor.position = s.document.len_chars();
        s.insert_text("!");

        s.run_command("save-buffers-kill-terminal", CommandArgs::default());

        assert!(s.should_quit);
        assert!(!s.document.is_dirty());
        #[allow(clippy::disallowed_methods)] // test fixture
        let saved = std::fs::read_to_string(&path).unwrap();
        assert_eq!(saved, "hello!");
    }

    #[test]
    fn save_buffers_kill_terminal_saves_dirty_stored_file_before_quit() {
        let dir = tempdir().unwrap();
        let first_path = dir.path().join("first.md");
        let second_path = dir.path().join("second.md");
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(&first_path, "first").unwrap();
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(&second_path, "second").unwrap();

        let mut s = TuiAppState::from_file(first_path.clone(), "first");
        s.cursor.position = s.document.len_chars();
        s.insert_text("!");
        #[allow(clippy::disallowed_methods)] // test fixture
        let second_content = std::fs::read_to_string(&second_path).unwrap();
        s.open_file_as_buffer(second_path, &second_content);

        s.run_command("save-buffers-kill-terminal", CommandArgs::default());

        assert!(s.should_quit);
        #[allow(clippy::disallowed_methods)] // test fixture
        let saved = std::fs::read_to_string(&first_path).unwrap();
        assert_eq!(saved, "first!");
    }

    /// Regression: jumping to a definition in a different file used
    /// to replace `self.document` in place, losing the previous
    /// buffer entirely. Should go through `open_file_as_buffer` so
    /// the current buffer gets pushed to `stored_buffers` and LSP is
    /// wired up properly.
    #[test]
    fn definition_response_preserves_current_buffer() {
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("src.rs");
        let def_path = dir.path().join("def.rs");
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(&src_path, "use crate::foo;\n").unwrap();
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(&def_path, "pub fn foo() {}\n").unwrap();

        let mut s = TuiAppState::new();
        #[allow(clippy::disallowed_methods)] // test fixture
        let src_content = std::fs::read_to_string(&src_path).unwrap();
        s.open_file_as_buffer(src_path.clone(), &src_content);
        let src_buffer_count_before = s.buffer_count();

        // Simulate rust-analyzer returning a single location in the
        // other file.
        let def_uri: lsp_types::Uri = rele_server::lsp::position::uri_from_path(&def_path).unwrap();
        let location = lsp_types::Location {
            uri: def_uri,
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
            },
        };
        s.handle_lsp_event(rele_server::lsp::LspEvent::DefinitionResponse {
            request_id: 1,
            locations: vec![location],
        });

        // The previous buffer must still be reachable: buffer_count
        // grew by one (old src.rs is now a stored buffer, def.rs is
        // active).
        assert_eq!(
            s.buffer_count(),
            src_buffer_count_before + 1,
            "jumping to a definition must not destroy the previous buffer"
        );
        let names = s.buffer_names();
        assert!(
            names.iter().any(|n| n == "src.rs"),
            "old buffer should still be in the list: {names:?}"
        );
        assert!(
            names.iter().any(|n| n == "def.rs"),
            "new buffer should be active: {names:?}"
        );
        // The active buffer is def.rs now.
        assert_eq!(s.current_buffer_name, "def.rs");
    }

    /// Regression: the FormattingResponse handler applies edits via
    /// `document.insert` / `document.remove` but previously didn't
    /// flush the change journal to the highlighter. After a format,
    /// tree-sitter would hold a stale tree.
    #[test]
    fn formatting_response_notifies_highlighter() {
        use lsp_types::{Position, Range, TextEdit};

        let dir = tempdir().unwrap();
        let path = dir.path().join("a.rs");
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(&path, "fn a( ){}\n").unwrap();

        let mut s = TuiAppState::new();
        #[allow(clippy::disallowed_methods)] // test fixture
        let content = std::fs::read_to_string(&path).unwrap();
        s.open_file_as_buffer(path, &content);
        s.ensure_highlight_fresh();
        let v_before = s.document.version();
        assert_eq!(s.buffer_highlighter_version, v_before);

        // Simulate the LSP server returning a one-character formatting
        // edit: replace the "( )" with "()" — remove the space at col 5.
        let edit = TextEdit {
            range: Range {
                start: Position {
                    line: 0,
                    character: 4,
                },
                end: Position {
                    line: 0,
                    character: 5,
                },
            },
            new_text: String::new(),
        };
        s.handle_lsp_event(rele_server::lsp::LspEvent::FormattingResponse {
            request_id: 1,
            edits: vec![edit],
        });

        assert!(s.document.version() > v_before);
        assert_eq!(
            s.buffer_highlighter_version,
            s.document.version(),
            "formatting must notify the highlighter"
        );
    }

    /// Regression: `kill_line` mutates the rope but previously forgot
    /// to drain the change journal into the highlighter / LSP. That
    /// left both consumers stuck on the pre-kill document version.
    #[test]
    fn kill_line_notifies_highlighter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("a.rs");
        #[allow(clippy::disallowed_methods)] // test fixture
        std::fs::write(&path, "fn a() {}\nfn b() {}\n").unwrap();

        let mut s = TuiAppState::new();
        #[allow(clippy::disallowed_methods)] // test fixture
        let content = std::fs::read_to_string(&path).unwrap();
        s.open_file_as_buffer(path, &content);
        s.ensure_highlight_fresh();
        let v_before = s.buffer_highlighter_version;
        assert_eq!(v_before, s.document.version());

        // C-k at start of line 0 kills "fn a() {}".
        s.cursor.position = 0;
        s.kill_line();

        // The kill mutated the rope — version must advance, and the
        // highlighter must be caught up to the new version.
        assert!(s.document.version() > v_before);
        assert_eq!(
            s.buffer_highlighter_version,
            s.document.version(),
            "kill_line must notify the highlighter"
        );
    }

    #[test]
    fn scroll_commands_fall_back_when_viewport_unmeasured() {
        // Before any draw, `last_viewport_height` is 0; commands should
        // still do *something* useful (fallback to a 20-line page).
        let mut s = TuiAppState::new();
        let text: String = (0..40).map(|i| format!("line {i}\n")).collect();
        s.insert_text(&text);
        s.cursor.position = 0;
        assert_eq!(s.last_viewport_height, 0);
        s.run_command("scroll-up-command", CommandArgs::default());
        // 20-line fallback — cursor ended up in the second half.
        assert!(s.cursor_line() >= 15);
    }

    /// Helper: install a dummy LSP buffer state with the given
    /// diagnostics so the tests can exercise the diagnostic
    /// accessors without running a real LSP server.
    fn attach_lsp(state: &mut TuiAppState, diags: Vec<lsp_types::Diagnostic>) {
        let uri: lsp_types::Uri = "file:///t/a.rs".parse().unwrap();
        let mut lsp = rele_server::lsp::LspBufferState::new(
            uri,
            "rust".to_string(),
            "rust-analyzer".to_string(),
        );
        lsp.diagnostics = diags;
        state.lsp_buffer_state = Some(lsp);
    }

    fn diag(
        line: u32,
        start: u32,
        end: u32,
        sev: lsp_types::DiagnosticSeverity,
        msg: &str,
    ) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line,
                    character: start,
                },
                end: lsp_types::Position {
                    line,
                    character: end,
                },
            },
            severity: Some(sev),
            code: None,
            code_description: None,
            source: None,
            message: msg.to_string(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    // ---- Window management ----

    #[test]
    fn default_layout_has_buffer_and_diagnostics_windows() {
        let s = TuiAppState::new();
        assert_eq!(s.windows.leaf_count(), 2);
        assert_eq!(s.focused_window, 0);
        // Focus starts on the buffer leaf, not on *Diagnostics*.
        assert_eq!(
            s.windows.content_at(0),
            Some(crate::windows::WindowContent::Buffer)
        );
        assert_eq!(
            s.windows.content_at(1),
            Some(crate::windows::WindowContent::Diagnostics)
        );
    }

    #[test]
    fn default_diagnostics_window_is_five_rows_tall() {
        use ratatui::layout::Rect;
        let s = TuiAppState::new();
        // Simulate a 30-row editor area.
        let (leaves, _seps) = s.windows.layout(
            Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 30,
            },
            0,
        );
        // Leaf 1 is the *Diagnostics* window; should be exactly 5 rows.
        assert_eq!(leaves[1].rect.height, 5);
    }

    #[test]
    fn pruning_diagnostics_leaves_only_buffer() {
        // Verify the auto-hide pipeline at the state level: if we
        // hide Diagnostics via `prune`, we get a single-leaf tree
        // pointing at Buffer.
        let s = TuiAppState::new();
        let hide =
            |c: crate::windows::WindowContent| c == crate::windows::WindowContent::Diagnostics;
        let pruned = s.windows.prune(&hide).unwrap();
        assert_eq!(pruned.leaf_count(), 1);
        assert_eq!(
            pruned.content_at(0),
            Some(crate::windows::WindowContent::Buffer)
        );
    }

    #[test]
    fn split_window_below_doubles_leaf_count() {
        let mut s = TuiAppState::new();
        let before = s.windows.leaf_count();
        s.split_window_below();
        assert_eq!(s.windows.leaf_count(), before + 1);
    }

    #[test]
    fn split_window_right_doubles_leaf_count() {
        let mut s = TuiAppState::new();
        let before = s.windows.leaf_count();
        s.split_window_right();
        assert_eq!(s.windows.leaf_count(), before + 1);
    }

    #[test]
    fn other_window_cycles_focus() {
        let mut s = TuiAppState::new();
        // Default has 2 leaves (buffer + diagnostics).
        assert_eq!(s.focused_window, 0);
        s.focus_next_window();
        assert_eq!(s.focused_window, 1);
        s.focus_next_window();
        assert_eq!(s.focused_window, 0);
    }

    #[test]
    fn delete_other_windows_collapses_to_focused() {
        let mut s = TuiAppState::new();
        s.split_window_right(); // now 3 leaves
        assert_eq!(s.windows.leaf_count(), 3);
        s.delete_other_windows();
        assert_eq!(s.windows.leaf_count(), 1);
        assert_eq!(s.focused_window, 0);
    }

    #[test]
    fn delete_window_refuses_when_only_one_left() {
        let mut s = TuiAppState::new();
        s.delete_other_windows(); // 1 leaf
        s.delete_window();
        assert_eq!(s.windows.leaf_count(), 1);
        assert!(
            s.message.as_deref().unwrap_or("").contains("only window"),
            "expected 'only window' message, got {:?}",
            s.message
        );
    }

    #[test]
    fn delete_window_removes_focused_and_reparents_sibling() {
        let mut s = TuiAppState::new();
        // Default: buffer(0) / diagnostics(1). Focus is on 0.
        s.focus_next_window(); // focus *Diagnostics*
        assert_eq!(s.focused_window, 1);
        s.delete_window();
        assert_eq!(s.windows.leaf_count(), 1);
        assert_eq!(
            s.windows.content_at(0),
            Some(crate::windows::WindowContent::Buffer),
            "deleting diagnostics should leave buffer as the sole window"
        );
    }

    #[test]
    fn diagnostic_at_cursor_returns_message_on_error_line() {
        let mut s = TuiAppState::new();
        s.insert_text("line 0\nline 1 bad\nline 2\n");
        s.cursor.position = 0;
        // Move to line 1.
        s.move_down(false);
        attach_lsp(
            &mut s,
            vec![diag(
                1,
                7,
                10,
                lsp_types::DiagnosticSeverity::ERROR,
                "undefined: bad",
            )],
        );
        let msg = s.diagnostic_at_cursor();
        assert_eq!(msg.as_deref(), Some("undefined: bad"));
    }

    #[test]
    fn diagnostic_at_cursor_none_on_clean_line() {
        let mut s = TuiAppState::new();
        s.insert_text("line 0\nline 1\n");
        s.cursor.position = 0;
        attach_lsp(
            &mut s,
            vec![diag(
                1,
                0,
                4,
                lsp_types::DiagnosticSeverity::WARNING,
                "later",
            )],
        );
        // Cursor on line 0, diagnostic is on line 1.
        assert!(s.diagnostic_at_cursor().is_none());
    }

    #[test]
    fn diagnostic_at_cursor_picks_most_severe_when_multiple() {
        let mut s = TuiAppState::new();
        s.insert_text("the line\n");
        // Move cursor back onto line 0.
        s.cursor.position = 0;
        attach_lsp(
            &mut s,
            vec![
                diag(0, 0, 8, lsp_types::DiagnosticSeverity::WARNING, "warn msg"),
                diag(0, 0, 8, lsp_types::DiagnosticSeverity::ERROR, "error msg"),
                diag(0, 0, 8, lsp_types::DiagnosticSeverity::HINT, "hint msg"),
            ],
        );
        assert_eq!(s.diagnostic_at_cursor().as_deref(), Some("error msg"));
    }

    #[test]
    fn goto_next_diagnostic_moves_cursor_and_wraps() {
        let mut s = TuiAppState::new();
        s.insert_text("line 0\nline 1\nline 2\nline 3\n");
        s.cursor.position = 0;
        attach_lsp(
            &mut s,
            vec![
                diag(1, 0, 3, lsp_types::DiagnosticSeverity::ERROR, "err1"),
                diag(3, 0, 3, lsp_types::DiagnosticSeverity::WARNING, "warn1"),
            ],
        );
        // First call: cursor -> line 1 (first diagnostic after cursor).
        assert!(s.goto_next_diagnostic());
        assert_eq!(s.cursor_line(), 1);
        // Second call: cursor -> line 3.
        assert!(s.goto_next_diagnostic());
        assert_eq!(s.cursor_line(), 3);
        // Third call wraps to line 1.
        assert!(s.goto_next_diagnostic());
        assert_eq!(s.cursor_line(), 1);
    }

    #[test]
    fn goto_prev_diagnostic_moves_backward_and_wraps() {
        let mut s = TuiAppState::new();
        s.insert_text("line 0\nline 1\nline 2\nline 3\n");
        attach_lsp(
            &mut s,
            vec![
                diag(1, 0, 3, lsp_types::DiagnosticSeverity::ERROR, "err1"),
                diag(3, 0, 3, lsp_types::DiagnosticSeverity::WARNING, "warn1"),
            ],
        );
        // Start at EOF, go prev.
        s.cursor.position = s.document.len_chars();
        assert!(s.goto_prev_diagnostic());
        assert_eq!(s.cursor_line(), 3);
        assert!(s.goto_prev_diagnostic());
        assert_eq!(s.cursor_line(), 1);
        // Wraps to last.
        assert!(s.goto_prev_diagnostic());
        assert_eq!(s.cursor_line(), 3);
    }

    #[test]
    fn goto_diagnostic_reports_when_none_present() {
        let mut s = TuiAppState::new();
        assert!(!s.goto_next_diagnostic());
        attach_lsp(&mut s, Vec::new());
        assert!(!s.goto_next_diagnostic());
    }

    #[test]
    fn buffer_names_lists_active_first_then_stored() {
        let mut s = TuiAppState::new();
        s.open_file_as_buffer(PathBuf::from("/tmp/x.md"), "x");
        s.open_file_as_buffer(PathBuf::from("/tmp/y.md"), "y");
        let names = s.buffer_names();
        assert_eq!(names[0], "y.md"); // active
        assert!(names.contains(&"x.md".to_string()));
        assert!(names.contains(&"*scratch*".to_string()));
    }

    // ---- Elisp drives navigation ----
    //
    // These tests synchronize the server-owned Lisp host with a live
    // TuiAppState and verify that (a) the Rust primitives wired into
    // the interpreter actually mutate the buffer, and (b) defuns
    // loaded from `lisp/commands.el` take precedence in
    // `run_command`. This is the regression surface for the migration:
    // if either half breaks, editing keys stop working.

    #[test]
    fn elisp_forward_char_primitive_moves_point() {
        let mut s = TuiAppState::new();
        s.insert_text("hello world");
        s.cursor.position = 0;
        s.install_elisp_editor_callbacks();

        // Directly eval `(forward-char 3)` — skips run_command entirely
        // so this tests the primitive surface without the defun layer.
        let call = rele_elisp::LispObject::cons(
            rele_elisp::LispObject::symbol("forward-char"),
            rele_elisp::LispObject::cons(
                rele_elisp::LispObject::integer(3),
                rele_elisp::LispObject::nil(),
            ),
        );
        s.eval_lisp(call).expect("forward-char eval");
        assert_eq!(s.cursor.position, 3);
    }

    #[test]
    fn elisp_defun_routes_through_run_command() {
        let mut s = TuiAppState::new();
        s.insert_text("line1\nline2\nline3\n");
        s.cursor.position = 0;
        s.install_elisp_editor_callbacks();

        // `next-line` is defined in lisp/commands.el as
        //   (defun next-line (&optional n) (forward-line (or n 1)))
        // If run_command correctly prefers the defun, cursor advances
        // to line 1.
        s.run_command("next-line", CommandArgs::default());
        assert_eq!(s.cursor_line(), 1);

        // And with a prefix arg of 2, it jumps two lines from line 1.
        s.run_command("next-line", CommandArgs::default().with_prefix(2));
        assert_eq!(s.cursor_line(), 3);
    }

    #[test]
    fn elisp_backward_char_defun_negates_through_forward_char() {
        let mut s = TuiAppState::new();
        s.insert_text("abcdef");
        s.cursor.position = 5;
        s.install_elisp_editor_callbacks();

        // backward-char is a defun delegating to (forward-char (- n)).
        s.run_command("backward-char", CommandArgs::default().with_prefix(3));
        assert_eq!(s.cursor.position, 2);
    }
}
