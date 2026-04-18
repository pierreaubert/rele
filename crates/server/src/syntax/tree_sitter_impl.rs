//! Tree-sitter-backed [`Highlighter`] implementation.
//!
//! Holds one `tree_sitter::Tree` per buffer, updated incrementally from
//! `DocumentBuffer::drain_changes()`. Queries the tree against the
//! shipped `highlights.scm` for each language and buckets matches per
//! line in the caller-supplied range.
//!
//! # Rope integration
//!
//! Tree-sitter's `parse_with_options` takes a callback that returns the
//! buffer contents as `&[u8]` chunks. We feed it directly from ropey via
//! `Rope::chunk_at_byte`, which returns the chunk containing a given
//! byte offset. No buffer-wide `String` allocation.

use std::collections::HashMap;

use tree_sitter::{
    InputEdit, Language, Parser, Point, Query, QueryCursor, StreamingIterator, Tree,
};

use crate::document::buffer::TextChange;
use crate::syntax::highlight::{Highlight, HighlightRange, Highlighter};

/// Supported tree-sitter grammars. The mapping from file extension to
/// language lives in [`language_for_extension`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TsLanguage {
    Markdown,
    Rust,
}

impl TsLanguage {
    /// The tree-sitter `Language` handle for this grammar.
    fn language(self) -> Language {
        match self {
            Self::Markdown => tree_sitter_md::LANGUAGE.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
        }
    }

    /// The shipped `highlights.scm` query source for this grammar.
    fn highlights_query(self) -> &'static str {
        match self {
            Self::Markdown => tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
            Self::Rust => tree_sitter_rust::HIGHLIGHTS_QUERY,
        }
    }
}

/// Resolve a file extension to a supported grammar, if any.
/// Returns `None` for unknown extensions — callers fall back to the
/// regex-based highlighter.
pub fn language_for_extension(ext: &str) -> Option<TsLanguage> {
    match ext {
        "md" | "markdown" => Some(TsLanguage::Markdown),
        "rs" => Some(TsLanguage::Rust),
        _ => None,
    }
}

/// Map a tree-sitter capture name (from the shipped `highlights.scm`) to
/// our semantic `Highlight` enum. Unknown captures return `None` and are
/// skipped.
fn capture_to_highlight(capture_name: &str) -> Option<Highlight> {
    // Tree-sitter capture names are dot-separated; we match on the prefix.
    // `@keyword.type` falls under "keyword", `@text.title` under "heading", etc.
    if capture_name == "keyword" || capture_name.starts_with("keyword.") {
        return Some(Highlight::Keyword);
    }
    if capture_name == "string" || capture_name.starts_with("string.") {
        return Some(Highlight::String);
    }
    if capture_name == "number" || capture_name.starts_with("number.") {
        return Some(Highlight::Number);
    }
    if capture_name == "comment" || capture_name.starts_with("comment.") {
        return Some(Highlight::Comment);
    }
    if capture_name == "function" || capture_name.starts_with("function.") {
        return Some(Highlight::Function);
    }
    if capture_name == "type" || capture_name.starts_with("type.") {
        return Some(Highlight::Type);
    }
    // Rust-specific: `@constructor`, `@constant` — treat as types/constants.
    if capture_name == "constructor" || capture_name == "constant" {
        return Some(Highlight::Type);
    }
    // Markdown-specific capture names shipped by tree-sitter-md.
    // See `tree-sitter-md/tree-sitter-markdown/queries/highlights.scm`.
    if capture_name == "text.title" {
        return Some(Highlight::Heading);
    }
    if capture_name == "text.emphasis" {
        return Some(Highlight::Emphasis);
    }
    if capture_name == "text.strong" {
        return Some(Highlight::Strong);
    }
    if capture_name == "text.literal" {
        return Some(Highlight::Code);
    }
    if capture_name == "text.uri" || capture_name == "text.reference" {
        return Some(Highlight::Link);
    }
    if capture_name == "punctuation.special" {
        // markdown h1/h2 markers, list bullets
        return Some(Highlight::ListMarker);
    }
    if capture_name == "punctuation.delimiter" {
        return Some(Highlight::Fence);
    }
    None
}

