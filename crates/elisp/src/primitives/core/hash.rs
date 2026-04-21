use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "hash-table-keys" => Some(prim_hash_table_keys(args)),
        "hash-table-values" => Some(prim_hash_table_values(args)),
        "remhash" => Some(prim_remhash(args)),
        "hash-table-count" => Some(prim_hash_table_count(args)),
        "hash-table-test" => Some(prim_hash_table_test(args)),
        "hash-table-size" => Some(prim_hash_table_size(args)),
        "hash-table-weakness" => Some(Ok(LispObject::nil())),
        "copy-hash-table" => Some(prim_copy_hash_table(args)),
        "sxhash-eq" | "sxhash-eql" | "sxhash-equal" | "sxhash-equal-including-properties" => {
            Some(prim_sxhash(args))
        }
        _ => None,
    }
}

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for &name in HASH_PRIMITIVE_NAMES {
        interp.define(name, LispObject::primitive(name));
    }
}

pub const HASH_PRIMITIVE_NAMES: &[&str] = &[
    "hash-table-keys",
    "hash-table-values",
    "remhash",
    "hash-table-count",
    "hash-table-test",
    "hash-table-size",
    "hash-table-weakness",
    "copy-hash-table",
    "sxhash-eq",
    "sxhash-eql",
    "sxhash-equal",
    "sxhash-equal-including-properties",
];

fn prim_hash_table_keys(args: &LispObject) -> ElispResult<LispObject> {
    let ht = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match ht {
        LispObject::HashTable(h) => {
            let mut result = LispObject::nil();
            for key in h.lock().data.keys() {
                let key_obj = match key {
                    crate::object::HashKey::Symbol(id) => LispObject::Symbol(*id),
                    crate::object::HashKey::Integer(i) => LispObject::integer(*i),
                    crate::object::HashKey::String(s) => LispObject::string(s),
                    crate::object::HashKey::Printed(s) => LispObject::string(s),
                    crate::object::HashKey::Identity(_) => LispObject::nil(),
                };
                result = LispObject::cons(key_obj, result);
            }
            Ok(result)
        }
        _ => Err(ElispError::WrongTypeArgument("hash-table".to_string())),
    }
}

fn prim_hash_table_values(args: &LispObject) -> ElispResult<LispObject> {
    let ht = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match ht {
        LispObject::HashTable(h) => {
            let mut result = LispObject::nil();
            for val in h.lock().data.values() {
                result = LispObject::cons(val.clone(), result);
            }
            Ok(result)
        }
        _ => Err(ElispError::WrongTypeArgument("hash-table".to_string())),
    }
}

fn prim_remhash(args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let table = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if let LispObject::HashTable(h) = table {
        let mut guard = h.lock();
        let hk = guard.make_key(&key);
        guard.data.remove(&hk);
        drop(guard);
        Ok(LispObject::nil())
    } else {
        Err(ElispError::WrongTypeArgument("hash-table".to_string()))
    }
}

fn prim_hash_table_count(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let LispObject::HashTable(h) = table {
        Ok(LispObject::integer(h.lock().data.len() as i64))
    } else {
        Err(ElispError::WrongTypeArgument("hash-table".to_string()))
    }
}

fn prim_hash_table_test(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let LispObject::HashTable(h) = table {
        let test_name = match h.lock().test {
            crate::object::HashTableTest::Eq => "eq",
            crate::object::HashTableTest::Eql => "eql",
            crate::object::HashTableTest::Equal => "equal",
        };
        Ok(LispObject::symbol(test_name))
    } else {
        Err(ElispError::WrongTypeArgument("hash-table".to_string()))
    }
}

fn prim_hash_table_size(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let LispObject::HashTable(h) = table {
        Ok(LispObject::integer(h.lock().data.len() as i64))
    } else {
        Err(ElispError::WrongTypeArgument("hash-table".to_string()))
    }
}

fn prim_copy_hash_table(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let LispObject::HashTable(h) = table {
        let guard = h.lock();
        let new_ht = crate::object::LispHashTable {
            data: guard.data.clone(),
            test: guard.test.clone(),
        };
        Ok(LispObject::HashTable(std::sync::Arc::new(
            parking_lot::Mutex::new(new_ht),
        )))
    } else {
        Err(ElispError::WrongTypeArgument("hash-table".to_string()))
    }
}

fn prim_sxhash(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().unwrap_or_else(LispObject::nil);
    let mut hasher = DefaultHasher::new();
    hash_lisp(&obj, &mut hasher);
    let h = hasher.finish() as i64;
    Ok(LispObject::integer(h & 0x3fff_ffff))
}

fn hash_lisp(obj: &LispObject, hasher: &mut DefaultHasher) {
    match obj {
        LispObject::Nil => 0u8.hash(hasher),
        LispObject::T => 1u8.hash(hasher),
        LispObject::Integer(i) => i.hash(hasher),
        LispObject::Float(f) => f.to_bits().hash(hasher),
        LispObject::String(s) => s.hash(hasher),
        LispObject::Symbol(id) => id.0.hash(hasher),
        _ => 42u8.hash(hasher),
    }
}
