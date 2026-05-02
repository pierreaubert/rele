//! Rust equivalents of Emacs `casefiddle.c` — character and string
//! case conversion. Buffer-region variants live in `rele-server`
//! since they depend on `DocumentBuffer`.

// ---- Character-level ----

/// `(upcase CHAR-OR-STRING)` for a single character.
pub fn upcase_char(c: char) -> char {
    if c == 'ß' {
        return 'ẞ';
    }
    c.to_uppercase().next().unwrap_or(c)
}

/// `(downcase CHAR-OR-STRING)` for a single character.
pub fn downcase_char(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Emacs character titlecase used by `capitalize` and `upcase-initials`.
pub fn titlecase_char(c: char) -> char {
    match c {
        'ß' => 'ẞ',
        'Ǆ' | 'ǅ' | 'ǆ' => 'ǅ',
        _ => upcase_char(c),
    }
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
                result.push_str(&titlecase_string(c));
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
                result.push_str(&titlecase_string(c));
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

fn titlecase_string(c: char) -> String {
    match c {
        'Ǆ' | 'ǅ' | 'ǆ' => "ǅ".to_string(),
        _ => {
            let upper = c.to_uppercase().collect::<String>();
            let mut chars = upper.chars();
            let mut result = String::new();
            if let Some(first) = chars.next() {
                result.push(first);
            }
            for rest in chars {
                result.extend(rest.to_lowercase());
            }
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upcase_char_basic() {
        assert_eq!(upcase_char('a'), 'A');
        assert_eq!(upcase_char('A'), 'A');
        assert_eq!(upcase_char('1'), '1');
        assert_eq!(upcase_char('ß'), 'ẞ');
    }

    #[test]
    fn downcase_char_basic() {
        assert_eq!(downcase_char('A'), 'a');
        assert_eq!(downcase_char('a'), 'a');
    }

    #[test]
    fn capitalize_string_basic() {
        assert_eq!(capitalize_string("hello world"), "Hello World");
        assert_eq!(capitalize_string("HELLO WORLD"), "Hello World");
        assert_eq!(capitalize_string("ǆungla"), "ǅungla");
    }

    #[test]
    fn upcase_initials_preserves_case() {
        assert_eq!(upcase_initials("hello WORLD"), "Hello WORLD");
        assert_eq!(upcase_initials("ǄUNGLA"), "ǅUNGLA");
    }

    #[test]
    fn titlecase_char_basic() {
        assert_eq!(titlecase_char('a'), 'A');
        assert_eq!(titlecase_char('ß'), 'ẞ');
        assert_eq!(titlecase_char('Ǆ'), 'ǅ');
        assert_eq!(titlecase_char('ǅ'), 'ǅ');
        assert_eq!(titlecase_char('ǆ'), 'ǅ');
    }
}
