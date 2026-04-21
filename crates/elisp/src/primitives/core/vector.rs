use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use std::sync::Arc;

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
    let idx = idx.as_integer().ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;

    match vec {
        LispObject::Vector(v) => {
            let guard = v.lock();
            let idx = if idx < 0 { (guard.len() as i64 + idx) } else { idx };
            if idx < 0 || (idx as usize) >= guard.len() {
                return Err(ElispError::EvalError("aref: index out of range".to_string()));
            }
            Ok(guard[idx as usize].clone())
        }
        LispObject::String(s) => {
            let idx = if idx < 0 { (s.len() as i64 + idx) } else { idx };
            if idx < 0 || (idx as usize) >= s.len() {
                return Err(ElispError::EvalError("aref: index out of range".to_string()));
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
    let idx = idx.as_integer().ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;

    match vec {
        LispObject::Vector(v) => {
            let mut guard = v.lock();
            let idx = if idx < 0 { (guard.len() as i64 + idx) } else { idx };
            if idx < 0 || (idx as usize) >= guard.len() {
                return Err(ElispError::EvalError("aset: index out of range".to_string()));
            }
            guard[idx as usize] = val.clone();
            Ok(val)
        }
        _ => Err(ElispError::WrongTypeArgument("array".to_string())),
    }
}

pub fn prim_make_vector(args: &LispObject) -> ElispResult<LispObject> {
    let length = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let init = args.nth(1).unwrap_or(LispObject::nil());
    let length = length.as_integer().ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;

    if length < 0 {
        return Err(ElispError::WrongTypeArgument("integer".to_string()));
    }

    let vec: Vec<LispObject> = vec![init; length as usize];
    Ok(LispObject::Vector(Arc::new(parking_lot::Mutex::new(vec))))
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

    Ok(LispObject::Vector(Arc::new(parking_lot::Mutex::new(result))))
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
    Ok(LispObject::Vector(Arc::new(parking_lot::Mutex::new(vec))))
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
    Ok(LispObject::Vector(Arc::new(parking_lot::Mutex::new(elems))))
}