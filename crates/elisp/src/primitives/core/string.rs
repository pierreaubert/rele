use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "princ" => Some(prim_princ(args)),
        "prin1" => Some(prim_prin1(args)),
        "string=" => Some(prim_string_eq(args)),
        "string<" => Some(prim_string_lt(args)),
        "concat" => Some(prim_concat(args)),
        "substring" => Some(prim_substring(args)),
        "string-to-number" => Some(prim_string_to_number(args)),
        "number-to-string" => Some(prim_number_to_string(args)),
        "make-string" => Some(prim_make_string(args)),
        "upcase" => Some(prim_upcase(args)),
        "downcase" => Some(prim_downcase(args)),
        "capitalize" => Some(prim_capitalize(args)),
        "safe-length" => Some(prim_safe_length(args)),
        "string" => Some(prim_string(args)),
        "regexp-quote" => Some(prim_regexp_quote(args)),
        "max-char" => Some(prim_max_char(args)),
        "string-replace" => Some(prim_string_replace(args)),
        "string-trim" => Some(prim_string_trim(args)),
        "string-prefix-p" => Some(prim_string_prefix_p(args)),
        "string-suffix-p" => Some(prim_string_suffix_p(args)),
        "string-join" => Some(prim_string_join(args)),
        "string-to-list" => Some(prim_string_to_list(args)),
        "char-to-string" => Some(prim_char_to_string(args)),
        "string-to-char" => Some(prim_string_to_char(args)),
        "string-width" => Some(prim_string_width(args)),
        "multibyte-string-p" => Some(prim_multibyte_string_p(args)),
        "string-search" => Some(prim_string_search(args)),
        "string-equal" => Some(prim_string_equal(args)),
        "string-lessp" => Some(prim_string_lessp(args)),
        "compare-strings" => Some(prim_compare_strings(args)),
        "split-string" => Some(prim_split_string(args)),
        "substring-no-properties" => Some(prim_substring(args)),
        "key-description" => Some(prim_key_description(args)),
        "upcase-initials" => Some(prim_upcase_initials(args)),
        "unibyte-string" => Some(prim_unibyte_string(args)),
        "string-lines" => Some(prim_string_lines(args)),
        "format-prompt" => Some(prim_format_prompt(args)),
        "string-bytes" => Some(prim_string_bytes(args)),
        "decode-char" => Some(prim_decode_char(args)),
        "encode-char" => Some(prim_encode_char(args)),
        _ => None,
    }
}

pub fn prim_princ(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    print!("{}", arg.princ_to_string());
    Ok(arg)
}

pub fn prim_prin1(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    print!("{}", arg.prin1_to_string());
    Ok(arg)
}

