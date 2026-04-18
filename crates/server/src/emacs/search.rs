//! Rust equivalents of Emacs `search.c` — regex and literal search,
//! match data, and replace operations on buffer text.
//!
//! **Literal search** (`search-forward`, `search-backward`) is zero-alloc:
//! it walks the rope's chunks directly with an overlap buffer for boundary matches.
//!
//! **Regex search** allocates only the searched slice (cursor..end or start..cursor),
//! not the entire buffer. Uses `CharByteMap` for O(1) byte↔char conversion on that slice.

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
    table: Option<Vec<u32>>,
    len_chars: usize,
}

impl CharByteMap {
    pub fn new(s: &str) -> Self {
        if s.is_ascii() {
            return Self {
                table: None,
                len_chars: s.len(),
            };
        }
        let mut table = Vec::with_capacity(s.len() + 1);
        let mut char_count = 0u32;
        for (byte_idx, _) in s.char_indices() {
            while table.len() < byte_idx {
                table.push(char_count);
            }
            table.push(char_count);
            char_count += 1;
        }
        while table.len() <= s.len() {
            table.push(char_count);
        }
        Self {
            table: Some(table),
            len_chars: char_count as usize,
        }
    }

    pub fn byte_to_char(&self, byte_pos: usize) -> usize {
        match &self.table {
            None => byte_pos,
            Some(t) => t.get(byte_pos).copied().unwrap_or(self.len_chars as u32) as usize,
        }
    }
}

// ---- Regex cache ----

/// Simple LRU regex cache.
pub struct RegexCache {
    cache: HashMap<String, Regex>,
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

