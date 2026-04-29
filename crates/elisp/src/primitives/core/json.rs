use crate::error::{ElispError, ElispResult, SignalData};
use crate::object::{HashKey, HashTableTest, LispHashTable, LispObject};
use num_bigint::BigInt;
use serde_json::Value as JsonValue;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone, Copy)]
enum ObjectType {
    HashTable,
    Alist,
    Plist,
}

#[derive(Clone, Copy)]
enum ArrayType {
    Vector,
    List,
}

struct ParseOptions {
    object_type: ObjectType,
    array_type: ArrayType,
    null_object: LispObject,
    false_object: LispObject,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            object_type: ObjectType::HashTable,
            array_type: ArrayType::Vector,
            null_object: LispObject::symbol(":null"),
            false_object: LispObject::symbol(":false"),
        }
    }
}

struct SerializeOptions {
    null_object: LispObject,
    false_object: LispObject,
}

impl Default for SerializeOptions {
    fn default() -> Self {
        Self {
            null_object: LispObject::symbol(":null"),
            false_object: LispObject::symbol(":false"),
        }
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "json-parse-string" | "json-read-from-string" | "json-decode" => {
            Some(prim_json_parse_string(args))
        }
        "json-parse-buffer" | "json-read" => Some(prim_json_parse_buffer(args)),
        "json-serialize" | "json-encode" => Some(prim_json_serialize(args)),
        _ => None,
    }
}

fn signal(symbol: &str, message: impl Into<String>) -> ElispError {
    ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol(symbol),
        data: LispObject::cons(LispObject::string(&message.into()), LispObject::nil()),
    }))
}

fn wrong_type(expected: &str) -> ElispError {
    ElispError::WrongTypeArgument(expected.to_string())
}

fn prim_json_parse_string(args: &LispObject) -> ElispResult<LispObject> {
    let text = args
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_string()
        .ok_or_else(|| wrong_type("string"))?
        .to_string();
    let options = parse_options(args.rest().unwrap_or_else(LispObject::nil))?;
    let (value, consumed) = parse_one_json(&text)?;
    let rest = &text[consumed..];
    if !rest.trim().is_empty() {
        return Err(signal("json-trailing-content", "trailing content"));
    }
    json_to_lisp(&value, &options)
}

fn prim_json_parse_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let options = parse_options(args.clone())?;
    let (start, text) = crate::buffer::with_current(|buffer| {
        (
            buffer.point,
            buffer.substring(buffer.point, buffer.point_max()),
        )
    });
    let (value, consumed) = parse_one_json(&text)?;
    let char_count = text[..consumed].chars().count();
    crate::buffer::with_current_mut(|buffer| {
        buffer.goto_char(start + char_count);
    });
    json_to_lisp(&value, &options)
}

fn prim_json_serialize(args: &LispObject) -> ElispResult<LispObject> {
    let value = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let options = serialize_options(args.rest().unwrap_or_else(LispObject::nil))?;
    let mut seen = HashSet::new();
    let out = serialize_lisp(&value, &options, &mut seen)?;
    Ok(LispObject::string(&out))
}

fn parse_options(mut args: LispObject) -> ElispResult<ParseOptions> {
    let mut options = ParseOptions::default();
    while let Some((key, rest)) = args.destructure_cons() {
        let Some(value) = rest.first() else {
            return Err(ElispError::WrongNumberOfArguments);
        };
        let key_name = key.as_symbol().ok_or_else(|| wrong_type("keyword"))?;
        match key_name.as_str() {
            ":object-type" => {
                options.object_type = match value.as_symbol().as_deref() {
                    Some("hash-table") => ObjectType::HashTable,
                    Some("alist") => ObjectType::Alist,
                    Some("plist") => ObjectType::Plist,
                    _ => return Err(wrong_type("json-object-type")),
                };
            }
            ":array-type" => {
                options.array_type = match value.as_symbol().as_deref() {
                    Some("array") | Some("vector") => ArrayType::Vector,
                    Some("list") => ArrayType::List,
                    _ => return Err(wrong_type("json-array-type")),
                };
            }
            ":null-object" => options.null_object = value,
            ":false-object" => options.false_object = value,
            _ => {}
        }
        args = rest.rest().unwrap_or_else(LispObject::nil);
    }
    if !args.is_nil() {
        return Err(wrong_type("list"));
    }
    Ok(options)
}

