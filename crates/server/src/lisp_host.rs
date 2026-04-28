//! Server-owned Emacs Lisp integration.
//!
//! Frontends render and collect interactive input; this module owns the
//! editor state surface that Lisp code mutates.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use rele_elisp::{EditorCallbacks, Interpreter, LispObject, add_primitives};

use crate::document::buffer_list::name_from_path;
use crate::{
    BufferId, BufferKind, CommandArgs, DocumentBuffer, EditHistory, EditorCursor, KillRing,
    StoredBuffer,
};

/// Result of trying to dispatch a command through elisp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LispCommandDispatch {
    Handled,
    Unhandled,
}

/// How a Lisp file should be evaluated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadMode {
    /// Every form must parse and evaluate. Use this for rele-owned Lisp.
    Strict,
    /// Best-effort mode for upstream Emacs stdlib files while the
    /// interpreter is still incomplete.
    PermissiveStdlib,
}

/// Summary of a source load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadReport {
    pub parsed_forms: usize,
    pub evaluated_forms: usize,
    pub errors: usize,
}

/// Shared editor state exposed to elisp.
#[derive(Clone)]
pub struct EditorCore {
    pub document: DocumentBuffer,
    pub cursor: EditorCursor,
    pub history: EditHistory,
    pub current_buffer_id: BufferId,
    pub current_buffer_name: String,
    pub current_buffer_kind: BufferKind,
    pub current_buffer_read_only: bool,
    pub stored_buffers: Vec<StoredBuffer>,
    pub next_buffer_id: u64,
    pub lsp_state: Option<crate::lsp::LspBufferState>,
    pub kill_ring: KillRing,
    pub scroll_line: usize,
    pub last_edit_was_char_insert: bool,
    pub last_move_was_vertical: bool,
    pub message: Option<String>,
    pub keyboard_quit_requested: bool,
}

impl Default for EditorCore {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorCore {
    pub fn new() -> Self {
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
            lsp_state: None,
            kill_ring: KillRing::default(),
            scroll_line: 0,
            last_edit_was_char_insert: false,
            last_move_was_vertical: false,
            message: None,
            keyboard_quit_requested: false,
        }
    }

    pub fn from_file(path: PathBuf, content: &str) -> Self {
        let mut core = Self::new();
        core.current_buffer_name = name_from_path(&path);
        core.current_buffer_kind = BufferKind::File;
        core.document = DocumentBuffer::from_file(path, content);
        core
    }

    pub fn buffer_count(&self) -> usize {
        1 + self.stored_buffers.len()
    }

    pub fn buffer_names(&self) -> Vec<String> {
        let mut names = Vec::with_capacity(self.buffer_count());
        names.push(self.current_buffer_name.clone());
        for buffer in &self.stored_buffers {
            names.push(buffer.name.clone());
        }
        names
    }

    pub fn current_major_mode(&self) -> String {
        match self.current_buffer_kind {
            BufferKind::Dired => "dired-mode".to_string(),
            BufferKind::BufferList => "Buffer-menu-mode".to_string(),
            BufferKind::Scratch => "lisp-interaction-mode".to_string(),
            BufferKind::File => self
                .document
                .file_path()
                .and_then(|path| path.extension())
                .and_then(|ext| ext.to_str())
                .map(|ext| match ext {
                    "el" => "emacs-lisp-mode",
                    "md" | "markdown" => "markdown-mode",
                    "rs" => "rust-mode",
                    _ => "text-mode",
                })
                .unwrap_or("text-mode")
                .to_string(),
        }
    }

    pub fn set_current_major_mode(&mut self, mode: &str) {
        match mode {
            "dired-mode" => {
                self.current_buffer_kind = BufferKind::Dired;
                self.current_buffer_read_only = true;
            }
            "Buffer-menu-mode" => {
                self.current_buffer_kind = BufferKind::BufferList;
                self.current_buffer_read_only = true;
            }
            "lisp-interaction-mode" => {
                self.current_buffer_kind = BufferKind::Scratch;
                self.current_buffer_read_only = false;
            }
            _ => {}
        }
    }

    pub fn switch_to_buffer_by_name(&mut self, name: &str) -> bool {
        if let Some(id) = self.find_buffer_by_name(name) {
            self.switch_to_buffer_id(id)
        } else {
            false
        }
    }

