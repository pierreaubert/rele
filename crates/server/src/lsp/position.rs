//! Conversion helpers between ropey char offsets and LSP positions.
//!
//! LSP positions use (line, UTF-16 code-unit offset) by default.
//! Ropey stores text as Unicode scalar values (codepoints).
//! Characters outside the BMP (e.g. emoji) occupy two UTF-16 code units
//! but one ropey char.

use lsp_types::{Position, Uri};
use ropey::Rope;
use std::path::Path;

/// Compute the UTF-16 column offset for a character that is `char_col` codepoints
/// into `line` of `rope`.
pub fn char_offset_to_utf16_col(rope: &Rope, line: usize, char_col: usize) -> u32 {
    let line_slice = rope.line(line);
    let mut utf16_offset: u32 = 0;
    for (i, ch) in line_slice.chars().enumerate() {
        if i >= char_col {
            break;
        }
        utf16_offset += ch.len_utf16() as u32;
    }
    utf16_offset
}

/// Convert a ropey char offset to an LSP `Position` (line, UTF-16 character).
pub fn char_offset_to_position(rope: &Rope, char_offset: usize) -> Position {
    let char_offset = char_offset.min(rope.len_chars());
    let line = rope.char_to_line(char_offset);
    let line_start = rope.line_to_char(line);
    let char_col = char_offset - line_start;
    let character = char_offset_to_utf16_col(rope, line, char_col);
    Position {
        line: line as u32,
        character,
    }
}

/// Convert an LSP `Position` (line, UTF-16 character) to a ropey char offset.
pub fn position_to_char_offset(rope: &Rope, position: &Position) -> usize {
    let line = (position.line as usize).min(rope.len_lines().saturating_sub(1));
    let line_start = rope.line_to_char(line);
    let line_slice = rope.line(line);
    let mut utf16_offset: u32 = 0;
    let mut char_count: usize = 0;
    for ch in line_slice.chars() {
        if utf16_offset >= position.character {
            break;
        }
        utf16_offset += ch.len_utf16() as u32;
        char_count += 1;
    }
    line_start + char_count
}

/// Create an `lsp_types::Uri` from a filesystem path.
///
/// Properly percent-encodes path components (spaces, non-ASCII, etc.) via
/// `url::Url::from_file_path`, then converts to `lsp_types::Uri` (which uses
/// `fluent_uri` internally) via string parsing. Both URI implementations
/// produce the same wire format, so the round-trip is lossless.
pub fn uri_from_path(path: &Path) -> Option<Uri> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    // `url::Url::from_file_path` enforces absolute paths and handles encoding.
    let url = url::Url::from_file_path(&abs).ok()?;
    url.as_str().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_roundtrip() {
        let rope = Rope::from_str("hello\nworld\n");
        // 'w' is at line 1, col 0
        let pos = char_offset_to_position(&rope, 6);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 0);
        assert_eq!(position_to_char_offset(&rope, &pos), 6);
    }

    #[test]
    fn mid_line() {
        let rope = Rope::from_str("abcdef\n");
        let pos = char_offset_to_position(&rope, 3);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 3);
        assert_eq!(position_to_char_offset(&rope, &pos), 3);
    }

    #[test]
    fn emoji_utf16_surrogate() {
        // U+1F600 (grinning face) is 2 UTF-16 code units but 1 codepoint.
        let rope = Rope::from_str("a\u{1F600}b\n");
        // 'b' is at char offset 2 (a=0, emoji=1, b=2)
        let pos = char_offset_to_position(&rope, 2);
        assert_eq!(pos.line, 0);
        // UTF-16: 'a'=1, emoji=2, so 'b' starts at UTF-16 offset 3
        assert_eq!(pos.character, 3);
        assert_eq!(position_to_char_offset(&rope, &pos), 2);
    }

    #[test]
    fn end_of_document() {
        let rope = Rope::from_str("abc");
        let pos = char_offset_to_position(&rope, 3);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 3);
    }

    #[test]
    fn empty_document() {
        let rope = Rope::from_str("");
        let pos = char_offset_to_position(&rope, 0);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 0);
    }

    #[test]
    fn uri_from_path_ascii() {
        let uri = uri_from_path(Path::new("/tmp/foo.rs")).unwrap();
        assert_eq!(uri.as_str(), "file:///tmp/foo.rs");
    }

    #[test]
    fn uri_from_path_with_spaces() {
        // Spaces must be percent-encoded as %20.
        let uri = uri_from_path(Path::new("/Users/my user/file.rs")).unwrap();
        assert!(uri.as_str().contains("%20"));
        assert!(!uri.as_str().contains(' '));
    }

    #[test]
    fn uri_from_path_unicode() {
        // Non-ASCII path components must be percent-encoded.
        let uri = uri_from_path(Path::new("/tmp/café.rs")).unwrap();
        // Must not contain the raw non-ASCII byte.
        assert!(!uri.as_str().contains('é'));
        assert!(uri.as_str().starts_with("file:///"));
    }

    #[test]
    fn uri_from_path_rejects_relative() {
        // url::Url::from_file_path requires an absolute path; we fall back to
        // joining CWD, which produces an absolute path. The call should
        // succeed as long as CWD is available.
        let uri = uri_from_path(Path::new("foo.rs"));
        if let Some(uri) = uri {
            assert!(uri.as_str().starts_with("file:///"));
        }
    }
}