    pub fn get(&mut self, pattern: &str) -> Option<&Regex> {
        if !self.cache.contains_key(pattern) {
            let re = Regex::new(pattern).ok()?;
            if self.cache.len() >= self.max_size
                && let Some(oldest) = self.order.first().cloned()
            {
                self.cache.remove(&oldest);
                self.order.remove(0);
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

#[derive(Debug, Clone, Default)]
pub struct MatchData {
    pub groups: Vec<Option<(usize, usize)>>,
}

impl MatchData {
    pub fn match_beginning(&self, n: usize) -> Option<usize> {
        self.groups.get(n).and_then(|g| g.map(|(s, _)| s + 1))
    }

    pub fn match_end(&self, n: usize) -> Option<usize> {
        self.groups.get(n).and_then(|g| g.map(|(_, e)| e + 1))
    }

    pub fn match_string(&self, n: usize, doc: &DocumentBuffer) -> Option<String> {
        let (start, end) = (*self.groups.get(n)?)?;
        if end <= doc.len_chars() {
            Some(doc.rope().slice(start..end).to_string())
        } else {
            None
        }
    }

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

// ---- Helpers ----

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

// ---- String match (operates on &str, not buffer) ----

pub fn string_match_re(
    re: &Regex,
    string: &str,
    start: usize,
    match_data: &mut MatchData,
) -> Option<usize> {
    let start_byte = if start == 0 {
        0
    } else {
        string
            .char_indices()
            .nth(start)
            .map(|(i, _)| i)
            .unwrap_or(string.len())
    };
    let search_str = &string[start_byte..];
    let caps = re.captures(search_str)?;
    let full = caps.get(0)?;
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

pub fn looking_at_re(
    re: &Regex,
    doc: &DocumentBuffer,
    cursor: &EditorCursor,
    match_data: &mut MatchData,
) -> bool {
    let pos = cursor.position.min(doc.len_chars());
    // Allocate only cursor..end, not the whole buffer
    let text = doc.rope().slice(pos..).to_string();

    let caps = match re.captures(&text) {
        Some(c) => c,
        None => return false,
    };
    if caps.get(0).is_none_or(|full| full.start() != 0) {
        return false;
    }

    let map = CharByteMap::new(&text);
    populate_match_data(&caps, match_data, &map, 0);
    for group in &mut match_data.groups {
        *group = group.map(|(s, e)| (s + pos, e + pos));
    }
    true
}

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

// ---- Literal search (zero-alloc, chunk-based) ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

/// Zero-allocation literal search over rope chunks.
///
/// Walks chunks from the rope directly. For matches that span a chunk boundary,
/// maintains an overlap buffer of `needle.len() - 1` bytes from the previous
/// chunk's tail.
pub fn search_literal(
    needle: &str,
    doc: &DocumentBuffer,
    cursor: &mut EditorCursor,
    direction: SearchDirection,
    bound: Option<usize>,
    match_data: &mut MatchData,
) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    let needle_chars = needle.chars().count();

    match direction {
        SearchDirection::Forward => {
            search_literal_forward(doc, cursor, needle, needle_chars, bound, match_data)
        }
        SearchDirection::Backward => {
            search_literal_backward(doc, cursor, needle, needle_chars, bound, match_data)
        }
    }
}

fn search_literal_forward(
    doc: &DocumentBuffer,
    cursor: &mut EditorCursor,
    needle: &str,
    needle_chars: usize,
    bound: Option<usize>,
    match_data: &mut MatchData,
) -> Option<usize> {
    let pos = cursor.position.min(doc.len_chars());
    let start_byte = doc.char_to_byte(pos);
    let end_byte = bound.map_or(doc.rope().len_bytes(), |b| {
        doc.char_to_byte(b.saturating_sub(1).min(doc.len_chars()))
    });

    if start_byte >= end_byte {
        return None;
    }

    let rope = doc.rope();
    let overlap_size = needle.len().saturating_sub(1);
    let mut overlap_buf = String::with_capacity(overlap_size + 1024);
    let mut found_byte: Option<usize> = None;

    // Walk chunks starting from the one containing start_byte
    let (chunks, first_chunk_byte, _, _) = rope.chunks_at_byte(start_byte);
    let mut chunk_byte_start = first_chunk_byte;
    for chunk in chunks {
        let chunk_byte_end = chunk_byte_start + chunk.len();

        // Skip chunks entirely before our search window
        if chunk_byte_end <= start_byte {
            chunk_byte_start = chunk_byte_end;
            continue;
        }
        // Stop if we've passed the bound
        if chunk_byte_start >= end_byte {
            break;
        }

        // Check the overlap region (previous tail + current head)
        if !overlap_buf.is_empty() {
            let take_from_current = needle.len().min(chunk.len());
            overlap_buf.push_str(&chunk[..take_from_current]);
            if let Some(off) = overlap_buf.find(needle) {
                // Match found in overlap. The match starts at:
                // chunk_byte_start - (overlap before current chunk) + off
                let overlap_prefix_len = overlap_buf.len() - take_from_current;
                let match_byte = chunk_byte_start - overlap_prefix_len + off;
                if match_byte >= start_byte && match_byte + needle.len() <= end_byte {
                    found_byte = Some(match_byte);
                    break;
                }
            }
        }

        // Search within this chunk (clipped to our search window)
        let local_start = start_byte.saturating_sub(chunk_byte_start);
        let local_end = (end_byte - chunk_byte_start).min(chunk.len());
        if local_start < local_end {
            let haystack = &chunk[local_start..local_end];
            if let Some(off) = haystack.find(needle) {
                let match_byte = chunk_byte_start + local_start + off;
                if match_byte + needle.len() <= end_byte {
                    found_byte = Some(match_byte);
                    break;
                }
            }
        }

        // Prepare overlap for next iteration: keep tail of this chunk
        overlap_buf.clear();
        if chunk.len() >= overlap_size {
            overlap_buf.push_str(&chunk[chunk.len() - overlap_size..]);
        } else {
            overlap_buf.push_str(chunk);
        }

        chunk_byte_start = chunk_byte_end;
    }

    let match_byte = found_byte?;
    let match_char = rope.byte_to_char(match_byte);
    let end_char = match_char + needle_chars;

    match_data.groups = vec![Some((match_char, end_char))];
    cursor.position = end_char;
    Some(end_char + 1)
}

fn search_literal_backward(
    doc: &DocumentBuffer,
    cursor: &mut EditorCursor,
    needle: &str,
    needle_chars: usize,
    bound: Option<usize>,
    match_data: &mut MatchData,
) -> Option<usize> {
    let pos = cursor.position.min(doc.len_chars());
    let end_byte = doc.char_to_byte(pos);
    let start_byte = bound.map_or(0, |b| {
        doc.char_to_byte(b.saturating_sub(1).min(doc.len_chars()))
    });

    if start_byte >= end_byte {
        return None;
    }

    // For backward search, extract just the search window as a slice.
    // This is more efficient than walking chunks in reverse for the typical case
    // (the search window is small: cursor position to bound, usually on-screen).
    let rope = doc.rope();
    let slice = rope.byte_slice(start_byte..end_byte);
    let text = slice.to_string();

    // rfind on the slice
    let byte_off = text.rfind(needle)?;
    let match_byte = start_byte + byte_off;
    let match_char = rope.byte_to_char(match_byte);
    let end_char = match_char + needle_chars;

    match_data.groups = vec![Some((match_char, end_char))];
    cursor.position = match_char;
    Some(match_char + 1)
}

// ---- Regex search (scoped allocation) ----

/// `(re-search-forward REGEXP)` / `(re-search-backward REGEXP)` — core version.
///
/// Allocates only the searched portion of the buffer (cursor..end for forward,
/// start..cursor for backward), not the entire buffer.
pub fn re_search_re(
    re: &Regex,
    doc: &DocumentBuffer,
    cursor: &mut EditorCursor,
    direction: SearchDirection,
    bound: Option<usize>,
    match_data: &mut MatchData,
) -> Option<usize> {
    let pos = cursor.position.min(doc.len_chars());

    match direction {
        SearchDirection::Forward => {
            let end_char = bound.unwrap_or(doc.len_chars() + 1).saturating_sub(1).min(doc.len_chars());
            // Allocate only pos..end_char
            let slice_text = doc.rope().slice(pos..end_char).to_string();
            let caps = re.captures(&slice_text)?;
            let full = caps.get(0)?;

            let map = CharByteMap::new(&slice_text);
            let match_end_char = map.byte_to_char(full.end()) + pos;

            populate_match_data(&caps, match_data, &map, 0);
            // Shift positions by `pos`
            for group in &mut match_data.groups {
                *group = group.map(|(s, e)| (s + pos, e + pos));
            }

            cursor.position = match_end_char;
            Some(match_end_char + 1)
        }
        SearchDirection::Backward => {
            let start_char = bound.map(|b| b.saturating_sub(1)).unwrap_or(0);
            // Allocate only start_char..pos
            let slice_text = doc.rope().slice(start_char..pos).to_string();

            // Find the last match
            let mut last_start = None;
            for m in re.find_iter(&slice_text) {
                last_start = Some(m.start());
            }
            let last_byte = last_start?;

            let tail = &slice_text[last_byte..];
            let caps = re.captures(tail)?;
            let map = CharByteMap::new(&slice_text);

            match_data.groups.clear();
            match_data.groups.reserve(caps.len());
            for i in 0..caps.len() {
                match caps.get(i) {
                    Some(m) => {
                        let s = map.byte_to_char(last_byte + m.start()) + start_char;
                        let e = map.byte_to_char(last_byte + m.end()) + start_char;
                        match_data.groups.push(Some((s, e)));
                    }
                    None => match_data.groups.push(None),
                }
            }

            let match_start_char = map.byte_to_char(last_byte) + start_char;
            cursor.position = match_start_char;
            Some(match_start_char + 1)
        }
    }
}

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
        assert!(map.table.is_none());
    }

    #[test]
    fn test_char_byte_map_unicode() {
        let s = "héllo";
        let map = CharByteMap::new(s);
        assert!(map.table.is_some());
        assert_eq!(map.byte_to_char(0), 0);
        assert_eq!(map.byte_to_char(1), 1);
        assert_eq!(map.byte_to_char(3), 2);
        assert_eq!(map.byte_to_char(5), 4);
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
        cache.get("c+");
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
    fn test_search_forward_at_start() {
        let (doc, mut cursor) = make_doc("hello world");
        let mut md = MatchData::default();
        let result = search_literal(
            "hello",
            &doc,
            &mut cursor,
            SearchDirection::Forward,
            None,
            &mut md,
        );
        assert_eq!(result, Some(6));
        assert_eq!(cursor.position, 5);
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
    fn test_search_backward_from_middle() {
        let (doc, mut cursor) = make_doc("abc def abc");
        cursor.position = 8; // after "def "
        let mut md = MatchData::default();
        let result = search_literal(
            "abc",
            &doc,
            &mut cursor,
            SearchDirection::Backward,
            None,
            &mut md,
        );
        assert_eq!(result, Some(1)); // first "abc"
    }

    #[test]
    fn test_search_empty_needle() {
        let (doc, mut cursor) = make_doc("hello");
        let mut md = MatchData::default();
        assert_eq!(
            search_literal("", &doc, &mut cursor, SearchDirection::Forward, None, &mut md),
            None
        );
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