    pub fn get_or_create_named_buffer(&mut self, name: &str) -> BufferId {
        if let Some(id) = self.find_buffer_by_name(name) {
            self.apply_special_attrs_for_buffer(id);
            return id;
        }

        let id = self.allocate_buffer_id();
        let mut buffer = StoredBuffer::new_scratch(id);
        buffer.name = name.to_string();
        if let Some((kind, read_only)) = special_buffer_attrs(name) {
            buffer.kind = kind;
            buffer.read_only = read_only;
        }
        self.stored_buffers.push(buffer);
        id
    }

    pub fn set_buffer_text_by_name(&mut self, name: &str, text: &str) -> bool {
        if !self.switch_to_buffer_by_name(name) {
            return false;
        }
        self.document.set_text(text);
        self.history = EditHistory::default();
        self.cursor = EditorCursor::new();
        self.scroll_line = 0;
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.apply_special_attrs_for_current_name();
        true
    }

    pub fn open_file_as_buffer(&mut self, path: PathBuf, content: &str) {
        #[allow(clippy::disallowed_methods)]
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);
        if let Some(id) = self.find_buffer_by_path(&canonical) {
            self.switch_to_buffer_id(id);
            return;
        }

        let name = name_from_path(&canonical);
        let outgoing = StoredBuffer {
            id: self.current_buffer_id,
            name: std::mem::replace(&mut self.current_buffer_name, name),
            document: std::mem::replace(
                &mut self.document,
                DocumentBuffer::from_file(canonical, content),
            ),
            history: std::mem::take(&mut self.history),
            cursor: std::mem::replace(&mut self.cursor, EditorCursor::new()),
            kind: std::mem::replace(&mut self.current_buffer_kind, BufferKind::File),
            read_only: std::mem::replace(&mut self.current_buffer_read_only, false),
            scroll_line: std::mem::replace(&mut self.scroll_line, 0),
            last_edit_was_char_insert: std::mem::replace(
                &mut self.last_edit_was_char_insert,
                false,
            ),
            last_move_was_vertical: std::mem::replace(&mut self.last_move_was_vertical, false),
            lsp_state: self.lsp_state.take(),
        };
        self.stored_buffers.push(outgoing);
        self.current_buffer_id = self.allocate_buffer_id();
    }

    pub fn kill_buffer_by_name(&mut self, name: &str) -> bool {
        if name.is_empty() || name == self.current_buffer_name {
            return self.kill_current_buffer();
        }
        if let Some(pos) = self
            .stored_buffers
            .iter()
            .position(|buffer| buffer.name == name)
        {
            self.stored_buffers.remove(pos);
            true
        } else {
            self.message = Some(format!("No such buffer: {name}"));
            false
        }
    }

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
        self.update_preferred_column();
    }

    pub fn delete_char(&mut self, n: i64) {
        if n > 0 {
            for _ in 0..n as usize {
                self.delete_forward();
            }
        } else if n < 0 {
            for _ in 0..(-n) as usize {
                self.backspace();
            }
        }
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
        self.update_preferred_column();
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
        self.update_preferred_column();
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
            self.document.remove(pos, pos + 1);
            self.kill_ring.push("\n".to_string());
        } else {
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
    }

    pub fn undo(&mut self) {
        self.last_edit_was_char_insert = false;
        if let Some((snapshot, cursor)) = self
            .history
            .undo(self.document.snapshot(), self.cursor.position)
        {
            self.document.restore(snapshot);
            self.cursor.position = cursor.min(self.document.len_chars());
            self.cursor.clear_selection();
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
        }
    }

    pub fn goto_char(&mut self, pos: usize) {
        self.cursor.position = pos.min(self.document.len_chars());
        self.cursor.clear_selection();
        self.update_preferred_column();
    }

    pub fn forward_char(&mut self, n: i64) {
        if n > 0 {
            for _ in 0..n as usize {
                self.move_right(false);
            }
        } else if n < 0 {
            for _ in 0..(-n) as usize {
                self.move_left(false);
            }
        }
    }

    pub fn forward_line(&mut self, n: i64) {
        if n > 0 {
            for _ in 0..n as usize {
                self.move_down(false);
            }
        } else if n < 0 {
            for _ in 0..(-n) as usize {
                self.move_up(false);
            }
        }
    }

    pub fn move_beginning_of_line(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        if self.document.len_chars() == 0 {
            self.cursor.move_to(0, false);
            self.update_preferred_column();
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        self.cursor.move_to(self.document.line_to_char(line), false);
        self.update_preferred_column();
    }

    pub fn move_end_of_line(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        if self.document.len_chars() == 0 {
            self.cursor.move_to(0, false);
            self.update_preferred_column();
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        let line_start = self.document.line_to_char(line);
        let line_len = self.document.line(line).len_chars();
        let mut line_end = line_start + line_len;
        if line_end > line_start && self.document.line(line).to_string().ends_with('\n') {
            line_end -= 1;
        }
        self.cursor
            .move_to(line_end.min(self.document.len_chars()), false);
        self.update_preferred_column();
    }

    pub fn move_beginning_of_buffer(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.cursor.move_to(0, false);
        self.update_preferred_column();
    }

    pub fn move_end_of_buffer(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.cursor.move_to(self.document.len_chars(), false);
        self.update_preferred_column();
    }

    pub fn save_buffer(&mut self) -> bool {
        let Some(path) = self.document.file_path().cloned() else {
            self.message = Some("No file name".to_string());
            return false;
        };
        match write_document_to_path(&mut self.document, &path) {
            Ok(()) => {
                self.message = Some(format!("Wrote {}", path.display()));
                true
            }
            Err(error) => {
                self.message = Some(format!("Error saving: {error}"));
                false
            }
        }
    }

    fn move_left(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
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

    fn move_right(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
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

    fn move_up(&mut self, extend: bool) {
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
        self.cursor
            .move_to(prev_line_start + target_col.min(max_col), extend);
        self.cursor.preferred_column = target_col;
        self.last_move_was_vertical = true;
    }

    fn move_down(&mut self, extend: bool) {
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
        self.cursor
            .move_to(next_line_start + target_col.min(max_col), extend);
        self.cursor.preferred_column = target_col;
        self.last_move_was_vertical = true;
    }

    fn allocate_buffer_id(&mut self) -> BufferId {
        let id = BufferId(self.next_buffer_id);
        self.next_buffer_id += 1;
        id
    }

    fn find_buffer_by_path(&self, path: &Path) -> Option<BufferId> {
        if self.document.file_path().map(PathBuf::as_path) == Some(path) {
            return Some(self.current_buffer_id);
        }
        self.stored_buffers
            .iter()
            .find(|buffer| buffer.document.file_path().map(PathBuf::as_path) == Some(path))
            .map(|buffer| buffer.id)
    }

    fn find_buffer_by_name(&self, name: &str) -> Option<BufferId> {
        if self.current_buffer_name == name {
            return Some(self.current_buffer_id);
        }
        self.stored_buffers
            .iter()
            .find(|buffer| buffer.name == name)
            .map(|buffer| buffer.id)
    }

    fn switch_to_buffer_id(&mut self, id: BufferId) -> bool {
        if id == self.current_buffer_id {
            return true;
        }
        let Some(idx) = self
            .stored_buffers
            .iter()
            .position(|buffer| buffer.id == id)
        else {
            return false;
        };
        self.swap_active_with_stored(idx);
        true
    }

    fn swap_active_with_stored(&mut self, stored_idx: usize) {
        let incoming = self.stored_buffers.remove(stored_idx);
        let incoming_lsp_state = incoming.lsp_state.clone();
        let outgoing = StoredBuffer {
            id: self.current_buffer_id,
            name: std::mem::replace(&mut self.current_buffer_name, incoming.name),
            document: std::mem::replace(&mut self.document, incoming.document),
            history: std::mem::replace(&mut self.history, incoming.history),
            cursor: std::mem::replace(&mut self.cursor, incoming.cursor),
            kind: std::mem::replace(&mut self.current_buffer_kind, incoming.kind),
            read_only: std::mem::replace(&mut self.current_buffer_read_only, incoming.read_only),
            scroll_line: std::mem::replace(&mut self.scroll_line, incoming.scroll_line),
            last_edit_was_char_insert: std::mem::replace(
                &mut self.last_edit_was_char_insert,
                incoming.last_edit_was_char_insert,
            ),
            last_move_was_vertical: std::mem::replace(
                &mut self.last_move_was_vertical,
                incoming.last_move_was_vertical,
            ),
            lsp_state: std::mem::replace(&mut self.lsp_state, incoming_lsp_state),
        };
        self.current_buffer_id = incoming.id;
        self.stored_buffers.push(outgoing);
    }

    fn kill_current_buffer(&mut self) -> bool {
        if self.buffer_count() <= 1 {
            self.message = Some("Cannot kill the only buffer".to_string());
            return false;
        }
        let last_idx = self.stored_buffers.len() - 1;
        self.swap_active_with_stored(last_idx);
        self.stored_buffers.pop();
        true
    }

    fn apply_special_attrs_for_buffer(&mut self, id: BufferId) {
        if id == self.current_buffer_id {
            self.apply_special_attrs_for_current_name();
            return;
        }
        if let Some(buffer) = self
            .stored_buffers
            .iter_mut()
            .find(|buffer| buffer.id == id)
            && let Some((kind, read_only)) = special_buffer_attrs(&buffer.name)
        {
            buffer.kind = kind;
            buffer.read_only = read_only;
        }
    }

    fn apply_special_attrs_for_current_name(&mut self) {
        if let Some((kind, read_only)) = special_buffer_attrs(&self.current_buffer_name) {
            self.current_buffer_kind = kind;
            self.current_buffer_read_only = read_only;
        }
    }

    fn clamp_cursor(&mut self) {
        self.cursor.position = self.cursor.position.min(self.document.len_chars());
    }

    fn update_preferred_column(&mut self) {
        if self.document.len_chars() == 0 {
            self.cursor.preferred_column = 0;
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        self.cursor.preferred_column = pos - self.document.line_to_char(line);
    }
}

/// Safe callback object installed into the interpreter.
#[derive(Clone)]
pub struct ServerEditorCallbacks {
    core: Arc<Mutex<EditorCore>>,
}

impl ServerEditorCallbacks {
    pub fn new(core: Arc<Mutex<EditorCore>>) -> Self {
        Self { core }
    }
}

impl EditorCallbacks for ServerEditorCallbacks {
    fn buffer_string(&self) -> String {
        self.core.lock().document.text()
    }

    fn buffer_size(&self) -> usize {
        self.core.lock().document.len_chars()
    }

    fn point(&self) -> usize {
        self.core.lock().cursor.position
    }

    fn point_min(&self) -> usize {
        0
    }

    fn point_max(&self) -> usize {
        self.core.lock().document.len_chars()
    }

    fn insert(&mut self, text: &str) {
        self.core.lock().insert_text(text);
    }

    fn delete_char(&mut self, n: i64) {
        self.core.lock().delete_char(n);
    }

    fn find_file(&mut self, path: &str) -> bool {
        let path = PathBuf::from(path);
        #[allow(clippy::disallowed_methods)]
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.core.lock().open_file_as_buffer(path, &content);
                true
            }
            Err(_) => false,
        }
    }

    fn save_buffer(&mut self) -> bool {
        self.core.lock().save_buffer()
    }

    fn goto_char(&mut self, pos: usize) {
        self.core.lock().goto_char(pos);
    }

    fn forward_char(&mut self, n: i64) {
        self.core.lock().forward_char(n);
    }

    fn forward_line(&mut self, n: i64) {
        self.core.lock().forward_line(n);
    }

    fn move_beginning_of_line(&mut self) {
        self.core.lock().move_beginning_of_line();
    }

    fn move_end_of_line(&mut self) {
        self.core.lock().move_end_of_line();
    }

    fn move_beginning_of_buffer(&mut self) {
        self.core.lock().move_beginning_of_buffer();
    }

    fn move_end_of_buffer(&mut self) {
        self.core.lock().move_end_of_buffer();
    }

    fn undo(&mut self) {
        self.core.lock().undo();
    }

    fn redo(&mut self) {
        self.core.lock().redo();
    }

    fn kill_line(&mut self) {
        self.core.lock().kill_line();
    }

    fn current_buffer_name(&self) -> Option<String> {
        Some(self.core.lock().current_buffer_name.clone())
    }

    fn list_buffer_names(&self) -> Vec<String> {
        self.core.lock().buffer_names()
    }

    fn switch_to_buffer_by_name(&mut self, name: &str) -> bool {
        self.core.lock().switch_to_buffer_by_name(name)
    }

    fn get_or_create_buffer(&mut self, name: &str) -> bool {
        self.core.lock().get_or_create_named_buffer(name);
        true
    }

    fn kill_buffer_by_name(&mut self, name: &str) -> bool {
        self.core.lock().kill_buffer_by_name(name)
    }

    fn set_buffer_text(&mut self, name: &str, text: &str) -> bool {
        self.core.lock().set_buffer_text_by_name(name, text)
    }

    fn current_buffer_major_mode(&self) -> Option<String> {
        Some(self.core.lock().current_major_mode())
    }

    fn set_current_buffer_major_mode(&mut self, mode: &str) {
        self.core.lock().set_current_major_mode(mode);
    }

    fn keyboard_quit(&mut self) {
        let mut core = self.core.lock();
        core.keyboard_quit_requested = true;
        core.message = Some("Quit".to_string());
    }
}

/// Server-side owner of an elisp interpreter plus editor callback state.
pub struct LispHost {
    interpreter: Interpreter,
    core: Arc<Mutex<EditorCore>>,
}

impl LispHost {
    pub fn new(core: EditorCore) -> Self {
        let mut interpreter = Interpreter::new();
        add_primitives(&mut interpreter);

        let core = Arc::new(Mutex::new(core));
        interpreter.set_editor(Box::new(ServerEditorCallbacks::new(core.clone())));

        Self { interpreter, core }
    }

    pub fn interpreter(&self) -> &Interpreter {
        &self.interpreter
    }

    pub fn core(&self) -> Arc<Mutex<EditorCore>> {
        self.core.clone()
    }

    pub fn core_snapshot(&self) -> EditorCore {
        self.core.lock().clone()
    }

    pub fn replace_core(&self, core: EditorCore) {
        *self.core.lock() = core;
    }

    pub fn eval(&self, expr: LispObject) -> rele_elisp::ElispResult<LispObject> {
        self.interpreter.eval(expr)
    }

    pub fn eval_source(&self, source: &str) -> Result<LispObject, (usize, rele_elisp::ElispError)> {
        self.interpreter.eval_source(source)
    }

    pub fn is_user_defined_command(&self, name: &str) -> bool {
        rele_elisp::is_user_defined_elisp_function(name, &self.interpreter.state)
    }

    pub fn run_command(
        &self,
        name: &str,
        args: &CommandArgs,
    ) -> Result<LispCommandDispatch, rele_elisp::ElispError> {
        if !self.is_user_defined_command(name) {
            return Ok(LispCommandDispatch::Unhandled);
        }

        let arg = match &args.string {
            Some(value) => LispObject::string(value),
            None => LispObject::integer(args.count() as i64),
        };
        let call = LispObject::cons(
            LispObject::symbol(name),
            LispObject::cons(arg, LispObject::nil()),
        );
        self.interpreter.eval(call)?;
        Ok(LispCommandDispatch::Handled)
    }

    pub fn run_hooks(&self, name: &str) -> rele_elisp::ElispResult<()> {
        self.interpreter.run_hooks(name)
    }

    pub fn load_builtin_commands(&self) -> Result<LoadReport, (usize, rele_elisp::ElispError)> {
        self.load_source(crate::BUILTIN_COMMANDS_EL, LoadMode::Strict)
    }

    pub fn load_source(
        &self,
        source: &str,
        mode: LoadMode,
    ) -> Result<LoadReport, (usize, rele_elisp::ElispError)> {
        let forms = rele_elisp::read_all(source).map_err(|error| (0, error))?;
        let parsed_forms = forms.len();
        let mut evaluated_forms = 0;
        let mut errors = 0;
        for (idx, form) in forms.into_iter().enumerate() {
            match self.interpreter.eval(form) {
                Ok(_) => evaluated_forms += 1,
                Err(error)
                    if mode == LoadMode::PermissiveStdlib && !error.is_eval_ops_exceeded() =>
                {
                    errors += 1;
                    log::debug!("permissive elisp load form {idx}: {error}");
                }
                Err(error) => return Err((idx, error)),
            }
        }
        Ok(LoadReport {
            parsed_forms,
            evaluated_forms,
            errors,
        })
    }

    pub fn bootstrap_emacs_stdlib_for_dired(&self) {
        let Some(lisp_dir) = rele_elisp::bootstrap::emacs_lisp_dir() else {
            return;
        };

        let subr_path = format!("{lisp_dir}/subr.el");
        #[allow(clippy::disallowed_methods)]
        if let Ok(src) = std::fs::read_to_string(&subr_path)
            && let Err((idx, error)) = self.load_source(&src, LoadMode::PermissiveStdlib)
        {
            log::warn!("subr.el bootstrap failed at form {idx}: {error:?}");
        }

        let workaround = r#"
(defun internal--build-binding (binding prev-var)
  (let ((b (cond
            ((symbolp binding) (list binding binding))
            ((null (cdr binding)) (list (make-symbol "s") (car binding)))
            ((eq '_ (car binding)) (list (make-symbol "s") (cadr binding)))
            (t binding))))
    (when (> (length b) 2)
      (signal 'error (cons "`let' bindings can have only one value-form" b)))
    (list (car b) (list 'and prev-var (cadr b)))))

(defun internal--build-bindings (bindings)
  (let ((prev-var t) (out nil))
    (dolist (binding bindings)
      (let ((entry (internal--build-binding binding prev-var)))
        (push entry out)
        (setq prev-var (car entry))))
    (nreverse out)))
"#;
        if let Err((idx, error)) = self.load_source(workaround, LoadMode::Strict) {
            log::warn!("if-let* workaround failed at form {idx}: {error:?}");
        }

        let setup = format!(
            "(progn \
                (defvar load-path nil) \
                (setq load-path (list {0:?} {1:?} {2:?} {3:?})))",
            lisp_dir,
            format!("{lisp_dir}/emacs-lisp"),
            format!("{lisp_dir}/textmodes"),
            format!("{lisp_dir}/../etc/themes"),
        );
        if let Err((idx, error)) = self.load_source(&setup, LoadMode::Strict) {
            log::warn!("load-path setup failed at form {idx}: {error:?}");
        }

        let _ = self.eval_source("(setq load-suffixes '(\".el\" \".elc\"))");
    }
}

fn special_buffer_attrs(name: &str) -> Option<(BufferKind, bool)> {
    if name == "*Buffer List*" {
        Some((BufferKind::BufferList, true))
    } else if name.starts_with("*Dired: ") {
        Some((BufferKind::Dired, true))
    } else {
        None
    }
}

#[allow(clippy::disallowed_methods)]
fn write_document_to_path(document: &mut DocumentBuffer, path: &Path) -> std::io::Result<()> {
    let content = document.text();
    std::fs::write(path, &content)?;
    document.mark_clean();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_builtin_commands_load_cleanly() {
        let host = LispHost::new(EditorCore::new());
        let report = host.load_builtin_commands().expect("builtin commands load");

        assert!(report.parsed_forms > 0);
        assert_eq!(report.errors, 0);
        assert_eq!(report.parsed_forms, report.evaluated_forms);
    }

    #[test]
    fn elisp_command_mutates_server_core() {
        let host = LispHost::new(EditorCore::new());
        host.load_builtin_commands().expect("builtin commands load");
        host.core.lock().insert_text("line1\nline2\n");
        host.core.lock().cursor.position = 0;

        host.run_command("next-line", &CommandArgs::default())
            .expect("next-line dispatch");

        assert_eq!(host.core.lock().cursor.position, 6);
    }

    #[test]
    fn list_buffers_uses_server_buffer_registry() {
        let host = LispHost::new(EditorCore::new());
        host.load_builtin_commands().expect("builtin commands load");
        host.core.lock().get_or_create_named_buffer("notes.md");

        host.run_command("list-buffers", &CommandArgs::default())
            .expect("list-buffers dispatch");

        let core = host.core.lock();
        assert_eq!(core.current_buffer_name, "*Buffer List*");
        assert_eq!(core.current_buffer_kind, BufferKind::BufferList);
        assert!(core.current_buffer_read_only);
        assert!(core.document.text().contains("*scratch*"));
        assert!(core.document.text().contains("notes.md"));
    }
}
