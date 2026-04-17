//! Rust equivalents of Emacs `casefiddle.c` — case conversion for
//! characters, strings, and buffer regions.

use crate::document::buffer::DocumentBuffer;

// ---- Character-level ----

/// `(upcase CHAR-OR-STRING)` for a single character.
pub fn upcase_char(c: char) -> char {
    c.to_uppercase().next().unwrap_or(c)
}

/// `(downcase CHAR-OR-STRING)` for a single character.
pub fn downcase_char(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

// ---- String-level ----

/// `(upcase STRING)` — convert string to uppercase.
pub fn upcase_string(s: &str) -> String {
    s.to_uppercase()
}

/// `(downcase STRING)` — convert string to lowercase.
pub fn downcase_string(s: &str) -> String {
    s.to_lowercase()
}

/// `(capitalize STRING)` — capitalize each word.
/// First character of each word is uppercased, rest lowercased.
pub fn capitalize_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut word_start = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if word_start {
                result.extend(c.to_uppercase());
                word_start = false;
            } else {
                result.extend(c.to_lowercase());
            }
        } else {
            result.push(c);
            word_start = true;
        }
    }
    result
}

/// `(upcase-initials STRING)` — uppercase just the first letter of each word.
/// Unlike `capitalize`, does NOT lowercase the rest of each word.
pub fn upcase_initials(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut word_start = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if word_start {
                result.extend(c.to_uppercase());
                word_start = false;
            } else {
                result.push(c);
            }
        } else {
            result.push(c);
            word_start = true;
        }
    }
    result
}

// ---- Buffer region operations ----

/// `(upcase-region START END)` — uppercase a region of the buffer (0-based positions).
pub fn upcase_region(doc: &mut DocumentBuffer, start: usize, end: usize) {
    convert_region(doc, start, end, |s| s.to_uppercase());
}

/// `(downcase-region START END)` — lowercase a region.
pub fn downcase_region(doc: &mut DocumentBuffer, start: usize, end: usize) {
    convert_region(doc, start, end, |s| s.to_lowercase());
}

/// `(capitalize-region START END)` — capitalize each word in region.
pub fn capitalize_region(doc: &mut DocumentBuffer, start: usize, end: usize) {
    convert_region(doc, start, end, capitalize_string);
}

/// Helper: extract region text, apply a string conversion, replace if changed.
fn convert_region(
    doc: &mut DocumentBuffer,
    start: usize,
    end: usize,
    convert: impl FnOnce(&str) -> String,
) {
    let end = end.min(doc.len_chars());
    if start >= end {
        return;
    }
    let text = doc.rope().slice(start..end).to_string();
    let converted = convert(&text);
    if text != converted {
        doc.remove(start, end);
        doc.insert(start, &converted);
    }
}

// ---- Word-level (operates at point) ----

/// Find the next word boundary starting at `pos`.
/// Returns `(word_start, word_end)` in 0-based positions, or None if no word found.
fn next_word(doc: &DocumentBuffer, pos: usize) -> Option<(usize, usize)> {
    let len = doc.len_chars();
    // Skip non-word chars
    let mut start = pos;
    while start < len {
        if char_at(doc, start).is_some_and(|c| c.is_alphanumeric()) {
            break;
        }
        start += 1;
    }
    if start >= len {
        return None;
    }
    // Find end of word
    let mut end = start;
    while end < len {
        if !char_at(doc, end).is_some_and(|c| c.is_alphanumeric()) {
            break;
        }
        end += 1;
    }
    Some((start, end))
}

/// `(upcase-word COUNT)` — uppercase COUNT words from position.
/// Returns the new position (0-based) after the converted words.
pub fn upcase_word(doc: &mut DocumentBuffer, pos: usize, count: i64) -> usize {
    apply_to_words(doc, pos, count, |s| s.to_uppercase())
}

/// `(downcase-word COUNT)` — lowercase COUNT words from position.
pub fn downcase_word(doc: &mut DocumentBuffer, pos: usize, count: i64) -> usize {
    apply_to_words(doc, pos, count, |s| s.to_lowercase())
}

