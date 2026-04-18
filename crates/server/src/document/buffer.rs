use ropey::Rope;
use std::cell::Cell;
use std::path::PathBuf;

use crate::lsp::position::char_offset_to_utf16_col;

/// An incremental text change recorded by the document buffer.
///
/// All positions refer to the **pre-edit** buffer state: they are
/// captured inside `insert` / `remove` before the rope is mutated.
/// `new_text` is what gets inserted at `[start..end_pre_edit)`.
///
/// Two coordinate systems are recorded:
///
/// 1. UTF-16 line/character — for LSP
///    `TextDocumentContentChangeEvent`.
/// 2. Byte offsets — for tree-sitter `InputEdit` (incremental parsing).
///
/// Both are computed at edit time; the post-edit byte positions
/// needed by tree-sitter follow from `start_byte` + `new_text.len()`.
#[derive(Debug, Clone)]
pub struct TextChange {
    // ---- UTF-16 positions (for LSP) ----
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,

    // ---- Byte positions (for tree-sitter) ----
    /// Absolute byte offset into the pre-edit rope where the edit starts.
    pub start_byte: usize,
    /// Byte column within `start_line` (bytes from line start to
    /// `start_byte`), in the pre-edit rope.
    pub start_byte_col: u32,
    /// Absolute byte offset into the pre-edit rope where the
    /// replaced range ends. Equal to `start_byte` for pure inserts.
    pub old_end_byte: usize,
    /// Byte column within `end_line` of `old_end_byte`, pre-edit.
    pub old_end_byte_col: u32,

    /// The text that now occupies `[start_byte..start_byte + new_text.len())`
    /// in the post-edit rope. Empty for pure deletions.
    pub new_text: String,
}

/// Cached buffer-wide derived statistics, keyed by `DocumentBuffer::version`.
/// All fields are `None` until first computed; invalidated when `version`
/// advances. Uses `Cell` so `&self` accessors can populate it.
#[derive(Debug, Default)]
struct DerivedCache {
    /// Version the cache was computed against. If this doesn't match
    /// `DocumentBuffer::version`, entries are stale.
    version: Cell<u64>,
    /// Cached `(buffer_text.split_whitespace().count())`.
    word_count: Cell<Option<usize>>,
}

/// Rope-backed document buffer with file tracking and dirty state.
pub struct DocumentBuffer {
    rope: Rope,
    file_path: Option<PathBuf>,
    dirty: bool,
    version: u64,
    /// Journal of incremental edits since the last `drain_changes()` call.
    change_journal: Vec<TextChange>,
    /// Set when a bulk operation (set_text, restore) makes incremental
    /// tracking impossible; the next LSP sync should send the full text.
    full_sync_needed: bool,
    /// Cached buffer-wide stats (word count, etc.). Invalidated via
    /// `version` bump on every mutation.
    derived: DerivedCache,
}

