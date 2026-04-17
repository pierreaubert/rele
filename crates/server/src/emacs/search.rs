//! Rust equivalents of Emacs `search.c` — regex and literal search,
//! match data, and replace operations on buffer text.
//!
//! Regex functions accept `&Regex` for the hot path. Convenience wrappers
//! that take `&str` patterns compile via `RegexCache`.
//!
//! All byte↔char conversions go through `CharByteMap` which detects
//! ASCII-only text (O(1) identity mapping) or precomputes the table once.

use std::collections::HashMap;

use regex::Regex;

use crate::document::buffer::DocumentBuffer;
use crate::document::cursor::EditorCursor;

// ---- Byte↔char offset mapping ----

/// Precomputed byte↔char mapping for a `&str`.
///
/// For ASCII-only text, byte offset == char offset (no table needed).
/// For non-ASCII text, builds a lookup table on construction so that
/// repeated `byte_to_char` calls are O(1) instead of O(n).
pub struct CharByteMap {
    /// `char_offsets[byte_idx]` = char offset at that byte position.
    /// Only populated for non-ASCII text.
    table: Option<Vec<u32>>,
    len_chars: usize,
}

impl CharByteMap {
    /// Build a mapping for the given string. O(n) once.
    pub fn new(s: &str) -> Self {
        if s.is_ascii() {
            return Self {
                table: None,
                len_chars: s.len(),
            };
        }
        // Build byte→char table: for each byte position, store the char count up to it.
        // We only need values at char boundaries, but storing for every byte
        // lets us do O(1) lookup without binary search.
        let mut table = Vec::with_capacity(s.len() + 1);
        let mut char_count = 0u32;
        for (byte_idx, _) in s.char_indices() {
            // Fill from previous entry to this byte with the current char count
            while table.len() < byte_idx {
                table.push(char_count);
            }
            table.push(char_count);
            char_count += 1;
        }
        // Fill remaining positions (up to and including s.len())
        while table.len() <= s.len() {
            table.push(char_count);
        }
        Self {
            table: Some(table),
            len_chars: char_count as usize,
        }
    }

    /// Convert a byte offset to a char offset. O(1).
    pub fn byte_to_char(&self, byte_pos: usize) -> usize {
        match &self.table {
            None => byte_pos, // ASCII fast path
            Some(t) => t.get(byte_pos).copied().unwrap_or(self.len_chars as u32) as usize,
        }
    }

    /// Convert a char offset to a byte offset. O(1) for ASCII, O(n) for non-ASCII.
    /// For non-ASCII, scans the table (could be optimized with a reverse table if needed).
    pub fn char_to_byte(&self, char_pos: usize, s: &str) -> usize {
        match &self.table {
            None => char_pos.min(s.len()), // ASCII fast path
            Some(_) => s
                .char_indices()
                .nth(char_pos)
                .map(|(i, _)| i)
                .unwrap_or(s.len()),
        }
    }
}

// ---- Regex cache ----

/// Simple LRU-ish regex cache. Avoids recompiling the same pattern every call.
pub struct RegexCache {
    cache: HashMap<String, Regex>,
    /// Insertion order for LRU eviction.
    order: Vec<String>,
    max_size: usize,
}

impl RegexCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::new(),
            order: Vec::new(),
            max_size,
        }
    }

    /// Get or compile a regex for the given pattern.
    pub fn get(&mut self, pattern: &str) -> Option<&Regex> {
        if !self.cache.contains_key(pattern) {
            let re = Regex::new(pattern).ok()?;
            if self.cache.len() >= self.max_size {
                // Evict oldest entry
                if let Some(oldest) = self.order.first().cloned() {
                    self.cache.remove(&oldest);
                    self.order.remove(0);
                }
            }
            self.order.push(pattern.to_string());
            self.cache.insert(pattern.to_string(), re);
        }
        self.cache.get(pattern)
    }
}

impl Default for RegexCache {
    fn default() -> Self {
        Self::new(64)
    }
}

// ---- Match data ----

