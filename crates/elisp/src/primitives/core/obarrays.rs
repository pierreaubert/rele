//! Obarray primitives. The default obarray is the process-wide
//! `GLOBAL_OBARRAY`; user-created obarrays are modelled as Lisp
//! hash-tables (`LispObject::HashTable`).

use std::sync::Arc;

use crate::error::ElispResult;
use crate::eval::SyncRefCell;
use crate::object::{HashKey, HashTableTest, LispHashTable, LispObject};

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "obarray-make",
        "obarray-get",
        "obarray-put",
        "obarray-clear",
        "unintern",
        "internal--obarray-buckets",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        "obarray-make" => Ok(prim_obarray_make(args)),
        "obarray-get" => Ok(prim_obarray_get(args)),
        "obarray-put" => Ok(prim_obarray_put(args)),
        "obarray-clear" => Ok(prim_obarray_clear(args)),
        "unintern" => Ok(LispObject::nil()),
        "internal--obarray-buckets" => Ok(prim_internal_obarray_buckets(args)),
        _ => return None,
    };
    Some(result)
}

fn make_empty_table() -> LispObject {
    LispObject::HashTable(Arc::new(SyncRefCell::new(LispHashTable {
        test: HashTableTest::Equal,
        data: std::collections::HashMap::new(),
    })))
}

fn prim_obarray_make(_args: &LispObject) -> LispObject {
    make_empty_table()
}

fn key_for(value: &LispObject) -> HashKey {
    if let Some(s) = value.as_string() {
        return HashKey::String(s.to_string());
    }
    if let Some(name) = value.as_symbol() {
        return HashKey::String(name.to_string());
    }
    HashKey::Printed(value.princ_to_string())
}

fn prim_obarray_get(args: &LispObject) -> LispObject {
    let table = args.first().unwrap_or_else(LispObject::nil);
    let name = args.nth(1).unwrap_or_else(LispObject::nil);
    if let LispObject::HashTable(h) = table {
        let key = key_for(&name);
        return h
            .lock()
            .data
            .get(&key)
            .cloned()
            .unwrap_or_else(LispObject::nil);
    }
    LispObject::nil()
}

fn prim_obarray_put(args: &LispObject) -> LispObject {
    let table = args.first().unwrap_or_else(LispObject::nil);
    let name = args.nth(1).unwrap_or_else(LispObject::nil);
    let value = args.nth(2).unwrap_or_else(LispObject::nil);
    if let LispObject::HashTable(h) = table {
        let key = key_for(&name);
        h.lock().data.insert(key, value.clone());
    }
    value
}

fn prim_obarray_clear(args: &LispObject) -> LispObject {
    let table = args.first().unwrap_or_else(LispObject::nil);
    if let LispObject::HashTable(h) = table {
        h.lock().data.clear();
    }
    LispObject::nil()
}

fn prim_internal_obarray_buckets(args: &LispObject) -> LispObject {
    let table = args.first().unwrap_or_else(LispObject::nil);
    if let LispObject::HashTable(h) = table {
        let len = h.lock().data.len() as i64;
        return LispObject::integer(len.max(1));
    }
    let count = crate::obarray::GLOBAL_OBARRAY.read().symbol_count() as i64;
    LispObject::integer(count.max(1))
}