impl DocumentBuffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            file_path: None,
            dirty: false,
            version: 0,
            change_journal: Vec::new(),
            full_sync_needed: false,
            derived: DerivedCache::default(),
        }
    }

    pub fn from_text(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            file_path: None,
            dirty: false,
            version: 0,
            change_journal: Vec::new(),
            full_sync_needed: false,
            derived: DerivedCache::default(),
        }
    }

    pub fn from_file(path: PathBuf, content: &str) -> Self {
        Self {
            rope: Rope::from_str(content),
            file_path: Some(path),
            dirty: false,
            version: 0,
            change_journal: Vec::new(),
            full_sync_needed: false,
            derived: DerivedCache::default(),
        }
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Word count (whitespace-separated), cached until next mutation.
    ///
    /// Callers should prefer this to `document.text().split_whitespace().count()`
    /// because the latter allocates the whole buffer and rescans O(n) per call.
    pub fn word_count(&self) -> usize {
        if self.derived.version.get() == self.version
            && let Some(cached) = self.derived.word_count.get()
        {
            return cached;
        }
        // Stream chunks from the rope to avoid allocating the full buffer.
        // `split_whitespace` across chunk boundaries requires stitching
        // pieces: we track whether the last char of the previous chunk was
        // whitespace so a word split across chunks is counted once.
        let mut count: usize = 0;
        let mut in_word = false;
        for chunk in self.rope.chunks() {
            for ch in chunk.chars() {
                if ch.is_whitespace() {
                    in_word = false;
                } else if !in_word {
                    in_word = true;
                    count += 1;
                }
            }
        }
        self.derived.version.set(self.version);
        self.derived.word_count.set(Some(count));
        count
    }

    pub fn line(&self, idx: usize) -> ropey::RopeSlice<'_> {
        self.rope.line(idx)
    }

    pub fn char_to_line(&self, char_idx: usize) -> usize {
        self.rope.char_to_line(char_idx)
    }

    pub fn line_to_char(&self, line_idx: usize) -> usize {
        self.rope.line_to_char(line_idx)
    }

    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.rope.char_to_byte(char_idx)
    }

    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        self.rope.byte_to_char(byte_idx)
    }

    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    pub fn set_file_path(&mut self, path: PathBuf) {
        self.file_path = Some(path);
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    /// Insert text at a character offset.
    pub fn insert(&mut self, char_idx: usize, text: &str) {
        // Record the change BEFORE mutating the rope. We compute both the
        // LSP positions (UTF-16 line/col) and the tree-sitter positions
        // (absolute byte + byte col within line) in the same pass, since
        // both need the pre-edit rope and going twice would double the
        // cost.
        let line = self.rope.char_to_line(char_idx);
        let line_start_char = self.rope.line_to_char(line);
        let col_utf16 = char_offset_to_utf16_col(&self.rope, line, char_idx - line_start_char);
        let start_byte = self.rope.char_to_byte(char_idx);
        let line_start_byte = self.rope.line_to_byte(line);
        let byte_col = (start_byte - line_start_byte) as u32;
        self.change_journal.push(TextChange {
            start_line: line as u32,
            start_character: col_utf16,
            end_line: line as u32,
            end_character: col_utf16,
            start_byte,
            start_byte_col: byte_col,
            old_end_byte: start_byte, // pure insert: no range replaced
            old_end_byte_col: byte_col,
            new_text: text.to_string(),
        });
        self.rope.insert(char_idx, text);
        self.dirty = true;
        self.version += 1;
    }

    /// Remove a range of characters [start..end).
    pub fn remove(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let len = self.rope.len_chars();
        let end = end.min(len);
        let start = start.min(end);
        if start == end {
            return;
        }
        // Record the change BEFORE mutating the rope — we need pre-edit
        // byte offsets for tree-sitter's `InputEdit`.
        let start_line = self.rope.char_to_line(start);
        let start_line_char = self.rope.line_to_char(start_line);
        let start_col_utf16 =
            char_offset_to_utf16_col(&self.rope, start_line, start - start_line_char);
        let end_line = self.rope.char_to_line(end);
        let end_line_char = self.rope.line_to_char(end_line);
        let end_col_utf16 =
            char_offset_to_utf16_col(&self.rope, end_line, end - end_line_char);
        let start_byte = self.rope.char_to_byte(start);
        let old_end_byte = self.rope.char_to_byte(end);
        let start_byte_col = (start_byte - self.rope.line_to_byte(start_line)) as u32;
        let old_end_byte_col = (old_end_byte - self.rope.line_to_byte(end_line)) as u32;
        self.change_journal.push(TextChange {
            start_line: start_line as u32,
            start_character: start_col_utf16,
            end_line: end_line as u32,
            end_character: end_col_utf16,
            start_byte,
            start_byte_col,
            old_end_byte,
            old_end_byte_col,
            new_text: String::new(),
        });
        self.rope.remove(start..end);
        self.dirty = true;
        self.version += 1;
    }

    /// Replace text content entirely (e.g., from file load). Marks document clean.
    pub fn set_text(&mut self, text: &str) {
        self.rope = Rope::from_str(text);
        self.dirty = false;
        self.version += 1;
        self.full_sync_needed = true;
        self.change_journal.clear();
    }

    /// Replace text content as an edit (e.g., replace-all). Marks document dirty.
    pub fn replace_content(&mut self, text: &str) {
        self.rope = Rope::from_str(text);
        self.dirty = true;
        self.version += 1;
        self.full_sync_needed = true;
        self.change_journal.clear();
    }

    /// Clone the rope for undo snapshots.
    pub fn snapshot(&self) -> Rope {
        self.rope.clone()
    }

    /// Restore from a snapshot.
    pub fn restore(&mut self, snapshot: Rope) {
        self.rope = snapshot;
        self.dirty = true;
        self.version += 1;
        self.full_sync_needed = true;
        self.change_journal.clear();
    }

    /// Drain the change journal, returning all accumulated changes and
    /// whether a full document sync is needed instead.
    /// Resets both the journal and the full-sync flag.
    pub fn drain_changes(&mut self) -> (Vec<TextChange>, bool) {
        let full = self.full_sync_needed;
        self.full_sync_needed = false;
        let changes = std::mem::take(&mut self.change_journal);
        (changes, full)
    }

    /// Peek the change journal without draining. Used by consumers
    /// that run on every edit but don't own the journal (e.g. the
    /// tree-sitter highlighter, which updates incrementally but lets
    /// the LSP client drain the canonical journal for `didChange`).
    ///
    /// Returns a slice into the journal and the `full_sync_needed`
    /// flag. The journal may contain changes already seen by this
    /// consumer on a previous peek; consumers that care need to
    /// track their own cursor or call this only from the single edit
    /// hook (which is how `MdAppState::notify_highlighter` uses it:
    /// called on every edit, drained immediately on the subsequent
    /// LSP `didChange`).
    pub fn peek_changes(&self) -> (&[TextChange], bool) {
        (&self.change_journal, self.full_sync_needed)
    }
}