/// Match data — stores positions of the last successful search.
/// Group 0 is the whole match; groups 1..N are capture groups.
#[derive(Debug, Clone, Default)]
pub struct MatchData {
    /// Each entry is (start, end) in 0-based buffer positions.
    /// Index 0 = whole match, 1..N = capture groups.
    pub groups: Vec<Option<(usize, usize)>>,
}

impl MatchData {
    /// `(match-beginning N)` — start of Nth match group (1-based for Emacs).
    pub fn match_beginning(&self, n: usize) -> Option<usize> {
        self.groups.get(n).and_then(|g| g.map(|(s, _)| s + 1))
    }

    /// `(match-end N)` — end of Nth match group (1-based for Emacs).
    pub fn match_end(&self, n: usize) -> Option<usize> {
        self.groups.get(n).and_then(|g| g.map(|(_, e)| e + 1))
    }

    /// `(match-string N)` — extract the matched text from the buffer.
    pub fn match_string(&self, n: usize, doc: &DocumentBuffer) -> Option<String> {
        let (start, end) = (*self.groups.get(n)?)?;
        if end <= doc.len_chars() {
            Some(doc.rope().slice(start..end).to_string())
        } else {
            None
        }
    }

    /// `(match-data)` — return flat list of (start end ...) as 1-based positions.
    pub fn as_positions(&self) -> Vec<Option<usize>> {
        let mut out = Vec::with_capacity(self.groups.len() * 2);
        for g in &self.groups {
            match g {
                Some((s, e)) => {
                    out.push(Some(s + 1));
                    out.push(Some(e + 1));
                }
                None => {
                    out.push(None);
                    out.push(None);
                }
            }
        }
        out
    }

    /// `(set-match-data LIST)` — set match data from flat list of 1-based positions.
    pub fn set_from_positions(&mut self, positions: &[Option<usize>]) {
        self.groups.clear();
        self.groups.reserve(positions.len() / 2);
        let mut i = 0;
        while i + 1 < positions.len() {
            match (positions[i], positions[i + 1]) {
                (Some(s), Some(e)) => {
                    self.groups
                        .push(Some((s.saturating_sub(1), e.saturating_sub(1))));
                }
                _ => self.groups.push(None),
            }
            i += 2;
        }
    }
}

// ---- Helper: populate match data from regex captures ----

fn populate_match_data(
    caps: &regex::Captures<'_>,
    match_data: &mut MatchData,
    map: &CharByteMap,
    byte_offset: usize,
) {
    match_data.groups.clear();
    match_data.groups.reserve(caps.len());
    for i in 0..caps.len() {
        match caps.get(i) {
            Some(m) => {
                let s = map.byte_to_char(byte_offset + m.start());
                let e = map.byte_to_char(byte_offset + m.end());
                match_data.groups.push(Some((s, e)));
            }
            None => match_data.groups.push(None),
        }
    }
}

// ---- String match ----

/// `(string-match REGEXP STRING &optional START)` — core version taking `&Regex`.
/// Returns the 0-based char index of the match start, or None.
pub fn string_match_re(
    re: &Regex,
    string: &str,
    start: usize,
    match_data: &mut MatchData,
) -> Option<usize> {
    let map = CharByteMap::new(string);
    let start_byte = map.char_to_byte(start, string);
    let search_str = &string[start_byte..];

    let caps = re.captures(search_str)?;
    let full = caps.get(0)?;

    // Build a map for the substring for byte→char within search_str
    let sub_map = CharByteMap::new(search_str);

    match_data.groups.clear();
    match_data.groups.reserve(caps.len());
    for i in 0..caps.len() {
        match caps.get(i) {
            Some(m) => {
                let s = sub_map.byte_to_char(m.start()) + start;
                let e = sub_map.byte_to_char(m.end()) + start;
                match_data.groups.push(Some((s, e)));
            }
            None => match_data.groups.push(None),
        }
    }

    Some(sub_map.byte_to_char(full.start()) + start)
}

