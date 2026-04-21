use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "elt" => Some(prim_elt(args)),
        "copy-alist" => Some(prim_copy_alist(args)),
        "plist-get" => Some(prim_plist_get(args)),
        "plist-put" => Some(prim_plist_put(args)),
        "plist-member" => Some(prim_plist_member(args)),
        "remove" => Some(prim_remove(args)),
        "remq" => Some(prim_remq(args)),
        "number-sequence" => Some(prim_number_sequence(args)),
        "proper-list-p" => Some(prim_proper_list_p(args)),
        "delete" => Some(prim_delete(args)),
        "rassq" => Some(prim_rassq(args)),
        "rassoc" => Some(prim_rassoc(args)),
        "take" => Some(prim_take(args)),
        "ntake" => Some(prim_take(args)),
        "length<" => Some(prim_length_lt(args)),
        "length>" => Some(prim_length_gt(args)),
        "length=" => Some(prim_length_eq(args)),
        "fillarray" => Some(prim_fillarray(args)),
        "memql" => Some(prim_memql(args)),
        _ => None,
    }
}

pub fn prim_elt(args: &LispObject) -> ElispResult<LispObject> {
    let seq = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let idx = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let idx = idx.as_integer().ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;

    match seq {
        LispObject::Nil => Err(ElispError::WrongTypeArgument("sequence".to_string())),
        LispObject::Cons(_) => {
            let mut current = seq;
            let mut i: i64 = 0;
            while let Some((car, cdr)) = current.destructure_cons() {
                if i == idx {
                    return Ok(car);
                }
                i += 1;
                current = cdr;
            }
            Err(ElispError::EvalError("elt: index out of range".to_string()))
        }
        LispObject::Vector(v) => {
            let guard = v.lock();
            if idx < 0 || (idx as usize) >= guard.len() {
                return Err(ElispError::EvalError("elt: index out of range".to_string()));
            }
            Ok(guard[idx as usize].clone())
        }
        LispObject::String(s) => {
            let chars: Vec<char> = s.chars().collect();
            if idx < 0 || (idx as usize) >= chars.len() {
                return Err(ElispError::EvalError("elt: index out of range".to_string()));
            }
            Ok(LispObject::integer(chars[idx as usize] as i64))
        }
        _ => Err(ElispError::WrongTypeArgument("sequence".to_string())),
    }
}

pub fn prim_copy_alist(args: &LispObject) -> ElispResult<LispObject> {
    let alist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut result = LispObject::nil();
    let mut current = alist;

    while let Some((entry, cdr)) = current.destructure_cons() {
        if let Some((key, val)) = entry.destructure_cons() {
            result = LispObject::cons(
                LispObject::cons(key.clone(), val.clone()),
                result,
            );
        }
        current = cdr;
    }

    let mut reversed = LispObject::nil();
    while let Some((entry, cdr)) = result.destructure_cons() {
        reversed = LispObject::cons(entry, reversed);
        result = cdr;
    }
    Ok(reversed)
}

pub fn prim_plist_get(args: &LispObject) -> ElispResult<LispObject> {
    let plist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let key = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = plist;

    while let Some((k, cdr)) = current.destructure_cons() {
        if let Some((v, rest)) = cdr.destructure_cons() {
            if eq_test(&k, &key) {
                return Ok(v);
            }
            current = rest;
        } else {
            break;
        }
    }
    Ok(LispObject::nil())
}

pub fn prim_plist_put(args: &LispObject) -> ElispResult<LispObject> {
    let plist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut items = Vec::new();
    let mut current = plist.clone();
    let mut found = false;
    while let Some((key, rest)) = current.destructure_cons() {
        if let Some((val, next)) = rest.destructure_cons() {
            if eq_test(&key, &prop) {
                items.push((key, value.clone()));
                found = true;
                current = next;
            } else {
                items.push((key, val));
                current = next;
            }
        } else {
            break;
        }
    }
    if !found {
        items.push((prop, value));
    }
    let mut result = LispObject::nil();
    for (k, v) in items.into_iter().rev() {
        result = LispObject::cons(v, result);
        result = LispObject::cons(k, result);
    }
    Ok(result)
}

pub fn prim_plist_member(args: &LispObject) -> ElispResult<LispObject> {
    let plist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = plist;
    while let Some((key, _rest)) = current.destructure_cons() {
        if eq_test(&key, &prop) {
            return Ok(current);
        }
        match _rest.destructure_cons() {
            Some((_, next)) => current = next,
            None => return Ok(LispObject::nil()),
        }
    }
    Ok(LispObject::nil())
}

pub fn prim_remove(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut items = Vec::new();
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        if car != elt {
            items.push(car);
        }
        current = cdr;
    }
    let mut result = LispObject::nil();
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

pub fn prim_remq(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut items = Vec::new();
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        if !eq_test(&elt, &car) {
            items.push(car);
        }
        current = cdr;
    }
    let mut result = LispObject::nil();
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

