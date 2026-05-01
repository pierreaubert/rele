//! Base64 / base64url encode/decode primitives. RFC 4648 alphabet.
//! Pure-Rust inline implementation — no external crate.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

const STD_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "base64-encode-string",
        "base64-decode-string",
        "base64-encode-region",
        "base64-decode-region",
        "base64url-encode-string",
        "base64url-encode-region",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "base64-encode-string" => Some(prim_base64_encode_string(args, STD_ALPHABET, true)),
        "base64url-encode-string" => Some(prim_base64_encode_string(args, URL_ALPHABET, false)),
        "base64-decode-string" => Some(prim_base64_decode_string(args)),
        "base64-encode-region" => Some(prim_base64_encode_region(args, STD_ALPHABET, true)),
        "base64url-encode-region" => Some(prim_base64_encode_region(args, URL_ALPHABET, false)),
        "base64-decode-region" => Some(prim_base64_decode_region(args)),
        _ => None,
    }
}

fn encode(input: &[u8], alphabet: &[u8; 64], pad: bool) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut chunks = input.chunks_exact(3);
    for chunk in &mut chunks {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]);
        out.push(alphabet[((n >> 18) & 0x3F) as usize] as char);
        out.push(alphabet[((n >> 12) & 0x3F) as usize] as char);
        out.push(alphabet[((n >> 6) & 0x3F) as usize] as char);
        out.push(alphabet[(n & 0x3F) as usize] as char);
    }
    let rem = chunks.remainder();
    match rem.len() {
        1 => {
            let n = u32::from(rem[0]) << 16;
            out.push(alphabet[((n >> 18) & 0x3F) as usize] as char);
            out.push(alphabet[((n >> 12) & 0x3F) as usize] as char);
            if pad {
                out.push_str("==");
            }
        }
        2 => {
            let n = (u32::from(rem[0]) << 16) | (u32::from(rem[1]) << 8);
            out.push(alphabet[((n >> 18) & 0x3F) as usize] as char);
            out.push(alphabet[((n >> 12) & 0x3F) as usize] as char);
            out.push(alphabet[((n >> 6) & 0x3F) as usize] as char);
            if pad {
                out.push('=');
            }
        }
        _ => {}
    }
    out
}

fn decode(input: &str) -> ElispResult<Vec<u8>> {
    let mut filtered: Vec<u8> = Vec::with_capacity(input.len());
    for byte in input.bytes() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        filtered.push(byte);
    }
    while filtered.last() == Some(&b'=') {
        filtered.pop();
    }
    let mut out = Vec::with_capacity(filtered.len() * 3 / 4);
    let mut buffer: u32 = 0;
    let mut bits = 0u32;
    for byte in &filtered {
        let val = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return Err(ElispError::EvalError("base64: invalid character".into())),
        };
        buffer = (buffer << 6) | u32::from(val);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xFF) as u8);
        }
    }
    Ok(out)
}

fn prim_base64_encode_string(
    args: &LispObject,
    alphabet: &[u8; 64],
    default_pad: bool,
) -> ElispResult<LispObject> {
    let s = args
        .first()
        .and_then(|a| a.as_string().cloned())
        .ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let no_pad = args.nth(1).is_some_and(|v| !v.is_nil());
    let pad = default_pad && !no_pad;
    Ok(LispObject::string(&encode(s.as_bytes(), alphabet, pad)))
}

fn prim_base64_decode_string(args: &LispObject) -> ElispResult<LispObject> {
    let s = args
        .first()
        .and_then(|a| a.as_string().cloned())
        .ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let bytes = decode(&s)?;
    let text = String::from_utf8(bytes)
        .map_err(|_| ElispError::EvalError("base64: invalid UTF-8 in decoded bytes".into()))?;
    Ok(LispObject::string(&text))
}

fn prim_base64_encode_region(
    args: &LispObject,
    alphabet: &[u8; 64],
    default_pad: bool,
) -> ElispResult<LispObject> {
    let start = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".into()))?
        .max(0) as usize;
    let end = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".into()))?
        .max(0) as usize;
    let no_pad = args.nth(2).is_some_and(|v| !v.is_nil());
    let pad = default_pad && !no_pad;
    let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
    let text = crate::buffer::with_current(|b| b.substring(lo, hi));
    let encoded = encode(text.as_bytes(), alphabet, pad);
    crate::buffer::with_current_mut(|b| {
        b.delete_region(lo, hi);
        b.goto_char(lo);
    });
    crate::buffer::with_registry_mut(|r| r.insert_current(&encoded, false));
    Ok(LispObject::integer(encoded.len() as i64))
}

fn prim_base64_decode_region(args: &LispObject) -> ElispResult<LispObject> {
    let start = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".into()))?
        .max(0) as usize;
    let end = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".into()))?
        .max(0) as usize;
    let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
    let text = crate::buffer::with_current(|b| b.substring(lo, hi));
    let bytes = decode(&text)?;
    let decoded = String::from_utf8(bytes)
        .map_err(|_| ElispError::EvalError("base64: invalid UTF-8 in decoded bytes".into()))?;
    crate::buffer::with_current_mut(|b| {
        b.delete_region(lo, hi);
        b.goto_char(lo);
    });
    crate::buffer::with_registry_mut(|r| r.insert_current(&decoded, false));
    Ok(LispObject::integer(decoded.chars().count() as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: Vec<LispObject>) -> LispObject {
        items
            .into_iter()
            .rev()
            .fold(LispObject::nil(), |tail, item| LispObject::cons(item, tail))
    }

    #[test]
    fn base64_round_trip() {
        let encoded =
            prim_base64_encode_string(&args(vec![LispObject::string("hello")]), STD_ALPHABET, true)
                .expect("encode ok");
        assert_eq!(encoded.as_string().map(String::as_str), Some("aGVsbG8="));
        let decoded = prim_base64_decode_string(&args(vec![encoded])).expect("decode ok");
        assert_eq!(decoded.as_string().map(String::as_str), Some("hello"));
    }

    #[test]
    fn base64url_no_padding_by_default() {
        let encoded = prim_base64_encode_string(
            &args(vec![LispObject::string("Hi?")]),
            URL_ALPHABET,
            false,
        )
        .expect("encode ok");
        assert_eq!(encoded.as_string().map(String::as_str), Some("SGk_"));
    }
}