/// Tree-sitter-backed highlighter for a single buffer.
pub struct TreeSitterHighlighter {
    parser: Parser,
    query: Query,
    /// Precomputed: capture-index -> Highlight. Captures we don't map
    /// become `None`, which lets us skip them in the hot loop.
    capture_map: Vec<Option<Highlight>>,
    /// Current parse tree. `None` before the first parse.
    tree: Option<Tree>,
}

impl std::fmt::Debug for TreeSitterHighlighter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreeSitterHighlighter")
            .field("has_tree", &self.tree.is_some())
            .field("captures", &self.capture_map.len())
            .finish()
    }
}

impl TreeSitterHighlighter {
    /// Create a highlighter for the given language. Fails if the grammar
    /// version is incompatible or the shipped query is malformed, which
    /// would indicate a programmer error (mismatched crate versions).
    pub fn new(language: TsLanguage) -> Result<Self, String> {
        let mut parser = Parser::new();
        let ts_lang = language.language();
        parser
            .set_language(&ts_lang)
            .map_err(|e| format!("set_language: {e}"))?;

        let query = Query::new(&ts_lang, language.highlights_query())
            .map_err(|e| format!("Query::new: {e}"))?;

        let capture_map: Vec<Option<Highlight>> = query
            .capture_names()
            .iter()
            .map(|name| capture_to_highlight(name))
            .collect();

        Ok(Self {
            parser,
            query,
            capture_map,
            tree: None,
        })
    }

    /// Parse the buffer, reusing the previous tree for incremental
    /// reparsing when available.
    fn reparse(&mut self, rope: &ropey::Rope) {
        // Tree-sitter's parse_with_options takes a callback that yields
        // byte chunks. We feed directly from the rope — no whole-buffer
        // allocation.
        let total_bytes = rope.len_bytes();
        let mut cb = |byte_offset: usize, _pos: Point| -> &[u8] {
            if byte_offset >= total_bytes {
                return b"";
            }
            let (chunk, chunk_start, _, _) = rope.chunk_at_byte(byte_offset);
            // Slice chunk from byte_offset onward.
            let within_chunk = byte_offset - chunk_start;
            chunk.as_bytes()[within_chunk..].as_ref()
        };
        let new_tree = self.parser.parse_with_options(&mut cb, self.tree.as_ref(), None);
        if new_tree.is_some() {
            self.tree = new_tree;
        }
    }

    /// Convert a [`TextChange`] to a [`tree_sitter::InputEdit`].
    ///
    /// `TextChange` already records pre-edit byte offsets / byte
    /// columns, so this is just field mapping — no rope reads. The
    /// new-end position is computed from the inserted text.
    fn input_edit_for(change: &TextChange) -> InputEdit {
        let start_byte = change.start_byte;
        let old_end_byte = change.old_end_byte;
        let new_end_byte = start_byte + change.new_text.len();

        let start_position = Point::new(
            change.start_line as usize,
            change.start_byte_col as usize,
        );
        let old_end_position = Point::new(
            change.end_line as usize,
            change.old_end_byte_col as usize,
        );
        let new_end_position = new_end_point(
            start_position,
            change.start_byte_col,
            &change.new_text,
        );

        InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position,
            old_end_position,
            new_end_position,
        }
    }
}

/// Given the starting point of an edit and the inserted text, compute
/// the (row, byte_col) after insertion. Tree-sitter needs this as part
/// of `InputEdit.new_end_position`.
fn new_end_point(start: Point, start_byte_col: u32, new_text: &str) -> Point {
    let mut row = start.row;
    let mut byte_col = start_byte_col as usize;
    for byte in new_text.bytes() {
        if byte == b'\n' {
            row += 1;
            byte_col = 0;
        } else {
            // Count UTF-8 bytes, not chars — tree-sitter Point columns
            // are byte offsets.
            byte_col += 1;
        }
    }
    Point::new(row, byte_col)
}