/// `(capitalize-word COUNT)` — capitalize COUNT words from position.
pub fn capitalize_word(doc: &mut DocumentBuffer, pos: usize, count: i64) -> usize {
    apply_to_words(doc, pos, count, capitalize_string)
}

/// Apply a string conversion to COUNT words starting from `pos`.
/// Returns the position after the last converted word.
fn apply_to_words(
    doc: &mut DocumentBuffer,
    pos: usize,
    count: i64,
    convert: impl Fn(&str) -> String,
) -> usize {
    if count <= 0 {
        return pos;
    }
    let mut current = pos;
    for _ in 0..count {
        let Some((word_start, word_end)) = next_word(doc, current) else {
            break;
        };
        convert_region(doc, word_start, word_end, &convert);
        current = word_end;
    }
    current
}

/// Get character at a 0-based position. O(log n) via ropey's B-tree.
fn char_at(doc: &DocumentBuffer, pos: usize) -> Option<char> {
    if pos >= doc.len_chars() {
        return None;
    }
    Some(doc.rope().char(pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upcase_string() {
        assert_eq!(upcase_string("hello"), "HELLO");
        assert_eq!(upcase_string("Hello World"), "HELLO WORLD");
    }

    #[test]
    fn test_downcase_string() {
        assert_eq!(downcase_string("HELLO"), "hello");
    }

    #[test]
    fn test_capitalize_string() {
        assert_eq!(capitalize_string("hello world"), "Hello World");
        assert_eq!(capitalize_string("HELLO WORLD"), "Hello World");
        assert_eq!(capitalize_string("hello-world"), "Hello-World");
    }

    #[test]
    fn test_upcase_initials() {
        assert_eq!(upcase_initials("hello world"), "Hello World");
        assert_eq!(upcase_initials("hELLO wORLD"), "HELLO WORLD");
    }

    #[test]
    fn test_upcase_region() {
        let mut doc = DocumentBuffer::from_text("hello world");
        upcase_region(&mut doc, 0, 5);
        assert_eq!(doc.text(), "HELLO world");
    }

    #[test]
    fn test_downcase_region() {
        let mut doc = DocumentBuffer::from_text("HELLO WORLD");
        downcase_region(&mut doc, 6, 11);
        assert_eq!(doc.text(), "HELLO world");
    }

    #[test]
    fn test_capitalize_region() {
        let mut doc = DocumentBuffer::from_text("hello world");
        capitalize_region(&mut doc, 0, 11);
        assert_eq!(doc.text(), "Hello World");
    }

    #[test]
    fn test_upcase_word() {
        let mut doc = DocumentBuffer::from_text("hello world");
        let pos = upcase_word(&mut doc, 0, 1);
        assert_eq!(doc.text(), "HELLO world");
        assert_eq!(pos, 5);
    }

    #[test]
    fn test_downcase_word() {
        let mut doc = DocumentBuffer::from_text("HELLO WORLD");
        let pos = downcase_word(&mut doc, 0, 2);
        assert_eq!(doc.text(), "hello world");
        assert_eq!(pos, 11);
    }

    #[test]
    fn test_capitalize_word() {
        let mut doc = DocumentBuffer::from_text("hello world");
        let pos = capitalize_word(&mut doc, 0, 2);
        assert_eq!(doc.text(), "Hello World");
        assert_eq!(pos, 11);
    }

    #[test]
    fn test_capitalize_word_mixed_case() {
        let mut doc = DocumentBuffer::from_text("hELLO wORLD");
        let pos = capitalize_word(&mut doc, 0, 2);
        assert_eq!(doc.text(), "Hello World");
        assert_eq!(pos, 11);
    }

    #[test]
    fn test_word_past_end() {
        let mut doc = DocumentBuffer::from_text("hi");
        let pos = upcase_word(&mut doc, 0, 5); // more words requested than exist
        assert_eq!(doc.text(), "HI");
        assert_eq!(pos, 2);
    }
}