fn serialize_options(mut args: LispObject) -> ElispResult<SerializeOptions> {
    let mut options = SerializeOptions::default();
    while let Some((key, rest)) = args.destructure_cons() {
        let Some(value) = rest.first() else {
            return Err(ElispError::WrongNumberOfArguments);
        };
        let key_name = key.as_symbol().ok_or_else(|| wrong_type("keyword"))?;
        match key_name.as_str() {
            ":null-object" => options.null_object = value,
            ":false-object" => options.false_object = value,
            ":object-type" | ":array-type" => return Err(wrong_type("keyword")),
            _ => {}
        }
        args = rest.rest().unwrap_or_else(LispObject::nil);
    }
    if !args.is_nil() {
        return Err(wrong_type("list"));
    }
    Ok(options)
}

fn parse_one_json(text: &str) -> ElispResult<(JsonValue, usize)> {
    if has_raw_byte_marker(text) || text.contains('\u{fffd}') {
        return Err(signal("json-utf8-decode-error", "invalid utf-8"));
    }
    if text.trim().is_empty() {
        return Err(signal("json-end-of-file", "end of file"));
    }

    let mut stream = serde_json::Deserializer::from_str(text).into_iter::<JsonValue>();
    match stream.next() {
        Some(Ok(value)) => Ok((value, stream.byte_offset())),
        Some(Err(err)) => Err(json_parse_signal(&err)),
        None => Err(signal("json-end-of-file", "end of file")),
    }
}

fn json_parse_signal(err: &serde_json::Error) -> ElispError {
    let message = err.to_string();
    match err.classify() {
        serde_json::error::Category::Eof => signal("json-end-of-file", message),
        serde_json::error::Category::Syntax | serde_json::error::Category::Data
            if message.contains("surrogate") =>
        {
            signal("json-utf8-decode-error", message)
        }
        _ => signal("json-parse-error", message),
    }
}

fn has_raw_byte_marker(text: &str) -> bool {
    text.chars().any(|c| ('\u{e080}'..='\u{e0ff}').contains(&c))
}

fn json_to_lisp(value: &JsonValue, options: &ParseOptions) -> ElispResult<LispObject> {
    match value {
        JsonValue::Null => Ok(options.null_object.clone()),
        JsonValue::Bool(false) => Ok(options.false_object.clone()),
        JsonValue::Bool(true) => Ok(LispObject::t()),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LispObject::bigint(BigInt::from(i)))
            } else if let Some(u) = n.as_u64() {
                Ok(LispObject::bigint(BigInt::from(u)))
            } else if let Some(f) = n.as_f64() {
                Ok(LispObject::float(f))
            } else {
                Err(signal("json-parse-error", "invalid number"))
            }
        }
        JsonValue::String(s) => Ok(LispObject::string(s)),
        JsonValue::Array(items) => {
            let values = items
                .iter()
                .map(|item| json_to_lisp(item, options))
                .collect::<ElispResult<Vec<_>>>()?;
            match options.array_type {
                ArrayType::Vector => Ok(LispObject::Vector(Arc::new(
                    crate::eval::SyncRefCell::new(values),
                ))),
                ArrayType::List => Ok(list_from_vec(values)),
            }
        }
        JsonValue::Object(map) => match options.object_type {
            ObjectType::HashTable => {
                let mut table = LispHashTable::new(HashTableTest::Equal);
                for (key, value) in map {
                    table.put(&LispObject::string(key), json_to_lisp(value, options)?);
                }
                Ok(LispObject::HashTable(Arc::new(
                    crate::eval::SyncRefCell::new(table),
                )))
            }
            ObjectType::Alist => {
                let pairs = map
                    .iter()
                    .map(|(key, value)| {
                        Ok(LispObject::cons(
                            LispObject::symbol(key),
                            json_to_lisp(value, options)?,
                        ))
                    })
                    .collect::<ElispResult<Vec<_>>>()?;
                Ok(list_from_vec(pairs))
            }
            ObjectType::Plist => {
                let mut values = Vec::with_capacity(map.len() * 2);
                for (key, value) in map {
                    values.push(LispObject::symbol(&format!(":{key}")));
                    values.push(json_to_lisp(value, options)?);
                }
                Ok(list_from_vec(values))
            }
        },
    }
}

fn list_from_vec(values: Vec<LispObject>) -> LispObject {
    values
        .into_iter()
        .rev()
        .fold(LispObject::nil(), |list, value| {
            LispObject::cons(value, list)
        })
}

