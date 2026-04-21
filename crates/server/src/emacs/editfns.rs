//! Rust equivalents of Emacs `editfns.c` — buffer text manipulation
//! functions that operate on `DocumentBuffer` and `EditorCursor`.
//!
//! These are the core primitives that Emacs Lisp scripts use to read and
//! modify buffer contents: `point`, `goto-char`, `insert`, `delete-region`,
//! `buffer-string`, `char-after`, etc.

use crate::document::buffer::DocumentBuffer;
use crate::document::cursor::EditorCursor;

// ---- Position queries ----

/// `(point)` — return current cursor position (1-based, Emacs convention).
pub fn point(cursor: &EditorCursor) -> usize {
    cursor.position + 1 // Emacs positions are 1-based
}

/// `(point-min)` — always 1 (no narrowing yet).
pub fn point_min() -> usize {
    1
}

/// `(point-max)` — buffer size + 1 (1-based).
pub fn point_max(doc: &DocumentBuffer) -> usize {
    doc.len_chars() + 1
}

/// `(buffer-size)` — number of characters in the buffer.
pub fn buffer_size(doc: &DocumentBuffer) -> usize {
    doc.len_chars()
}

/// `(goto-char POS)` — move cursor to 1-based position POS.
/// Clamps to valid range.
pub fn goto_char(cursor: &mut EditorCursor, doc: &DocumentBuffer, pos: usize) -> usize {
    let zero_based = pos.saturating_sub(1).min(doc.len_chars());
    cursor.position = zero_based;
    cursor.clear_selection();
    pos.max(1).min(doc.len_chars() + 1)
}

// ---- Character queries ----

/// `(char-after &optional POS)` — character at POS (1-based), or None if at end.
/// O(log n) via ropey's B-tree random char access.
pub fn char_after(doc: &DocumentBuffer, pos: usize) -> Option<char> {
    let idx = pos.saturating_sub(1);
    if idx >= doc.len_chars() {
        return None;
    }
    Some(doc.rope().char(idx))
}

/// `(char-before &optional POS)` — character before POS (1-based), or None if at start.
pub fn char_before(doc: &DocumentBuffer, pos: usize) -> Option<char> {
    if pos <= 1 {
        return None;
    }
    char_after(doc, pos - 1)
}

/// `(following-char)` — character at point, or 0 if at end.
pub fn following_char(doc: &DocumentBuffer, cursor: &EditorCursor) -> char {
    char_after(doc, point(cursor)).unwrap_or('\0')
}

/// `(preceding-char)` — character before point, or 0 if at start.
pub fn preceding_char(doc: &DocumentBuffer, cursor: &EditorCursor) -> char {
    char_before(doc, point(cursor)).unwrap_or('\0')
}

// ---- Predicate queries ----

/// `(bobp)` — true if point is at beginning of buffer.
pub fn bobp(cursor: &EditorCursor) -> bool {
    cursor.position == 0
}

/// `(eobp)` — true if point is at end of buffer.
pub fn eobp(doc: &DocumentBuffer, cursor: &EditorCursor) -> bool {
    cursor.position >= doc.len_chars()
}

/// `(bolp)` — true if point is at beginning of line.
pub fn bolp(doc: &DocumentBuffer, cursor: &EditorCursor) -> bool {
    if doc.len_chars() == 0 {
        return true;
    }
    let pos = cursor.position.min(doc.len_chars());
    let line = doc.char_to_line(pos);
    pos == doc.line_to_char(line)
}

/// `(eolp)` — true if point is at end of line (at newline or end of buffer).
pub fn eolp(doc: &DocumentBuffer, cursor: &EditorCursor) -> bool {
    if doc.len_chars() == 0 {
        return true;
    }
    let pos = cursor.position.min(doc.len_chars());
    if pos >= doc.len_chars() {
        return true;
    }
    char_after(doc, pos + 1) == Some('\n')
}

// ---- Line position queries ----

/// `(pos-bol)` / `(line-beginning-position)` — position of start of current line (1-based).
pub fn pos_bol(doc: &DocumentBuffer, cursor: &EditorCursor) -> usize {
    if doc.len_chars() == 0 {
        return 1;
    }
    let pos = cursor.position.min(doc.len_chars());
    let line = doc.char_to_line(pos);
    doc.line_to_char(line) + 1
}

