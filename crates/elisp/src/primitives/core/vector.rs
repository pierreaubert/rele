use crate::error::{ElispError, ElispResult};
use crate::object::{HashKey, HashTableTest, LispHashTable, LispObject};
use std::sync::Arc;

const CHAR_TABLE_MARKER: i64 = -1;
const CHAR_TABLE_DEFAULT: i64 = -2;
const CHAR_TABLE_PARENT: i64 = -3;
const CHAR_TABLE_EXTRA_BASE: i64 = -100;
const MAX_CHAR_CODE: i64 = 0x10ffff;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "aref" => Some(prim_aref(args)),
        "aset" => Some(prim_aset(args)),
        "make-vector" => Some(prim_make_vector(args)),
        "vconcat" => Some(prim_vconcat(args)),
        "vectorp" => Some(prim_vectorp(args)),
        "bool-vector" => Some(prim_bool_vector(args)),
        "make-bool-vector" => Some(prim_make_bool_vector(args)),
        _ => None,
    }
}

pub fn prim_aref(args: &LispObject) -> ElispResult<LispObject> {
    let vec = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let idx = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let idx = idx
        .as_integer()
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;

    match vec {
        LispObject::Vector(v) => {
            let guard = v.lock();
            let idx = if idx < 0 {
                guard.len() as i64 + idx
            } else {
                idx
            };
            if idx < 0 || (idx as usize) >= guard.len() {
                return Err(ElispError::EvalError(
                    "aref: index out of range".to_string(),
                ));
            }
            Ok(guard[idx as usize].clone())
        }
        LispObject::HashTable(h) if is_char_table(&vec) => {
            let guard = h.lock();
            Ok(guard
                .data
                .get(&HashKey::Integer(idx))
                .cloned()
                .or_else(|| {
                    guard
                        .data
                        .get(&HashKey::Integer(CHAR_TABLE_DEFAULT))
                        .cloned()
                })
                .unwrap_or_else(LispObject::nil))
        }
        LispObject::String(s) => {
            let idx = if idx < 0 { s.len() as i64 + idx } else { idx };
            if idx < 0 || (idx as usize) >= s.len() {
                return Err(ElispError::EvalError(
                    "aref: index out of range".to_string(),
                ));
            }
            let ch = s.chars().nth(idx as usize).unwrap_or('?');
            Ok(LispObject::integer(ch as i64))
        }
        _ => Err(ElispError::WrongTypeArgument("array".to_string())),
    }
}

pub fn prim_aset(args: &LispObject) -> ElispResult<LispObject> {
    let vec = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let idx = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let idx = idx
        .as_integer()
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;

    match vec {
        LispObject::Vector(v) => {
            let mut guard = v.lock();
            let idx = if idx < 0 {
                guard.len() as i64 + idx
            } else {
                idx
            };
            if idx < 0 || (idx as usize) >= guard.len() {
                return Err(ElispError::EvalError(
                    "aset: index out of range".to_string(),
                ));
            }
            guard[idx as usize] = val.clone();
            Ok(val)
        }
        LispObject::HashTable(h) if is_char_table(&vec) => {
            h.lock().data.insert(HashKey::Integer(idx), val.clone());
            Ok(val)
        }
        _ => Err(ElispError::WrongTypeArgument("array".to_string())),
    }
}

pub fn prim_make_vector(args: &LispObject) -> ElispResult<LispObject> {
    let length = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let init = args.nth(1).unwrap_or(LispObject::nil());
    let length = length
        .as_integer()
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;

    if length < 0 {
        return Err(ElispError::WrongTypeArgument("integer".to_string()));
    }

    let vec: Vec<LispObject> = vec![init; length as usize];
    Ok(LispObject::Vector(Arc::new(crate::eval::SyncRefCell::new(
        vec,
    ))))
}

pub fn prim_vconcat(args: &LispObject) -> ElispResult<LispObject> {
    let mut result = Vec::new();
    let mut current = args.clone();

    while let Some((arg, rest)) = current.destructure_cons() {
        match arg {
            LispObject::Vector(v) => {
                let guard = v.lock();
                result.extend(guard.iter().cloned());
            }
            LispObject::String(s) => {
                for c in s.chars() {
                    result.push(LispObject::integer(c as i64));
                }
            }
            LispObject::Nil => {}
            _ => {
                return Err(ElispError::WrongTypeArgument("sequence".to_string()));
            }
        }
        current = rest;
    }

    Ok(LispObject::Vector(Arc::new(crate::eval::SyncRefCell::new(
        result,
    ))))
}

pub fn prim_vectorp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(matches!(arg, LispObject::Vector(_))))
}

pub fn prim_make_bool_vector(args: &LispObject) -> ElispResult<LispObject> {
    let length = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let init = args.nth(1).unwrap_or(LispObject::nil());
    let length = length
        .as_integer()
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    if length < 0 {
        return Err(ElispError::WrongTypeArgument("integer".to_string()));
    }
    let val = if init.is_nil() {
        LispObject::nil()
    } else {
        LispObject::t()
    };
    let vec: Vec<LispObject> = vec![val; length as usize];
    Ok(LispObject::Vector(Arc::new(crate::eval::SyncRefCell::new(
        vec,
    ))))
}

