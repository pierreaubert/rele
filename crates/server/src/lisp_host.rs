//! Server-owned Emacs Lisp integration.
//!
//! Frontends render and collect interactive input; this module owns the
//! editor state surface that Lisp code mutates.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use regex::Regex;
use rele_elisp::{EditorCallbacks, Interpreter, LispObject, add_primitives};

use crate::document::buffer_list::name_from_path;
use crate::emacs::search::{self, MatchData, SearchDirection};
use crate::{
    BufferId, BufferKind, CommandArgs, CommandPlan, DocumentBuffer, EditHistory, EditorCursor,
    InteractiveSpec, KillRing, StoredBuffer,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplacementMode {
    Literal,
    Regexp,
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
    pub current_major_mode: Option<String>,
    pub stored_buffers: Vec<StoredBuffer>,
    pub next_buffer_id: u64,
    pub lsp_state: Option<crate::lsp::LspBufferState>,
    pub kill_ring: KillRing,
    pub search_match_data: MatchData,
    pub scroll_line: usize,
    pub last_edit_was_char_insert: bool,
    pub last_move_was_vertical: bool,
    pub message: Option<String>,
    pub keyboard_quit_requested: bool,
    pub preview_toggle_requested: bool,
    pub line_numbers_toggle_requested: bool,
    pub preview_line_numbers_toggle_requested: bool,
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
            current_major_mode: Some("lisp-interaction-mode".to_string()),
            stored_buffers: Vec::new(),
            next_buffer_id: 1,
            lsp_state: None,
            kill_ring: KillRing::default(),
            search_match_data: MatchData::default(),
            scroll_line: 0,
            last_edit_was_char_insert: false,
            last_move_was_vertical: false,
            message: None,
            keyboard_quit_requested: false,
            preview_toggle_requested: false,
            line_numbers_toggle_requested: false,
            preview_line_numbers_toggle_requested: false,
        }
    }

    pub fn from_file(path: PathBuf, content: &str) -> Self {
        let mut core = Self::new();
        core.current_buffer_name = name_from_path(&path);
        core.current_buffer_kind = BufferKind::File;
        core.current_major_mode = None;
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
        if let Some(mode) = &self.current_major_mode {
            return mode.clone();
        }
        match self.current_buffer_kind {
            BufferKind::Dired => "dired-mode".to_string(),
            BufferKind::BufferList => "Buffer-menu-mode".to_string(),
            BufferKind::Scratch => "lisp-interaction-mode".to_string(),
            BufferKind::File => {
                inferred_major_mode_for_path(self.document.file_path().map(PathBuf::as_path))
                    .to_string()
            }
        }
    }

    pub fn set_current_major_mode(&mut self, mode: &str) {
        self.current_major_mode = Some(mode.to_string());
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
            let major_mode = special_major_mode(&kind);
            buffer.kind = kind;
            buffer.read_only = read_only;
            buffer.major_mode = Some(major_mode.to_string());
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
            major_mode: std::mem::replace(&mut self.current_major_mode, None),
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
        } else {
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
        } else {
            self.cursor.clear_selection();
            if self.cursor.position < self.document.len_chars() {
                self.history
                    .push_undo(self.document.snapshot(), self.cursor.position);
                self.document
                    .remove(self.cursor.position, self.cursor.position + 1);
            }
        }
        self.update_preferred_column();
    }

    pub fn delete_selection(&mut self) -> Option<String> {
        let (start, end) = self.cursor.selection()?;
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.history
            .push_undo(self.document.snapshot(), self.cursor.position);
        let text = self.document.rope().slice(start..end).to_string();
        self.document.remove(start, end);
        self.cursor.position = start;
        self.cursor.clear_selection();
        self.update_preferred_column();
        Some(text)
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

    pub fn kill_word(&mut self, n: i64) {
        if n > 0 {
            for _ in 0..n as usize {
                self.kill_word_forward();
            }
        } else if n < 0 {
            for _ in 0..(-n) as usize {
                self.kill_word_backward();
            }
        }
    }

    pub fn kill_word_forward(&mut self) {
        self.last_edit_was_char_insert = false;
        let start = self.cursor.position.min(self.document.len_chars());
        let rope = self.document.rope();
        let len = rope.len_chars();
        let mut end = start;

        while end < len {
            let ch = rope.char(end);
            if ch.is_alphanumeric() || ch == '_' {
                break;
            }
            end += 1;
        }
        while end < len {
            let ch = rope.char(end);
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            end += 1;
        }

        if end > start {
            let killed = self.document.rope().slice(start..end).to_string();
            self.history.push_undo(self.document.snapshot(), start);
            self.document.remove(start, end);
            self.cursor.position = start;
            self.kill_ring.push(killed);
        }
    }

    pub fn kill_word_backward(&mut self) {
        self.last_edit_was_char_insert = false;
        let end = self.cursor.position.min(self.document.len_chars());
        let rope = self.document.rope();
        let mut start = end;

        while start > 0 {
            let ch = rope.char(start - 1);
            if ch.is_alphanumeric() || ch == '_' {
                break;
            }
            start -= 1;
        }
        while start > 0 {
            let ch = rope.char(start - 1);
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            start -= 1;
        }

        if end > start {
            let killed = self.document.rope().slice(start..end).to_string();
            self.history.push_undo(self.document.snapshot(), end);
            self.cursor.position = start;
            self.document.remove(start, end);
            self.kill_ring.push_prepend(killed);
        }
    }

    pub fn kill_region(&mut self) {
        if let Some(text) = self.delete_selection() {
            self.kill_ring.push(text);
        }
    }

    pub fn copy_region_as_kill(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        if let Some((start, end)) = self.cursor.selection() {
            let text = self.document.rope().slice(start..end).to_string();
            self.kill_ring.push(text);
            self.kill_ring.clear_kill_flag();
            self.cursor.clear_selection();
        }
    }

    pub fn yank(&mut self) {
        self.last_edit_was_char_insert = false;
        if let Some(text) = self.kill_ring.yank().map(str::to_string) {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);

            if let Some((start, end)) = self.cursor.selection() {
                self.document.remove(start, end);
                self.cursor.position = start;
                self.cursor.clear_selection();
            } else {
                self.cursor.clear_selection();
            }

            let start = self.cursor.position;
            self.document.insert(start, &text);
            let end = start + text.chars().count();
            self.cursor.position = end;
            self.kill_ring.mark_yank(start, end);
        }
    }

    pub fn yank_pop(&mut self) {
        self.last_edit_was_char_insert = false;
        if let Some((start, end)) = self.kill_ring.last_yank_range()
            && let Some(text) = self.kill_ring.yank_pop().map(str::to_string)
        {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            self.document.remove(start, end);
            self.document.insert(start, &text);
            let new_end = start + text.chars().count();
            self.cursor.position = new_end;
            self.kill_ring.mark_yank(start, new_end);
        }
    }

    pub fn rectangle_selection(&self) -> Option<(usize, usize, usize, usize)> {
        let anchor = self.cursor.anchor?;
        if self.document.len_chars() == 0 {
            return None;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let anchor = anchor.min(self.document.len_chars());

        let pos_line = self.document.char_to_line(pos);
        let pos_col = pos - self.document.line_to_char(pos_line);
        let anchor_line = self.document.char_to_line(anchor);
        let anchor_col = anchor - self.document.line_to_char(anchor_line);

        Some((
            pos_line.min(anchor_line),
            pos_col.min(anchor_col),
            pos_line.max(anchor_line),
            pos_col.max(anchor_col),
        ))
    }

    pub fn delete_rectangle(&mut self) {
        let Some((start_line, start_col, end_line, end_col)) = self.rectangle_selection() else {
            return;
        };
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.history
            .push_undo(self.document.snapshot(), self.cursor.position);
        self.delete_rectangle_range(start_line, start_col, end_line, end_col);
        self.cursor.clear_selection();
    }

    pub fn kill_rectangle(&mut self) {
        let Some((start_line, start_col, end_line, end_col)) = self.rectangle_selection() else {
            return;
        };
        let mut lines = Vec::with_capacity(end_line - start_line + 1);
        for line_idx in start_line..=end_line {
            let line_start = self.document.line_to_char(line_idx);
            let content_len = self.line_content_len(line_idx);
            let col_start = start_col.min(content_len);
            let col_end = end_col.min(content_len);
            lines.push(
                self.document
                    .rope()
                    .slice(line_start + col_start..line_start + col_end)
                    .to_string(),
            );
        }
        self.kill_ring.rectangle_buffer = Some(lines);
        self.delete_rectangle();
    }

    pub fn yank_rectangle(&mut self) {
        let Some(rectangle) = self.kill_ring.rectangle_buffer.clone() else {
            return;
        };
        if rectangle.is_empty() {
            return;
        }
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.history
            .push_undo(self.document.snapshot(), self.cursor.position);

        let pos = self.cursor.position.min(self.document.len_chars());
        let (start_line, start_col) = if self.document.len_chars() == 0 {
            (0, 0)
        } else {
            let line = self.document.char_to_line(pos);
            (line, pos - self.document.line_to_char(line))
        };

        for (idx, rect_line) in rectangle.iter().enumerate() {
            let target_line = start_line + idx;
            if target_line >= self.document.len_lines() {
                let doc_end = self.document.len_chars();
                self.document
                    .insert(doc_end, &format!("\n{}", " ".repeat(start_col)));
                let insert_pos = self.document.len_chars();
                self.document.insert(insert_pos, rect_line);
            } else {
                self.pad_line_to_col(target_line, start_col);
                let insert_pos = self.document.line_to_char(target_line) + start_col;
                self.document.insert(insert_pos, rect_line);
            }
        }
        self.cursor.clear_selection();
        self.update_preferred_column();
    }

    pub fn open_rectangle(&mut self) {
        let Some((start_line, start_col, end_line, end_col)) = self.rectangle_selection() else {
            return;
        };
        let width = end_col.saturating_sub(start_col);
        if width == 0 {
            return;
        }
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.history
            .push_undo(self.document.snapshot(), self.cursor.position);
        let spaces = " ".repeat(width);
        for line_idx in (start_line..=end_line).rev() {
            let line_start = self.document.line_to_char(line_idx);
            let col = start_col.min(self.line_content_len(line_idx));
            self.document.insert(line_start + col, &spaces);
        }
        self.cursor.clear_selection();
        self.update_preferred_column();
    }

    pub fn clear_rectangle(&mut self) {
        let Some((start_line, start_col, end_line, end_col)) = self.rectangle_selection() else {
            return;
        };
        let width = end_col.saturating_sub(start_col);
        if width == 0 {
            return;
        }
        self.replace_rectangle(start_line, start_col, end_line, end_col, &" ".repeat(width));
    }

    pub fn string_rectangle(&mut self, text: &str) {
        let Some((start_line, start_col, end_line, end_col)) = self.rectangle_selection() else {
            return;
        };
        self.replace_rectangle(start_line, start_col, end_line, end_col, text);
    }

    pub fn search_forward(&mut self, needle: &str) -> Option<usize> {
        self.search_literal(needle, SearchDirection::Forward)
    }

    pub fn search_backward(&mut self, needle: &str) -> Option<usize> {
        self.search_literal(needle, SearchDirection::Backward)
    }

    pub fn re_search_forward(&mut self, pattern: &str) -> Option<usize> {
        self.re_search(pattern, SearchDirection::Forward)
    }

    pub fn re_search_backward(&mut self, pattern: &str) -> Option<usize> {
        self.re_search(pattern, SearchDirection::Backward)
    }

    pub fn replace_match(&mut self, replacement: &str) -> bool {
        self.replace_current_match(replacement, ReplacementMode::Regexp)
    }

    pub fn replace_match_literal(&mut self, replacement: &str) -> bool {
        self.replace_current_match(replacement, ReplacementMode::Literal)
    }

    pub fn replace_match_regexp(&mut self, replacement: &str) -> bool {
        self.replace_current_match(replacement, ReplacementMode::Regexp)
    }

    pub fn replace_string(&mut self, from: &str, to: &str) -> usize {
        self.replace_literal_from_point(from, to, "Replaced")
    }

    pub fn query_replace(&mut self, from: &str, to: &str) -> usize {
        self.replace_literal_from_point(from, to, "Query replaced")
    }

    pub fn replace_regexp(&mut self, pattern: &str, replacement: &str) -> usize {
        self.replace_regexp_from_point(pattern, replacement, "Replaced")
    }

    pub fn query_replace_regexp(&mut self, pattern: &str, replacement: &str) -> usize {
        self.replace_regexp_from_point(pattern, replacement, "Query replaced")
    }

    fn replace_current_match(&mut self, replacement: &str, mode: ReplacementMode) -> bool {
        let Some((match_start, match_end)) =
            self.search_match_data.groups.first().copied().flatten()
        else {
            self.message = Some("No match data".to_string());
            return false;
        };
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let expanded = match mode {
            ReplacementMode::Literal => replacement.to_string(),
            ReplacementMode::Regexp => {
                expand_match_replacement(replacement, &self.document, &self.search_match_data)
            }
        };
        let len_before = self.document.len_chars();
        self.history
            .push_undo(self.document.snapshot(), self.cursor.position);
        let replaced = search::replace_match(
            &mut self.document,
            &mut self.cursor,
            &self.search_match_data,
            &expanded,
            0,
        );
        if replaced {
            if match_start == match_end && match_end < len_before {
                let replacement_len = expanded.chars().count();
                self.cursor.position =
                    (match_start + replacement_len + 1).min(self.document.len_chars());
            }
            self.cursor.clear_selection();
            self.update_preferred_column();
            self.message = Some("Replaced".to_string());
        } else {
            self.message = Some("No match data".to_string());
        }
        replaced
    }

    pub fn upcase_word(&mut self) {
        self.replace_word_at_point(|word| word.to_uppercase());
    }

    pub fn downcase_word(&mut self) {
        self.replace_word_at_point(|word| word.to_lowercase());
    }

    pub fn transpose_chars(&mut self) {
        self.last_edit_was_char_insert = false;
        let pos = self.cursor.position;
        let len = self.document.len_chars();
        if len < 2 || pos == 0 {
            return;
        }

        let (a, b) = if pos >= len {
            (len - 2, len - 1)
        } else {
            (pos - 1, pos)
        };
        let rope = self.document.rope();
        let ch_a = rope.char(a);
        let ch_b = rope.char(b);
        if ch_a == ch_b {
            self.cursor.position = (b + 1).min(len);
            return;
        }

        self.history
            .push_undo(self.document.snapshot(), self.cursor.position);
        self.document.remove(a, b + 1);
        self.document.insert(a, &format!("{ch_b}{ch_a}"));
        self.cursor.position = (b + 1).min(self.document.len_chars());
    }

    pub fn transpose_words(&mut self) {
        self.last_edit_was_char_insert = false;
        let original_pos = self.cursor.position;

        self.move_word_right(false);
        let word2_end = self.cursor.position;
        self.move_word_left(false);
        let word2_start = self.cursor.position;
        if word2_start == 0 {
            self.cursor.position = original_pos;
            return;
        }

        self.cursor.position = word2_start;
        self.move_word_left(false);
        let word1_start = self.cursor.position;
        self.move_word_right(false);
        let word1_end = self.cursor.position;

        if word1_start == word2_start || word1_end > word2_start {
            self.cursor.position = original_pos;
            return;
        }

        let rope = self.document.rope();
        let word1 = rope.slice(word1_start..word1_end).to_string();
        let word2 = rope.slice(word2_start..word2_end).to_string();

        self.history
            .push_undo(self.document.snapshot(), original_pos);
        self.document.remove(word2_start, word2_end);
        self.document.insert(word2_start, &word1);
        self.document.remove(word1_start, word1_end);
        self.document.insert(word1_start, &word2);
        self.cursor.position = word2_start + word1.chars().count();
    }

    pub fn set_mark(&mut self) {
        self.cursor.anchor = Some(self.cursor.position);
    }

    pub fn exchange_point_and_mark(&mut self) {
        if let Some(anchor) = self.cursor.anchor {
            let old_pos = self.cursor.position;
            self.cursor.position = anchor;
            self.cursor.anchor = Some(old_pos);
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

    pub fn switch_to_next_buffer(&mut self) {
        if !self.stored_buffers.is_empty() {
            self.swap_active_with_stored(0);
        }
    }

    pub fn switch_to_previous_buffer(&mut self) {
        if !self.stored_buffers.is_empty() {
            let last = self.stored_buffers.len() - 1;
            self.swap_active_with_stored(last);
        }
    }

    pub fn goto_char(&mut self, pos: usize) {
        self.cursor.position = pos.min(self.document.len_chars());
        self.cursor.clear_selection();
        self.update_preferred_column();
    }

    pub fn forward_char(&mut self, n: i64) {
        let extend = self.cursor.anchor.is_some();
        if n > 0 {
            for _ in 0..n as usize {
                self.move_right(extend);
            }
        } else if n < 0 {
            for _ in 0..(-n) as usize {
                self.move_left(extend);
            }
        }
    }

    pub fn forward_line(&mut self, n: i64) {
        let extend = self.cursor.anchor.is_some();
        if n > 0 {
            for _ in 0..n as usize {
                self.move_down(extend);
            }
        } else if n < 0 {
            for _ in 0..(-n) as usize {
                self.move_up(extend);
            }
        }
    }

    pub fn move_beginning_of_line(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let extend = self.cursor.anchor.is_some();
        if self.document.len_chars() == 0 {
            self.cursor.move_to(0, extend);
            self.update_preferred_column();
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        self.cursor
            .move_to(self.document.line_to_char(line), extend);
        self.update_preferred_column();
    }

    pub fn move_end_of_line(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let extend = self.cursor.anchor.is_some();
        if self.document.len_chars() == 0 {
            self.cursor.move_to(0, extend);
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
            .move_to(line_end.min(self.document.len_chars()), extend);
        self.update_preferred_column();
    }

    pub fn move_beginning_of_buffer(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.cursor.move_to(0, self.cursor.anchor.is_some());
        self.update_preferred_column();
    }

    pub fn move_end_of_buffer(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.cursor
            .move_to(self.document.len_chars(), self.cursor.anchor.is_some());
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

    fn move_word_left(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let rope = self.document.rope();
        let mut pos = self.cursor.position.min(rope.len_chars());
        while pos > 0 {
            let ch = rope.char(pos - 1);
            if ch.is_alphanumeric() || ch == '_' {
                break;
            }
            pos -= 1;
        }
        while pos > 0 {
            let ch = rope.char(pos - 1);
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            pos -= 1;
        }
        self.cursor.move_to(pos, extend);
        self.update_preferred_column();
    }

    fn move_word_right(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let rope = self.document.rope();
        let len = rope.len_chars();
        let mut pos = self.cursor.position.min(len);
        while pos < len {
            let ch = rope.char(pos);
            if ch.is_alphanumeric() || ch == '_' {
                break;
            }
            pos += 1;
        }
        while pos < len {
            let ch = rope.char(pos);
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            pos += 1;
        }
        self.cursor.move_to(pos, extend);
        self.update_preferred_column();
    }

    fn replace_word_at_point(&mut self, replace: impl FnOnce(&str) -> String) {
        self.last_edit_was_char_insert = false;
        let start = self.cursor.position.min(self.document.len_chars());
        self.move_word_right(false);
        let end = self.cursor.position;
        if end <= start || end > self.document.len_chars() {
            return;
        }

        let word = self.document.rope().slice(start..end).to_string();
        let replacement = replace(&word);
        self.history.push_undo(self.document.snapshot(), start);
        self.document.remove(start, end);
        self.document.insert(start, &replacement);
        self.cursor.position = start + replacement.chars().count();
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
            let major_mode = special_major_mode(&kind);
            buffer.kind = kind;
            buffer.read_only = read_only;
            buffer.major_mode = Some(major_mode.to_string());
        }
    }

    fn apply_special_attrs_for_current_name(&mut self) {
        if let Some((kind, read_only)) = special_buffer_attrs(&self.current_buffer_name) {
            let major_mode = special_major_mode(&kind);
            self.current_buffer_kind = kind;
            self.current_buffer_read_only = read_only;
            self.current_major_mode = Some(major_mode.to_string());
        }
    }

    fn delete_rectangle_range(
        &mut self,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) {
        for line_idx in (start_line..=end_line).rev() {
            let line_start = self.document.line_to_char(line_idx);
            let content_len = self.line_content_len(line_idx);
            let col_start = start_col.min(content_len);
            let col_end = end_col.min(content_len);
            if col_start < col_end {
                self.document
                    .remove(line_start + col_start, line_start + col_end);
            }
        }
    }

    fn replace_rectangle(
        &mut self,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
        replacement: &str,
    ) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.history
            .push_undo(self.document.snapshot(), self.cursor.position);
        for line_idx in (start_line..=end_line).rev() {
            self.replace_rectangle_line(line_idx, start_col, end_col, replacement);
        }
        self.cursor.clear_selection();
        self.update_preferred_column();
    }

    fn replace_rectangle_line(
        &mut self,
        line_idx: usize,
        start_col: usize,
        end_col: usize,
        replacement: &str,
    ) {
        let line_start = self.document.line_to_char(line_idx);
        let content_len = self.line_content_len(line_idx);
        let col_start = start_col.min(content_len);
        let col_end = end_col.min(content_len);
        if col_start < col_end {
            self.document
                .remove(line_start + col_start, line_start + col_end);
        }
        self.pad_line_to_col(line_idx, start_col);
        let insert_pos = self.document.line_to_char(line_idx) + start_col;
        self.document.insert(insert_pos, replacement);
    }

    fn pad_line_to_col(&mut self, line_idx: usize, col: usize) {
        let content_len = self.line_content_len(line_idx);
        if content_len >= col {
            return;
        }
        let line_start = self.document.line_to_char(line_idx);
        self.document
            .insert(line_start + content_len, &" ".repeat(col - content_len));
    }

    fn line_content_len(&self, line_idx: usize) -> usize {
        let line = self.document.line(line_idx);
        let mut len = line.len_chars();
        if len > 0 && line.char(len - 1) == '\n' {
            len -= 1;
        }
        if len > 0 && line.char(len - 1) == '\r' {
            len -= 1;
        }
        len
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

    fn search_literal(&mut self, needle: &str, direction: SearchDirection) -> Option<usize> {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.cursor.clear_selection();
        let result = search::search_literal(
            needle,
            &self.document,
            &mut self.cursor,
            direction,
            None,
            &mut self.search_match_data,
        );
        self.update_preferred_column();
        self.message = Some(match result {
            Some(_) => format!("Search: {needle}"),
            None => format!("Search failed: {needle}"),
        });
        result
    }

    fn re_search(&mut self, pattern: &str, direction: SearchDirection) -> Option<usize> {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.cursor.clear_selection();
        let rust_pattern = search::emacs_regex_to_rust(pattern);
        let Ok(re) = Regex::new(&rust_pattern) else {
            self.message = Some(format!("Invalid regexp: {pattern}"));
            return None;
        };
        let result = search::re_search_re(
            &re,
            &self.document,
            &mut self.cursor,
            direction,
            None,
            &mut self.search_match_data,
        );
        self.update_preferred_column();
        self.message = Some(match result {
            Some(_) => format!("Regexp search: {pattern}"),
            None => format!("Regexp search failed: {pattern}"),
        });
        result
    }

    fn replace_literal_from_point(&mut self, from: &str, to: &str, verb: &str) -> usize {
        if from.is_empty() {
            self.message = Some("Cannot replace empty string".to_string());
            return 0;
        }

        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let snapshot = self.document.snapshot();
        let undo_cursor = self.cursor.position;
        let mut cursor = self.cursor;
        let mut match_data = MatchData::default();
        let mut count = 0;

        while search::search_literal(
            from,
            &self.document,
            &mut cursor,
            SearchDirection::Forward,
            None,
            &mut match_data,
        )
        .is_some()
        {
            if count == 0 {
                self.history.push_undo(snapshot.clone(), undo_cursor);
            }
            if !search::replace_match(&mut self.document, &mut cursor, &match_data, to, 0) {
                break;
            }
            count += 1;
        }

        self.cursor = cursor;
        self.search_match_data = match_data;
        self.cursor.clear_selection();
        self.update_preferred_column();
        self.message = Some(format!("{verb} {count} occurrence{}", plural(count)));
        count
    }

    fn replace_regexp_from_point(&mut self, pattern: &str, replacement: &str, verb: &str) -> usize {
        let rust_pattern = search::emacs_regex_to_rust(pattern);
        let Ok(re) = Regex::new(&rust_pattern) else {
            self.message = Some(format!("Invalid regexp: {pattern}"));
            return 0;
        };

        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let snapshot = self.document.snapshot();
        let undo_cursor = self.cursor.position;
        let mut cursor = self.cursor;
        let mut match_data = MatchData::default();
        let mut count = 0;

        while search::re_search_re(
            &re,
            &self.document,
            &mut cursor,
            SearchDirection::Forward,
            None,
            &mut match_data,
        )
        .is_some()
        {
            let Some((match_start, match_end)) = match_data.groups.first().copied().flatten()
            else {
                break;
            };
            let len_before = self.document.len_chars();
            let expanded = expand_match_replacement(replacement, &self.document, &match_data);
            if count == 0 {
                self.history.push_undo(snapshot.clone(), undo_cursor);
            }
            if !search::replace_match(&mut self.document, &mut cursor, &match_data, &expanded, 0) {
                break;
            }
            count += 1;

            if match_start == match_end {
                if match_end >= len_before {
                    break;
                }
                let replacement_len = expanded.chars().count();
                cursor.position =
                    (match_start + replacement_len + 1).min(self.document.len_chars());
            }
        }

        self.cursor = cursor;
        self.search_match_data = match_data;
        self.cursor.clear_selection();
        self.update_preferred_column();
        self.message = Some(format!("{verb} {count} occurrence{}", plural(count)));
        count
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn expand_match_replacement(
    replacement: &str,
    doc: &DocumentBuffer,
    match_data: &MatchData,
) -> String {
    let mut out = String::new();
    let mut chars = replacement.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('&') => {
                if let Some(value) = match_data.match_string(0, doc) {
                    out.push_str(&value);
                }
            }
            Some(group @ '0'..='9') => {
                let index = match group {
                    '0' => 0,
                    '1' => 1,
                    '2' => 2,
                    '3' => 3,
                    '4' => 4,
                    '5' => 5,
                    '6' => 6,
                    '7' => 7,
                    '8' => 8,
                    '9' => 9,
                    _ => unreachable!("group is constrained by the match arm"),
                };
                if let Some(value) = match_data.match_string(index, doc) {
                    out.push_str(&value);
                }
            }
            Some('\\') => out.push('\\'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn list_obj(items: Vec<LispObject>) -> LispObject {
    let mut out = LispObject::nil();
    for item in items.into_iter().rev() {
        out = LispObject::cons(item, out);
    }
    out
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

    fn kill_word(&mut self, n: i64) {
        self.core.lock().kill_word(n);
    }

    fn kill_region(&mut self) {
        self.core.lock().kill_region();
    }

    fn copy_region_as_kill(&mut self) {
        self.core.lock().copy_region_as_kill();
    }

    fn yank(&mut self) {
        self.core.lock().yank();
    }

    fn yank_pop(&mut self) {
        self.core.lock().yank_pop();
    }

    fn delete_rectangle(&mut self) {
        self.core.lock().delete_rectangle();
    }

    fn kill_rectangle(&mut self) {
        self.core.lock().kill_rectangle();
    }

    fn yank_rectangle(&mut self) {
        self.core.lock().yank_rectangle();
    }

    fn open_rectangle(&mut self) {
        self.core.lock().open_rectangle();
    }

    fn clear_rectangle(&mut self) {
        self.core.lock().clear_rectangle();
    }

    fn string_rectangle(&mut self, text: &str) {
        self.core.lock().string_rectangle(text);
    }

    fn search_forward(&mut self, needle: &str) -> Option<usize> {
        self.core.lock().search_forward(needle)
    }

    fn search_backward(&mut self, needle: &str) -> Option<usize> {
        self.core.lock().search_backward(needle)
    }

    fn re_search_forward(&mut self, pattern: &str) -> Option<usize> {
        self.core.lock().re_search_forward(pattern)
    }

    fn re_search_backward(&mut self, pattern: &str) -> Option<usize> {
        self.core.lock().re_search_backward(pattern)
    }

    fn replace_match(&mut self, replacement: &str) -> bool {
        self.core.lock().replace_match(replacement)
    }

    fn replace_string(&mut self, from: &str, to: &str) -> usize {
        self.core.lock().replace_string(from, to)
    }

    fn query_replace(&mut self, from: &str, to: &str) -> usize {
        self.core.lock().query_replace(from, to)
    }

    fn replace_regexp(&mut self, pattern: &str, replacement: &str) -> usize {
        self.core.lock().replace_regexp(pattern, replacement)
    }

    fn query_replace_regexp(&mut self, pattern: &str, replacement: &str) -> usize {
        self.core.lock().query_replace_regexp(pattern, replacement)
    }

    fn upcase_word(&mut self) {
        self.core.lock().upcase_word();
    }

    fn downcase_word(&mut self) {
        self.core.lock().downcase_word();
    }

    fn transpose_chars(&mut self) {
        self.core.lock().transpose_chars();
    }

    fn transpose_words(&mut self) {
        self.core.lock().transpose_words();
    }

    fn set_mark(&mut self) {
        self.core.lock().set_mark();
    }

    fn exchange_point_and_mark(&mut self) {
        self.core.lock().exchange_point_and_mark();
    }

    fn next_buffer(&mut self) {
        self.core.lock().switch_to_next_buffer();
    }

    fn previous_buffer(&mut self) {
        self.core.lock().switch_to_previous_buffer();
    }

    fn toggle_preview(&mut self) {
        self.core.lock().preview_toggle_requested = true;
    }

    fn toggle_line_numbers(&mut self) {
        self.core.lock().line_numbers_toggle_requested = true;
    }

    fn toggle_preview_line_numbers(&mut self) {
        self.core.lock().preview_line_numbers_toggle_requested = true;
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
        Self::with_interpreter(core, interpreter)
    }

    pub fn with_interpreter(core: EditorCore, interpreter: Interpreter) -> Self {
        let core = Arc::new(Mutex::new(core));
        interpreter.set_editor(Box::new(ServerEditorCallbacks::new(core.clone())));

        let host = Self { interpreter, core };
        if let Err((idx, error)) = host.load_default_init() {
            log::error!("default elisp init failed at form {idx}: {error:?}");
        }
        host
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

    pub fn is_autoloaded_command(&self, name: &str) -> bool {
        self.interpreter.state.autoloads.read().contains_key(name)
    }

    pub fn interactive_spec_for(&self, name: &str) -> Option<InteractiveSpec> {
        rele_elisp::interactive_spec_for(name, &self.interpreter.state)
            .as_deref()
            .and_then(InteractiveSpec::from_elisp_spec)
    }

    pub fn plan_command(
        &self,
        name: &str,
        rust_interactive: Option<InteractiveSpec>,
    ) -> CommandPlan {
        if let Some(interactive) = self.interactive_spec_for(name) {
            return command_plan_for_interactive(interactive);
        }
        if let Some(interactive) = rust_interactive {
            return command_plan_for_interactive(interactive);
        }
        if self.is_user_defined_command(name) {
            CommandPlan::Run
        } else if self.is_autoloaded_command(name) {
            CommandPlan::Run
        } else {
            CommandPlan::Missing
        }
    }

    pub fn has_command(&self, name: &str, rust_exists: bool) -> bool {
        rust_exists || self.is_user_defined_command(name) || self.is_autoloaded_command(name)
    }

    pub fn user_command_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .interpreter
            .state
            .symbol_cells
            .read()
            .function_cells()
            .into_iter()
            .filter_map(|(symbol, cell)| {
                if matches!(cell, LispObject::Primitive(_)) {
                    None
                } else {
                    Some(rele_elisp::obarray::symbol_name(symbol))
                }
            })
            .collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn autoload_command_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .interpreter
            .state
            .autoloads
            .read()
            .keys()
            .cloned()
            .collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn command_completion_candidates<I, S>(&self, rust_names: I) -> Vec<String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut candidates: Vec<String> = rust_names.into_iter().map(Into::into).collect();
        candidates.extend(self.user_command_names());
        candidates.extend(self.autoload_command_names());
        candidates.sort();
        candidates.dedup();
        candidates
    }

    fn load_autoloaded_command(&self, name: &str) -> Result<bool, (usize, rele_elisp::ElispError)> {
        let Some(file) = self.interpreter.state.autoloads.read().get(name).cloned() else {
            return Ok(false);
        };
        self.load_builtin_library(&file)
    }

    pub fn run_command(
        &self,
        name: &str,
        args: &CommandArgs,
    ) -> Result<LispCommandDispatch, rele_elisp::ElispError> {
        if !self.is_user_defined_command(name) {
            match self.load_autoloaded_command(name) {
                Ok(true) => {}
                Ok(false) => return Ok(LispCommandDispatch::Unhandled),
                Err((_, error)) => return Err(error),
            }
        }
        if !self.is_user_defined_command(name) {
            return Ok(LispCommandDispatch::Unhandled);
        }

        let string_args = args.string_args();
        let call_args = if string_args.is_empty() {
            vec![LispObject::integer(args.count() as i64)]
        } else {
            string_args
                .into_iter()
                .map(|value| LispObject::string(&value))
                .collect()
        };
        let call = LispObject::cons(LispObject::symbol(name), list_obj(call_args));
        self.interpreter.eval(call)?;
        Ok(LispCommandDispatch::Handled)
    }

    pub fn run_hooks(&self, name: &str) -> rele_elisp::ElispResult<()> {
        self.interpreter.run_hooks(name)
    }

    pub fn load_builtin_commands(&self) -> Result<LoadReport, (usize, rele_elisp::ElispError)> {
        self.load_source(crate::BUILTIN_COMMANDS_EL, LoadMode::Strict)
    }

    pub fn load_default_init(&self) -> Result<LoadReport, (usize, rele_elisp::ElispError)> {
        self.load_source(crate::DEFAULT_INIT_EL, LoadMode::Strict)
    }

    pub fn load_builtin_library(
        &self,
        library: &str,
    ) -> Result<bool, (usize, rele_elisp::ElispError)> {
        match library.trim_end_matches(".el") {
            "rust-mode" => {
                self.load_source(crate::RUST_MODE_EL, LoadMode::Strict)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub fn auto_mode_for_path(
        &self,
        path: Option<&Path>,
    ) -> Result<Option<String>, (usize, rele_elisp::ElispError)> {
        let Some(path) = path else {
            return Ok(None);
        };
        let path = path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let result = self.eval_source(&format!("(rele--auto-mode-for-file \"{path}\")"))?;
        Ok(match result {
            LispObject::Symbol(id) => Some(rele_elisp::obarray::symbol_name(id)),
            LispObject::String(mode) => Some(mode),
            _ => None,
        })
    }

    pub fn activate_mode_for_path(
        &self,
        path: Option<&Path>,
    ) -> Result<Option<String>, (usize, rele_elisp::ElispError)> {
        let Some(mode) = self.auto_mode_for_path(path)? else {
            return Ok(None);
        };
        if self.core.lock().current_major_mode.as_deref() == Some(mode.as_str()) {
            return Ok(Some(mode));
        }
        self.run_command(&mode, &CommandArgs::default())
            .map_err(|error| (0, error))?;
        Ok(Some(mode))
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

fn command_plan_for_interactive(interactive: InteractiveSpec) -> CommandPlan {
    if matches!(interactive, InteractiveSpec::None) {
        CommandPlan::Run
    } else {
        CommandPlan::Prompt(interactive)
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

fn special_major_mode(kind: &BufferKind) -> &'static str {
    match kind {
        BufferKind::Dired => "dired-mode",
        BufferKind::BufferList => "Buffer-menu-mode",
        BufferKind::Scratch => "lisp-interaction-mode",
        BufferKind::File => "text-mode",
    }
}

fn inferred_major_mode_for_path(path: Option<&Path>) -> &'static str {
    path.and_then(|path| path.extension())
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            if ext.eq_ignore_ascii_case("el") {
                "emacs-lisp-mode"
            } else if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
                "markdown-mode"
            } else if ext.eq_ignore_ascii_case("rs") {
                "rust-mode"
            } else {
                "text-mode"
            }
        })
        .unwrap_or("text-mode")
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
    fn rust_mode_loads_for_rust_paths() {
        let host = LispHost::new(EditorCore::from_file(
            PathBuf::from("src/main.rs"),
            "fn main() {}\n",
        ));

        let mode = host
            .activate_mode_for_path(Some(Path::new("src/main.rs")))
            .expect("rust mode activates");

        assert_eq!(mode.as_deref(), Some("rust-mode"));
        assert_eq!(host.core_snapshot().current_major_mode(), "rust-mode");
        assert!(!matches!(
            host.eval_source("(featurep 'rust-mode)")
                .expect("featurep evaluates"),
            LispObject::Nil
        ));
        assert_eq!(
            rele_elisp::lookup_mode_key(host.interpreter(), "rust-mode", "C-c C-c"),
            Some("rust-compile".to_string())
        );
    }

    #[test]
    fn rust_mode_is_autoloaded_for_m_x_before_loading_mode_file() {
        let host = LispHost::new(EditorCore::new());

        assert!(host.has_command("rust-mode", false));
        assert!(
            host.command_completion_candidates(std::iter::empty::<&str>())
                .iter()
                .any(|name| name == "rust-mode")
        );
        assert!(matches!(
            host.eval_source("(featurep 'rust-mode)")
                .expect("featurep evaluates before load"),
            LispObject::Nil
        ));

        host.run_command("rust-mode", &CommandArgs::default())
            .expect("autoloaded rust-mode runs");

        assert_eq!(host.core_snapshot().current_major_mode(), "rust-mode");
        assert!(!matches!(
            host.eval_source("(featurep 'rust-mode)")
                .expect("featurep evaluates after load"),
            LispObject::Nil
        ));
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
    fn kill_region_and_yank_mutate_server_core() {
        let host = LispHost::new(EditorCore::new());
        host.load_builtin_commands().expect("builtin commands load");
        {
            let mut core = host.core.lock();
            core.insert_text("abcdef");
            core.cursor.anchor = Some(1);
            core.cursor.position = 4;
        }

        host.run_command("kill-region", &CommandArgs::default())
            .expect("kill-region dispatch");

        {
            let core = host.core.lock();
            assert_eq!(core.document.text(), "aef");
            assert_eq!(core.cursor.position, 1);
            assert!(!core.cursor.has_selection());
        }

        host.run_command("yank", &CommandArgs::default())
            .expect("yank dispatch");

        assert_eq!(host.core.lock().document.text(), "abcdef");
    }

    #[test]
    fn copy_region_as_kill_keeps_text_and_feeds_yank() {
        let host = LispHost::new(EditorCore::new());
        host.load_builtin_commands().expect("builtin commands load");
        {
            let mut core = host.core.lock();
            core.insert_text("abcdef");
            core.cursor.anchor = Some(1);
            core.cursor.position = 4;
        }

        host.run_command("copy-region-as-kill", &CommandArgs::default())
            .expect("copy-region-as-kill dispatch");

        {
            let mut core = host.core.lock();
            assert_eq!(core.document.text(), "abcdef");
            assert!(!core.cursor.has_selection());
            core.cursor.position = core.document.len_chars();
        }

        host.run_command("yank", &CommandArgs::default())
            .expect("yank dispatch");

        assert_eq!(host.core.lock().document.text(), "abcdefbcd");
    }

    #[test]
    fn zero_length_mark_is_cleared_by_insert_and_yank() {
        let mut core = EditorCore::new();
        core.insert_text("abc");
        core.cursor.position = 1;
        core.set_mark();
        core.insert_text("X");

        assert_eq!(core.document.text(), "aXbc");
        assert!(!core.cursor.has_selection());
        assert!(core.cursor.anchor.is_none());

        core.kill_ring.push("Y".to_string());
        core.set_mark();
        core.yank();

        assert_eq!(core.document.text(), "aXYbc");
        assert!(!core.cursor.has_selection());
        assert!(core.cursor.anchor.is_none());
    }

    #[test]
    fn active_mark_extends_region_with_movement() {
        let mut core = EditorCore::new();
        core.insert_text("abcdef");
        core.cursor.position = 1;
        core.set_mark();

        core.forward_char(2);

        assert_eq!(core.cursor.selection(), Some((1, 3)));
        core.kill_region();
        assert_eq!(core.document.text(), "adef");
    }

    #[test]
    fn rectangle_commands_round_trip_through_elisp() {
        let host = LispHost::new(EditorCore::new());
        host.load_builtin_commands().expect("builtin commands load");
        {
            let mut core = host.core.lock();
            core.insert_text("abcd\nefgh\nijkl");
            core.cursor.anchor = Some(1);
            core.cursor.position = 13;
        }

        host.run_command("kill-rectangle", &CommandArgs::default())
            .expect("kill-rectangle dispatch");

        {
            let mut core = host.core.lock();
            assert_eq!(core.document.text(), "ad\neh\nil");
            assert_eq!(
                core.kill_ring.rectangle_buffer.as_ref(),
                Some(&vec!["bc".to_string(), "fg".to_string(), "jk".to_string()])
            );
            core.cursor.position = 1;
        }

        host.run_command("yank-rectangle", &CommandArgs::default())
            .expect("yank-rectangle dispatch");

        assert_eq!(host.core.lock().document.text(), "abcd\nefgh\nijkl");
    }

    #[test]
    fn clear_open_and_string_rectangle_mutate_server_core() {
        let host = LispHost::new(EditorCore::new());
        host.load_builtin_commands().expect("builtin commands load");
        {
            let mut core = host.core.lock();
            core.insert_text("abcde\nfghij\nklmno");
            core.cursor.anchor = Some(1);
            core.cursor.position = 15;
        }

        host.run_command("clear-rectangle", &CommandArgs::default())
            .expect("clear-rectangle dispatch");
        assert_eq!(host.core.lock().document.text(), "a  de\nf  ij\nk  no");

        {
            let mut core = host.core.lock();
            core.document.set_text("abcde\nfghij\nklmno");
            core.cursor.anchor = Some(1);
            core.cursor.position = 15;
        }
        host.run_command(
            "string-rectangle",
            &CommandArgs::default().with_string("XX".to_string()),
        )
        .expect("string-rectangle dispatch");
        assert_eq!(host.core.lock().document.text(), "aXXde\nfXXij\nkXXno");

        {
            let mut core = host.core.lock();
            core.document.set_text("abcde\nfghij\nklmno");
            core.cursor.anchor = Some(1);
            core.cursor.position = 15;
        }
        host.run_command("open-rectangle", &CommandArgs::default())
            .expect("open-rectangle dispatch");
        assert_eq!(
            host.core.lock().document.text(),
            "a  bcde\nf  ghij\nk  lmno"
        );
    }

    #[test]
    fn search_forward_and_replace_match_keep_match_data_in_core() {
        let host = LispHost::new(EditorCore::new());
        host.load_builtin_commands().expect("builtin commands load");
        host.core.lock().insert_text("alpha beta beta");
        host.core.lock().cursor.position = 0;

        host.run_command(
            "search-forward",
            &CommandArgs::default().with_string("beta".to_string()),
        )
        .expect("search-forward dispatch");

        {
            let core = host.core.lock();
            assert_eq!(core.cursor.position, 10);
            assert_eq!(core.search_match_data.groups, vec![Some((6, 10))]);
        }

        host.run_command(
            "replace-match",
            &CommandArgs::default().with_string("rust".to_string()),
        )
        .expect("replace-match dispatch");

        assert_eq!(host.core.lock().document.text(), "alpha rust beta");
    }

    #[test]
    fn replace_string_uses_multi_prompt_args() {
        let host = LispHost::new(EditorCore::new());
        host.load_builtin_commands().expect("builtin commands load");
        host.core.lock().insert_text("one two one two");
        host.core.lock().cursor.position = 0;

        host.run_command(
            "replace-string",
            &CommandArgs::default().with_strings(vec!["one".to_string(), "1".to_string()]),
        )
        .expect("replace-string dispatch");

        assert_eq!(host.core.lock().document.text(), "1 two 1 two");
    }

    #[test]
    fn replace_regexp_expands_capture_references() {
        let host = LispHost::new(EditorCore::new());
        host.load_builtin_commands().expect("builtin commands load");
        host.core.lock().insert_text("abc-123 def-456");
        host.core.lock().cursor.position = 0;

        host.run_command(
            "replace-regexp",
            &CommandArgs::default().with_strings(vec![
                "\\([a-z]+\\)-\\([0-9]+\\)".to_string(),
                "\\2/\\1".to_string(),
            ]),
        )
        .expect("replace-regexp dispatch");

        assert_eq!(host.core.lock().document.text(), "123/abc 456/def");
    }

    #[test]
    fn replace_match_expands_recent_regexp_match() {
        let host = LispHost::new(EditorCore::new());
        host.load_builtin_commands().expect("builtin commands load");
        host.core.lock().insert_text("alpha beta");
        host.core.lock().cursor.position = 0;

        host.run_command(
            "re-search-forward",
            &CommandArgs::default().with_string("\\([a-z]+\\)".to_string()),
        )
        .expect("re-search-forward dispatch");
        host.run_command(
            "replace-match",
            &CommandArgs::default().with_string("\\1-\\&".to_string()),
        )
        .expect("replace-match dispatch");

        assert_eq!(host.core.lock().document.text(), "alpha-alpha beta");
    }

    #[test]
    fn re_search_forward_accepts_emacs_group_syntax() {
        let host = LispHost::new(EditorCore::new());
        host.load_builtin_commands().expect("builtin commands load");
        host.core.lock().insert_text("alpha beta gamma");
        host.core.lock().cursor.position = 0;

        host.run_command(
            "re-search-forward",
            &CommandArgs::default().with_string("\\(beta\\|delta\\)".to_string()),
        )
        .expect("re-search-forward dispatch");

        assert_eq!(host.core.lock().cursor.position, 10);
    }

    #[test]
    fn user_command_names_include_loaded_defuns_not_primitives() {
        let host = LispHost::new(EditorCore::new());
        host.load_builtin_commands().expect("builtin commands load");

        let names = host.user_command_names();

        assert!(names.iter().any(|name| name == "next-line"));
        assert!(!names.iter().any(|name| name == "forward-line"));
    }

    #[test]
    fn command_completion_candidates_merge_rust_and_elisp_names() {
        let host = LispHost::new(EditorCore::new());
        host.load_builtin_commands().expect("builtin commands load");

        let names = host.command_completion_candidates(["save-buffer", "next-line"]);

        assert!(names.iter().any(|name| name == "save-buffer"));
        assert!(names.iter().any(|name| name == "next-line"));
        assert_eq!(names.iter().filter(|name| *name == "next-line").count(), 1);
    }

    #[test]
    fn plan_command_prefers_elisp_interactive_spec() {
        let host = LispHost::new(EditorCore::new());
        host.eval_source("(defun prompt-me (x) (interactive \"sSay\") x)")
            .expect("defun loads");

        let plan = host.plan_command(
            "prompt-me",
            Some(InteractiveSpec::Buffer {
                prompt: "Rust buffer".to_string(),
            }),
        );

        assert_eq!(
            plan,
            CommandPlan::Prompt(InteractiveSpec::String {
                prompt: "Say".to_string(),
            })
        );
    }

    #[test]
    fn plan_command_parses_multi_string_interactive_spec() {
        let host = LispHost::new(EditorCore::new());
        host.eval_source("(defun prompt-two (a b) (interactive \"sFrom: \\nsTo: \") (list a b))")
            .expect("defun loads");

        let plan = host.plan_command("prompt-two", None);

        assert_eq!(
            plan,
            CommandPlan::Prompt(InteractiveSpec::StringList {
                prompts: vec!["From: ".to_string(), "To: ".to_string()],
            })
        );
    }

    #[test]
    fn plan_command_falls_back_to_rust_metadata() {
        let host = LispHost::new(EditorCore::new());

        let plan = host.plan_command(
            "find-file",
            Some(InteractiveSpec::File {
                prompt: "Find file".to_string(),
            }),
        );

        assert_eq!(
            plan,
            CommandPlan::Prompt(InteractiveSpec::File {
                prompt: "Find file".to_string(),
            })
        );
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
