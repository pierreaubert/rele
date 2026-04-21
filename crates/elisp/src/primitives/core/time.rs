use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "time-equal-p" => Some(prim_time_equal_p(args)),
        "time-less-p" => Some(prim_time_less_p(args)),
        "time-convert" => Some(prim_time_convert(args)),
        "format-time-string" => Some(prim_format_time_string(args)),
        "encode-time" => Some(prim_encode_time(args)),
        "current-time" => Some(prim_current_time()),
        "current-time-string" => Some(Ok(LispObject::string(""))),
        "float-time" => Some(prim_float_time(args)),
        _ => None,
    }
}

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for &name in TIME_PRIMITIVE_NAMES {
        interp.define(name, LispObject::primitive(name));
    }
}

pub const TIME_PRIMITIVE_NAMES: &[&str] = &[
    "time-equal-p",
    "time-less-p",
    "time-convert",
    "format-time-string",
    "encode-time",
    "current-time",
    "current-time-string",
    "float-time",
];

fn time_to_seconds(obj: &LispObject) -> Option<f64> {
    match obj {
        LispObject::Nil => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?;
            Some(now.as_secs_f64())
        }
        LispObject::Integer(i) => Some(*i as f64),
        LispObject::Float(f) => Some(*f),
        LispObject::Cons(_) => {
            let (car, cdr) = obj.destructure_cons()?;
            let car_i = car.as_integer()?;
            match cdr {
                LispObject::Integer(hz) => {
                    if hz == 0 {
                        None
                    } else {
                        Some(car_i as f64 / hz as f64)
                    }
                }
                LispObject::Cons(_) => {
                    let high = car_i;
                    let low = cdr.first().and_then(|o| o.as_integer()).unwrap_or(0);
                    let usec_obj = cdr.nth(1);
                    let usec = usec_obj
                        .as_ref()
                        .and_then(LispObject::as_integer)
                        .unwrap_or(0);
                    Some(
                        (high as f64) * 65536.0 + (low as f64) + (usec as f64) / 1_000_000.0,
                    )
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn prim_time_equal_p(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let ea = time_to_seconds(&a);
    let eb = time_to_seconds(&b);
    Ok(LispObject::from(ea == eb && ea.is_some()))
}

fn prim_time_less_p(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match (time_to_seconds(&a), time_to_seconds(&b)) {
        (Some(x), Some(y)) => Ok(LispObject::from(x < y)),
        _ => Err(ElispError::WrongTypeArgument("time".to_string())),
    }
}

fn prim_time_convert(args: &LispObject) -> ElispResult<LispObject> {
    let t = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let secs =
        time_to_seconds(&t).ok_or_else(|| ElispError::WrongTypeArgument("time".to_string()))?;
    Ok(LispObject::float(secs))
}

fn prim_format_time_string(args: &LispObject) -> ElispResult<LispObject> {
    let fmt = args
        .first()
        .and_then(|a| match a {
            LispObject::String(s) => Some(s),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    let time_obj = args.nth(1).unwrap_or_else(LispObject::nil);
    let secs = time_to_seconds(&time_obj).unwrap_or(0.0);
    if fmt.contains('%') {
        Ok(LispObject::string(&format!("{secs}")))
    } else {
        Ok(LispObject::string(&fmt))
    }
}

fn prim_encode_time(args: &LispObject) -> ElispResult<LispObject> {
    let first = args.first().unwrap_or_else(LispObject::nil);
    let parts: Vec<LispObject> = if let LispObject::Cons(_) = first {
        let mut v = Vec::new();
        let mut cur = first;
        while let Some((c, r)) = cur.destructure_cons() {
            v.push(c);
            cur = r;
        }
        v
    } else {
        let mut v = Vec::new();
        let mut cur = args.clone();
        while let Some((c, r)) = cur.destructure_cons() {
            v.push(c);
            cur = r;
        }
        v
    };
    let get = |i: usize| parts.get(i).and_then(LispObject::as_integer).unwrap_or(0);
    let sec = get(0);
    let min = get(1);
    let hour = get(2);
    let day = get(3);
    let mon = get(4);
    let year = get(5);
    let days_since_epoch = ((year - 1970) * 365) + (mon - 1) * 30 + (day - 1);
    let secs = days_since_epoch * 86400 + hour * 3600 + min * 60 + sec;
    Ok(LispObject::float(secs as f64))
}

fn prim_current_time() -> ElispResult<LispObject> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let secs = now as i64;
    let high = secs >> 16;
    let low = secs & 0xffff;
    let usec = ((now - secs as f64) * 1_000_000.0) as i64;
    Ok(LispObject::cons(
        LispObject::integer(high),
        LispObject::cons(
            LispObject::integer(low),
            LispObject::cons(
                LispObject::integer(usec),
                LispObject::cons(LispObject::integer(0), LispObject::nil()),
            ),
        ),
    ))
}

fn prim_float_time(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().unwrap_or_else(LispObject::nil);
    let secs = time_to_seconds(&arg).unwrap_or(0.0);
    Ok(LispObject::float(secs))
}
