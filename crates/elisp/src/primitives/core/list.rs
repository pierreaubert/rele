use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "cons" => Some(prim_cons(args)),
        "car" => Some(prim_car(args)),
        "cdr" => Some(prim_cdr(args)),
        "list" => Some(prim_list(args)),
        "length" => Some(prim_length(args)),
        "append" => Some(prim_append(args)),
        "reverse" => Some(prim_reverse(args)),
        "member" => Some(prim_member(args)),
        "assoc" => Some(prim_assoc(args)),
        "nth" => Some(prim_nth(args)),
        "nthcdr" => Some(prim_nthcdr(args)),
        "setcar" => Some(prim_setcar(args)),
        "setcdr" => Some(prim_setcdr(args)),
        "nconc" => Some(prim_nconc(args)),
        "nreverse" => Some(prim_nreverse(args)),
        "delq" => Some(prim_delq(args)),
        "memq" => Some(prim_memq(args)),
        "assq" => Some(prim_assq(args)),
        "last" => Some(prim_last(args)),
        "copy-sequence" => Some(prim_copy_sequence(args)),
        "cadr" => Some(prim_cadr(args)),
        "cddr" => Some(prim_cddr(args)),
        "caar" => Some(prim_caar(args)),
        "cdar" => Some(prim_cdar(args)),
        "car-safe" => Some(prim_car_safe(args)),
        "cdr-safe" => Some(prim_cdr_safe(args)),
        "make-list" => Some(prim_make_list(args)),
        _ => None,
    }
}

pub const LIST_PRIMITIVE_NAMES: &[&str] = &[
    "cons", "car", "cdr", "list", "length", "append", "reverse", "member", "assoc",
    "nth", "nthcdr", "setcar", "setcdr", "nconc", "nreverse", "delq", "memq", "assq",
    "last", "copy-sequence", "cadr", "cddr", "caar", "cdar", "car-safe", "cdr-safe",
    "make-list",
];

const MAX_LIST_WALK: u64 = 1 << 24;

pub fn prim_cons(args: &LispObject) -> ElispResult<LispObject> {
    let car = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let cdr = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::cons(car, cdr))
}

pub fn prim_car(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.clone().first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Nil => Ok(LispObject::nil()),
        LispObject::Cons(cell) => Ok(cell.lock().0.clone()),
        _ => Err(ElispError::WrongTypeArgument("list".to_string())),
    }
}

pub fn prim_cdr(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.clone().first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Nil => Ok(LispObject::nil()),
        LispObject::Cons(cell) => Ok(cell.lock().1.clone()),
        _ => Err(ElispError::WrongTypeArgument("list".to_string())),
    }
}

pub fn prim_list(args: &LispObject) -> ElispResult<LispObject> {
    Ok(args.clone())
}

pub fn prim_length(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.clone().first().ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::Nil => Ok(LispObject::integer(0)),
        LispObject::Cons(_) => {
            let mut count: u64 = 0;
            let mut current = arg.clone();
            while let Some((_, rest)) = current.destructure_cons() {
                count += 1;
                if count > MAX_LIST_WALK {
                    return Err(ElispError::EvalError(
                        "length: list too long or circular".to_string(),
                    ));
                }
                current = rest;
            }
            Ok(LispObject::integer(count as i64))
        }
        LispObject::String(s) => Ok(LispObject::integer(s.chars().count() as i64)),
        LispObject::Vector(v) => Ok(LispObject::integer(v.lock().len() as i64)),
        LispObject::HashTable(ht) => Ok(LispObject::integer(ht.lock().data.len() as i64)),
        _ => Err(ElispError::WrongTypeArgument(
            "sequence (list, string, vector)".to_string(),
        )),
    }
}