impl Highlighter for TreeSitterHighlighter {
    fn on_edit(&mut self, rope: &ropey::Rope, changes: &[TextChange], full_sync: bool) {
        // Full sync or first-ever parse: discard any old tree, reparse.
        if full_sync || self.tree.is_none() {
            self.tree = None;
            self.reparse(rope);
            return;
        }

        // Incremental path: for each recorded change, apply a structural
        // `tree.edit()` using pre-edit byte offsets captured in the
        // change journal. Then re-run the parser with the old tree as
        // the baseline — tree-sitter reuses every subtree that wasn't
        // touched by any edit.
        //
        // Each `tree.edit()` is O(1) in edit size; the subsequent
        // `parse_with_options(..., Some(&tree))` is O(edit size + new
        // text size) on average. For a single-char insert in a 1000-
        // section markdown file this should be well under a millisecond
        // — verified by `crates/server/benches/syntax.rs`.
        if let Some(tree) = self.tree.as_mut() {
            for change in changes {
                let edit = Self::input_edit_for(change);
                tree.edit(&edit);
            }
        }
        self.reparse(rope);
    }

    fn highlight_range(
        &self,
        rope: &ropey::Rope,
        start_line: usize,
        end_line: usize,
    ) -> Vec<(usize, Vec<HighlightRange>)> {
        let Some(tree) = self.tree.as_ref() else {
            return Vec::new();
        };
        if end_line <= start_line {
            return Vec::new();
        }

        // Restrict the query to the byte range covering the target lines.
        let total_lines = rope.len_lines();
        let clamped_start = start_line.min(total_lines);
        let clamped_end = end_line.min(total_lines);
        let start_byte = rope.line_to_byte(clamped_start);
        let end_byte = rope.line_to_byte(clamped_end);

        // Group results by line.
        let mut per_line: HashMap<usize, Vec<HighlightRange>> = HashMap::new();

        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(start_byte..end_byte);

        // The TextProvider closure feeds tree-sitter the bytes covered by
        // a node, streaming from the rope without a whole-buffer clone.
        let text_provider = RopeBytesProvider { rope };

        let mut matches = cursor.matches(&self.query, tree.root_node(), text_provider);
        while let Some(m) = matches.next() {
            for cap in m.captures {
                let Some(Some(hl)) = self.capture_map.get(cap.index as usize).copied() else {
                    continue;
                };
                let node = cap.node;
                let start = node.start_position();
                let end = node.end_position();
                // Clip to requested line window.
                if start.row >= clamped_end || end.row < clamped_start {
                    continue;
                }
                let row_start = start.row.max(clamped_start);
                let row_end = end.row.min(clamped_end.saturating_sub(1));
                for row in row_start..=row_end {
                    let line_slice = rope.line(row);
                    // Convert byte columns from tree-sitter into character columns.
                    // Tree-sitter reports byte columns (0-based within the line);
                    // our HighlightRange uses char columns.
                    let start_col_bytes = if row == start.row { start.column } else { 0 };
                    let end_col_bytes = if row == end.row {
                        end.column
                    } else {
                        line_slice.len_bytes()
                    };
                    let start_col = byte_col_to_char_col(line_slice, start_col_bytes);
                    let end_col = byte_col_to_char_col(line_slice, end_col_bytes);
                    if end_col <= start_col {
                        continue;
                    }
                    per_line.entry(row).or_default().push(HighlightRange {
                        start_col,
                        end_col,
                        kind: hl,
                    });
                }
            }
        }

        // Sort ranges per line by start_col; emit lines in order.
        let mut out: Vec<(usize, Vec<HighlightRange>)> = per_line.into_iter().collect();
        out.sort_by_key(|(line, _)| *line);
        for (_, ranges) in &mut out {
            ranges.sort_by_key(|r| r.start_col);
        }
        out
    }
}

