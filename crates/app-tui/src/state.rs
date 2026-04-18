use std::path::PathBuf;

use rele_elisp::{Interpreter, add_primitives};
use rele_server::{
    BufferId, BufferKind, CommandArgs, DocumentBuffer, EditHistory, EditorCursor, KillRing,
    MacroState, StoredBuffer,
};
use rele_server::document::buffer_list::name_from_path;
use rele_server::CancellationFlag;
use rele_server::lsp::{
    LspBufferState, LspConfig, LspEvent, LspRegistry,
    position::uri_from_path,
};
use tokio::sync::mpsc;

use crate::commands::{CommandRegistry, register_builtin_commands};

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

    // ---- Inactive buffers ----
    pub stored_buffers: Vec<StoredBuffer>,
    next_buffer_id: u64,

    // ---- Editor subsystems ----
    pub kill_ring: KillRing,
    pub macros: MacroState,
    pub commands: CommandRegistry,
    pub elisp: Interpreter,

    // ---- Viewport ----
    pub scroll_line: usize,

    // ---- Editing flags ----
    pub last_edit_was_char_insert: bool,
    pub last_move_was_vertical: bool,

    // ---- Prefix keys ----
    pub universal_arg: Option<usize>,
    pub meta_pending: bool,
    pub c_x_pending: bool,

    // ---- Status line message (auto-clears after one frame) ----
    pub message: Option<String>,

    // ---- Quit flag ----
    pub should_quit: bool,

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

impl TuiAppState {
    pub fn new() -> Self {
        let mut commands = CommandRegistry::new();
        register_builtin_commands(&mut commands);

        let mut interp = Interpreter::new();
        add_primitives(&mut interp);

        Self {
            document: DocumentBuffer::from_text(""),
            cursor: EditorCursor::new(),
            history: EditHistory::default(),
            current_buffer_id: BufferId(0),
            current_buffer_name: "*scratch*".to_string(),
            current_buffer_kind: BufferKind::Scratch,
            current_buffer_read_only: false,
            stored_buffers: Vec::new(),
            next_buffer_id: 1,
            kill_ring: KillRing::default(),
            macros: MacroState::default(),
            commands,
            elisp: interp,
            scroll_line: 0,
            last_edit_was_char_insert: false,
            last_move_was_vertical: false,
            universal_arg: None,
            meta_pending: false,
            c_x_pending: false,
            message: None,
            should_quit: false,
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
        state.document = DocumentBuffer::from_file(path, content);
        state
    }

    // ---- Command dispatch ----

    pub fn run_command(&mut self, name: &str, args: CommandArgs) {
        let handler = self.commands.get(name).map(|c| c.handler.clone());
        if let Some(h) = handler {
            h(self, args);
        } else {
            self.message = Some(format!("[Unknown command: {name}]"));
        }
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
        }

        self.document.insert(self.cursor.position, text);
        self.cursor.position += text.chars().count();
        self.last_edit_was_char_insert = is_single_char;
        self.lsp_completion_visible = false;
        self.update_preferred_column();
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
        } else if self.cursor.position > 0 {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            self.cursor.position -= 1;
            self.document
                .remove(self.cursor.position, self.cursor.position + 1);
        }
        self.lsp_completion_visible = false;
        self.update_preferred_column();
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
        } else if self.cursor.position < self.document.len_chars() {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            self.document
                .remove(self.cursor.position, self.cursor.position + 1);
        }
        self.lsp_completion_visible = false;
        self.update_preferred_column();
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
            let end = if line_end > 0
                && self.document.line(line).to_string().ends_with('\n')
            {
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
            self.lsp_did_change();
        }
    }

    // ---- Cursor movement ----

    pub fn move_left(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        if !extend
            && let Some((start, _)) = self.cursor.selection()
        {
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
        if !extend
            && let Some((_, end)) = self.cursor.selection()
        {
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
        self.cursor.move_to(0, extend);
        self.update_preferred_column();
    }

    pub fn move_to_buffer_end(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.cursor.move_to(self.document.len_chars(), extend);
        self.update_preferred_column();
    }

    // ---- File I/O ----

    pub fn save_file(&mut self) {
        if let Some(path) = self.document.file_path().cloned() {
            let content = self.document.text();
            match std::fs::write(&path, &content) {
                Ok(()) => {
                    self.document.mark_clean();
                    self.message = Some(format!("Wrote {}", path.display()));
                    self.lsp_did_save();
                }
                Err(e) => {
                    self.message = Some(format!("Error saving: {e}"));
                }
            }
        } else {
            self.message = Some("No file name".to_string());
        }
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
                if let Some(ref mut lsp_state) = self.lsp_buffer_state {
                    if lsp_state.uri == uri {
                        lsp_state.diagnostics = diagnostics;
                    }
                }
            }
            LspEvent::CompletionResponse { items, .. } => {
                self.lsp_completion_items = items;
                self.lsp_completion_selected = 0;
                self.lsp_completion_visible = !self.lsp_completion_items.is_empty();
            }
            LspEvent::HoverResponse { contents, .. } => {
                self.lsp_hover_text =
                    contents.map(rele_server::lsp::hover_contents_to_string);
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
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            self.document = DocumentBuffer::from_file(path, &content);
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
                    self.message =
                        Some(format!("References: {} results", locations.len()));
                }
            }
        }
    }
}
