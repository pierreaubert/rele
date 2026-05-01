//! `md5`, `secure-hash`, `buffer-hash` backed by the `md-5`, `sha1`,
//! `sha2` crates. Output is a lowercase hex string matching Emacs.

use digest::Digest;

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in ["md5", "secure-hash", "buffer-hash"] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        "md5" => prim_md5(args),
        "secure-hash" => prim_secure_hash(args),
        "buffer-hash" => prim_buffer_hash(args),
        _ => return None,
    };
    Some(result)
}

fn input_bytes(args: &LispObject, obj_index: usize) -> ElispResult<Vec<u8>> {
    let obj = args
        .nth(obj_index)
        .ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(s) = obj.as_string() {
        return Ok(s.as_bytes().to_vec());
    }
    // Buffers are tagged cons cells `(buffer . ID)` in this build —
    // see primitives/buffer.rs::make_buffer_object. Anything that
    // isn't a plain string falls back to the current buffer's text.
    let text = crate::buffer::with_current(|b| b.buffer_string());
    Ok(text.into_bytes())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn prim_md5(args: &LispObject) -> ElispResult<LispObject> {
    let bytes = input_bytes(args, 0)?;
    let digest = md5::Md5::digest(&bytes);
    Ok(LispObject::string(&hex(&digest)))
}

fn prim_secure_hash(args: &LispObject) -> ElispResult<LispObject> {
    let algo = args
        .first()
        .and_then(|a| a.as_symbol())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let mut bytes = input_bytes(args, 1)?;
    // (secure-hash ALGORITHM OBJECT &optional START END BINARY)
    let start = args.nth(2).and_then(|v| v.as_integer()).unwrap_or(0);
    let end = args
        .nth(3)
        .and_then(|v| v.as_integer())
        .unwrap_or(bytes.len() as i64);
    if start > 0 || (end as usize) < bytes.len() {
        let lo = start.max(0) as usize;
        let hi = (end.max(0) as usize).min(bytes.len());
        bytes = bytes[lo..hi].to_vec();
    }
    let binary = args.nth(4).is_some_and(|v| !v.is_nil());

    let digest_bytes: Vec<u8> = match algo.as_str() {
        "md5" => md5::Md5::digest(&bytes).to_vec(),
        "sha1" => sha1::Sha1::digest(&bytes).to_vec(),
        "sha224" => sha2::Sha224::digest(&bytes).to_vec(),
        "sha256" => sha2::Sha256::digest(&bytes).to_vec(),
        "sha384" => sha2::Sha384::digest(&bytes).to_vec(),
        "sha512" => sha2::Sha512::digest(&bytes).to_vec(),
        other => {
            return Err(ElispError::EvalError(format!(
                "secure-hash: unknown algorithm `{other}`"
            )));
        }
    };
    if binary {
        // Emacs returns a unibyte string with raw bytes.
        Ok(LispObject::string(&String::from_utf8_lossy(&digest_bytes)))
    } else {
        Ok(LispObject::string(&hex(&digest_bytes)))
    }
}

/// `(buffer-hash &optional BUFFER)` — Emacs returns SHA-1 hex of the
/// buffer text.
fn prim_buffer_hash(_args: &LispObject) -> ElispResult<LispObject> {
    let text = crate::buffer::with_current(|b| b.buffer_string());
    let digest = sha1::Sha1::digest(text.as_bytes());
    Ok(LispObject::string(&hex(&digest)))
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
    fn md5_known_vector() {
        let r = prim_md5(&args(vec![LispObject::string("abc")])).expect("md5");
        assert_eq!(
            r.as_string().map(String::as_str),
            Some("900150983cd24fb0d6963f7d28e17f72")
        );
    }

    #[test]
    fn secure_hash_sha1_known_vector() {
        let r = prim_secure_hash(&args(vec![
            LispObject::symbol("sha1"),
            LispObject::string("abc"),
        ]))
        .expect("sha1");
        assert_eq!(
            r.as_string().map(String::as_str),
            Some("a9993e364706816aba3e25717850c26c9cd0d89d")
        );
    }

    #[test]
    fn secure_hash_sha256_known_vector() {
        let r = prim_secure_hash(&args(vec![
            LispObject::symbol("sha256"),
            LispObject::string("abc"),
        ]))
        .expect("sha256");
        assert_eq!(
            r.as_string().map(String::as_str),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }
}