/// `TextProvider` impl that streams rope chunks for a given node's byte
/// range. Tree-sitter calls this when a query predicate needs to read
/// the text of a capture (e.g. `#match?`).
#[derive(Clone, Copy)]
struct RopeBytesProvider<'a> {
    rope: &'a ropey::Rope,
}

impl<'a> tree_sitter::TextProvider<&'a [u8]> for RopeBytesProvider<'a> {
    type I = RopeBytesIter<'a>;

    fn text(&mut self, node: tree_sitter::Node<'_>) -> Self::I {
        RopeBytesIter {
            rope: self.rope,
            cursor: node.start_byte(),
            end: node.end_byte(),
        }
    }
}

struct RopeBytesIter<'a> {
    rope: &'a ropey::Rope,
    cursor: usize,
    end: usize,
}

impl<'a> Iterator for RopeBytesIter<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.end {
            return None;
        }
        let (chunk, chunk_start, _, _) = self.rope.chunk_at_byte(self.cursor);
        let within = self.cursor - chunk_start;
        let chunk_end_in_rope = chunk_start + chunk.len();
        let stop_in_rope = self.end.min(chunk_end_in_rope);
        let slice_end = stop_in_rope - chunk_start;
        let out = &chunk.as_bytes()[within..slice_end];
        self.cursor = stop_in_rope;
        Some(out)
    }
}

