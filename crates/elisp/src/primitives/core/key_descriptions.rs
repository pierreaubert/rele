//! `text-char-description`, `single-key-description`, `listify-key-sequence`.
//! Kept out of `stubs.rs` so the inventory script doesn't scan their
//! internal `match` arms as stub records.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

/// `text-char-description CHAR` — caret form for control chars
/// (`^A`, `^I`, `^?`), the char itself for printables, empty string
/// for eight-bit raw bytes (128..=255). Signals `wrong-type-argument`
/// for non-character inputs (strings, vectors, modifier-bit chars).
pub fn text_char_description(args: &LispObject) -> ElispResult<LispObject> {
    let value = args.first().unwrap_or_else(LispObject::nil);
    let Some(n) = value.as_integer() else {
        return Err(ElispError::WrongTypeArgument("characterp".into()));
    };
    if n < 0 {
        return Err(ElispError::WrongTypeArgument("characterp".into()));
    }
    // Emacs encodes modifier bits in the high bits of the character
    // integer (control 0x0400_0000, meta 0x0800_0000, super 0x0010_0000,
    // etc.). text-char-description rejects any char with non-ASCII
    // modifier bits set — anything ≥ 0x40_0000 is out of bounds.
    if n >= 0x0040_0000 {
        return Err(ElispError::WrongTypeArgument("characterp".into()));
    }
    let n = n as u32;
    let s = match n {
        0..=31 => format!("^{}", char::from_u32(n + 64).unwrap_or('?')),
        127 => "^?".to_string(),
        // C1 control range — Emacs returns the empty unibyte string;
        // these don't have a printable representation.
        128..=159 => return Ok(LispObject::string("")),
        // Latin-1 supplement and above — printable as the char itself.
        _ => char::from_u32(n).map(|c| c.to_string()).unwrap_or_default(),
    };
    Ok(LispObject::string(&s))
}

/// `single-key-description KEY &optional NO-ANGLES` — describe one
/// keyboard event using Emacs's `C-a`, `M-x`, `<left>` format.
pub fn single_key_description(args: &LispObject) -> LispObject {
    let Some(key) = args.first() else {
        return LispObject::string("");
    };
    let no_angles = args.nth(1).is_some_and(|v| !v.is_nil());
    if let Some(sym) = key.as_symbol() {
        let name = sym.to_string();
        return if no_angles {
            LispObject::string(&name)
        } else {
            LispObject::string(&format!("<{name}>"))
        };
    }
    let Some(n) = key.as_integer() else {
        return LispObject::string("");
    };
    let n = n as u32;
    let modifiers_meta = (n & 0x4000_0000) != 0;
    let modifiers_ctrl = (n & 0x0400_0000) != 0;
    let base = n & 0x003F_FFFF;
    let mut out = String::new();
    if modifiers_meta {
        out.push_str("M-");
    }
    let base_desc = match base {
        9 => "TAB".into(),
        10 => "LFD".into(),
        13 => "RET".into(),
        27 => "ESC".into(),
        32 => "SPC".into(),
        127 => "DEL".into(),
        0..=31 if modifiers_ctrl => format!("C-{}", char::from_u32(base + 96).unwrap_or('?')),
        0..=31 => format!("C-{}", char::from_u32(base + 96).unwrap_or('?')),
        _ => char::from_u32(base).map(|c| c.to_string()).unwrap_or_default(),
    };
    out.push_str(&base_desc);
    LispObject::string(&out)
}

/// `(listify-key-sequence KEYSEQ)` — strings yield character codes,
/// vectors yield their elements.
pub fn listify_key_sequence(args: &LispObject) -> LispObject {
    let Some(seq) = args.first() else {
        return LispObject::nil();
    };
    if let Some(s) = seq.as_string() {
        let chars: Vec<LispObject> = s.chars().map(|c| LispObject::integer(c as i64)).collect();
        return chars
            .into_iter()
            .rev()
            .fold(LispObject::nil(), |tail, item| LispObject::cons(item, tail));
    }
    if let LispObject::Vector(items) = &seq {
        let cloned: Vec<LispObject> = items.lock().iter().cloned().collect();
        return cloned
            .into_iter()
            .rev()
            .fold(LispObject::nil(), |tail, item| LispObject::cons(item, tail));
    }
    seq
}

/// `(split-char CHAR)` — decompose a character into `(charset code...)`.
/// We only model ASCII vs unicode-the-rest, since the headless build
/// has no charset metadata beyond that.
pub fn split_char(args: &LispObject) -> LispObject {
    let Some(ch) = args.first().and_then(|a| a.as_integer()) else {
        return LispObject::nil();
    };
    let charset = if (0..=127).contains(&ch) {
        LispObject::symbol("ascii")
    } else if (128..=255).contains(&ch) {
        LispObject::symbol("eight-bit")
    } else {
        LispObject::symbol("unicode")
    };
    LispObject::cons(
        charset,
        LispObject::cons(LispObject::integer(ch), LispObject::nil()),
    )
}

/// Standard Emacs `secure-hash-algorithms` list, in priority order.
pub fn secure_hash_algorithms() -> LispObject {
    let names = ["md5", "sha1", "sha224", "sha256", "sha384", "sha512"];
    names
        .iter()
        .rev()
        .fold(LispObject::nil(), |tail, name| {
            LispObject::cons(LispObject::symbol(name), tail)
        })
}