fn serialize_lisp(
    value: &LispObject,
    options: &SerializeOptions,
    seen: &mut HashSet<usize>,
) -> ElispResult<String> {
    if value == &options.null_object {
        return Ok("null".to_string());
    }
    if value == &options.false_object {
        return Ok("false".to_string());
    }
    match value {
        LispObject::Nil => serialize_object_pairs(Vec::new()),
        LispObject::T => Ok("true".to_string()),
        LispObject::Integer(i) => Ok(i.to_string()),
        LispObject::BigInt(i) => Ok(i.to_string()),
        LispObject::Float(f) if f.is_finite() => Ok(f.to_string()),
        LispObject::Float(_) => Err(wrong_type("number")),
        LispObject::String(s) => serialize_string(s),
        LispObject::Vector(items) => {
            let guard = items.lock();
            let mut out = String::from("[");
            for (idx, item) in guard.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                out.push_str(&serialize_lisp(item, options, seen)?);
            }
            out.push(']');
            Ok(out)
        }
        LispObject::HashTable(table) => {
            let guard = table.lock();
            let mut pairs = Vec::with_capacity(guard.data.len());
            for (key, value) in &guard.data {
                pairs.push((hash_json_key(key)?, serialize_lisp(value, options, seen)?));
            }
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            serialize_object_pairs(pairs)
        }
        LispObject::Cons(cell) => {
            let ptr = Arc::as_ptr(cell) as usize;
            if !seen.insert(ptr) {
                return Err(signal("circular-list", "circular list"));
            }
            let result = serialize_list_object(value, options, seen);
            seen.remove(&ptr);
            result
        }
        LispObject::Symbol(id) => {
            let name = crate::obarray::symbol_name(*id);
            match name.as_str() {
                "t" => Ok("true".to_string()),
                ":null" => Ok("null".to_string()),
                ":false" => Ok("false".to_string()),
                _ => Err(wrong_type("json-value")),
            }
        }
        _ => Err(wrong_type("json-value")),
    }
}

fn serialize_string(s: &str) -> ElispResult<String> {
    if has_raw_byte_marker(s) || s.contains('\u{fffd}') {
        return Err(wrong_type("string"));
    }
    serde_json::to_string(s).map_err(|err| signal("json-error", err.to_string()))
}

fn hash_json_key(key: &HashKey) -> ElispResult<String> {
    match key {
        HashKey::String(s) => Ok(s.clone()),
        HashKey::Printed(s) => serde_json::from_str::<String>(s).map_err(|_| wrong_type("string")),
        _ => Err(wrong_type("string")),
    }
}

fn serialize_list_object(
    value: &LispObject,
    options: &SerializeOptions,
    seen: &mut HashSet<usize>,
) -> ElispResult<String> {
    let first = value.first().unwrap_or_else(LispObject::nil);
    if first.is_cons() {
        serialize_alist(value, options, seen)
    } else {
        serialize_plist(value, options, seen)
    }
}

fn serialize_alist(
    value: &LispObject,
    options: &SerializeOptions,
    seen: &mut HashSet<usize>,
) -> ElispResult<String> {
    let mut current = value.clone();
    let mut pairs = Vec::new();
    let mut keys = HashSet::new();
    let mut list_seen = HashSet::new();
    while let Some((entry, rest)) = current.destructure_cons() {
        if let LispObject::Cons(cell) = &current
            && !list_seen.insert(Arc::as_ptr(cell) as usize)
        {
            return Err(signal("circular-list", "circular list"));
        }
        let Some((key_obj, value_obj)) = entry.destructure_cons() else {
            return Err(wrong_type("cons"));
        };
        let key = lisp_object_key(&key_obj)?;
        if value_obj.is_cons() {
            return Err(wrong_type("cons"));
        }
        if keys.insert(key.clone()) {
            pairs.push((key, serialize_lisp(&value_obj, options, seen)?));
        }
        current = rest;
    }
    if !current.is_nil() {
        return Err(wrong_type("list"));
    }
    serialize_object_pairs(pairs)
}

fn serialize_plist(
    value: &LispObject,
    options: &SerializeOptions,
    seen: &mut HashSet<usize>,
) -> ElispResult<String> {
    let mut current = value.clone();
    let mut pairs = Vec::new();
    let mut keys = HashSet::new();
    let mut list_seen = HashSet::new();
    while let Some((key_obj, rest)) = current.destructure_cons() {
        if let LispObject::Cons(cell) = &current
            && !list_seen.insert(Arc::as_ptr(cell) as usize)
        {
            return Err(signal("circular-list", "circular list"));
        }
        let Some((value_obj, next)) = rest.destructure_cons() else {
            return Err(wrong_type("list"));
        };
        let key = lisp_object_key(&key_obj)?;
        if keys.insert(key.clone()) {
            pairs.push((key, serialize_lisp(&value_obj, options, seen)?));
        }
        current = next;
    }
    if !current.is_nil() {
        return Err(wrong_type("list"));
    }
    serialize_object_pairs(pairs)
}