pub fn prim_append(args: &LispObject) -> ElispResult<LispObject> {
    let mut all_args = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        all_args.push(arg);
        current = rest;
    }
    if all_args.is_empty() {
        return Ok(LispObject::nil());
    }
    let tail = all_args.pop().unwrap();
    let mut items = Vec::new();
    for arg in &all_args {
        let mut cur = arg.clone();
        let mut steps: u64 = 0;
        while let Some((car, cdr)) = cur.destructure_cons() {
            steps += 1;
            if steps > MAX_LIST_WALK {
                return Err(ElispError::EvalError(
                    "append: list too long or circular".to_string(),
                ));
            }
            items.push(car);
            cur = cdr;
        }
    }
    let mut result = tail;
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

pub fn prim_reverse(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.clone().first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut result = LispObject::nil();
    let mut current = arg.clone();
    let mut steps: u64 = 0;
    while let Some((car, cdr)) = current.destructure_cons() {
        steps += 1;
        if steps > MAX_LIST_WALK {
            return Err(ElispError::EvalError(
                "reverse: list too long or circular".to_string(),
            ));
        }
        result = LispObject::cons(car, result);
        current = cdr;
    }
    Ok(result)
}

pub fn prim_member(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = list;
    let mut steps: u64 = 0;
    while let Some((car, cdr)) = current.destructure_cons() {
        steps += 1;
        if steps > MAX_LIST_WALK {
            return Err(ElispError::EvalError(
                "member: list too long or circular".to_string(),
            ));
        }
        if obj == car {
            return Ok(current);
        }
        current = cdr;
    }
    Ok(LispObject::nil())
}

pub fn prim_assoc(args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let alist = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = alist;
    let mut steps: u64 = 0;
    while let Some((entry, rest)) = current.destructure_cons() {
        steps += 1;
        if steps > MAX_LIST_WALK {
            return Err(ElispError::EvalError(
                "assoc: list too long or circular".to_string(),
            ));
        }
        if let Some((k, _)) = entry.destructure_cons() {
            if key == k {
                return Ok(entry);
            }
        }
        current = rest;
    }
    Ok(LispObject::nil())
}

pub fn prim_nth(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if n < 0 {
        return Ok(LispObject::nil());
    }
    let mut current = list;
    for _ in 0..n {
        match current.destructure_cons() {
            Some((_, cdr)) => current = cdr,
            None => return Ok(LispObject::nil()),
        }
    }
    match current.destructure_cons() {
        Some((car, _)) => Ok(car),
        None => Ok(LispObject::nil()),
    }
}

pub fn prim_nthcdr(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if n < 0 {
        return Ok(list);
    }
    let mut current = list;
    for _ in 0..n {
        match current.destructure_cons() {
            Some((_, cdr)) => current = cdr,
            None => return Ok(LispObject::nil()),
        }
    }
    Ok(current)
}

pub fn prim_setcar(args: &LispObject) -> ElispResult<LispObject> {
    let cell = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let new_car = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match &cell {
        LispObject::Cons(_) => {
            cell.set_car(new_car.clone());
            Ok(new_car)
        }
        _ => Err(ElispError::WrongTypeArgument("cons".to_string())),
    }
}

pub fn prim_setcdr(args: &LispObject) -> ElispResult<LispObject> {
    let cell = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let new_cdr = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match &cell {
        LispObject::Cons(_) => {
            cell.set_cdr(new_cdr.clone());
            Ok(new_cdr)
        }
        _ => Err(ElispError::WrongTypeArgument("cons".to_string())),
    }
}

pub fn prim_nconc(args: &LispObject) -> ElispResult<LispObject> {
    prim_append(args)
}

pub fn prim_nreverse(args: &LispObject) -> ElispResult<LispObject> {
    prim_reverse(args)
}