/// Convenience: `string-match` from a pattern string.
pub fn string_match(
    pattern: &str,
    string: &str,
    start: usize,
    match_data: &mut MatchData,
) -> Option<usize> {
    let re = Regex::new(pattern).ok()?;
    string_match_re(&re, string, start, match_data)
}

// ---- Looking at ----

/// `(looking-at REGEXP)` — core version taking `&Regex`.
pub fn looking_at_re(
    re: &Regex,
    doc: &DocumentBuffer,
    cursor: &EditorCursor,
    match_data: &mut MatchData,
) -> bool {
    let pos = cursor.position.min(doc.len_chars());
    let text = doc.rope().slice(pos..).to_string();

    let caps = match re.captures(&text) {
        Some(c) => c,
        None => return false,
    };
    // Only match if it starts at position 0 (anchored at point)
    if caps.get(0).is_none_or(|full| full.start() != 0) {
        return false;
    }

    let map = CharByteMap::new(&text);
    populate_match_data(&caps, match_data, &map, 0);
    // Adjust all positions by adding `pos`
    for group in &mut match_data.groups {
        *group = group.map(|(s, e)| (s + pos, e + pos));
    }
    true
}

/// Convenience: `looking-at` from a pattern string.
pub fn looking_at(
    pattern: &str,
    doc: &DocumentBuffer,
    cursor: &EditorCursor,
    match_data: &mut MatchData,
) -> bool {
    let anchored = if pattern.starts_with('^') || pattern.starts_with("\\`") {
        pattern.to_string()
    } else {
        format!("^(?:{pattern})")
    };
    let re = match Regex::new(&anchored) {
        Ok(r) => r,
        Err(_) => return false,
    };
    looking_at_re(&re, doc, cursor, match_data)
}

// ---- Literal search ----

/// Search direction for `search-forward` / `search-backward`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

/// `(search-forward STRING)` / `(search-backward STRING)` — literal search.
/// Returns the 1-based position after the match end (forward) or match start (backward).
pub fn search_literal(
    needle: &str,
    doc: &DocumentBuffer,
    cursor: &mut EditorCursor,
    direction: SearchDirection,
    bound: Option<usize>,
    match_data: &mut MatchData,
) -> Option<usize> {
    let text = doc.text();
    let pos = cursor.position.min(doc.len_chars());
    let map = CharByteMap::new(&text);
    let needle_chars = needle.chars().count();

    let found = match direction {
        SearchDirection::Forward => {
            let start_byte = map.char_to_byte(pos, &text);
            let end_byte = bound.map_or(text.len(), |b| map.char_to_byte(b.saturating_sub(1), &text));
            let haystack = &text[start_byte..end_byte.min(text.len())];
            haystack.find(needle).map(|byte_off| {
                let char_off = map.byte_to_char(start_byte + byte_off);
                (char_off, char_off + needle_chars)
            })
        }
        SearchDirection::Backward => {
            let end_byte = map.char_to_byte(pos, &text);
            let start_byte = bound.map_or(0, |b| map.char_to_byte(b.saturating_sub(1), &text));
            let haystack = &text[start_byte..end_byte];
            haystack.rfind(needle).map(|byte_off| {
                let char_off = map.byte_to_char(start_byte + byte_off);
                (char_off, char_off + needle_chars)
            })
        }
    };

    match found {
        Some((start, end)) => {
            match_data.groups = vec![Some((start, end))];
            match direction {
                SearchDirection::Forward => {
                    cursor.position = end;
                    Some(end + 1)
                }
                SearchDirection::Backward => {
                    cursor.position = start;
                    Some(start + 1)
                }
            }
        }
        None => None,
    }
}

// ---- Regex search ----

