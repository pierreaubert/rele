//! Proptest generators for the Elisp AST subset used in differential testing.
//!
//! Generates random well-formed s-expressions within the oracle subset
//! (integers, strings, symbols, let/dlet, setq, if, progn, lambda, throw/catch,
//! unwind-protect) and serialises them as JSON for the Lean oracle.

use rele_elisp::LispObject;
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};

/// JSON-serialisable Elisp value matching the Lean oracle's encoding.
///
/// Encoding:
///   null         → nil
///   true         → t
///   { "int": n } → integer
///   { "str": s } → string
///   { "sym": s } → symbol
///   { "cons": [car, cdr] } → cons cell
#[derive(Debug, Clone, PartialEq)]
pub enum JsonVal {
    Nil,
    T,
    Int { int: i64 },
    Str { str: String },
    Sym { sym: String },
    Cons { cons: Box<(JsonVal, JsonVal)> },
}

impl Serialize for JsonVal {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Nil => serializer.serialize_none(),
            Self::T => serializer.serialize_bool(true),
            Self::Int { int } => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("int", int)?;
                map.end()
            }
            Self::Str { str } => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("str", str)?;
                map.end()
            }
            Self::Sym { sym } => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("sym", sym)?;
                map.end()
            }
            Self::Cons { cons } => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("cons", &[&cons.0, &cons.1])?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for JsonVal {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(deserializer)?;
        json_value_to_json_val(&v).map_err(serde::de::Error::custom)
    }
}

fn json_value_to_json_val(v: &serde_json::Value) -> Result<JsonVal, String> {
    match v {
        serde_json::Value::Null => Ok(JsonVal::Nil),
        serde_json::Value::Bool(true) => Ok(JsonVal::T),
        serde_json::Value::Bool(false) => Ok(JsonVal::Nil),
        serde_json::Value::Number(n) => {
            let i = n.as_i64().ok_or("number not i64")?;
            Ok(JsonVal::Int { int: i })
        }
        serde_json::Value::String(s) => Ok(JsonVal::Str { str: s.clone() }),
        serde_json::Value::Object(m) => {
            if let Some(v) = m.get("int") {
                let i = v.as_i64().ok_or("int field not i64")?;
                Ok(JsonVal::Int { int: i })
            } else if let Some(v) = m.get("str") {
                let s = v.as_str().ok_or("str field not string")?;
                Ok(JsonVal::Str { str: s.to_string() })
            } else if let Some(v) = m.get("sym") {
                let s = v.as_str().ok_or("sym field not string")?;
                Ok(JsonVal::Sym { sym: s.to_string() })
            } else if let Some(v) = m.get("cons") {
                let arr = v.as_array().ok_or("cons field not array")?;
                if arr.len() != 2 {
                    return Err("cons must have 2 elements".to_string());
                }
                let car = json_value_to_json_val(&arr[0])?;
                let cdr = json_value_to_json_val(&arr[1])?;
                Ok(JsonVal::cons(car, cdr))
            } else if let Some(v) = m.get("ok") {
                // For OracleOutput parsing — unwrap the ok wrapper
                json_value_to_json_val(v)
            } else {
                Err(format!("unknown object keys: {m:?}"))
            }
        }
        serde_json::Value::Array(a) => {
            let items: Result<Vec<_>, _> = a.iter().map(json_value_to_json_val).collect();
            Ok(JsonVal::list(items?))
        }
    }
}

impl JsonVal {
    pub fn nil() -> Self {
        Self::Nil
    }
    pub fn t() -> Self {
        Self::T
    }
    pub fn int(n: i64) -> Self {
        Self::Int { int: n }
    }
    pub fn string(s: impl Into<String>) -> Self {
        Self::Str { str: s.into() }
    }
    pub fn sym(s: impl Into<String>) -> Self {
        Self::Sym { sym: s.into() }
    }
    pub fn cons(car: Self, cdr: Self) -> Self {
        Self::Cons {
            cons: Box::new((car, cdr)),
        }
    }
    pub fn list(items: Vec<Self>) -> Self {
        items
            .into_iter()
            .rev()
            .fold(Self::nil(), |acc, item| Self::cons(item, acc))
    }
}

/// Convert a `JsonVal` to a `LispObject` for evaluation by the Rust interpreter.
pub fn json_val_to_lisp(val: &JsonVal) -> LispObject {
    match val {
        JsonVal::Nil => LispObject::nil(),
        JsonVal::T => LispObject::t(),
        JsonVal::Int { int } => LispObject::Integer(*int),
        JsonVal::Str { str } => LispObject::String(str.clone()),
        JsonVal::Sym { sym } => LispObject::symbol(sym),
        JsonVal::Cons { cons } => {
            let (car, cdr) = cons.as_ref();
            LispObject::cons(json_val_to_lisp(car), json_val_to_lisp(cdr))
        }
    }
}

/// Convert a `LispObject` result back to `JsonVal` for comparison.
pub fn lisp_to_json_val(obj: &LispObject) -> JsonVal {
    match obj {
        LispObject::Nil => JsonVal::nil(),
        LispObject::T => JsonVal::t(),
        LispObject::Integer(n) => JsonVal::int(*n),
        LispObject::Float(_) => JsonVal::string("<float>"),
        LispObject::String(s) => JsonVal::string(s.as_str()),
        LispObject::Symbol(sid) => {
            let name = rele_elisp::obarray::symbol_name(*sid);
            JsonVal::sym(name)
        }
        LispObject::Cons(cell) => {
            let (car, cdr) = &*cell.lock();
            JsonVal::cons(lisp_to_json_val(car), lisp_to_json_val(cdr))
        }
        LispObject::Primitive(name) => JsonVal::sym(format!("<primitive:{name}>")),
        LispObject::Vector(v) => {
            let items: Vec<_> = v.lock().iter().map(lisp_to_json_val).collect();
            JsonVal::list(items)
        }
        LispObject::BytecodeFn(_) => JsonVal::string("<bytecode>"),
        LispObject::HashTable(_) => JsonVal::string("<hash-table>"),
        LispObject::BigInt(_) => JsonVal::string("<bigint>"),
    }
}

/// Input format sent to the Lean oracle process.
#[derive(Debug, Serialize, Deserialize)]
pub struct OracleInput {
    pub expr: JsonVal,
}

/// Output format received from the Lean oracle process.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OracleOutput {
    Ok { ok: JsonVal },
    Error { error: String },
}

// proptest strategies are in tests/differential.rs to keep this crate
// free of dev-dependency leaks.