/// `(pos-eol)` / `(line-end-position)` — 1-based position of end of current line.
/// Returns the position of the last character before the newline (or end of buffer).
/// Emacs: `(line-end-position)` on "abc\n" with point on that line returns 4 (the `c`),
/// not 5 (the `\n`).
pub fn pos_eol(doc: &DocumentBuffer, cursor: &EditorCursor) -> usize {
    if doc.len_chars() == 0 {
        return 1;
    }
    let pos = cursor.position.min(doc.len_chars());
    let line = doc.char_to_line(pos);
    let line_start = doc.line_to_char(line);
    let line_len = doc.line(line).len_chars();
    let line_end = line_start + line_len; // 0-based, one past last char including \n
    // For non-last lines, the last char is \n — point before it.
    // For the last line (no trailing \n), point at end of buffer.
    if line < doc.len_lines() - 1 {
        line_end // 0-based position of the \n; as 1-based this is line_end (the \n itself)
    } else {
        line_end + 1 // 1-based: past last char
    }
}

// ---- Text extraction ----

/// `(buffer-string)` — entire buffer contents as a string.
pub fn buffer_string(doc: &DocumentBuffer) -> String {
    doc.text()
}

/// `(buffer-substring START END)` — substring from START to END (1-based, exclusive).
pub fn buffer_substring(doc: &DocumentBuffer, start: usize, end: usize) -> Option<String> {
    let s = start.saturating_sub(1);
    let e = end.saturating_sub(1);
    if s > e || e > doc.len_chars() {
        return None;
    }
    Some(doc.rope().slice(s..e).to_string())
}

// ---- Text modification ----

/// `(insert &rest ARGS)` — insert text at point.
/// Returns the number of characters inserted.
pub fn insert(doc: &mut DocumentBuffer, cursor: &mut EditorCursor, text: &str) -> usize {
    let pos = cursor.position.min(doc.len_chars());
    doc.insert(pos, text);
    let char_count = text.chars().count();
    cursor.position = pos + char_count;
    char_count
}

/// `(delete-region START END)` — delete text between START and END (1-based).
pub fn delete_region(
    doc: &mut DocumentBuffer,
    cursor: &mut EditorCursor,
    start: usize,
    end: usize,
) {
    let s = start.saturating_sub(1);
    let e = end.saturating_sub(1).min(doc.len_chars());
    if s < e {
        doc.remove(s, e);
        // If cursor was in or after the deleted region, adjust
        if cursor.position >= e {
            cursor.position -= e - s;
        } else if cursor.position > s {
            cursor.position = s;
        }
    }
}

/// `(delete-char N)` — delete N characters after point (positive) or before (negative).
pub fn delete_char(doc: &mut DocumentBuffer, cursor: &mut EditorCursor, n: i64) {
    let pos = cursor.position.min(doc.len_chars());
    if n > 0 {
        let count = usize::try_from(n).unwrap_or(usize::MAX);
        let end = pos.saturating_add(count).min(doc.len_chars());
        doc.remove(pos, end);
    } else if n < 0 {
        let count = usize::try_from(n.unsigned_abs()).unwrap_or(usize::MAX);
        let start = pos.saturating_sub(count);
        doc.remove(start, pos);
        cursor.position = start;
    }
}

/// `(char-to-string CHAR)` — convert a character to a single-character string.
pub fn char_to_string(c: char) -> String {
    c.to_string()
}

/// `(string-to-char STRING)` — return the first character of STRING, or 0 if empty.
pub fn string_to_char(s: &str) -> char {
    s.chars().next().unwrap_or('\0')
}

/// `(byte-to-string BYTE)` — convert a byte value to a single-byte string.
pub fn byte_to_string(byte: u8) -> String {
    String::from(byte as char)
}

// ---- Narrowing (stubs for now) ----

/// Narrowing state — restricts the accessible portion of the buffer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Restriction {
    /// 0-based start of accessible region.
    pub start: usize,
    /// 0-based end of accessible region (exclusive).
    pub end: Option<usize>,
}

impl Restriction {
    /// `(narrow-to-region START END)` — restrict buffer to [start, end) (1-based inputs).
    pub fn narrow(&mut self, start: usize, end: usize) {
        self.start = start.saturating_sub(1);
        self.end = Some(end.saturating_sub(1));
    }

    /// `(widen)` — remove narrowing.
    pub fn widen(&mut self) {
        self.start = 0;
        self.end = None;
    }

    /// Effective point-min (0-based).
    pub fn effective_min(&self) -> usize {
        self.start
    }