pub fn prim_bool_vector(args: &LispObject) -> ElispResult<LispObject> {
    let mut elems = Vec::new();
    let mut cur = args.clone();
    while let Some((arg, rest)) = cur.destructure_cons() {
        elems.push(if arg.is_nil() {
            LispObject::nil()
        } else {
            LispObject::t()
        });
        cur = rest;
    }
    Ok(LispObject::Vector(Arc::new(crate::eval::SyncRefCell::new(
        elems,
    ))))
}

pub fn make_char_table(purpose: LispObject, init: LispObject) -> LispObject {
    let mut table = LispHashTable::new(HashTableTest::Eql);
    table
        .data
        .insert(HashKey::Integer(CHAR_TABLE_MARKER), purpose);
    table
        .data
        .insert(HashKey::Integer(CHAR_TABLE_DEFAULT), init);
    LispObject::HashTable(Arc::new(crate::eval::SyncRefCell::new(table)))
}

pub fn is_char_table(obj: &LispObject) -> bool {
    matches!(obj, LispObject::HashTable(h) if h.lock().data.contains_key(&HashKey::Integer(CHAR_TABLE_MARKER)))
}

pub fn char_table_set_range(
    table: &LispObject,
    range: &LispObject,
    value: LispObject,
) -> ElispResult<LispObject> {
    let LispObject::HashTable(h) = table else {
        return Ok(value);
    };
    if !is_char_table(table) {
        return Ok(value);
    }

    let mut guard = h.lock();
    if range.is_nil() {
        guard
            .data
            .insert(HashKey::Integer(CHAR_TABLE_DEFAULT), value.clone());
        return Ok(value);
    }
    if let Some(code) = range.as_integer() {
        if (0..=MAX_CHAR_CODE).contains(&code) {
            guard.data.insert(HashKey::Integer(code), value.clone());
        }
        return Ok(value);
    }
    if let Some((from, to)) = range.destructure_cons() {
        let Some(start) = from.as_integer() else {
            return Ok(value);
        };
        let Some(end) = to.as_integer() else {
            return Ok(value);
        };
        let start = start.clamp(0, MAX_CHAR_CODE);
        let end = end.clamp(0, MAX_CHAR_CODE);
        if start <= end {
            for code in start..=end {
                guard.data.insert(HashKey::Integer(code), value.clone());
            }
        }
    }
    Ok(value)
}

pub fn char_table_range(table: &LispObject, code: &LispObject) -> ElispResult<LispObject> {
    let LispObject::HashTable(h) = table else {
        return Ok(LispObject::nil());
    };
    if !is_char_table(table) {
        return Ok(LispObject::nil());
    }
    let Some(code) = code.as_integer() else {
        return Ok(LispObject::nil());
    };
    let guard = h.lock();
    Ok(guard
        .data
        .get(&HashKey::Integer(code))
        .cloned()
        .or_else(|| {
            guard
                .data
                .get(&HashKey::Integer(CHAR_TABLE_DEFAULT))
                .cloned()
        })
        .unwrap_or_else(LispObject::nil))
}

pub fn char_table_set_parent(table: &LispObject, parent: LispObject) -> ElispResult<LispObject> {
    let LispObject::HashTable(h) = table else {
        return Ok(parent);
    };
    if !is_char_table(table) {
        return Ok(parent);
    }
    h.lock()
        .data
        .insert(HashKey::Integer(CHAR_TABLE_PARENT), parent.clone());
    Ok(parent)
}

pub fn char_table_parent(table: &LispObject) -> ElispResult<LispObject> {
    let LispObject::HashTable(h) = table else {
        return Ok(LispObject::nil());
    };
    if !is_char_table(table) {
        return Ok(LispObject::nil());
    }
    Ok(h.lock()
        .data
        .get(&HashKey::Integer(CHAR_TABLE_PARENT))
        .cloned()
        .unwrap_or_else(LispObject::nil))
}

pub fn char_table_set_extra_slot(
    table: &LispObject,
    slot: &LispObject,
    value: LispObject,
) -> ElispResult<LispObject> {
    let LispObject::HashTable(h) = table else {
        return Ok(value);
    };
    if !is_char_table(table) {
        return Ok(value);
    }
    let slot = slot.as_integer().unwrap_or(0);
    h.lock().data.insert(
        HashKey::Integer(CHAR_TABLE_EXTRA_BASE - slot),
        value.clone(),
    );
    Ok(value)
}

pub fn char_table_extra_slot(table: &LispObject, slot: &LispObject) -> ElispResult<LispObject> {
    let LispObject::HashTable(h) = table else {
        return Ok(LispObject::nil());
    };
    if !is_char_table(table) {
        return Ok(LispObject::nil());
    }
    let slot = slot.as_integer().unwrap_or(0);
    Ok(h.lock()
        .data
        .get(&HashKey::Integer(CHAR_TABLE_EXTRA_BASE - slot))
        .cloned()
        .unwrap_or_else(LispObject::nil))
}
