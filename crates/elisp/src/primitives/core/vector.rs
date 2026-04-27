use crate::error::{ElispError, ElispResult};
use crate::object::{HashKey, HashTableTest, LispHashTable, LispObject};
use std::sync::Arc;

const CHAR_TABLE_MARKER: i64 = -1;
const CHAR_TABLE_DEFAULT: i64 = -2;
const CHAR_TABLE_PARENT: i64 = -3;
const CHAR_TABLE_EXTRA_BASE: i64 = -100;
const BOOL_VECTOR_MARKER: i64 = -10_000;
const BOOL_VECTOR_LENGTH: i64 = -10_001;
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
        "bool-vector-count-population" => Some(prim_bool_vector_count_population(args)),
        "bool-vector-count-consecutive" => Some(prim_bool_vector_count_consecutive(args)),
        "bool-vector-not" => Some(prim_bool_vector_not(args)),
        "bool-vector-subsetp" => Some(prim_bool_vector_subsetp(args)),
        "bool-vector-union" => Some(prim_bool_vector_union(args)),
        "bool-vector-intersection" => Some(prim_bool_vector_intersection(args)),
        "bool-vector-exclusive-or" => Some(prim_bool_vector_exclusive_or(args)),
        "bool-vector-set-difference" => Some(prim_bool_vector_set_difference(args)),
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
        LispObject::HashTable(_) if is_bool_vector(&vec) => {
            let len = bool_vector_len(&vec).unwrap_or(0);
            let idx = if idx < 0 { len as i64 + idx } else { idx };
            if idx < 0 || (idx as usize) >= len {
                return Err(ElispError::EvalError(
                    "aref: index out of range".to_string(),
                ));
            }
            Ok(LispObject::from(bool_vector_bit(&vec, idx as usize)))
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
            let len = s.chars().count();
            let idx = if idx < 0 { len as i64 + idx } else { idx };
            if idx < 0 || (idx as usize) >= len {
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
        LispObject::HashTable(_) if is_bool_vector(&vec) => {
            let len = bool_vector_len(&vec).unwrap_or(0);
            let idx = if idx < 0 { len as i64 + idx } else { idx };
            if idx < 0 || (idx as usize) >= len {
                return Err(ElispError::EvalError(
                    "aset: index out of range".to_string(),
                ));
            }
            bool_vector_set(&vec, idx as usize, !val.is_nil());
            Ok(val)
        }
        LispObject::HashTable(h) if is_char_table(&vec) => {
            h.lock().data.insert(HashKey::Integer(idx), val.clone());
            Ok(val)
        }
        LispObject::String(s) => {
            let current = crate::object::current_string_value(&s);
            let len = current.chars().count();
            let idx = if idx < 0 { len as i64 + idx } else { idx };
            if idx < 0 || (idx as usize) >= len {
                return Err(ElispError::EvalError(
                    "aset: index out of range".to_string(),
                ));
            }
            let code = val
                .as_integer()
                .ok_or_else(|| ElispError::WrongTypeArgument("character".to_string()))?;
            let is_multibyte = crate::object::string_is_multibyte(&s);
            let old = current.chars().nth(idx as usize).unwrap_or('\0');
            if is_multibyte && (!old.is_ascii() || !(0..=0x7f).contains(&code)) {
                return Err(ElispError::WrongTypeArgument("character".to_string()));
            }
            if !is_multibyte && !(0..=0xff).contains(&code) {
                return Err(ElispError::WrongTypeArgument("character".to_string()));
            }
            let ch = char::from_u32(code as u32)
                .ok_or_else(|| ElispError::WrongTypeArgument("character".to_string()))?;
            let mut chars: Vec<char> = current.chars().collect();
            chars[idx as usize] = ch;
            crate::object::mutate_string_value(&s, chars.into_iter().collect());
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
    Ok(make_bool_vector(length as usize, !init.is_nil()))
}

pub fn prim_bool_vector(args: &LispObject) -> ElispResult<LispObject> {
    let mut bits = Vec::new();
    let mut cur = args.clone();
    while let Some((arg, rest)) = cur.destructure_cons() {
        bits.push(!arg.is_nil());
        cur = rest;
    }
    Ok(bool_vector_from_bits(bits))
}

pub fn prim_bool_vector_count_population(args: &LispObject) -> ElispResult<LispObject> {
    let bv = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    require_bool_vector(&bv)?;
    Ok(LispObject::integer(bool_vector_true_count(&bv) as i64))
}

pub fn prim_bool_vector_count_consecutive(args: &LispObject) -> ElispResult<LispObject> {
    let bv = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let bit = !args.nth(1).unwrap_or_else(LispObject::nil).is_nil();
    let start = args
        .nth(2)
        .and_then(|obj| obj.as_integer())
        .unwrap_or(0)
        .max(0) as usize;
    require_bool_vector(&bv)?;
    let len = bool_vector_len(&bv).unwrap_or(0);
    let mut count = 0usize;
    for idx in start..len {
        if bool_vector_bit(&bv, idx) == bit {
            count += 1;
        } else {
            break;
        }
    }
    Ok(LispObject::integer(count as i64))
}

pub fn prim_bool_vector_not(args: &LispObject) -> ElispResult<LispObject> {
    let bv = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    require_bool_vector(&bv)?;
    let len = bool_vector_len(&bv).unwrap_or(0);
    let bits: Vec<bool> = (0..len).map(|idx| !bool_vector_bit(&bv, idx)).collect();
    Ok(bool_vector_from_bits(bits))
}

pub fn prim_bool_vector_subsetp(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    require_same_len_bool_vectors(&a, &b)?;
    let len = bool_vector_len(&a).unwrap_or(0);
    let subset = (0..len).all(|idx| !bool_vector_bit(&a, idx) || bool_vector_bit(&b, idx));
    Ok(LispObject::from(subset))
}

pub fn prim_bool_vector_union(args: &LispObject) -> ElispResult<LispObject> {
    bool_vector_binop(args, |a, b| a || b)
}

pub fn prim_bool_vector_intersection(args: &LispObject) -> ElispResult<LispObject> {
    bool_vector_binop(args, |a, b| a && b)
}

pub fn prim_bool_vector_exclusive_or(args: &LispObject) -> ElispResult<LispObject> {
    bool_vector_binop(args, |a, b| a ^ b)
}

pub fn prim_bool_vector_set_difference(args: &LispObject) -> ElispResult<LispObject> {
    bool_vector_binop(args, |a, b| a && !b)
}

fn bool_vector_binop(
    args: &LispObject,
    op: impl Fn(bool, bool) -> bool,
) -> ElispResult<LispObject> {
    let left = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let right = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    require_same_len_bool_vectors(&left, &right)?;
    let len = bool_vector_len(&left).unwrap_or(0);
    let bits: Vec<bool> = (0..len)
        .map(|idx| op(bool_vector_bit(&left, idx), bool_vector_bit(&right, idx)))
        .collect();

    if let Some(target) = args.nth(2) {
        if target.is_nil() {
            return Ok(bool_vector_from_bits(bits));
        }
        require_bool_vector(&target)?;
        if bool_vector_len(&target).unwrap_or(0) != len {
            return Err(ElispError::WrongTypeArgument("bool-vector".to_string()));
        }
        let mut changed = false;
        for (idx, bit) in bits.into_iter().enumerate() {
            if bool_vector_bit(&target, idx) != bit {
                bool_vector_set(&target, idx, bit);
                changed = true;
            }
        }
        Ok(if changed { target } else { LispObject::nil() })
    } else {
        Ok(bool_vector_from_bits(bits))
    }
}

pub fn is_bool_vector(obj: &LispObject) -> bool {
    matches!(obj, LispObject::HashTable(h) if h.lock().data.contains_key(&HashKey::Integer(BOOL_VECTOR_MARKER)))
}

pub fn bool_vector_to_list(obj: &LispObject) -> Option<LispObject> {
    if !is_bool_vector(obj) {
        return None;
    }
    let len = bool_vector_len(obj)?;
    let mut out = LispObject::nil();
    for idx in (0..len).rev() {
        out = LispObject::cons(LispObject::from(bool_vector_bit(obj, idx)), out);
    }
    Some(out)
}

pub fn bool_vector_length(obj: &LispObject) -> Option<usize> {
    if is_bool_vector(obj) {
        bool_vector_len(obj)
    } else {
        None
    }
}

fn make_bool_vector(length: usize, init: bool) -> LispObject {
    let bits = vec![init; length];
    bool_vector_from_bits(bits)
}

fn bool_vector_from_bits<I>(bits: I) -> LispObject
where
    I: IntoIterator<Item = bool>,
{
    let mut table = LispHashTable::new(HashTableTest::Eql);
    table
        .data
        .insert(HashKey::Integer(BOOL_VECTOR_MARKER), LispObject::t());
    let mut len = 0usize;
    for (idx, bit) in bits.into_iter().enumerate() {
        len = idx + 1;
        if bit {
            table
                .data
                .insert(HashKey::Integer(idx as i64), LispObject::t());
        }
    }
    table.data.insert(
        HashKey::Integer(BOOL_VECTOR_LENGTH),
        LispObject::integer(len as i64),
    );
    LispObject::HashTable(Arc::new(crate::eval::SyncRefCell::new(table)))
}

fn require_bool_vector(obj: &LispObject) -> ElispResult<()> {
    if is_bool_vector(obj) {
        Ok(())
    } else {
        Err(ElispError::WrongTypeArgument("bool-vector".to_string()))
    }
}

fn require_same_len_bool_vectors(left: &LispObject, right: &LispObject) -> ElispResult<()> {
    require_bool_vector(left)?;
    require_bool_vector(right)?;
    if bool_vector_len(left) == bool_vector_len(right) {
        Ok(())
    } else {
        Err(ElispError::WrongTypeArgument("bool-vector".to_string()))
    }
}

fn bool_vector_len(obj: &LispObject) -> Option<usize> {
    let LispObject::HashTable(h) = obj else {
        return None;
    };
    h.lock()
        .data
        .get(&HashKey::Integer(BOOL_VECTOR_LENGTH))
        .and_then(LispObject::as_integer)
        .map(|n| n.max(0) as usize)
}

fn bool_vector_bit(obj: &LispObject, idx: usize) -> bool {
    let LispObject::HashTable(h) = obj else {
        return false;
    };
    h.lock()
        .data
        .get(&HashKey::Integer(idx as i64))
        .is_some_and(|value| !value.is_nil())
}

fn bool_vector_set(obj: &LispObject, idx: usize, bit: bool) {
    if let LispObject::HashTable(h) = obj {
        let mut guard = h.lock();
        if bit {
            guard
                .data
                .insert(HashKey::Integer(idx as i64), LispObject::t());
        } else {
            guard.data.remove(&HashKey::Integer(idx as i64));
        }
    }
}

fn bool_vector_true_count(obj: &LispObject) -> usize {
    let len = bool_vector_len(obj).unwrap_or(0);
    (0..len).filter(|&idx| bool_vector_bit(obj, idx)).count()
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
    let guard = h.lock();
    let default = || {
        guard
            .data
            .get(&HashKey::Integer(CHAR_TABLE_DEFAULT))
            .cloned()
            .unwrap_or_else(LispObject::nil)
    };
    if code.is_nil() {
        return Ok(default());
    }
    if let Some(code) = code.as_integer() {
        return Ok(guard
            .data
            .get(&HashKey::Integer(code))
            .cloned()
            .unwrap_or_else(default));
    }
    if let Some((from, to)) = code.destructure_cons() {
        let Some(start) = from.as_integer() else {
            return Ok(LispObject::nil());
        };
        let Some(end) = to.as_integer() else {
            return Ok(LispObject::nil());
        };
        let start = start.clamp(0, MAX_CHAR_CODE);
        let end = end.clamp(0, MAX_CHAR_CODE);
        if start > end {
            return Ok(LispObject::nil());
        }
        // Return the common value if every code in [start..=end] maps to
        // the same value; otherwise nil. Matches Emacs's behavior of
        // returning the uniform value when the range was set with
        // set-char-table-range.
        let first = guard
            .data
            .get(&HashKey::Integer(start))
            .cloned()
            .unwrap_or_else(default);
        for code in (start + 1)..=end {
            let v = guard
                .data
                .get(&HashKey::Integer(code))
                .cloned()
                .unwrap_or_else(default);
            if !v.eq(&first) {
                return Ok(LispObject::nil());
            }
        }
        return Ok(first);
    }
    Ok(LispObject::nil())
}

pub fn char_table_subtype(table: &LispObject) -> LispObject {
    let LispObject::HashTable(h) = table else {
        return LispObject::nil();
    };
    if !is_char_table(table) {
        return LispObject::nil();
    }
    h.lock()
        .data
        .get(&HashKey::Integer(CHAR_TABLE_MARKER))
        .cloned()
        .unwrap_or_else(LispObject::nil)
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