/// `(re-search-forward REGEXP)` / `(re-search-backward REGEXP)` — core version.
pub fn re_search_re(
    re: &Regex,
    doc: &DocumentBuffer,
    cursor: &mut EditorCursor,
    direction: SearchDirection,
    bound: Option<usize>,
    match_data: &mut MatchData,
) -> Option<usize> {
    let text = doc.text();
    let pos = cursor.position.min(doc.len_chars());
    let map = CharByteMap::new(&text);

    match direction {
        SearchDirection::Forward => {
            let start_byte = map.char_to_byte(pos, &text);
            let search_str = &text[start_byte..];
            let caps = re.captures(search_str)?;
            let full = caps.get(0)?;
            let match_end_char = map.byte_to_char(start_byte + full.end());

            if let Some(b) = bound
                && match_end_char > b.saturating_sub(1)
            {
                return None;
            }

            populate_match_data(&caps, match_data, &map, start_byte);
            cursor.position = match_end_char;
            Some(match_end_char + 1)
        }
        SearchDirection::Backward => {
            let end_byte = map.char_to_byte(pos, &text);
            let search_str = &text[..end_byte];
            let bound_char = bound.map(|b| b.saturating_sub(1)).unwrap_or(0);

            // Find the last match at or after bound.
            let mut last_start = None;
            for m in re.find_iter(search_str) {
                let start_char = map.byte_to_char(m.start());
                if start_char >= bound_char {
                    last_start = Some(m.start());
                }
            }

            let last_byte = last_start?;
            let tail = &search_str[last_byte..];
            let caps = re.captures(tail)?;

            match_data.groups.clear();
            match_data.groups.reserve(caps.len());
            for i in 0..caps.len() {
                match caps.get(i) {
                    Some(m) => {
                        let s = map.byte_to_char(last_byte + m.start());
                        let e = map.byte_to_char(last_byte + m.end());
                        match_data.groups.push(Some((s, e)));
                    }
                    None => match_data.groups.push(None),
                }
            }

            let start_char = map.byte_to_char(last_byte);
            cursor.position = start_char;
            Some(start_char + 1)
        }
    }
}

/// Convenience: `re-search-forward` / `re-search-backward` from a pattern string.
pub fn re_search(
    pattern: &str,
    doc: &DocumentBuffer,
    cursor: &mut EditorCursor,
    direction: SearchDirection,
    bound: Option<usize>,
    match_data: &mut MatchData,
) -> Option<usize> {
    let re = Regex::new(pattern).ok()?;
    re_search_re(&re, doc, cursor, direction, bound, match_data)
}

// ---- Replace ----

/// `(replace-match NEWTEXT)` — replace the last match with NEWTEXT.
pub fn replace_match(
    doc: &mut DocumentBuffer,
    cursor: &mut EditorCursor,
    match_data: &MatchData,
    replacement: &str,
    subexp: usize,
) -> bool {
    let Some(Some((start, end))) = match_data.groups.get(subexp) else {
        return false;
    };
    let start = *start;
    let end = *end;
    if end > doc.len_chars() {
        return false;
    }
    doc.remove(start, end);
    doc.insert(start, replacement);
    cursor.position = start + replacement.chars().count();
    true
}