/// Convert a byte column within a `RopeSlice` (one line) to a character
/// column. Tree-sitter speaks bytes; our `HighlightRange` speaks chars.
fn byte_col_to_char_col(line: ropey::RopeSlice<'_>, byte_col: usize) -> usize {
    let mut bytes: usize = 0;
    let mut chars: usize = 0;
    for ch in line.chars() {
        if bytes >= byte_col {
            break;
        }
        bytes += ch.len_utf8();
        chars += 1;
    }
    chars
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn language_for_extension_maps_known() {
        assert_eq!(language_for_extension("md"), Some(TsLanguage::Markdown));
        assert_eq!(language_for_extension("rs"), Some(TsLanguage::Rust));
        assert_eq!(language_for_extension("txt"), None);
    }

    #[test]
    fn markdown_highlighter_can_be_created() {
        let h = TreeSitterHighlighter::new(TsLanguage::Markdown);
        assert!(h.is_ok(), "markdown highlighter init failed: {:?}", h.err());
    }

    #[test]
    fn rust_highlighter_can_be_created() {
        let h = TreeSitterHighlighter::new(TsLanguage::Rust);
        assert!(h.is_ok(), "rust highlighter init failed: {:?}", h.err());
    }

    #[test]
    fn parses_empty_buffer() {
        let mut h = TreeSitterHighlighter::new(TsLanguage::Rust).unwrap();
        let rope = Rope::from_str("");
        h.on_edit(&rope, &[], true);
        let out = h.highlight_range(&rope, 0, 1);
        assert!(out.is_empty());
    }

    #[test]
    fn highlights_rust_keyword() {
        let mut h = TreeSitterHighlighter::new(TsLanguage::Rust).unwrap();
        let rope = Rope::from_str("fn main() {}\n");
        h.on_edit(&rope, &[], true);
        let out = h.highlight_range(&rope, 0, 1);
        // Expect at least one range on line 0 classified as a keyword.
        let line0 = out.iter().find(|(l, _)| *l == 0).expect("line 0 captures");
        assert!(
            line0.1.iter().any(|r| r.kind == Highlight::Keyword),
            "expected a Keyword on line 0, got {:?}",
            line0.1
        );
    }

    #[test]
    fn highlights_markdown_heading() {
        let mut h = TreeSitterHighlighter::new(TsLanguage::Markdown).unwrap();
        let rope = Rope::from_str("# Heading\n\nParagraph.\n");
        h.on_edit(&rope, &[], true);
        let out = h.highlight_range(&rope, 0, 1);
        let line0 = out.iter().find(|(l, _)| *l == 0).expect("line 0 captures");
        // Markdown query emits @text.title (our Heading) + punctuation marks.
        assert!(
            line0.1.iter().any(|r| r.kind == Highlight::Heading
                || r.kind == Highlight::ListMarker),
            "expected heading/marker captures, got {:?}",
            line0.1
        );
    }

    #[test]
    fn incremental_parse_updates_highlights() {
        use crate::document::buffer::TextChange;

        let mut h = TreeSitterHighlighter::new(TsLanguage::Rust).unwrap();
        // Initial state: one fn.
        let rope1 = Rope::from_str("fn a() {}\n");
        h.on_edit(&rope1, &[], true);
        let out1 = h.highlight_range(&rope1, 0, 1);
        let line0_kinds: Vec<_> = out1
            .iter()
            .find(|(l, _)| *l == 0)
            .map(|(_, r)| r.iter().map(|h| h.kind).collect())
            .unwrap_or_default();
        assert!(line0_kinds.contains(&Highlight::Keyword));

        // Apply an incremental edit: append a new function on line 1.
        // Before edit: bytes 0..10 = "fn a() {}\n".
        // After edit: bytes 0..23 = "fn a() {}\nfn b() {}\n".
        // Start of insert: byte 10 (line 1 col 0).
        let rope2 = Rope::from_str("fn a() {}\nfn b() {}\n");
        let change = TextChange {
            start_line: 1,
            start_character: 0,
            end_line: 1,
            end_character: 0,
            start_byte: 10,
            start_byte_col: 0,
            old_end_byte: 10,
            old_end_byte_col: 0,
            new_text: "fn b() {}\n".to_string(),
        };
        h.on_edit(&rope2, &[change], false);

        // Query line 1 — expect a Keyword on the new `fn`.
        let out2 = h.highlight_range(&rope2, 1, 2);
        let line1 = out2.iter().find(|(l, _)| *l == 1);
        assert!(
            line1.is_some()
                && line1
                    .unwrap()
                    .1
                    .iter()
                    .any(|r| r.kind == Highlight::Keyword),
            "incremental parse should expose the new `fn` keyword on line 1"
        );
    }

    #[test]
    fn incremental_parse_full_sync_resets_tree() {
        let mut h = TreeSitterHighlighter::new(TsLanguage::Rust).unwrap();
        let rope1 = Rope::from_str("fn a() {}\n");
        h.on_edit(&rope1, &[], true);
        // full_sync=true with empty changes must still reparse correctly
        // when the rope changes underneath (e.g. after undo).
        let rope2 = Rope::from_str("const X: u32 = 5;\n");
        h.on_edit(&rope2, &[], true);
        let out = h.highlight_range(&rope2, 0, 1);
        let line0 = out.iter().find(|(l, _)| *l == 0).expect("line 0 captures");
        // Expect a Type or Keyword for `const`.
        assert!(
            line0.1.iter().any(|r| r.kind == Highlight::Keyword
                || r.kind == Highlight::Type),
            "after full reparse we should see fresh highlights, got {:?}",
            line0.1
        );
    }

    #[test]
    fn highlights_clipped_to_requested_lines() {
        let mut h = TreeSitterHighlighter::new(TsLanguage::Rust).unwrap();
        let rope = Rope::from_str("fn a() {}\nfn b() {}\nfn c() {}\n");
        h.on_edit(&rope, &[], true);
        let out = h.highlight_range(&rope, 1, 2);
        // Nothing from line 0 or line 2 should leak in.
        for (line, _) in &out {
            assert_eq!(*line, 1, "unexpected line {line} in clipped output");
        }
    }
}