impl Default for DocumentBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_journal_empty_initially() {
        let mut buf = DocumentBuffer::from_text("hello");
        let (changes, full) = buf.drain_changes();
        assert!(changes.is_empty());
        assert!(!full);
    }

    #[test]
    fn insert_records_change_at_correct_position() {
        let mut buf = DocumentBuffer::from_text("hello");
        buf.insert(5, " world");
        let (changes, full) = buf.drain_changes();
        assert_eq!(changes.len(), 1);
        assert!(!full);
        let c = &changes[0];
        assert_eq!(c.start_line, 0);
        assert_eq!(c.start_character, 5);
        // Pure insertion: start == end.
        assert_eq!(c.end_line, 0);
        assert_eq!(c.end_character, 5);
        assert_eq!(c.new_text, " world");
    }

    #[test]
    fn remove_records_range_before_deletion() {
        let mut buf = DocumentBuffer::from_text("hello world");
        // Remove "hello " (chars 0..6).
        buf.remove(0, 6);
        let (changes, full) = buf.drain_changes();
        assert_eq!(changes.len(), 1);
        assert!(!full);
        let c = &changes[0];
        assert_eq!(c.start_line, 0);
        assert_eq!(c.start_character, 0);
        assert_eq!(c.end_line, 0);
        assert_eq!(c.end_character, 6);
        assert!(c.new_text.is_empty());
    }

    #[test]
    fn remove_across_newlines_records_multi_line_range() {
        let mut buf = DocumentBuffer::from_text("line1\nline2\nline3");
        // Remove from line 0 col 3 to line 2 col 1.
        // char_to_byte positions: line1=[0..5], \n=5, line2=[6..11], \n=11, line3=[12..17].
        // Remove chars 3..13: "e1\nline2\nl".
        buf.remove(3, 13);
        let (changes, _) = buf.drain_changes();
        assert_eq!(changes.len(), 1);
        let c = &changes[0];
        assert_eq!(c.start_line, 0);
        assert_eq!(c.start_character, 3);
        assert_eq!(c.end_line, 2);
        assert_eq!(c.end_character, 1);
    }

    #[test]
    fn drain_resets_journal() {
        let mut buf = DocumentBuffer::from_text("");
        buf.insert(0, "a");
        buf.insert(1, "b");
        let (changes1, _) = buf.drain_changes();
        assert_eq!(changes1.len(), 2);
        let (changes2, _) = buf.drain_changes();
        assert!(changes2.is_empty(), "journal should be empty after drain");
    }

    #[test]
    fn set_text_triggers_full_sync() {
        let mut buf = DocumentBuffer::from_text("old");
        buf.insert(3, "!"); // would be an incremental change...
        buf.set_text("entirely new"); // ...but this supersedes it
        let (changes, full) = buf.drain_changes();
        assert!(changes.is_empty(), "incremental changes must be discarded");
        assert!(full, "full_sync_needed must be set");
    }

    #[test]
    fn restore_triggers_full_sync() {
        let mut buf = DocumentBuffer::from_text("hello");
        let snapshot = buf.snapshot();
        buf.insert(5, " world");
        buf.restore(snapshot);
        let (changes, full) = buf.drain_changes();
        assert!(changes.is_empty());
        assert!(full);
    }

    #[test]
    fn replace_content_triggers_full_sync() {
        let mut buf = DocumentBuffer::from_text("original");
        buf.replace_content("replaced");
        let (changes, full) = buf.drain_changes();
        assert!(changes.is_empty());
        assert!(full);
    }

    #[test]
    fn word_count_basic() {
        let buf = DocumentBuffer::from_text("hello world foo bar");
        assert_eq!(buf.word_count(), 4);
    }

    #[test]
    fn word_count_empty_and_whitespace_only() {
        assert_eq!(DocumentBuffer::from_text("").word_count(), 0);
        assert_eq!(DocumentBuffer::from_text("   \n\t ").word_count(), 0);
    }

    #[test]
    fn word_count_cached_across_calls_same_version() {
        let buf = DocumentBuffer::from_text("a b c");
        assert_eq!(buf.word_count(), 3);
        // Should still be 3, reading from cache.
        assert_eq!(buf.word_count(), 3);
    }

    #[test]
    fn word_count_invalidates_on_insert() {
        let mut buf = DocumentBuffer::from_text("a b c");
        assert_eq!(buf.word_count(), 3);
        buf.insert(5, " d");
        assert_eq!(buf.word_count(), 4);
    }

    #[test]
    fn word_count_invalidates_on_remove() {
        let mut buf = DocumentBuffer::from_text("a b c d");
        assert_eq!(buf.word_count(), 4);
        buf.remove(5, 7); // remove " d" (chars 5..7 inclusive of space)
        assert_eq!(buf.word_count(), 3);
    }

    #[test]
    fn word_count_invalidates_on_set_text() {
        let mut buf = DocumentBuffer::from_text("one two three");
        assert_eq!(buf.word_count(), 3);
        buf.set_text("just one");
        assert_eq!(buf.word_count(), 2);
    }

    #[test]
    fn insert_records_byte_positions() {
        let mut buf = DocumentBuffer::from_text("hello\nworld");
        // Insert at char 6 = line 1 col 0 = byte 6
        buf.insert(6, "!");
        let (changes, _) = buf.drain_changes();
        assert_eq!(changes.len(), 1);
        let c = &changes[0];
        assert_eq!(c.start_byte, 6);
        assert_eq!(c.start_byte_col, 0);
        // Pure insert: old_end == start.
        assert_eq!(c.old_end_byte, 6);
        assert_eq!(c.old_end_byte_col, 0);
    }

    #[test]
    fn remove_records_byte_positions() {
        let mut buf = DocumentBuffer::from_text("hello world");
        // Remove chars 6..11 ("world") — bytes 6..11.
        buf.remove(6, 11);
        let (changes, _) = buf.drain_changes();
        assert_eq!(changes.len(), 1);
        let c = &changes[0];
        assert_eq!(c.start_byte, 6);
        assert_eq!(c.start_byte_col, 6);
        assert_eq!(c.old_end_byte, 11);
        assert_eq!(c.old_end_byte_col, 11);
        assert!(c.new_text.is_empty());
    }

    #[test]
    fn remove_across_newlines_byte_positions() {
        // "line1\nline2" — bytes 0..5 = "line1", byte 5 = '\n', bytes 6..11 = "line2"
        let mut buf = DocumentBuffer::from_text("line1\nline2");
        // Remove chars 3..8 = "e1\nli"
        buf.remove(3, 8);
        let (changes, _) = buf.drain_changes();
        let c = &changes[0];
        assert_eq!(c.start_line, 0);
        assert_eq!(c.start_byte, 3);
        assert_eq!(c.start_byte_col, 3);
        assert_eq!(c.end_line, 1);
        assert_eq!(c.old_end_byte, 8);
        // Byte col within line 1 = 8 - 6 = 2.
        assert_eq!(c.old_end_byte_col, 2);
    }

    #[test]
    fn insert_emoji_records_byte_positions() {
        // Grinning face = U+1F600 = 4 bytes in UTF-8, 2 UTF-16 units.
        let mut buf = DocumentBuffer::from_text("a\u{1F600}b");
        // Insert at char 3 (after 'b'). Bytes: 'a'=1, emoji=4, 'b'=1 -> byte 6.
        buf.insert(3, "c");
        let (changes, _) = buf.drain_changes();
        let c = &changes[0];
        assert_eq!(c.start_byte, 6);
        assert_eq!(c.start_byte_col, 6);
        // UTF-16 col: a=1, emoji=2, b=1 -> col 4.
        assert_eq!(c.start_character, 4);
    }

    #[test]
    fn insert_emoji_records_utf16_offset() {
        let mut buf = DocumentBuffer::from_text("a\u{1F600}b");
        // Insert at char offset 3 (after 'b'). UTF-16 col of offset 3:
        // 'a'=1, emoji=2, 'b'=1 -> column 4.
        buf.insert(3, "c");
        let (changes, _) = buf.drain_changes();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].start_character, 4);
    }
}
