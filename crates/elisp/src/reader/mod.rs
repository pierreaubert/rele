use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use std::collections::HashMap;

/// Look up a Unicode character by its official Unicode name.
/// Returns `None` for names not in the built-in table.
/// This is a best-effort implementation covering the characters that appear
/// most frequently in Emacs Lisp source; unknown names yield `None` so the
/// reader can substitute `\0` and continue rather than hard-erroring.
fn unicode_name_to_char(name: &str) -> Option<char> {
    // Normalise: upper-case and collapse internal runs of whitespace/hyphens
    // to a single space so that both "LATIN SMALL LETTER A" and
    // "latin-small-letter-a" work.
    let key: String = name
        .chars()
        .map(|c| {
            if c == '-' {
                ' '
            } else {
                c.to_ascii_uppercase()
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    // Tiny built-in table of names that appear in Emacs test corpora.
    // Add more entries as needed without touching any other file.
    let ch = match key.as_str() {
        // Latin letters
        "LATIN SMALL LETTER A" => 'a',
        "LATIN SMALL LETTER B" => 'b',
        "LATIN SMALL LETTER C" => 'c',
        "LATIN SMALL LETTER D" => 'd',
        "LATIN SMALL LETTER E" => 'e',
        "LATIN SMALL LETTER F" => 'f',
        "LATIN SMALL LETTER G" => 'g',
        "LATIN SMALL LETTER H" => 'h',
        "LATIN SMALL LETTER I" => 'i',
        "LATIN SMALL LETTER J" => 'j',
        "LATIN SMALL LETTER K" => 'k',
        "LATIN SMALL LETTER L" => 'l',
        "LATIN SMALL LETTER M" => 'm',
        "LATIN SMALL LETTER N" => 'n',
        "LATIN SMALL LETTER O" => 'o',
        "LATIN SMALL LETTER P" => 'p',
        "LATIN SMALL LETTER Q" => 'q',
        "LATIN SMALL LETTER R" => 'r',
        "LATIN SMALL LETTER S" => 's',
        "LATIN SMALL LETTER T" => 't',
        "LATIN SMALL LETTER U" => 'u',
        "LATIN SMALL LETTER V" => 'v',
        "LATIN SMALL LETTER W" => 'w',
        "LATIN SMALL LETTER X" => 'x',
        "LATIN SMALL LETTER Y" => 'y',
        "LATIN SMALL LETTER Z" => 'z',
        "LATIN CAPITAL LETTER A" => 'A',
        "LATIN CAPITAL LETTER B" => 'B',
        "LATIN CAPITAL LETTER C" => 'C',
        "LATIN CAPITAL LETTER D" => 'D',
        "LATIN CAPITAL LETTER E" => 'E',
        "LATIN CAPITAL LETTER F" => 'F',
        "LATIN CAPITAL LETTER G" => 'G',
        "LATIN CAPITAL LETTER H" => 'H',
        "LATIN CAPITAL LETTER I" => 'I',
        "LATIN CAPITAL LETTER J" => 'J',
        "LATIN CAPITAL LETTER K" => 'K',
        "LATIN CAPITAL LETTER L" => 'L',
        "LATIN CAPITAL LETTER M" => 'M',
        "LATIN CAPITAL LETTER N" => 'N',
        "LATIN CAPITAL LETTER O" => 'O',
        "LATIN CAPITAL LETTER P" => 'P',
        "LATIN CAPITAL LETTER Q" => 'Q',
        "LATIN CAPITAL LETTER R" => 'R',
        "LATIN CAPITAL LETTER S" => 'S',
        "LATIN CAPITAL LETTER T" => 'T',
        "LATIN CAPITAL LETTER U" => 'U',
        "LATIN CAPITAL LETTER V" => 'V',
        "LATIN CAPITAL LETTER W" => 'W',
        "LATIN CAPITAL LETTER X" => 'X',
        "LATIN CAPITAL LETTER Y" => 'Y',
        "LATIN CAPITAL LETTER Z" => 'Z',
        // Greek letters (common in mathematical contexts)
        "GREEK SMALL LETTER ALPHA" => '\u{03B1}',
        "GREEK SMALL LETTER BETA" => '\u{03B2}',
        "GREEK SMALL LETTER GAMMA" => '\u{03B3}',
        "GREEK SMALL LETTER DELTA" => '\u{03B4}',
        "GREEK SMALL LETTER EPSILON" => '\u{03B5}',
        "GREEK SMALL LETTER ZETA" => '\u{03B6}',
        "GREEK SMALL LETTER ETA" => '\u{03B7}',
        "GREEK SMALL LETTER THETA" => '\u{03B8}',
        "GREEK SMALL LETTER IOTA" => '\u{03B9}',
        "GREEK SMALL LETTER KAPPA" => '\u{03BA}',
        "GREEK SMALL LETTER LAMDA" => '\u{03BB}',
        "GREEK SMALL LETTER LAMBDA" => '\u{03BB}',
        "GREEK SMALL LETTER MU" => '\u{03BC}',
        "GREEK SMALL LETTER NU" => '\u{03BD}',
        "GREEK SMALL LETTER XI" => '\u{03BE}',
        "GREEK SMALL LETTER OMICRON" => '\u{03BF}',
        "GREEK SMALL LETTER PI" => '\u{03C0}',
        "GREEK SMALL LETTER RHO" => '\u{03C1}',
        "GREEK SMALL LETTER SIGMA" => '\u{03C3}',
        "GREEK SMALL LETTER TAU" => '\u{03C4}',
        "GREEK SMALL LETTER UPSILON" => '\u{03C5}',
        "GREEK SMALL LETTER PHI" => '\u{03C6}',
        "GREEK SMALL LETTER CHI" => '\u{03C7}',
        "GREEK SMALL LETTER PSI" => '\u{03C8}',
        "GREEK SMALL LETTER OMEGA" => '\u{03C9}',
        "GREEK CAPITAL LETTER ALPHA" => '\u{0391}',
        "GREEK CAPITAL LETTER BETA" => '\u{0392}',
        "GREEK CAPITAL LETTER GAMMA" => '\u{0393}',
        "GREEK CAPITAL LETTER DELTA" => '\u{0394}',
        "GREEK CAPITAL LETTER EPSILON" => '\u{0395}',
        "GREEK CAPITAL LETTER ZETA" => '\u{0396}',
        "GREEK CAPITAL LETTER ETA" => '\u{0397}',
        "GREEK CAPITAL LETTER THETA" => '\u{0398}',
        "GREEK CAPITAL LETTER IOTA" => '\u{0399}',
        "GREEK CAPITAL LETTER KAPPA" => '\u{039A}',
        "GREEK CAPITAL LETTER LAMDA" => '\u{039B}',
        "GREEK CAPITAL LETTER LAMBDA" => '\u{039B}',
        "GREEK CAPITAL LETTER MU" => '\u{039C}',
        "GREEK CAPITAL LETTER NU" => '\u{039D}',
        "GREEK CAPITAL LETTER XI" => '\u{039E}',
        "GREEK CAPITAL LETTER OMICRON" => '\u{039F}',
        "GREEK CAPITAL LETTER PI" => '\u{03A0}',
        "GREEK CAPITAL LETTER RHO" => '\u{03A1}',
        "GREEK CAPITAL LETTER SIGMA" => '\u{03A3}',
        "GREEK CAPITAL LETTER TAU" => '\u{03A4}',
        "GREEK CAPITAL LETTER UPSILON" => '\u{03A5}',
        "GREEK CAPITAL LETTER PHI" => '\u{03A6}',
        "GREEK CAPITAL LETTER CHI" => '\u{03A7}',
        "GREEK CAPITAL LETTER PSI" => '\u{03A8}',
        "GREEK CAPITAL LETTER OMEGA" => '\u{03A9}',
        // Common punctuation / symbols
        "NULL" => '\0',
        "SPACE" => ' ',
        "EXCLAMATION MARK" => '!',
        "QUOTATION MARK" => '"',
        "NUMBER SIGN" => '#',
        "DOLLAR SIGN" => '$',
        "PERCENT SIGN" => '%',
        "AMPERSAND" => '&',
        "APOSTROPHE" => '\'',
        "LEFT PARENTHESIS" => '(',
        "RIGHT PARENTHESIS" => ')',
        "ASTERISK" => '*',
        "PLUS SIGN" => '+',
        "COMMA" => ',',
        "HYPHEN MINUS" => '-',
        "HYPHEN-MINUS" => '-',
        "FULL STOP" => '.',
        "SOLIDUS" => '/',
        "COLON" => ':',
        "SEMICOLON" => ';',
        "LESS THAN SIGN" => '<',
        "EQUALS SIGN" => '=',
        "GREATER THAN SIGN" => '>',
        "QUESTION MARK" => '?',
        "COMMERCIAL AT" => '@',
        "LEFT SQUARE BRACKET" => '[',
        "REVERSE SOLIDUS" => '\\',
        "RIGHT SQUARE BRACKET" => ']',
        "CIRCUMFLEX ACCENT" => '^',
        "LOW LINE" => '_',
        "GRAVE ACCENT" => '`',
        "LEFT CURLY BRACKET" => '{',
        "VERTICAL LINE" => '|',
        "RIGHT CURLY BRACKET" => '}',
        "TILDE" => '~',
        "DELETE" => '\x7F',
        // Control characters
        "HORIZONTAL TABULATION" | "CHARACTER TABULATION" => '\t',
        "LINE FEED" | "NEW LINE" | "NEWLINE" => '\n',
        "CARRIAGE RETURN" => '\r',
        "ESCAPE" => '\x1B',
        "FORM FEED" => '\x0C',
        "BACKSPACE" => '\x08',
        "ALERT" | "BELL" => '\x07',
        // Common special characters
        "LATIN SMALL LETTER SHARP S" => '\u{00DF}',
        "LATIN SMALL LETTER AE" => '\u{00E6}',
        "LATIN CAPITAL LETTER AE" => '\u{00C6}',
        "PILCROW SIGN" | "PARAGRAPH SIGN" => '\u{00B6}',
        "SECTION SIGN" => '\u{00A7}',
        "COPYRIGHT SIGN" => '\u{00A9}',
        "REGISTERED SIGN" => '\u{00AE}',
        "TRADE MARK SIGN" => '\u{2122}',
        "DEGREE SIGN" => '\u{00B0}',
        "PLUS MINUS SIGN" => '\u{00B1}',
        "MULTIPLICATION SIGN" => '\u{00D7}',
        "DIVISION SIGN" => '\u{00F7}',
        "MICRO SIGN" => '\u{00B5}',
        "MIDDLE DOT" => '\u{00B7}',
        "BULLET" => '\u{2022}',
        "HORIZONTAL ELLIPSIS" => '\u{2026}',
        "EN DASH" => '\u{2013}',
        "EM DASH" => '\u{2014}',
        "LEFT SINGLE QUOTATION MARK" => '\u{2018}',
        "RIGHT SINGLE QUOTATION MARK" => '\u{2019}',
        "LEFT DOUBLE QUOTATION MARK" => '\u{201C}',
        "RIGHT DOUBLE QUOTATION MARK" => '\u{201D}',
        "SNOWMAN" => '\u{2603}',
        "SNOWFLAKE" => '\u{2745}',
        "BLACK HEART SUIT" => '\u{2665}',
        "WHITE SMILING FACE" => '\u{263A}',
        _ => return None,
    };
    Some(ch)
}

pub struct Reader {
    input: Vec<char>,
    pos: usize,
    /// Shared-structure table for `#N=` / `#N#` notation used in `.elc` files.
    /// `#N=FORM` stores FORM under label N; `#N#` retrieves the stored object.
    shared: HashMap<u64, LispObject>,
}

fn is_symbol_char(c: char) -> bool {
    c.is_alphanumeric()
        || !c.is_ascii() // Allow non-ASCII characters in symbols (e.g. Unicode ellipsis)
        || matches!(
            c,
            '*' | '/'
                | '='
                | '<'
                | '>'
                | '_'
                | '-'
                | '+'
                | '?'
                | '!'
                | '&'
                | ':'
                | '.'
                | '%'
                | '$'
                | '@'
                | '~'
                | '^'
                | '|'
                | '{'
                | '}'
        )
}

fn is_symbol_initial(c: char) -> bool {
    c.is_alphabetic()
        || (!c.is_ascii() && !c.is_whitespace()) // Allow non-ASCII characters in symbols
        || matches!(
            c,
            '*' | '/'
                | '='
                | '<'
                | '>'
                | '_'
                | '-'
                | '+'
                | '?'
                | '!'
                | '&'
                | ':'
                | '%'
                | '$'
                | '@'
                | '~'
                | '^'
                | '|'
                | '{'
                | '}'
        )
}

impl Reader {
    pub fn new(source: &str) -> Self {
        Reader {
            input: source.chars().collect(),
            pos: 0,
            shared: HashMap::new(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn peek_ahead(&self, n: usize) -> Option<char> {
        self.input.get(self.pos + n).copied()
    }

    fn advance(&mut self) -> Option<char> {
        if self.pos < self.input.len() {
            let ch = self.input[self.pos];
            self.pos += 1;
            Some(ch)
        } else {
            None
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else if c == ';' {
                while let Some(c) = self.advance() {
                    if c == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn is_delimiter(c: char) -> bool {
        c.is_whitespace() || c == '(' || c == ')' || c == '"' || c == ';' || c == '\''
    }

    /// Return the current byte-offset into the source (in chars).
    /// Useful for `read-from-string` which needs to report where
    /// the reader stopped.
    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn read(&mut self) -> ElispResult<LispObject> {
        self.skip_whitespace();

        let c = match self.advance() {
            Some(c) => c,
            None => {
                return Err(ElispError::ReaderError(
                    "unexpected end of input".to_string(),
                ));
            }
        };

        match c {
            '(' => self.read_list(),
            ')' => Err(ElispError::ReaderError("unexpected )".to_string())),
            '\'' => {
                let quoted = self.read()?;
                Ok(LispObject::cons(
                    LispObject::symbol("quote"),
                    LispObject::cons(quoted, LispObject::nil()),
                ))
            }
            '`' => {
                let form = self.read()?;
                Ok(LispObject::cons(
                    LispObject::symbol("`"),
                    LispObject::cons(form, LispObject::nil()),
                ))
            }
            ',' => {
                if self.peek() == Some('@') {
                    self.advance();
                    let form = self.read()?;
                    Ok(LispObject::cons(
                        LispObject::symbol(",@"),
                        LispObject::cons(form, LispObject::nil()),
                    ))
                } else {
                    let form = self.read()?;
                    Ok(LispObject::cons(
                        LispObject::symbol(","),
                        LispObject::cons(form, LispObject::nil()),
                    ))
                }
            }
            '#' => self.read_hash(),
            '?' => self.read_char_literal(),
            '"' => self.read_string(),
            '+' | '-' => {
                let next = self.peek();
                if next.map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    self.read_number_from(c)
                } else if next == Some('.')
                    && self
                        .peek_ahead(1)
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false)
                {
                    // -.5 / +.5 style floats
                    self.read_number_from(c)
                } else if next.map(is_symbol_char).unwrap_or(false) {
                    // +abba, -foo, +1plus etc. are symbols; the leading sign is
                    // part of the symbol name when followed by any symbol
                    // constituent (rule-1: don't drop the sign).
                    self.read_symbol(c)
                } else {
                    // Standalone '+' or '-' (operator symbol)
                    let s = String::from(c);
                    Ok(LispObject::symbol(&s))
                }
            }
            c if c.is_ascii_digit() => {
                // Check if this is a number or a symbol starting with digits (e.g. 1value, 1+)
                // Peek ahead to see if the token ends with non-numeric symbol chars
                let saved_pos = self.pos;
                let result = self.read_number_from(c);
                // If the next char is a symbol char (not delimiter/whitespace), it's a symbol
                if let Some(next) = self.peek() {
                    if is_symbol_char(next)
                        && !next.is_ascii_digit()
                        && next != '.'
                        && next != 'e'
                        && next != 'E'
                    {
                        // Rewind and read as symbol
                        self.pos = saved_pos;
                        return self.read_symbol(c);
                    }
                }
                result
            }
            '.' => {
                // Could be a float like .5 or a symbol starting with .
                if self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    self.read_number_from('.')
                } else {
                    self.read_symbol('.')
                }
            }
            '\\' => {
                // Escaped symbol: \` \, \,@ etc.
                // The backslash makes the next character(s) part of a symbol name
                let next = self.advance().ok_or_else(|| {
                    ElispError::ReaderError("unexpected end of input after \\".to_string())
                })?;
                self.read_escaped_symbol(next)
            }
            c if is_symbol_initial(c) => self.read_symbol(c),
            '[' => self.read_vector(),
            _ => Err(ElispError::ReaderError(format!(
                "unexpected character: {}",
                c
            ))),
        }
    }

    fn read_list(&mut self) -> ElispResult<LispObject> {
        self.skip_whitespace();

        if let Some(c) = self.peek() {
            if c == ')' {
                self.advance();
                return Ok(LispObject::nil());
            }
        }

        let car = self.read()?;

        self.skip_whitespace();

        // Check for dotted pair: (a . b)
        if self.peek() == Some('.') {
            // Peek ahead to check it's a dot delimiter, not a symbol or number
            let after_dot = self.peek_ahead(1);
            if after_dot.map(Self::is_delimiter).unwrap_or(true) {
                self.advance(); // consume '.'
                let cdr = self.read()?;
                self.skip_whitespace();
                if self.peek() != Some(')') {
                    return Err(ElispError::ReaderError(
                        "expected ) after dotted pair".to_string(),
                    ));
                }
                self.advance(); // consume ')'
                return Ok(LispObject::cons(car, cdr));
            }
        }

        let cdr = self.read_list()?;
        Ok(LispObject::cons(car, cdr))
    }

    fn read_vector(&mut self) -> ElispResult<LispObject> {
        let mut elements = Vec::new();
        loop {
            self.skip_whitespace();
            if let Some(c) = self.peek() {
                if c == ']' {
                    self.advance();
                    break;
                }
            } else {
                return Err(ElispError::ReaderError(
                    "unterminated vector literal".to_string(),
                ));
            }
            elements.push(self.read()?);
        }
        Ok(LispObject::Vector(std::sync::Arc::new(
            crate::eval::SyncRefCell::new(elements),
        )))
    }

    fn read_hash(&mut self) -> ElispResult<LispObject> {
        let c = self.peek().ok_or_else(|| {
            ElispError::ReaderError("unexpected end of input after #".to_string())
        })?;
        match c {
            '\'' => {
                self.advance();
                let form = self.read()?;
                Ok(LispObject::cons(
                    LispObject::symbol("function"),
                    LispObject::cons(form, LispObject::nil()),
                ))
            }
            'x' | 'X' => {
                self.advance();
                self.read_radix_number(16)
            }
            'o' | 'O' => {
                self.advance();
                self.read_radix_number(8)
            }
            'b' | 'B' => {
                self.advance();
                self.read_radix_number(2)
            }
            '&' => {
                // #&LEN"BITS" — bool-vector literal (Emacs internal .elc
                // format). We don't implement bool-vectors; skip the length
                // digits and the quoted string, return nil.
                self.advance(); // consume '&'
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.advance();
                }
                if self.peek() == Some('"') {
                    self.advance();
                    while let Some(c) = self.advance() {
                        if c == '\\' {
                            self.advance(); // skip escaped char
                        } else if c == '"' {
                            break;
                        }
                    }
                }
                Ok(LispObject::nil())
            }
            '#' => {
                // ## — uninterned empty symbol (also written ||). Emacs's
                // print/read pair for a symbol with no name.
                self.advance(); // consume second '#'
                Ok(LispObject::symbol(""))
            }
            's' => {
                self.advance();
                // #s(TYPE ...) — record/struct literal.
                // #s(hash-table ...) for hash tables, #s(cl-struct-type ...)
                // for cl-defstruct records. Read as a tagged list.
                if self.peek() == Some('(') {
                    self.advance();
                    let inner = self.read_list_to_vec()?;
                    // Distinguish hash-table from struct records
                    let tag = inner
                        .first()
                        .and_then(|o| o.as_symbol().map(|s| s.to_string()));
                    if tag.as_deref() == Some("hash-table") {
                        let mut list = LispObject::nil();
                        for e in inner.into_iter().rev() {
                            list = LispObject::cons(e, list);
                        }
                        Ok(LispObject::cons(
                            LispObject::symbol("hash-table-literal"),
                            list,
                        ))
                    } else {
                        // CL struct record: return as a vector (Emacs records
                        // are vector-like). The first element is the type tag.
                        Ok(LispObject::Vector(std::sync::Arc::new(
                            crate::eval::SyncRefCell::new(inner),
                        )))
                    }
                } else {
                    Err(ElispError::ReaderError("expected ( after #s".to_string()))
                }
            }
            '[' => {
                self.advance();
                self.read_bytecode_literal()
            }
            '@' => {
                // #@NN — skip NN bytes (doc string reference in .elc files)
                self.advance();
                let mut n_str = String::new();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        n_str.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
                let n: usize = n_str.parse().unwrap_or(0);
                // Skip n bytes (characters) including the newline
                for _ in 0..n {
                    self.advance();
                }
                // After skipping, read the next form
                self.read()
            }
            '$' => {
                // #$ — current load file name (used in .elc for lazy docstrings)
                self.advance();
                Ok(LispObject::nil()) // stub: no file context
            }
            '<' => {
                // #<...> — unreadable object notation. In real Emacs this
                // always signals invalid-read-syntax, but we skip to the
                // matching '>' so we can continue parsing the rest of the
                // file (the notation occasionally appears in docstrings
                // or comments that leak into read_all).
                self.advance(); // consume '<'
                let mut depth = 1u32;
                while depth > 0 {
                    match self.advance() {
                        Some('<') => depth += 1,
                        Some('>') => depth -= 1,
                        None => break,
                        _ => {}
                    }
                }
                Ok(LispObject::nil())
            }
            '^' => {
                // #^[...] — char-table literal (e.g. syntax/category tables in .elc).
                // #^^[...] — sub-char-table literal (also in .elc files).
                // We don't model char-tables or sub-char-tables; read the bracketed
                // content as a plain vector in both cases so containing forms parse.
                self.advance(); // consume first '^'
                // Consume optional second '^' for #^^[...]
                if self.peek() == Some('^') {
                    self.advance(); // consume second '^'
                }
                if self.peek() == Some('[') {
                    self.advance(); // consume '['
                    self.read_vector()
                } else {
                    Err(ElispError::ReaderError(
                        "expected [ after #^ or #^^".to_string(),
                    ))
                }
            }
            '(' => {
                // #(STRING PROPS...) — string with text properties.
                // We don't preserve properties; just return the string.
                self.advance(); // consume '('
                let first = self.read()?;
                // Skip the rest of the list
                loop {
                    self.skip_whitespace();
                    if self.peek() == Some(')') {
                        self.advance();
                        break;
                    }
                    if self.peek().is_none() {
                        return Err(ElispError::ReaderError(
                            "unterminated #(...) string-with-properties".to_string(),
                        ));
                    }
                    let _ = self.read()?;
                }
                Ok(first)
            }
            ':' => {
                // #:name — uninterned symbol. We don't track intern status,
                // so just return a regular interned symbol with the same name.
                self.advance(); // consume ':'
                let first = self.advance().ok_or_else(|| {
                    ElispError::ReaderError("unexpected end of input in #: symbol".to_string())
                })?;
                self.read_symbol(first)
            }
            c if c.is_ascii_digit() => {
                // #N= — define shared-structure label N for the following object.
                // #N# — back-reference to the object previously labelled N.
                let mut n: u64 = u64::from(c.to_digit(10).unwrap_or(0));
                self.advance(); // consume first digit
                while let Some(d) = self.peek() {
                    if let Some(v) = d.to_digit(10) {
                        n = n * 10 + u64::from(v);
                        self.advance();
                    } else {
                        break;
                    }
                }
                match self.advance() {
                    Some('=') => {
                        let obj = self.read()?;
                        self.shared.insert(n, obj.clone());
                        Ok(obj)
                    }
                    Some('#') => {
                        // Forward references are valid in circular
                        // structures; substitute nil as a placeholder.
                        Ok(self.shared.get(&n).cloned().unwrap_or(LispObject::nil()))
                    }
                    Some('r') | Some('R') => {
                        // #NrDIGITS — read integer in radix N.
                        self.read_radix_number(n as u32)
                    }
                    Some(other) => Err(ElispError::ReaderError(format!(
                        "expected = or # after #{n}, got {other}"
                    ))),
                    None => Err(ElispError::ReaderError(format!(
                        "unexpected end of input after #{n}"
                    ))),
                }
            }
            '_' => {
                // #_ — read and discard the next sexp (comment syntax).
                self.advance(); // consume '_'
                let _discarded = self.read()?;
                // Return the NEXT form instead — the discarded one is gone.
                self.skip_whitespace();
                self.read()
            }
            _ => Err(ElispError::ReaderError(format!(
                "unknown # dispatch: #{}",
                c
            ))),
        }
    }

    fn read_list_to_vec(&mut self) -> ElispResult<Vec<LispObject>> {
        let mut elements = Vec::new();
        loop {
            self.skip_whitespace();
            if let Some(c) = self.peek() {
                if c == ')' {
                    self.advance();
                    break;
                }
            } else {
                return Err(ElispError::ReaderError("unterminated list".to_string()));
            }
            elements.push(self.read()?);
        }
        Ok(elements)
    }

    fn read_bytecode_literal(&mut self) -> ElispResult<LispObject> {
        use crate::object::BytecodeFunction;

        // 1. Read arglist (an integer)
        self.skip_whitespace();
        let argdesc_obj = self.read()?;
        let argdesc = argdesc_obj.as_integer().ok_or_else(|| {
            ElispError::ReaderError(format!(
                "bytecode arglist must be an integer, got {:?}",
                argdesc_obj
            ))
        })?;

        // 2. Read bytecode string (raw opcodes encoded as string chars)
        self.skip_whitespace();
        let bytecode_obj = self.read()?;
        let bytecode_str = bytecode_obj
            .as_string()
            .ok_or_else(|| ElispError::ReaderError("bytecode must be a string".to_string()))?;
        let bytecode: Vec<u8> = bytecode_str.chars().map(|c| c as u8).collect();

        // 3. Read constants vector (elements until ']')
        self.skip_whitespace();
        let constants = if self.peek() == Some('[') {
            self.advance(); // consume '['
            let mut elems = Vec::new();
            loop {
                self.skip_whitespace();
                match self.peek() {
                    Some(']') => {
                        self.advance();
                        break;
                    }
                    None => {
                        return Err(ElispError::ReaderError(
                            "unterminated constants vector in bytecode literal".to_string(),
                        ));
                    }
                    _ => elems.push(self.read()?),
                }
            }
            elems
        } else {
            // The constants vector may be wrapped in a #N= shared-structure
            // label (producing a LispObject::Vector after read()) or be nil
            // (for functions with no constants).
            let obj = self.read()?;
            match obj {
                LispObject::Nil => Vec::new(),
                LispObject::Vector(v) => v.lock().clone(),
                _ => {
                    return Err(ElispError::ReaderError(format!(
                        "bytecode constants must be a vector or nil, got {:?}",
                        obj
                    )));
                }
            }
        };

        // 4. Read maxdepth (an integer)
        self.skip_whitespace();
        let maxdepth_obj = self.read()?;
        let maxdepth = maxdepth_obj.as_integer().ok_or_else(|| {
            ElispError::ReaderError(format!(
                "bytecode maxdepth must be an integer, got {:?}",
                maxdepth_obj
            ))
        })? as usize;

        // 5. Optionally read docstring and interactive spec, then consume until ']'
        let mut docstring: Option<String> = None;
        let mut interactive: Option<Box<LispObject>> = None;

        self.skip_whitespace();
        if self.peek() != Some(']') {
            let doc_obj = self.read()?;
            if let Some(s) = doc_obj.as_string() {
                docstring = Some(s.clone());
            } else if doc_obj.as_integer().is_some() {
                // Integer docstring reference (file offset) — store as string
                docstring = Some(doc_obj.prin1_to_string());
            }
            // else: ignore non-string, non-integer doc slot

            self.skip_whitespace();
            if self.peek() != Some(']') {
                let inter_obj = self.read()?;
                if !inter_obj.is_nil() {
                    interactive = Some(Box::new(inter_obj));
                }
            }
        }

        // 6. Discard any remaining elements until ']'
        loop {
            self.skip_whitespace();
            match self.peek() {
                Some(']') => {
                    self.advance();
                    break;
                }
                None => {
                    return Err(ElispError::ReaderError(
                        "unterminated bytecode literal".to_string(),
                    ));
                }
                _ => {
                    self.read()?; // discard
                }
            }
        }

        Ok(LispObject::BytecodeFn(BytecodeFunction {
            argdesc,
            bytecode,
            constants,
            maxdepth,
            docstring,
            interactive,
        }))
    }

    fn read_char_literal(&mut self) -> ElispResult<LispObject> {
        let c = self.advance().ok_or_else(|| {
            ElispError::ReaderError("unexpected end of input in char literal".to_string())
        })?;
        if c == '\\' {
            // Escape sequence
            let esc = self.advance().ok_or_else(|| {
                ElispError::ReaderError("unexpected end of input in char escape".to_string())
            })?;
            let ch = match esc {
                'M' => {
                    // Meta modifier: \M-X adds 0x8000000
                    self.advance(); // skip '-'
                    let inner = self.read_char_literal()?;
                    let val = inner.as_integer().unwrap_or(0);
                    return Ok(LispObject::Integer(val | 0x8000000));
                }
                'C' | '^' => {
                    // Control modifier: \C-X or \^X
                    if esc == 'C' {
                        self.advance(); // skip '-'
                    }
                    let inner = self.read_char_literal()?;
                    let val = inner.as_integer().unwrap_or(0);
                    return Ok(LispObject::Integer(val & 0x1F));
                }
                'S' => {
                    // Shift modifier: \S-X adds 0x2000000
                    self.advance(); // skip '-'
                    let inner = self.read_char_literal()?;
                    let val = inner.as_integer().unwrap_or(0);
                    return Ok(LispObject::Integer(val | 0x2000000));
                }
                'A' => {
                    // Alt modifier: \A-X adds 0x400000
                    self.advance(); // skip '-'
                    let inner = self.read_char_literal()?;
                    let val = inner.as_integer().unwrap_or(0);
                    return Ok(LispObject::Integer(val | 0x400000));
                }
                'H' => {
                    // Hyper modifier: \H-X adds 0x1000000
                    self.advance(); // skip '-'
                    let inner = self.read_char_literal()?;
                    let val = inner.as_integer().unwrap_or(0);
                    return Ok(LispObject::Integer(val | 0x1000000));
                }
                's' => {
                    // Super modifier when followed by '-': \s-X adds 0x800000
                    // Otherwise: space character
                    if self.peek() == Some('-') {
                        self.advance(); // skip '-'
                        let inner = self.read_char_literal()?;
                        let val = inner.as_integer().unwrap_or(0);
                        return Ok(LispObject::Integer(val | 0x800000));
                    }
                    ' ' // space
                }
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                'a' => '\x07', // bell
                'b' => '\x08', // backspace
                'f' => '\x0C', // form feed
                'e' => '\x1B', // escape
                'd' => '\x7F', // delete
                '\\' => '\\',
                '\'' => '\'',
                '"' => '"',
                '(' => '(',
                ')' => ')',
                '[' => '[',
                ']' => ']',
                'x' => {
                    // Hex character: ?\xNN
                    let mut hex = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_hexdigit() {
                            hex.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let code = u32::from_str_radix(&hex, 16).map_err(|_| {
                        ElispError::ReaderError(format!("invalid hex char: \\x{}", hex))
                    })?;
                    char::from_u32(code).ok_or_else(|| {
                        ElispError::ReaderError(format!("invalid unicode: \\x{}", hex))
                    })?
                }
                'u' | 'U' => {
                    // Unicode character: \uNNNN or \U00NNNNNN
                    let mut hex = String::new();
                    let limit = if esc == 'u' { 4 } else { 8 };
                    for _ in 0..limit {
                        match self.peek() {
                            Some(c) if c.is_ascii_hexdigit() => {
                                hex.push(c);
                                self.advance();
                            }
                            _ => break,
                        }
                    }
                    let code = u32::from_str_radix(&hex, 16).unwrap_or(0);
                    char::from_u32(code).unwrap_or('\0')
                }
                'N' => {
                    // Named Unicode character: ?\N{UNICODE CHARACTER NAME}
                    // Consumes the {NAME} block and looks up the character.
                    // Unknown names yield U+0000 so parsing can continue.
                    if self.peek() == Some('{') {
                        self.advance(); // consume '{'
                        let mut name = String::new();
                        loop {
                            match self.advance() {
                                Some('}') => break,
                                Some(c) => name.push(c),
                                None => {
                                    return Err(ElispError::ReaderError(
                                        "unterminated \\N{} unicode name".to_string(),
                                    ));
                                }
                            }
                        }
                        unicode_name_to_char(&name).unwrap_or('\0')
                    } else {
                        'N' // ?\N without { is just the character N
                    }
                }
                '0'..='7' => {
                    // Octal character: \NNN
                    let mut oct = String::new();
                    oct.push(esc);
                    while let Some(c) = self.peek() {
                        if ('0'..='7').contains(&c) {
                            oct.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let code = i64::from_str_radix(&oct, 8).unwrap_or(0);
                    return Ok(LispObject::Integer(code));
                }
                c => c, // ?\X for any other X is just X
            };
            Ok(LispObject::Integer(ch as i64))
        } else {
            // Plain character: ?a
            Ok(LispObject::Integer(c as i64))
        }
    }

    fn read_string(&mut self) -> ElispResult<LispObject> {
        let mut s = String::new();
        let mut escaped = false;

        while let Some(c) = self.advance() {
            if escaped {
                match c {
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    'r' => s.push('\r'),
                    'a' => s.push('\x07'),
                    'b' => s.push('\x08'),
                    'f' => s.push('\x0C'),
                    'e' => s.push('\x1B'),
                    '"' => s.push('"'),
                    '\\' => s.push('\\'),
                    '\n' => {} // backslash-newline: skip both
                    'x' => {
                        // Hex escape: \xNN
                        let mut hex = String::new();
                        while let Some(h) = self.peek() {
                            if h.is_ascii_hexdigit() {
                                hex.push(h);
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        if hex.is_empty() {
                            s.push('\\');
                            s.push('x');
                        } else {
                            let code = u32::from_str_radix(&hex, 16).map_err(|_| {
                                ElispError::ReaderError(format!("invalid hex escape: \\x{}", hex))
                            })?;
                            // Emacs allows hex escapes > U+10FFFF (they
                            // map to raw bytes in unibyte strings). We
                            // substitute the replacement character for
                            // out-of-range values rather than erroring.
                            let ch = char::from_u32(code).unwrap_or('\u{FFFD}');
                            s.push(ch);
                        }
                    }
                    c if c.is_ascii_digit() && c < '8' => {
                        // Octal escape: \NNN (up to 3 octal digits)
                        let mut oct = String::new();
                        oct.push(c);
                        for _ in 0..2 {
                            if let Some(d) = self.peek() {
                                if d.is_ascii_digit() && d < '8' {
                                    oct.push(d);
                                    self.advance();
                                } else {
                                    break;
                                }
                            }
                        }
                        let code = u32::from_str_radix(&oct, 8).unwrap_or(0);
                        if let Some(ch) = char::from_u32(code) {
                            s.push(ch);
                        }
                    }
                    _ => {
                        s.push('\\');
                        s.push(c);
                    }
                }
                escaped = false;
                continue;
            }
            if c == '\\' {
                escaped = true;
                continue;
            }
            if c == '"' {
                return Ok(LispObject::string(&s));
            }
            s.push(c);
        }

        Err(ElispError::ReaderError("unterminated string".to_string()))
    }

    fn read_number_from(&mut self, first: char) -> ElispResult<LispObject> {
        let mut s = String::new();
        s.push(first);
        let mut has_dot = first == '.';
        let mut has_exp = false;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else if c == '.' && !has_dot && !has_exp {
                // Check it's a decimal dot, not a dotted-pair dot
                let next = self.peek_ahead(1);
                if next
                    .map(|c| c.is_ascii_digit() || c == 'e' || c == 'E')
                    .unwrap_or(false)
                {
                    has_dot = true;
                    s.push(c);
                    self.advance();
                } else {
                    // Trailing dot: 1. means float 1.0
                    has_dot = true;
                    s.push(c);
                    self.advance();
                    break;
                }
            } else if (c == 'e' || c == 'E') && !has_exp {
                has_exp = true;
                has_dot = true; // exponent makes it a float
                s.push(c);
                self.advance();
                // Optional sign after exponent
                if let Some(sign) = self.peek() {
                    if sign == '+' || sign == '-' {
                        s.push(sign);
                        self.advance();
                    }
                }
                // Emacs accepts INF / NaN as exponent mantissas, e.g. 1.0e+INF.
                // Rust's f64::parse doesn't, so detect and substitute.
                let rest_is_inf = self.peek().map(|c| matches!(c, 'I' | 'i')).unwrap_or(false);
                let rest_is_nan = self.peek().map(|c| matches!(c, 'N' | 'n')).unwrap_or(false);
                if rest_is_inf || rest_is_nan {
                    // Consume the 3 letters (INF or NaN).
                    for _ in 0..3 {
                        if let Some(ch) = self.peek() {
                            if ch.is_ascii_alphabetic() {
                                self.advance();
                            }
                        }
                    }
                    let sign_is_negative = s.trim_start().starts_with('-');
                    return Ok(LispObject::float(if rest_is_inf {
                        if sign_is_negative {
                            f64::NEG_INFINITY
                        } else {
                            f64::INFINITY
                        }
                    } else {
                        f64::NAN
                    }));
                }
            } else {
                break;
            }
        }

        if has_dot || has_exp {
            // Ensure exponent has at least one digit (Rust requires it;
            // Emacs reads "0E" as 0.0).
            if has_exp && s.ends_with(|c: char| c == 'e' || c == 'E' || c == '+' || c == '-') {
                s.push('0');
            }
            let f: f64 = s
                .parse()
                .map_err(|_| ElispError::ReaderError(format!("invalid float: {}", s)))?;
            Ok(LispObject::float(f))
        } else {
            match s.parse::<i64>() {
                Ok(n) => Ok(LispObject::integer(n)),
                Err(_) => s
                    .parse::<num_bigint::BigInt>()
                    .map(LispObject::BigInt)
                    .map_err(|_| ElispError::ReaderError(format!("invalid integer: {}", s))),
            }
        }
    }

    fn read_radix_number(&mut self, radix: u32) -> ElispResult<LispObject> {
        let mut s = String::new();
        let mut has_sign = false;
        if let Some(c) = self.peek() {
            if c == '+' || c == '-' {
                has_sign = true;
                s.push(c);
                self.advance();
            }
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        let digits = if has_sign { &s[1..] } else { &s };
        if digits.is_empty() {
            return Err(ElispError::ReaderError(format!(
                "invalid radix-{} number: #{}{}",
                radix,
                match radix {
                    16 => "x",
                    8 => "o",
                    2 => "b",
                    _ => "?",
                },
                s
            )));
        }
        let Some(mut n) = num_bigint::BigInt::parse_bytes(digits.as_bytes(), radix) else {
            return Err(ElispError::ReaderError(format!(
                "invalid radix-{} number: {}",
                radix, s
            )));
        };
        if has_sign && s.starts_with('-') {
            n = -n;
        }
        if let Ok(small) = n.to_string().parse::<i64>() {
            Ok(LispObject::integer(small))
        } else {
            Ok(LispObject::BigInt(n))
        }
    }

    fn read_escaped_symbol(&mut self, first_escaped: char) -> ElispResult<LispObject> {
        // First char was already escaped by \, so it's always literal
        let mut s = String::new();
        s.push(first_escaped);

        while let Some(c) = self.peek() {
            if c == '\\' {
                self.advance();
                if let Some(escaped) = self.advance() {
                    s.push(escaped);
                }
            } else if is_symbol_char(c) {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        // Escaped symbols are never nil or t
        Ok(LispObject::symbol(&s))
    }

    fn read_symbol(&mut self, first: char) -> ElispResult<LispObject> {
        let mut s = String::new();
        s.push(first);
        let mut had_escape = false;

        while let Some(c) = self.peek() {
            if c == '\\' {
                had_escape = true;
                self.advance();
                if let Some(escaped) = self.advance() {
                    s.push(escaped);
                }
            } else if is_symbol_char(c) {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        if had_escape {
            // Escaped symbols are never interned as nil/t
            Ok(LispObject::symbol(&s))
        } else {
            match s.as_str() {
                "nil" => Ok(LispObject::nil()),
                "t" => Ok(LispObject::t()),
                _ => Ok(LispObject::symbol(&s)),
            }
        }
    }

    pub fn read_all(&mut self) -> ElispResult<Vec<LispObject>> {
        let mut result = Vec::new();
        self.skip_whitespace();
        while self.pos < self.input.len() {
            result.push(self.read()?);
            self.skip_whitespace();
        }
        Ok(result)
    }
}

pub fn read(source: &str) -> ElispResult<LispObject> {
    let mut reader = Reader::new(source);
    reader.read()
}

pub fn read_all(source: &str) -> ElispResult<Vec<LispObject>> {
    let mut reader = Reader::new(source);
    reader.read_all()
}

/// Check the first line of an Emacs Lisp source file for `lexical-binding: t`.
/// Returns `true` when the file-local variable annotation is present, e.g.:
/// `;;; -*- lexical-binding: t; -*-`
pub fn detect_lexical_binding(source: &str) -> bool {
    if let Some(first_line) = source.lines().next() {
        first_line.contains("lexical-binding: t")
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_nil() {
        assert_eq!(read("nil").unwrap(), LispObject::nil());
    }

    #[test]
    fn test_read_empty_list() {
        assert_eq!(read("()").unwrap(), LispObject::nil());
    }

    #[test]
    fn test_read_t() {
        assert_eq!(read("t").unwrap(), LispObject::t());
    }

    #[test]
    fn test_read_integer() {
        assert_eq!(read("42").unwrap(), LispObject::integer(42));
        assert_eq!(read("-10").unwrap(), LispObject::integer(-10));
        assert_eq!(read("+5").unwrap(), LispObject::integer(5));
    }

    #[test]
    fn test_read_float() {
        assert_eq!(read("3.14").unwrap(), LispObject::float(3.14));
        assert_eq!(read("-1.5").unwrap(), LispObject::float(-1.5));
        assert_eq!(read("1e10").unwrap(), LispObject::float(1e10));
        assert_eq!(read("1.5e-3").unwrap(), LispObject::float(1.5e-3));
        assert_eq!(read("2.0").unwrap(), LispObject::float(2.0));
        assert_eq!(read(".5").unwrap(), LispObject::float(0.5));
        // Trailing dot: 1. is float 1.0
        assert_eq!(read("1.").unwrap(), LispObject::float(1.0));
    }

    #[test]
    fn test_read_symbol() {
        assert_eq!(read("foo").unwrap(), LispObject::symbol("foo"));
        assert_eq!(read("bar-baz").unwrap(), LispObject::symbol("bar-baz"));
        assert_eq!(read(":keyword").unwrap(), LispObject::symbol(":keyword"));
        assert_eq!(read("&rest").unwrap(), LispObject::symbol("&rest"));
        assert_eq!(read("&optional").unwrap(), LispObject::symbol("&optional"));
    }

    #[test]
    fn test_read_string() {
        assert_eq!(read("\"hello\"").unwrap(), LispObject::string("hello"));
        assert_eq!(
            read("\"say \\\"hi\\\"\"").unwrap(),
            LispObject::string("say \"hi\"")
        );
    }

    #[test]
    fn test_read_quote() {
        let result = read("'foo").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol("quote"),
            LispObject::cons(LispObject::symbol("foo"), LispObject::nil()),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_read_backquote() {
        let result = read("`foo").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol("`"),
            LispObject::cons(LispObject::symbol("foo"), LispObject::nil()),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_read_unquote() {
        let result = read(",foo").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol(","),
            LispObject::cons(LispObject::symbol("foo"), LispObject::nil()),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_read_unquote_splice() {
        let result = read(",@foo").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol(",@"),
            LispObject::cons(LispObject::symbol("foo"), LispObject::nil()),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_read_function_shorthand() {
        let result = read("#'foo").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol("function"),
            LispObject::cons(LispObject::symbol("foo"), LispObject::nil()),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_read_char_literal() {
        assert_eq!(read("?a").unwrap(), LispObject::integer(97));
        assert_eq!(read("?A").unwrap(), LispObject::integer(65));
        assert_eq!(read("?\\n").unwrap(), LispObject::integer(10));
        assert_eq!(read("?\\t").unwrap(), LispObject::integer(9));
        assert_eq!(read("?\\\n").unwrap(), LispObject::integer(10));
        assert_eq!(read("?\\x41").unwrap(), LispObject::integer(65));
        assert_eq!(read("? ").unwrap(), LispObject::integer(32));
    }

    #[test]
    fn test_read_radix() {
        assert_eq!(read("#xff").unwrap(), LispObject::integer(255));
        assert_eq!(read("#o77").unwrap(), LispObject::integer(63));
        assert_eq!(read("#b1010").unwrap(), LispObject::integer(10));
        assert_eq!(read("#xFF").unwrap(), LispObject::integer(255));
    }

    #[test]
    fn test_read_dotted_pair() {
        let result = read("(a . b)").unwrap();
        assert_eq!(
            result,
            LispObject::cons(LispObject::symbol("a"), LispObject::symbol("b"))
        );
    }

    #[test]
    fn test_read_dotted_pair_numbers() {
        let result = read("(1 . 2)").unwrap();
        assert_eq!(
            result,
            LispObject::cons(LispObject::integer(1), LispObject::integer(2))
        );
    }

    #[test]
    fn test_read_list() {
        let result = read("(a b c)").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol("a"),
            LispObject::cons(
                LispObject::symbol("b"),
                LispObject::cons(LispObject::symbol("c"), LispObject::nil()),
            ),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_read_multiple() {
        let result = read_all("nil t 42").unwrap();
        assert_eq!(
            result,
            vec![LispObject::nil(), LispObject::t(), LispObject::integer(42),]
        );
    }

    #[test]
    fn test_reader_quote_detail() {
        let r = read("'(2 3)").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol("quote"),
            LispObject::cons(
                LispObject::cons(
                    LispObject::integer(2),
                    LispObject::cons(LispObject::integer(3), LispObject::nil()),
                ),
                LispObject::nil(),
            ),
        );
        assert_eq!(r, expected);
    }

    #[test]
    fn test_read_comments() {
        assert_eq!(
            read("42 ; this is a comment").unwrap(),
            LispObject::integer(42)
        );
        assert_eq!(
            read_all("; comment\n42").unwrap(),
            vec![LispObject::integer(42)]
        );
        assert_eq!(
            read_all("; first\n; second\n42").unwrap(),
            vec![LispObject::integer(42)]
        );
        assert_eq!(
            read("(a ; comment\n b)").unwrap(),
            LispObject::cons(
                LispObject::symbol("a"),
                LispObject::cons(LispObject::symbol("b"), LispObject::nil()),
            )
        );
        assert!(read_all("; just a comment").unwrap().is_empty());
    }

    #[test]
    fn test_read_nested_backquote() {
        // `(a ,b ,@c)
        let result = read("`(a ,b ,@c)").unwrap();
        // Should be (\` (a (\, b) (\,@ c)))
        assert!(result.is_cons());
        assert_eq!(result.first().unwrap(), LispObject::symbol("`"));
    }

    #[test]
    fn test_read_vector_literal() {
        let result = read("[1 2 3]").unwrap();
        assert!(matches!(result, LispObject::Vector(_)));
        if let LispObject::Vector(v) = &result {
            assert_eq!(v.lock().len(), 3);
        }
    }

    #[test]
    fn test_read_real_elisp() {
        let source = r#"
;; Test file for reader
(defun my-func (x y &optional z)
  "A docstring."
  (let ((a (+ x y))
        (b (* x 2)))
    (if (> a b)
        (message "a > b: %d" a)
      (message "b >= a: %d" b))))

;; Backquote usage
(defmacro my-when (cond &rest body)
  `(if ,cond (progn ,@body)))

;; Character literals
(defvar my-char ?a)
(defvar my-newline ?\n)

;; Dotted pairs
(setq my-alist '((a . 1) (b . 2) (c . 3)))

;; Hex numbers
(setq my-hex #xff)

;; Float
(setq my-float 3.14)
(setq my-sci 1.5e-3)

;; Function reference
(mapcar #'1+ '(1 2 3))
"#;
        let forms = read_all(source).unwrap();
        assert_eq!(forms.len(), 9); // defun, defmacro, 2x defvar, 3x setq, mapcar
    }

    #[test]
    fn test_read_bytecode_literal() {
        // Simple: #[257 "\x54\x87" [] 2]
        let result = read("#[257 \"\\x54\\x87\" [] 2]").unwrap();
        assert!(matches!(result, LispObject::BytecodeFn(_)));
        if let LispObject::BytecodeFn(bc) = result {
            assert_eq!(bc.argdesc, 257);
            assert_eq!(bc.bytecode, vec![0x54, 0x87]);
            assert_eq!(bc.constants.len(), 0);
            assert_eq!(bc.maxdepth, 2);
            assert!(bc.docstring.is_none());
            assert!(bc.interactive.is_none());
        }
    }

    #[test]
    fn test_read_bytecode_with_constants() {
        // #[513 "\x01\x02" [foo bar] 4]
        let result = read("#[513 \"\\x01\\x02\" [foo bar] 4]").unwrap();
        if let LispObject::BytecodeFn(bc) = result {
            assert_eq!(bc.argdesc, 513);
            assert_eq!(bc.bytecode, vec![0x01, 0x02]);
            assert_eq!(bc.constants.len(), 2);
            assert_eq!(bc.constants[0], LispObject::symbol("foo"));
            assert_eq!(bc.constants[1], LispObject::symbol("bar"));
            assert_eq!(bc.maxdepth, 4);
        } else {
            panic!("expected BytecodeFn");
        }
    }

    #[test]
    fn test_read_bytecode_with_docstring() {
        let result = read("#[257 \"\\x54\" [] 2 \"A docstring.\"]").unwrap();
        if let LispObject::BytecodeFn(bc) = result {
            assert_eq!(bc.argdesc, 257);
            assert_eq!(bc.maxdepth, 2);
            assert_eq!(bc.docstring, Some("A docstring.".to_string()));
            assert!(bc.interactive.is_none());
        } else {
            panic!("expected BytecodeFn");
        }
    }

    #[test]
    fn test_read_bytecode_with_interactive() {
        let result = read("#[257 \"\\x54\" [] 2 \"doc\" (interactive \"p\")]").unwrap();
        if let LispObject::BytecodeFn(bc) = result {
            assert_eq!(bc.docstring, Some("doc".to_string()));
            assert!(bc.interactive.is_some());
        } else {
            panic!("expected BytecodeFn");
        }
    }

    #[test]
    fn test_read_bytecode_nil_constants() {
        // Some bytecode uses nil instead of [] for empty constants
        let result = read("#[0 \"\" nil 0]").unwrap();
        if let LispObject::BytecodeFn(bc) = result {
            assert_eq!(bc.argdesc, 0);
            assert!(bc.bytecode.is_empty());
            assert!(bc.constants.is_empty());
            assert_eq!(bc.maxdepth, 0);
        } else {
            panic!("expected BytecodeFn");
        }
    }

    #[test]
    fn test_read_string_hex_escape() {
        let result = read("\"\\x41\\x42\"").unwrap();
        assert_eq!(result, LispObject::string("AB"));
    }

    #[test]
    fn test_parse_debug_early_el() {
        let source = std::fs::read_to_string(format!(
            "{}/emacs-lisp/debug-early.el",
            crate::eval::bootstrap::STDLIB_DIR
        ));
        if let Ok(source) = source {
            let forms = read_all(&source).expect("failed to parse debug-early.el");
            assert!(
                forms.len() >= 5,
                "expected at least 5 forms, got {}",
                forms.len()
            );
        }
    }

    #[test]
    fn test_parse_byte_run_el() {
        let source = std::fs::read_to_string(format!(
            "{}/emacs-lisp/byte-run.el",
            crate::eval::bootstrap::STDLIB_DIR
        ));
        if let Ok(source) = source {
            let forms = read_all(&source).expect("failed to parse byte-run.el");
            assert!(
                forms.len() >= 10,
                "expected at least 10 forms, got {}",
                forms.len()
            );
        }
    }

    #[test]
    fn test_parse_backquote_el() {
        let source = std::fs::read_to_string(format!(
            "{}/emacs-lisp/backquote.el",
            crate::eval::bootstrap::STDLIB_DIR
        ));
        if let Ok(source) = source {
            let forms = read_all(&source).expect("failed to parse backquote.el");
            assert!(
                forms.len() >= 5,
                "expected at least 5 forms, got {}",
                forms.len()
            );
        }
    }

    #[test]
    fn test_parse_subr_el() {
        let source =
            std::fs::read_to_string(format!("{}/subr.el", crate::eval::bootstrap::STDLIB_DIR));
        if let Ok(source) = source {
            let forms = read_all(&source).expect("failed to parse subr.el");
            assert!(
                forms.len() >= 100,
                "expected at least 100 forms, got {}",
                forms.len()
            );
        }
    }
}