    /// Effective point-max (0-based).
    pub fn effective_max(&self, doc: &DocumentBuffer) -> usize {
        self.end.unwrap_or(doc.len_chars())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(text: &str) -> (DocumentBuffer, EditorCursor) {
        (DocumentBuffer::from_text(text), EditorCursor::new())
    }

    #[test]
    fn test_point_at_start() {
        let (_, cursor) = make_doc("hello");
        assert_eq!(point(&cursor), 1);
    }

    #[test]
    fn test_goto_char() {
        let (doc, mut cursor) = make_doc("hello");
        goto_char(&mut cursor, &doc, 3);
        assert_eq!(point(&cursor), 3);
        assert_eq!(cursor.position, 2);
    }

    #[test]
    fn test_goto_char_clamps() {
        let (doc, mut cursor) = make_doc("hi");
        let result = goto_char(&mut cursor, &doc, 100);
        assert_eq!(result, 3); // clamped to point-max
        assert_eq!(cursor.position, 2);
    }

    #[test]
    fn test_char_after() {
        let (doc, _) = make_doc("hello");
        assert_eq!(char_after(&doc, 1), Some('h'));
        assert_eq!(char_after(&doc, 3), Some('l'));
        assert_eq!(char_after(&doc, 6), None); // past end
    }

    #[test]
    fn test_char_before() {
        let (doc, _) = make_doc("hello");
        assert_eq!(char_before(&doc, 1), None); // nothing before pos 1
        assert_eq!(char_before(&doc, 2), Some('h'));
        assert_eq!(char_before(&doc, 6), Some('o'));
    }

    #[test]
    fn test_bobp_eobp() {
        let (doc, cursor) = make_doc("hi");
        assert!(bobp(&cursor));
        assert!(!eobp(&doc, &cursor));

        let cursor2 = EditorCursor::at(2);
        assert!(!bobp(&cursor2));
        assert!(eobp(&doc, &cursor2));
    }

    #[test]
    fn test_bolp_eolp() {
        let (doc, cursor) = make_doc("ab\ncd");
        assert!(bolp(&doc, &cursor)); // at start of line

        let cursor_mid = EditorCursor::at(1);
        assert!(!bolp(&doc, &cursor_mid));
        assert!(!eolp(&doc, &cursor_mid));

        let cursor_nl = EditorCursor::at(2);
        assert!(eolp(&doc, &cursor_nl)); // at the newline
    }

    #[test]
    fn test_buffer_size() {
        let (doc, _) = make_doc("hello");
        assert_eq!(buffer_size(&doc), 5);
        assert_eq!(point_min(), 1);
        assert_eq!(point_max(&doc), 6);
    }

    #[test]
    fn test_buffer_string() {
        let (doc, _) = make_doc("hello world");
        assert_eq!(buffer_string(&doc), "hello world");
    }

    #[test]
    fn test_buffer_substring() {
        let (doc, _) = make_doc("hello world");
        assert_eq!(buffer_substring(&doc, 1, 6), Some("hello".to_string()));
        assert_eq!(buffer_substring(&doc, 7, 12), Some("world".to_string()));
        assert_eq!(buffer_substring(&doc, 5, 3), None); // start > end
    }

    #[test]
    fn test_insert() {
        let (mut doc, mut cursor) = make_doc("helo");
        goto_char(&mut cursor, &doc, 4); // after "hel"
        let n = insert(&mut doc, &mut cursor, "l");
        assert_eq!(n, 1);
        assert_eq!(doc.text(), "hello");
        assert_eq!(cursor.position, 4);
    }

    #[test]
    fn test_delete_region() {
        let (mut doc, mut cursor) = make_doc("hello world");
        delete_region(&mut doc, &mut cursor, 6, 12); // delete " world"
        assert_eq!(doc.text(), "hello");
    }

    #[test]
    fn test_delete_char_forward() {
        let (mut doc, mut cursor) = make_doc("hello");
        goto_char(&mut cursor, &doc, 1);
        delete_char(&mut doc, &mut cursor, 1);
        assert_eq!(doc.text(), "ello");
    }

    #[test]
    fn test_delete_char_backward() {
        let (mut doc, mut cursor) = make_doc("hello");
        goto_char(&mut cursor, &doc, 6); // end
        delete_char(&mut doc, &mut cursor, -1);
        assert_eq!(doc.text(), "hell");
    }

    #[test]
    fn test_pos_bol_eol() {
        let (doc, mut cursor) = make_doc("line1\nline2\nline3");
        goto_char(&mut cursor, &doc, 8); // middle of "line2"
        assert_eq!(pos_bol(&doc, &cursor), 7); // start of "line2" (1-based)
    }

    #[test]
    fn test_narrowing() {
        let (doc, _) = make_doc("hello world");
        let mut restriction = Restriction::default();
        assert_eq!(restriction.effective_min(), 0);
        assert_eq!(restriction.effective_max(&doc), 11);

        restriction.narrow(3, 8); // "llo w"
        assert_eq!(restriction.effective_min(), 2);
        assert_eq!(restriction.effective_max(&doc), 7);

        restriction.widen();
        assert_eq!(restriction.effective_min(), 0);
        assert_eq!(restriction.effective_max(&doc), 11);
    }
}