pub fn prim_number_sequence(args: &LispObject) -> ElispResult<LispObject> {
    let from = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let to = args.nth(1).and_then(|a| a.as_integer());
    let step = args.nth(2).and_then(|a| a.as_integer());

    let to = match to {
        Some(t) => t,
        None => {
            return Ok(LispObject::cons(
                LispObject::integer(from),
                LispObject::nil(),
            ))
        }
    };
    let step = step.unwrap_or(if from <= to { 1 } else { -1 });
    if step == 0 {
        return Err(ElispError::EvalError("number-sequence: step must be non-zero".to_string()));
    }
    let mut items = Vec::new();
    let mut i = from;
    if step > 0 {
        while i <= to {
            items.push(LispObject::integer(i));
            i += step;
        }
    } else {
        while i >= to {
            items.push(LispObject::integer(i));
            i += step;
        }
    }
    let mut result = LispObject::nil();
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

pub fn prim_proper_list_p(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if arg.is_nil() {
        return Ok(LispObject::t());
    }
    let mut slow = arg.clone();
    let mut fast = arg.clone();
    let mut count: u64 = 0;
    loop {
        let Some((_, cdr_fast)) = fast.destructure_cons() else {
            return Ok(LispObject::from(fast.is_nil()));
        };
        let Some((_, cdr_fast2)) = cdr_fast.destructure_cons() else {
            return Ok(LispObject::from(cdr_fast.is_nil()));
        };
        fast = cdr_fast2;
        slow = slow.destructure_cons().map(|(_, c)| c).unwrap_or_else(LispObject::nil);
        count += 1;
        if count > (1 << 24) {
            return Ok(LispObject::nil());
        }
        if eq_test(&slow, &fast) {
            return Ok(LispObject::nil());
        }
    }
}

pub fn prim_delete(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut items = Vec::new();
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        if car != elt {
            items.push(car);
        }
        current = cdr;
    }
    let mut result = LispObject::nil();
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

pub fn prim_rassq(args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let alist = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = alist;
    while let Some((entry, rest)) = current.destructure_cons() {
        if let Some((_, val)) = entry.destructure_cons() {
            if eq_test(&val, &key) {
                return Ok(entry);
            }
        }
        current = rest;
    }
    Ok(LispObject::nil())
}

pub fn prim_rassoc(args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let alist = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = alist;
    while let Some((entry, rest)) = current.destructure_cons() {
        if let Some((_, val)) = entry.destructure_cons() {
            if val == key {
                return Ok(entry);
            }
        }
        current = rest;
    }
    Ok(LispObject::nil())
}

pub fn prim_take(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if n <= 0 {
        return Ok(LispObject::nil());
    }
    let mut items = Vec::new();
    let mut current = list;
    let mut remaining = n;
    while remaining > 0 {
        if let Some((car, cdr)) = current.destructure_cons() {
            items.push(car);
            current = cdr;
            remaining -= 1;
        } else {
            break;
        }
    }
    let mut result = LispObject::nil();
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

fn seq_length(obj: &LispObject) -> ElispResult<i64> {
    match obj {
        LispObject::Nil => Ok(0),
        LispObject::Cons(_) => {
            let mut count: i64 = 0;
            let mut current = obj.clone();
            while current.destructure_cons().is_some() {
                count += 1;
                current = current.destructure_cons().unwrap().1;
            }
            Ok(count)
        }
        LispObject::Vector(v) => Ok(v.lock().len() as i64),
        LispObject::String(s) => Ok(s.chars().count() as i64),
        _ => Err(ElispError::WrongTypeArgument("sequence".to_string())),
    }
}

pub fn prim_length_lt(args: &LispObject) -> ElispResult<LispObject> {
    let seq = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = args.nth(1).and_then(|a| a.as_integer()).ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(seq_length(&seq)? < n))
}

pub fn prim_length_gt(args: &LispObject) -> ElispResult<LispObject> {
    let seq = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = args.nth(1).and_then(|a| a.as_integer()).ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(seq_length(&seq)? > n))
}

pub fn prim_length_eq(args: &LispObject) -> ElispResult<LispObject> {
    let seq = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = args.nth(1).and_then(|a| a.as_integer()).ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(seq_length(&seq)? == n))
}

pub fn prim_fillarray(args: &LispObject) -> ElispResult<LispObject> {
    let array = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let item = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if let LispObject::Vector(v) = &array {
        let mut guard = v.lock();
        for el in guard.iter_mut() {
            *el = item.clone();
        }
        Ok(array)
    } else {
        Err(ElispError::WrongTypeArgument("array".to_string()))
    }
}

pub fn prim_memql(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        let equal = match (&elt, &car) {
            (LispObject::Nil, LispObject::Nil) => true,
            (LispObject::T, LispObject::T) => true,
            (LispObject::Symbol(a), LispObject::Symbol(b)) => a == b,
            (LispObject::Integer(a), LispObject::Integer(b)) => a == b,
            (LispObject::Float(a), LispObject::Float(b)) => a.to_bits() == b.to_bits(),
            _ => false,
        };
        if equal {
            return Ok(current);
        }
        current = cdr;
    }
    Ok(LispObject::nil())
}

fn eq_test(a: &LispObject, b: &LispObject) -> bool {
    match (a, b) {
        (LispObject::Nil, LispObject::Nil) => true,
        (LispObject::T, LispObject::T) => true,
        (LispObject::Integer(x), LispObject::Integer(y)) => x == y,
        (LispObject::Symbol(x), LispObject::Symbol(y)) => x == y,
        _ => false,
    }
}