pub fn prim_delq(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut result = LispObject::nil();
    let mut current = list;
    let mut steps: u64 = 0;
    while let Some((car, cdr)) = current.destructure_cons() {
        steps += 1;
        if steps > MAX_LIST_WALK {
            return Err(ElispError::EvalError(
                "delq: list too long or circular".to_string(),
            ));
        }
        if !eq_test(&elt, &car) {
            result = LispObject::cons(car, result);
        }
        current = cdr;
    }
    prim_reverse(&LispObject::cons(result, LispObject::nil()))
}

pub fn prim_memq(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = list;
    let mut steps: u64 = 0;
    while let Some((car, cdr)) = current.destructure_cons() {
        steps += 1;
        if steps > MAX_LIST_WALK {
            return Err(ElispError::EvalError(
                "memq: list too long or circular".to_string(),
            ));
        }
        if eq_test(&elt, &car) {
            return Ok(current);
        }
        current = cdr;
    }
    Ok(LispObject::nil())
}

pub fn prim_assq(args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let alist = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = alist;
    let mut steps: u64 = 0;
    while let Some((entry, rest)) = current.destructure_cons() {
        steps += 1;
        if steps > MAX_LIST_WALK {
            return Err(ElispError::EvalError(
                "assq: list too long or circular".to_string(),
            ));
        }
        if let Some((k, _)) = entry.destructure_cons() {
            if eq_test(&key, &k) {
                return Ok(entry);
            }
        }
        current = rest;
    }
    Ok(LispObject::nil())
}

pub fn prim_last(args: &LispObject) -> ElispResult<LispObject> {
    let list = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = args.nth(1).and_then(|a| a.as_integer()).unwrap_or(1);
    if n <= 0 {
        return Ok(LispObject::nil());
    }
    let mut len: i64 = 0;
    let mut current = list.clone();
    while let Some((_, cdr)) = current.destructure_cons() {
        len += 1;
        if len as u64 > MAX_LIST_WALK {
            return Err(ElispError::EvalError(
                "last: list too long or circular".to_string(),
            ));
        }
        current = cdr;
    }
    let skip = (len - n).max(0);
    let mut current = list;
    for _ in 0..skip {
        if let Some((_, cdr)) = current.destructure_cons() {
            current = cdr;
        }
    }
    Ok(current)
}

pub fn prim_copy_sequence(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(arg.clone())
}

pub fn prim_cadr(args: &LispObject) -> ElispResult<LispObject> {
    let cdr_args = prim_cdr(args)?;
    let wrapped = LispObject::cons(cdr_args, LispObject::nil());
    prim_car(&wrapped)
}

pub fn prim_cddr(args: &LispObject) -> ElispResult<LispObject> {
    let cdr_args = prim_cdr(args)?;
    let wrapped = LispObject::cons(cdr_args, LispObject::nil());
    prim_cdr(&wrapped)
}

pub fn prim_caar(args: &LispObject) -> ElispResult<LispObject> {
    let car_args = prim_car(args)?;
    let wrapped = LispObject::cons(car_args, LispObject::nil());
    prim_car(&wrapped)
}

pub fn prim_cdar(args: &LispObject) -> ElispResult<LispObject> {
    let car_args = prim_car(args)?;
    let wrapped = LispObject::cons(car_args, LispObject::nil());
    prim_cdr(&wrapped)
}

pub fn prim_car_safe(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Cons(_) => {
            let wrapped = LispObject::cons(arg, LispObject::nil());
            prim_car(&wrapped)
        }
        _ => Ok(LispObject::nil()),
    }
}

pub fn prim_cdr_safe(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Cons(_) => {
            let wrapped = LispObject::cons(arg, LispObject::nil());
            prim_cdr(&wrapped)
        }
        _ => Ok(LispObject::nil()),
    }
}

pub fn prim_make_list(args: &LispObject) -> ElispResult<LispObject> {
    let length = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let init = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if length < 0 {
        return Ok(LispObject::nil());
    }
    let mut result = LispObject::nil();
    for _ in 0..length {
        result = LispObject::cons(init.clone(), result);
    }
    Ok(result)
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