//! Rust equivalents of Emacs `search.c` — pure regex-related helpers.
//! Buffer-search variants live in `rele-server` since they depend on
//! `DocumentBuffer` / `EditorCursor`.

/// `(regexp-quote STRING)` — escape regex metacharacters so the
/// resulting pattern matches STRING literally.
pub fn regexp_quote(s: &str) -> String {
    regex::escape(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regexp_quote_escapes_metachars() {
        assert_eq!(regexp_quote("a.b"), "a\\.b");
        assert_eq!(regexp_quote("1+1"), "1\\+1");
        assert_eq!(regexp_quote("[foo]"), "\\[foo\\]");
    }

    #[test]
    fn regexp_quote_passes_plain_text() {
        assert_eq!(regexp_quote("hello"), "hello");
    }
}