/// `(regexp-quote STRING)` — quote special regex characters.
pub fn regexp_quote(s: &str) -> String {
    regex::escape(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(text: &str) -> (DocumentBuffer, EditorCursor) {
        (DocumentBuffer::from_text(text), EditorCursor::new())
    }

    #[test]
    fn test_char_byte_map_ascii() {
        let map = CharByteMap::new("hello");
        assert_eq!(map.byte_to_char(0), 0);
        assert_eq!(map.byte_to_char(3), 3);
        assert_eq!(map.byte_to_char(5), 5);
        assert!(map.table.is_none()); // ASCII fast path
    }

    #[test]
    fn test_char_byte_map_unicode() {
        let s = "héllo"; // é is 2 bytes
        let map = CharByteMap::new(s);
        assert!(map.table.is_some());
        assert_eq!(map.byte_to_char(0), 0); // 'h'
        assert_eq!(map.byte_to_char(1), 1); // start of 'é'
        assert_eq!(map.byte_to_char(3), 2); // 'l' (byte 3 = char 2)
        assert_eq!(map.byte_to_char(5), 4); // 'o'
        assert_eq!(map.len_chars, 5);
    }

    #[test]
    fn test_regex_cache() {
        let mut cache = RegexCache::default();
        assert!(cache.get(r"\d+").is_some());
        assert!(cache.get(r"\d+").is_some());
        assert!(cache.get("[invalid").is_none());
    }

    #[test]
    fn test_regex_cache_eviction() {
        let mut cache = RegexCache::new(2);
        cache.get("a+");
        cache.get("b+");
        cache.get("c+"); // should evict "a+"
        assert_eq!(cache.cache.len(), 2);
        assert!(!cache.cache.contains_key("a+"));
    }

    #[test]
    fn test_string_match() {
        let mut md = MatchData::default();
        let result = string_match("l+", "hello", 0, &mut md);
        assert_eq!(result, Some(2));
        assert_eq!(md.match_beginning(0), Some(3));
        assert_eq!(md.match_end(0), Some(5));
    }

    #[test]
    fn test_string_match_with_groups() {
        let mut md = MatchData::default();
        let result = string_match("(h)(e)", "hello", 0, &mut md);
        assert_eq!(result, Some(0));
        assert_eq!(md.match_beginning(1), Some(1));
        assert_eq!(md.match_end(1), Some(2));
        assert_eq!(md.match_beginning(2), Some(2));
        assert_eq!(md.match_end(2), Some(3));
    }

    #[test]
    fn test_looking_at() {
        let (doc, cursor) = make_doc("hello world");
        let mut md = MatchData::default();
        assert!(looking_at("hel+o", &doc, &cursor, &mut md));
        assert!(!looking_at("world", &doc, &cursor, &mut md));
    }

    #[test]
    fn test_search_forward() {
        let (doc, mut cursor) = make_doc("hello world");
        let mut md = MatchData::default();
        let result = search_literal(
            "world",
            &doc,
            &mut cursor,
            SearchDirection::Forward,
            None,
            &mut md,
        );
        assert_eq!(result, Some(12));
        assert_eq!(cursor.position, 11);
    }

    #[test]
    fn test_search_backward() {
        let (doc, mut cursor) = make_doc("hello hello");
        cursor.position = 11;
        let mut md = MatchData::default();
        let result = search_literal(
            "hello",
            &doc,
            &mut cursor,
            SearchDirection::Backward,
            None,
            &mut md,
        );
        assert_eq!(result, Some(7));
    }

    #[test]
    fn test_re_search_forward() {
        let (doc, mut cursor) = make_doc("abc 123 def");
        let mut md = MatchData::default();
        let result = re_search(
            r"\d+",
            &doc,
            &mut cursor,
            SearchDirection::Forward,
            None,
            &mut md,
        );
        assert_eq!(result, Some(8));
        assert_eq!(md.match_beginning(0), Some(5));
    }

    #[test]
    fn test_re_search_backward() {
        let (doc, mut cursor) = make_doc("abc 123 def 456 ghi");
        cursor.position = 19;
        let mut md = MatchData::default();
        let result = re_search(
            r"\d+",
            &doc,
            &mut cursor,
            SearchDirection::Backward,
            None,
            &mut md,
        );
        assert_eq!(result, Some(13));
        assert_eq!(md.match_beginning(0), Some(13));
        assert_eq!(md.match_end(0), Some(16));
    }

    #[test]
    fn test_replace_match() {
        let (mut doc, mut cursor) = make_doc("hello world");
        let mut md = MatchData::default();
        string_match("world", &doc.text(), 0, &mut md);
        replace_match(&mut doc, &mut cursor, &md, "rust", 0);
        assert_eq!(doc.text(), "hello rust");
    }

    #[test]
    fn test_regexp_quote() {
        assert_eq!(regexp_quote("hello.world"), r"hello\.world");
        assert_eq!(regexp_quote("a+b*c"), r"a\+b\*c");
    }

    #[test]
    fn test_match_data_roundtrip() {
        let mut md = MatchData::default();
        md.groups = vec![Some((0, 5)), Some((2, 3))];
        let positions = md.as_positions();
        assert_eq!(positions, vec![Some(1), Some(6), Some(3), Some(4)]);

        let mut md2 = MatchData::default();
        md2.set_from_positions(&positions);
        assert_eq!(md2.groups, md.groups);
    }
}