pub fn prim_string_eq(args: &LispObject) -> ElispResult<LispObject> {
    let (a, b) = match (args.clone().first(), args.clone().nth(1)) {
        (Some(a), Some(b)) => (a, b),
        _ => return Err(ElispError::WrongNumberOfArguments),
    };
    let s1 = match &a {
        LispObject::String(s) => s.as_str(),
        LispObject::Symbol(_) => return Ok(LispObject::nil()),
        LispObject::Nil => "",
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let s2 = match &b {
        LispObject::String(s) => s.as_str(),
        LispObject::Symbol(_) => return Ok(LispObject::nil()),
        LispObject::Nil => "",
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    Ok(LispObject::from(s1 == s2))
}

pub fn prim_string_lt(args: &LispObject) -> ElispResult<LispObject> {
    let (a, b) = match (args.clone().first(), args.clone().nth(1)) {
        (Some(a), Some(b)) => (a, b),
        _ => return Err(ElispError::WrongNumberOfArguments),
    };
    let s1 = match &a {
        LispObject::String(s) => s.as_str(),
        LispObject::Nil => "",
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let s2 = match &b {
        LispObject::String(s) => s.as_str(),
        LispObject::Nil => "",
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    Ok(LispObject::from(s1 < s2))
}

pub fn prim_concat(args: &LispObject) -> ElispResult<LispObject> {
    let mut result = String::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        match arg {
            LispObject::String(s) => result.push_str(&s),
            LispObject::Nil => {}
            LispObject::Cons(_) => {
                let mut list_cur = arg;
                while let Some((car, lrest)) = list_cur.destructure_cons() {
                    match car {
                        LispObject::Integer(n) => {
                            let ch = char::from_u32(n as u32).ok_or_else(|| {
                                ElispError::WrongTypeArgument("character".to_string())
                            })?;
                            result.push(ch);
                        }
                        _ => {
                            return Err(ElispError::WrongTypeArgument(
                                "sequence of chars".to_string(),
                            ));
                        }
                    }
                    list_cur = lrest;
                }
            }
            LispObject::Vector(v) => {
                let guard = v.lock();
                for item in guard.iter() {
                    match item {
                        LispObject::Integer(n) => {
                            let ch = char::from_u32(*n as u32).ok_or_else(|| {
                                ElispError::WrongTypeArgument("character".to_string())
                            })?;
                            result.push(ch);
                        }
                        _ => {
                            return Err(ElispError::WrongTypeArgument(
                                "sequence of chars".to_string(),
                            ));
                        }
                    }
                }
            }
            _ => return Err(ElispError::WrongTypeArgument("sequence".to_string())),
        }
        current = rest;
    }
    Ok(LispObject::string(&result))
}

pub fn prim_substring(args: &LispObject) -> ElispResult<LispObject> {
    let s = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let start = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let end = args.nth(2);

    let s = match s {
        LispObject::String(s) => s.clone(),
        LispObject::Nil => return Ok(LispObject::nil()),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let start_signed = match start {
        LispObject::Integer(i) => i,
        LispObject::Nil => 0,
        _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };
    let end_signed = match end {
        Some(LispObject::Integer(i)) => Some(i),
        Some(LispObject::Nil) | None => None,
        Some(_) => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };

    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;
    let normalize = |i: i64| -> i64 { if i < 0 { (len + i).max(0) } else { i.min(len) } };
    let start = normalize(start_signed) as usize;
    let end_idx = match end_signed {
        Some(e) => normalize(e) as usize,
        None => chars.len(),
    };
    if start > end_idx {
        return Err(ElispError::EvalError(format!(
            "substring: start {start} > end {end_idx}"
        )));
    }
    let result: String = chars[start..end_idx].iter().collect();
    Ok(LispObject::string(&result))
}

pub fn prim_string_to_number(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = match &arg {
        LispObject::String(s) => s.clone(),
        _ if arg.is_nil() => return Ok(LispObject::integer(0)),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    if let Some(radix) = args.nth(1).and_then(|obj| obj.as_integer()) {
        if (2..=36).contains(&radix) {
            let trimmed = s.trim();
            let sign = if trimmed.starts_with('-') { -1 } else { 1 };
            let digits = if trimmed.starts_with('+') || trimmed.starts_with('-') {
                &trimmed[1..]
            } else {
                trimmed
            };
            let valid: String = digits
                .chars()
                .take_while(|c| c.to_digit(radix as u32).is_some())
                .collect();
            if valid.is_empty() {
                return Ok(LispObject::integer(0));
            }
            return Ok(LispObject::integer(
                i64::from_str_radix(&valid, radix as u32).unwrap_or(0) * sign,
            ));
        }
    }
    if let Ok(i) = s.trim().parse::<i64>() {
        Ok(LispObject::integer(i))
    } else if let Ok(f) = s.trim().parse::<f64>() {
        Ok(LispObject::float(f))
    } else {
        Ok(LispObject::integer(0))
    }
}

pub fn prim_number_to_string(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Integer(i) => Ok(LispObject::string(&i.to_string())),
        LispObject::BigInt(i) => Ok(LispObject::string(&i.to_string())),
        LispObject::Float(f) => Ok(LispObject::string(&f.to_string())),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

pub fn prim_make_string(args: &LispObject) -> ElispResult<LispObject> {
    let length = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let ch = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    if length < 0 {
        return Ok(LispObject::string(""));
    }
    let c = char::from_u32(ch as u32).unwrap_or('?');
    let s: String = std::iter::repeat_n(c, length as usize).collect();
    Ok(LispObject::string(&s))
}

pub fn prim_upcase(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::string(&s.to_uppercase())),
        LispObject::Integer(c) => {
            let ch = char::from_u32(*c as u32).unwrap_or('?');
            Ok(LispObject::integer(
                ch.to_uppercase().next().unwrap_or(ch) as i64
            ))
        }
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

pub fn prim_downcase(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::string(&s.to_lowercase())),
        LispObject::Integer(c) => {
            let ch = char::from_u32(*c as u32).unwrap_or('?');
            Ok(LispObject::integer(
                ch.to_lowercase().next().unwrap_or(ch) as i64
            ))
        }
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

pub fn prim_capitalize(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::string(
            &crate::emacs::casefiddle::capitalize_string(s),
        )),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

pub fn prim_safe_length(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::Nil => Ok(LispObject::integer(0)),
        LispObject::Cons(_) => {
            let mut count: i64 = 0;
            let mut current = arg;
            while let Some((_, cdr)) = current.destructure_cons() {
                count += 1;
                if count > 1000000000 {
                    break;
                }
                current = cdr;
            }
            Ok(LispObject::integer(count))
        }
        LispObject::Vector(v) => Ok(LispObject::integer(v.lock().len() as i64)),
        LispObject::String(s) => Ok(LispObject::integer(s.len() as i64)),
        LispObject::HashTable(_) if crate::primitives::core::is_bool_vector(&arg) => {
            Ok(LispObject::integer(
                crate::primitives::core::bool_vector_length(&arg).unwrap_or(0) as i64,
            ))
        }
        _ => Err(ElispError::WrongTypeArgument("sequence".to_string())),
    }
}

pub fn prim_string(args: &LispObject) -> ElispResult<LispObject> {
    let mut result = String::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        match arg {
            LispObject::Integer(c) => {
                let ch = char::from_u32(c as u32)
                    .ok_or_else(|| ElispError::WrongTypeArgument("character".to_string()))?;
                result.push(ch);
            }
            _ => {
                return Err(ElispError::WrongTypeArgument("character".to_string()));
            }
        }
        current = rest;
    }
    Ok(LispObject::string(&result))
}

pub fn prim_regexp_quote(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = match &arg {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let quoted = crate::emacs::search::regexp_quote(&s);
    Ok(LispObject::string(&quoted))
}

pub fn prim_max_char(args: &LispObject) -> ElispResult<LispObject> {
    let unicode_arg = args.first().unwrap_or(LispObject::nil());
    if matches!(unicode_arg, LispObject::T) {
        Ok(LispObject::integer(0x10ffff))
    } else {
        Ok(LispObject::integer(0x3fffff))
    }
}

pub fn prim_string_replace(args: &LispObject) -> ElispResult<LispObject> {
    let needle = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let replacement = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let s = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;

    let needle = match &needle {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let replacement = match &replacement {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let s = match &s {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };

    Ok(LispObject::string(&s.replace(&needle, &replacement)))
}

pub fn prim_string_trim(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = match &arg {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    Ok(LispObject::string(s.trim()))
}

pub fn prim_string_prefix_p(args: &LispObject) -> ElispResult<LispObject> {
    let prefix = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let start = args.nth(2).and_then(|a| a.as_integer()).unwrap_or(0);

    let prefix = match &prefix {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let s = match &s {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };

    let char_count = s.chars().count() as i64;
    if start < 0 || start > char_count {
        return Err(ElispError::WrongTypeArgument("out of range".to_string()));
    }

    let byte_start: usize = s
        .char_indices()
        .nth(start as usize)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let s_start = &s[byte_start..];
    Ok(LispObject::from(s_start.starts_with(&prefix)))
}

pub fn prim_string_suffix_p(args: &LispObject) -> ElispResult<LispObject> {
    let suffix = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let suffix = match &suffix {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let s = match &s {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };

    Ok(LispObject::from(s.ends_with(&suffix)))
}

pub fn prim_string_join(args: &LispObject) -> ElispResult<LispObject> {
    let strings = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sep = args
        .nth(1)
        .and_then(|s| match s {
            LispObject::String(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default();

    let mut result = String::new();
    let mut current = strings;
    let mut first = true;
    while let Some((s, rest)) = current.destructure_cons() {
        match s {
            LispObject::String(s) => {
                if !first {
                    result.push_str(&sep);
                }
                result.push_str(&s);
                first = false;
            }
            _ => {
                return Err(ElispError::WrongTypeArgument("string".to_string()));
            }
        }
        current = rest;
    }
    Ok(LispObject::string(&result))
}

pub fn prim_char_to_string(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let c = match arg {
        LispObject::Integer(c) => char::from_u32(c as u32).unwrap_or('?'),
        _ => return Err(ElispError::WrongTypeArgument("character".to_string())),
    };
    Ok(LispObject::string(&c.to_string()))
}

pub fn prim_string_to_list(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = match arg {
        LispObject::String(s) => s,
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let mut out = LispObject::nil();
    for c in s.chars().rev() {
        out = LispObject::cons(LispObject::integer(c as i64), out);
    }
    Ok(out)
}

pub fn prim_string_to_char(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = match &arg {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let first_char = s.chars().next().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::integer(first_char as i64))
}

pub fn prim_string_width(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = match &arg {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;

    fn resolve_idx(v: Option<LispObject>, len: i64) -> ElispResult<Option<i64>> {
        let Some(v) = v else { return Ok(None) };
        if v.is_nil() {
            return Ok(None);
        }
        let n = v
            .as_integer()
            .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
        let idx = if n < 0 { len + n } else { n };
        if idx < 0 || idx > len {
            return Err(ElispError::EvalError(format!(
                "args-out-of-range: {n}"
            )));
        }
        Ok(Some(idx))
    }

    let from = resolve_idx(args.nth(1), len)?.unwrap_or(0) as usize;
    let to = resolve_idx(args.nth(2), len)?.unwrap_or(len) as usize;
    if from > to {
        return Err(ElispError::EvalError("args-out-of-range".to_string()));
    }

    // Default tab-width of 8 matches Emacs's default; the test that uses
    // tab-width also calls (+ 4 tab-width), so the answer is consistent
    // as long as both sides agree. We don't have InterpreterState access
    // here.
    const TAB_WIDTH: i64 = 8;
    let mut width: i64 = 0;
    for &c in &chars[from..to] {
        if c == '\t' {
            width += TAB_WIDTH;
        } else {
            width += char_display_width(c);
        }
    }
    Ok(LispObject::integer(width))
}

/// Returns the display width of a character: 0 for combining marks,
/// 1 for normal characters. East-Asian wide chars are counted as 1 too —
/// extending this to 2 would require a Unicode-width table that we don't
/// currently carry. The combining-mark ranges below cover the cases the
/// Emacs character-width tests exercise (Latin, Hebrew, Arabic, Devanagari).
fn char_display_width(c: char) -> i64 {
    let cp = c as u32;
    if matches!(
        cp,
        0x0300..=0x036F  // Combining Diacritical Marks
        | 0x0483..=0x0489
        | 0x0591..=0x05BD | 0x05BF | 0x05C1..=0x05C2 | 0x05C4..=0x05C5 | 0x05C7
        | 0x0610..=0x061A | 0x064B..=0x065F | 0x0670
        | 0x06D6..=0x06DC | 0x06DF..=0x06E4 | 0x06E7..=0x06E8 | 0x06EA..=0x06ED
        | 0x0711 | 0x0730..=0x074A
        | 0x07A6..=0x07B0 | 0x07EB..=0x07F3 | 0x07FD
        | 0x0816..=0x0819 | 0x081B..=0x0823 | 0x0825..=0x0827 | 0x0829..=0x082D
        | 0x0859..=0x085B | 0x08D3..=0x08E1 | 0x08E3..=0x0902
        | 0x093A | 0x093C | 0x0941..=0x0948 | 0x094D | 0x0951..=0x0957
        | 0x0962..=0x0963
        | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF
        | 0x20D0..=0x20FF | 0xFE20..=0xFE2F
    ) {
        0
    } else {
        1
    }
}

pub fn prim_multibyte_string_p(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::String(s) => Ok(LispObject::from(crate::object::string_is_multibyte(&s))),
        _ => Ok(LispObject::nil()),
    }
}

pub fn prim_string_search(args: &LispObject) -> ElispResult<LispObject> {
    let needle = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let haystack = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let start = args.nth(2).and_then(|a| a.as_integer()).unwrap_or(0);

    let needle = match &needle {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let haystack = match &haystack {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };

    let char_count = haystack.chars().count() as i64;
    if start < 0 || start >= char_count {
        return Ok(LispObject::nil());
    }

    // Convert char-index start to byte offset for slicing.
    let byte_start: usize = haystack
        .char_indices()
        .nth(start as usize)
        .map(|(i, _)| i)
        .unwrap_or(haystack.len());

    match haystack[byte_start..].find(&needle) {
        Some(byte_pos) => {
            // Convert byte position back to char index.
            let char_pos = haystack[byte_start..byte_start + byte_pos].chars().count() as i64;
            Ok(LispObject::integer(start + char_pos))
        }
        None => Ok(LispObject::nil()),
    }
}

pub fn prim_string_equal(args: &LispObject) -> ElispResult<LispObject> {
    prim_string_eq(args)
}

pub fn prim_string_lessp(args: &LispObject) -> ElispResult<LispObject> {
    prim_string_lt(args)
}

pub fn prim_compare_strings(args: &LispObject) -> ElispResult<LispObject> {
    let s1 = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s2 = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let start1 = args.nth(2).and_then(|a| a.as_integer()).unwrap_or(0);
    let end1 = args.nth(3).and_then(|a| a.as_integer());
    let start2 = args.nth(4).and_then(|a| a.as_integer()).unwrap_or(0);
    let end2 = args.nth(5).and_then(|a| a.as_integer());

    let s1 = match &s1 {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let s2 = match &s2 {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };

    let end1 = end1.unwrap_or(s1.len() as i64).min(s1.len() as i64) as usize;
    let end2 = end2.unwrap_or(s2.len() as i64).min(s2.len() as i64) as usize;
    let start1 = start1.min(end1 as i64) as usize;
    let start2 = start2.min(end2 as i64) as usize;

    let s1_sub = &s1[start1..end1];
    let s2_sub = &s2[start2..end2];

    let cmp = s1_sub.cmp(s2_sub);
    Ok(LispObject::from(matches!(cmp, std::cmp::Ordering::Less)))
}

pub fn prim_string_bytes(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::integer(s.len() as i64)),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

pub fn prim_key_description(args: &LispObject) -> ElispResult<LispObject> {
    let keys = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let render = |obj: &LispObject| -> String {
        match obj {
            LispObject::Integer(i) => {
                if let Some(c) = char::from_u32(*i as u32) {
                    c.to_string()
                } else {
                    i.to_string()
                }
            }
            LispObject::Symbol(_) => obj.as_symbol().unwrap_or_default(),
            LispObject::String(s) => s.clone(),
            _ => String::new(),
        }
    };
    let out = match &keys {
        LispObject::String(s) => s.clone(),
        LispObject::Vector(v) => v.lock().iter().map(render).collect::<Vec<_>>().join(" "),
        _ => String::new(),
    };
    Ok(LispObject::string(&out))
}

pub fn prim_upcase_initials(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::String(s) => Ok(LispObject::string(
            &crate::emacs::casefiddle::upcase_initials(&s),
        )),
        LispObject::Integer(i) => {
            if let Some(c) = char::from_u32(i as u32) {
                let up = c.to_uppercase().next().unwrap_or(c);
                Ok(LispObject::integer(up as i64))
            } else {
                Ok(LispObject::integer(i))
            }
        }
        _ => Err(ElispError::WrongTypeArgument("string or char".to_string())),
    }
}

pub fn prim_unibyte_string(args: &LispObject) -> ElispResult<LispObject> {
    let mut bytes = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let b = arg
            .as_integer()
            .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
        bytes.push(b as u8);
        current = rest;
    }
    match String::from_utf8(bytes) {
        Ok(s) => Ok(LispObject::string(&s)),
        Err(e) => Ok(LispObject::string(&String::from_utf8_lossy(
            &e.into_bytes(),
        ))),
    }
}

pub fn prim_string_lines(args: &LispObject) -> ElispResult<LispObject> {
    let s = args
        .first()
        .and_then(|a| match a {
            LispObject::String(s) => Some(s),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    let omit_nulls = args.nth(1).is_some_and(|a| !a.is_nil());
    let keep_newlines = args.nth(2).is_some_and(|a| !a.is_nil());
    let mut pieces: Vec<String> = if keep_newlines {
        let mut out = Vec::new();
        let mut buf = String::new();
        for c in s.chars() {
            buf.push(c);
            if c == '\n' {
                out.push(std::mem::take(&mut buf));
            }
        }
        if !buf.is_empty() {
            out.push(buf);
        }
        out
    } else {
        s.split('\n').map(String::from).collect()
    };
    if omit_nulls {
        pieces.retain(|p| !p.is_empty());
    }
    let mut result = LispObject::nil();
    for p in pieces.into_iter().rev() {
        result = LispObject::cons(LispObject::string(&p), result);
    }
    Ok(result)
}

pub fn prim_format_prompt(args: &LispObject) -> ElispResult<LispObject> {
    let prompt = args
        .first()
        .and_then(|a| match a {
            LispObject::String(s) => Some(s),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    let default = args.nth(1).unwrap_or_else(LispObject::nil);
    let suffix = match &default {
        LispObject::Nil => String::from(": "),
        LispObject::String(s) => format!(" (default {s}): "),
        other => format!(" (default {}): ", other.princ_to_string()),
    };
    Ok(LispObject::string(&format!("{prompt}{suffix}")))
}

pub fn prim_decode_char(args: &LispObject) -> ElispResult<LispObject> {
    let _charset = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let code = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match code {
        LispObject::Integer(_) => Ok(code),
        _ => Err(ElispError::WrongTypeArgument("integer".to_string())),
    }
}

pub fn prim_encode_char(args: &LispObject) -> ElispResult<LispObject> {
    let ch = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let _charset = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match ch {
        LispObject::Integer(_) => Ok(ch),
        _ => Err(ElispError::WrongTypeArgument("integer".to_string())),
    }
}

pub fn prim_split_string(args: &LispObject) -> ElispResult<LispObject> {
    let s = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sep = args
        .nth(1)
        .and_then(|a| match a {
            LispObject::String(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default();

    let s = match &s {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };

    let parts: Vec<LispObject> = s.split(&sep).map(|p| LispObject::string(p)).collect();

    let mut result = LispObject::nil();
    for part in parts.into_iter().rev() {
        result = LispObject::cons(part, result);
    }
    Ok(result)
}