fn lisp_object_key(value: &LispObject) -> ElispResult<String> {
    match value {
        LispObject::String(s) => Ok(s.clone()),
        LispObject::Symbol(id) => {
            let name = crate::obarray::symbol_name(*id);
            Ok(name.strip_prefix(':').unwrap_or(&name).to_string())
        }
        _ => Err(wrong_type("string")),
    }
}

fn serialize_object_pairs(pairs: Vec<(String, String)>) -> ElispResult<String> {
    let mut out = String::from("{");
    for (idx, (key, value)) in pairs.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push_str(&serialize_string(key)?);
        out.push(':');
        out.push_str(value);
    }
    out.push('}');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: Vec<LispObject>) -> LispObject {
        list_from_vec(items)
    }

    #[test]
    fn parse_string_rejects_trailing_content() {
        let result = prim_json_parse_string(&args(vec![LispObject::string("[1] [2]")]));
        assert!(
            matches!(result, Err(ElispError::Signal(sig)) if sig.symbol.as_symbol().as_deref() == Some("json-trailing-content"))
        );
    }

    #[test]
    fn parse_string_rejects_replacement_char() {
        let result = prim_json_parse_string(&args(vec![LispObject::string("[\"\u{fffd}\"]")]));
        assert!(
            matches!(result, Err(ElispError::Signal(sig)) if sig.symbol.as_symbol().as_deref() == Some("json-utf8-decode-error"))
        );
    }

    #[test]
    fn serialize_rejects_circular_object_list() {
        let object = crate::read("#1=((a . 1) . #1#)").expect("read circular object");
        let result = prim_json_serialize(&args(vec![object]));
        assert!(result.is_err());
    }

    #[test]
    fn serialize_nested_object_shapes() {
        let mut table = LispHashTable::new(HashTableTest::Equal);
        table.put(&LispObject::string("bla"), LispObject::string("ble"));
        let table = LispObject::HashTable(Arc::new(crate::eval::SyncRefCell::new(table)));
        let alist = list_from_vec(vec![LispObject::cons(
            LispObject::symbol("bla"),
            LispObject::string("ble"),
        )]);
        let plist = list_from_vec(vec![LispObject::symbol(":bla"), LispObject::string("ble")]);
        let object = list_from_vec(vec![
            LispObject::symbol(":detect-hash-table"),
            table,
            LispObject::symbol(":detect-alist"),
            alist,
            LispObject::symbol(":detect-plist"),
            plist,
        ]);
        assert_eq!(
            prim_json_serialize(&args(vec![object]))
                .unwrap()
                .as_string()
                .map(String::as_str),
            Some(
                "{\"detect-hash-table\":{\"bla\":\"ble\"},\"detect-alist\":{\"bla\":\"ble\"},\"detect-plist\":{\"bla\":\"ble\"}}"
            )
        );
    }

    #[test]
    fn serialize_reader_hash_table_record() {
        let table =
            crate::read("#s(hash-table test equal data (\"bla\" \"ble\"))").expect("read table");
        assert_eq!(
            prim_json_serialize(&args(vec![table]))
                .unwrap()
                .as_string()
                .map(String::as_str),
            Some("{\"bla\":\"ble\"}")
        );
    }

    #[test]
    fn eval_serialize_reader_hash_table_record_in_plist() {
        let mut interp = crate::Interpreter::new();
        crate::add_primitives(&mut interp);
        let value = interp
            .eval_source(
                "(equal
                  (json-serialize
                   (list :detect-hash-table #s(hash-table test equal data (\"bla\" \"ble\"))
                         :detect-alist '((bla . \"ble\"))
                         :detect-plist '(:bla \"ble\")))
                  \"\\
{\\
\\\"detect-hash-table\\\":{\\\"bla\\\":\\\"ble\\\"},\\
\\\"detect-alist\\\":{\\\"bla\\\":\\\"ble\\\"},\\
\\\"detect-plist\\\":{\\\"bla\\\":\\\"ble\\\"}\\
}\")",
            )
            .unwrap();
        assert_eq!(value, LispObject::t());
    }

    #[test]
    fn serialize_vector_and_alist() {
        let vector = LispObject::Vector(Arc::new(crate::eval::SyncRefCell::new(vec![
            LispObject::integer(1),
            LispObject::symbol(":false"),
        ])));
        assert_eq!(
            prim_json_serialize(&args(vec![vector]))
                .unwrap()
                .as_string()
                .map(String::as_str),
            Some("[1,false]")
        );

        let alist = list_from_vec(vec![LispObject::cons(
            LispObject::symbol("abc"),
            LispObject::integer(1),
        )]);
        assert_eq!(
            prim_json_serialize(&args(vec![alist]))
                .unwrap()
                .as_string()
                .map(String::as_str),
            Some("{\"abc\":1}")
        );
    }
}
