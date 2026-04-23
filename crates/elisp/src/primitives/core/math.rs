use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use std::sync::atomic::{AtomicI64, Ordering};

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "+" => Some(prim_add(args)),
        "-" => Some(prim_sub(args)),
        "*" => Some(prim_mul(args)),
        "/" => Some(prim_div(args)),
        "=" => Some(prim_num_eq(args)),
        "<" => Some(prim_lt(args)),
        ">" => Some(prim_gt(args)),
        "<=" => Some(prim_le(args)),
        ">=" => Some(prim_ge(args)),
        "/=" => Some(prim_ne(args)),
        "1+" => Some(prim_1_plus(args)),
        "1-" => Some(prim_1_minus(args)),
        "mod" | "%" => Some(prim_mod(args)),
        "abs" => Some(prim_abs(args)),
        "max" => Some(prim_max(args)),
        "min" => Some(prim_min(args)),
        "floor" => Some(prim_floor(args)),
        "ceiling" => Some(prim_ceiling(args)),
        "round" => Some(prim_round(args)),
        "truncate" => Some(prim_truncate(args)),
        "float" => Some(prim_float(args)),
        "ash" => Some(prim_ash(args)),
        "logand" => Some(prim_logand(args)),
        "logior" => Some(prim_logior(args)),
        "lognot" => Some(prim_lognot(args)),
        "logcount" => Some(prim_logcount(args)),
        "logxor" => Some(prim_logxor(args)),
        "logb" => Some(prim_logb(args)),
        "random" => Some(prim_random(args)),
        "evenp" => Some(prim_evenp(args)),
        "oddp" => Some(prim_oddp(args)),
        "plusp" => Some(prim_plusp(args)),
        "minusp" => Some(prim_minusp(args)),
        "isnan" => Some(prim_isnan(args)),
        "copysign" => Some(prim_copysign(args)),
        "frexp" => Some(prim_frexp(args)),
        "ldexp" => Some(prim_ldexp(args)),
        // Transcendental
        "sin" => Some(prim_math_1(args, f64::sin)),
        "cos" => Some(prim_math_1(args, f64::cos)),
        "tan" => Some(prim_math_1(args, f64::tan)),
        "asin" => Some(prim_math_1(args, f64::asin)),
        "acos" => Some(prim_math_1(args, f64::acos)),
        "atan" => Some(prim_atan(args)),
        "exp" => Some(prim_math_1(args, f64::exp)),
        "log" => Some(prim_log(args)),
        "sqrt" => Some(prim_math_1(args, f64::sqrt)),
        "expt" => Some(prim_expt(args)),
        _ => None,
    }
}

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for &name in MATH_PRIMITIVE_NAMES {
        interp.define(name, LispObject::primitive(name));
    }
}

pub const MATH_PRIMITIVE_NAMES: &[&str] = &[
    "+",
    "-",
    "*",
    "/",
    "=",
    "<",
    ">",
    "<=",
    ">=",
    "/=",
    "1+",
    "1-",
    "mod",
    "abs",
    "max",
    "min",
    "floor",
    "ceiling",
    "round",
    "truncate",
    "float",
    "ash",
    "logand",
    "logior",
    "lognot",
    "%",
    "logcount",
    "logxor",
    "logb",
    "random",
    "evenp",
    "oddp",
    "plusp",
    "minusp",
    "isnan",
    "copysign",
    "frexp",
    "ldexp",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "exp",
    "log",
    "sqrt",
    "expt",
    "byteorder",
];

pub fn get_number(obj: &LispObject) -> Option<f64> {
    match obj {
        LispObject::Integer(i) => Some(*i as f64),
        LispObject::Float(f) => Some(*f),
        LispObject::Nil => Some(0.0),
        LispObject::T => Some(1.0),
        _ => None,
    }
}

fn get_float_arg(args: &LispObject) -> ElispResult<f64> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match a {
        LispObject::Integer(i) => Ok(i as f64),
        LispObject::Float(f) => Ok(f),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

pub fn prim_add(args: &LispObject) -> ElispResult<LispObject> {
    let mut raw = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        raw.push(arg);
        current = rest;
    }
    let all_int = raw.iter().all(|a| matches!(a, LispObject::Integer(_)));
    if all_int {
        let sum: i64 = raw
            .iter()
            .map(|a| a.as_integer().unwrap())
            .fold(0i64, |a, b| a.wrapping_add(b));
        Ok(LispObject::integer(sum))
    } else {
        let sum: f64 = raw
            .iter()
            .map(|a| get_number(a).ok_or_else(|| ElispError::WrongTypeArgument("number".into())))
            .collect::<ElispResult<Vec<_>>>()?
            .into_iter()
            .sum();
        Ok(LispObject::float(sum))
    }
}

pub fn prim_sub(args: &LispObject) -> ElispResult<LispObject> {
    let mut raw = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        raw.push(arg);
        current = rest;
    }
    if raw.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }
    let all_int = raw.iter().all(|a| matches!(a, LispObject::Integer(_)));
    if all_int {
        let ints: Vec<i64> = raw.iter().map(|a| a.as_integer().unwrap()).collect();
        let result = if ints.len() == 1 {
            ints[0].wrapping_neg()
        } else {
            ints.iter()
                .skip(1)
                .fold(ints[0], |acc, &x| acc.wrapping_sub(x))
        };
        Ok(LispObject::integer(result))
    } else {
        let nums: Vec<f64> = raw
            .iter()
            .map(|a| get_number(a).ok_or_else(|| ElispError::WrongTypeArgument("number".into())))
            .collect::<ElispResult<Vec<_>>>()?;
        let result = if nums.len() == 1 {
            -nums[0]
        } else {
            nums.iter().skip(1).fold(nums[0], |acc, &x| acc - x)
        };
        Ok(LispObject::float(result))
    }
}

pub fn prim_mul(args: &LispObject) -> ElispResult<LispObject> {
    let mut raw = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        raw.push(arg);
        current = rest;
    }
    let all_int = raw.iter().all(|a| matches!(a, LispObject::Integer(_)));
    if all_int {
        let product: i64 = raw
            .iter()
            .map(|a| a.as_integer().unwrap())
            .fold(1i64, |a, b| a.wrapping_mul(b));
        Ok(LispObject::integer(product))
    } else {
        let product: f64 = raw
            .iter()
            .map(|a| get_number(a).ok_or_else(|| ElispError::WrongTypeArgument("number".into())))
            .collect::<ElispResult<Vec<_>>>()?
            .into_iter()
            .product();
        Ok(LispObject::float(product))
    }
}

pub fn prim_div(args: &LispObject) -> ElispResult<LispObject> {
    let mut raw_args: Vec<LispObject> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        raw_args.push(arg);
        current = rest;
    }
    if raw_args.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }
    for a in &raw_args {
        if get_number(a).is_none() {
            return Err(ElispError::WrongTypeArgument("number".to_string()));
        }
    }
    let all_integer = raw_args.iter().all(|a| matches!(a, LispObject::Integer(_)));
    if all_integer {
        let ints: Vec<i64> = raw_args.iter().map(|a| a.as_integer().unwrap()).collect();
        for &d in &ints[1..] {
            if d == 0 {
                return Err(ElispError::DivisionByZero);
            }
        }
        if ints.len() == 1 {
            if ints[0] == 0 {
                return Err(ElispError::DivisionByZero);
            }
            return Ok(LispObject::integer(1i64.wrapping_div(ints[0])));
        }
        let result = ints
            .iter()
            .skip(1)
            .fold(ints[0], |acc, &x| acc.wrapping_div(x));
        Ok(LispObject::integer(result))
    } else {
        let numbers: Vec<f64> = raw_args.iter().map(|a| get_number(a).unwrap()).collect();
        for &d in &numbers[1..] {
            if d == 0.0 {
                return Err(ElispError::DivisionByZero);
            }
        }
        if numbers.len() == 1 {
            if numbers[0] == 0.0 {
                return Err(ElispError::DivisionByZero);
            }
            return Ok(LispObject::float(1.0 / numbers[0]));
        }
        let result = numbers.iter().skip(1).fold(numbers[0], |acc, &x| acc / x);
        Ok(LispObject::float(result))
    }
}

pub fn prim_num_eq(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }
    let first = numbers[0];
    Ok(LispObject::from(numbers.iter().all(|&x| x == first)))
}

pub fn prim_lt(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.len() < 2 {
        return Err(ElispError::WrongNumberOfArguments);
    }
    for w in numbers.windows(2) {
        if w[0].partial_cmp(&w[1]) != Some(std::cmp::Ordering::Less) {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::t())
}

pub fn prim_gt(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.len() < 2 {
        return Err(ElispError::WrongNumberOfArguments);
    }
    for w in numbers.windows(2) {
        if w[0].partial_cmp(&w[1]) != Some(std::cmp::Ordering::Greater) {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::t())
}

pub fn prim_le(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.len() < 2 {
        return Err(ElispError::WrongNumberOfArguments);
    }
    for w in numbers.windows(2) {
        if !matches!(
            w[0].partial_cmp(&w[1]),
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        ) {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::t())
}

pub fn prim_ge(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.len() < 2 {
        return Err(ElispError::WrongNumberOfArguments);
    }
    for w in numbers.windows(2) {
        if !matches!(
            w[0].partial_cmp(&w[1]),
            Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
        ) {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::t())
}

pub fn prim_ne(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let na = get_number(&a).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    let nb = get_number(&b).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::from(na != nb))
}

pub fn prim_1_plus(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match n {
        LispObject::Integer(i) => Ok(LispObject::integer(i.wrapping_add(1))),
        LispObject::Float(f) => Ok(LispObject::float(f + 1.0)),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

pub fn prim_1_minus(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match n {
        LispObject::Integer(i) => Ok(LispObject::integer(i.wrapping_sub(1))),
        LispObject::Float(f) => Ok(LispObject::float(f - 1.0)),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

pub fn prim_mod(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let (Some(na), Some(nb)) = (a.as_integer(), b.as_integer()) else {
        return Err(ElispError::WrongTypeArgument("integer".to_string()));
    };
    if nb == 0 {
        return Err(ElispError::DivisionByZero);
    }
    Ok(LispObject::integer(na % nb))
}

pub fn prim_abs(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match n {
        LispObject::Integer(i) => Ok(LispObject::integer(i.abs())),
        LispObject::Float(f) => Ok(LispObject::float(f.abs())),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

pub fn prim_max(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }
    let result = numbers.into_iter().fold(f64::NEG_INFINITY, f64::max);
    Ok(LispObject::float(result))
}

pub fn prim_min(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }
    let result = numbers.into_iter().fold(f64::INFINITY, f64::min);
    Ok(LispObject::float(result))
}

pub fn prim_floor(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match n {
        LispObject::Integer(_) => Ok(n),
        LispObject::Float(f) => Ok(LispObject::integer(f.floor() as i64)),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

pub fn prim_ceiling(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match n {
        LispObject::Integer(_) => Ok(n),
        LispObject::Float(f) => Ok(LispObject::integer(f.ceil() as i64)),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

pub fn prim_round(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match n {
        LispObject::Integer(_) => Ok(n),
        LispObject::Float(f) => Ok(LispObject::integer(f.round() as i64)),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

pub fn prim_truncate(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match n {
        LispObject::Integer(_) => Ok(n),
        LispObject::Float(f) => Ok(LispObject::integer(f.trunc() as i64)),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

pub fn prim_float(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match n {
        LispObject::Integer(i) => Ok(LispObject::float(i as f64)),
        LispObject::Float(_) => Ok(n),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

pub fn prim_ash(args: &LispObject) -> ElispResult<LispObject> {
    let value = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let count = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let (Some(v), Some(c)) = (value.as_integer(), count.as_integer()) else {
        return Err(ElispError::WrongTypeArgument("integer".to_string()));
    };
    if c >= 0 {
        Ok(LispObject::integer(v.wrapping_shl(c as u32)))
    } else {
        Ok(LispObject::integer(v.wrapping_shr((-c) as u32)))
    }
}

pub fn prim_logand(args: &LispObject) -> ElispResult<LispObject> {
    let mut result: i64 = -1;
    let mut current = args.clone();
    let mut empty = true;
    while let Some((arg, rest)) = current.destructure_cons() {
        empty = false;
        let n = arg
            .as_integer()
            .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
        result &= n;
        current = rest;
    }
    if empty {
        return Err(ElispError::WrongNumberOfArguments);
    }
    Ok(LispObject::integer(result))
}

pub fn prim_logior(args: &LispObject) -> ElispResult<LispObject> {
    let mut result: i64 = 0;
    let mut current = args.clone();
    let mut empty = true;
    while let Some((arg, rest)) = current.destructure_cons() {
        empty = false;
        let n = arg
            .as_integer()
            .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
        result |= n;
        current = rest;
    }
    if empty {
        return Err(ElispError::WrongNumberOfArguments);
    }
    Ok(LispObject::integer(result))
}

pub fn prim_lognot(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = n
        .as_integer()
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    Ok(LispObject::integer(!n))
}

pub fn prim_logcount(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = n
        .as_integer()
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    Ok(LispObject::integer(n.count_ones() as i64))
}

pub fn prim_logxor(args: &LispObject) -> ElispResult<LispObject> {
    let mut result: i64 = 0;
    let mut current = args.clone();
    let mut empty = true;
    while let Some((arg, rest)) = current.destructure_cons() {
        empty = false;
        let n = arg
            .as_integer()
            .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
        result ^= n;
        current = rest;
    }
    if empty {
        return Err(ElispError::WrongNumberOfArguments);
    }
    Ok(LispObject::integer(result))
}

// -- Extended numeric primitives --

fn prim_logb(args: &LispObject) -> ElispResult<LispObject> {
    let n = get_float_arg(args)?;
    if n == 0.0 {
        Ok(LispObject::integer(i64::MIN))
    } else {
        Ok(LispObject::integer(n.abs().log2().floor() as i64))
    }
}

/// Simple LCG-based pseudo-random number generator state.
static RANDOM_STATE: AtomicI64 = AtomicI64::new(0);

fn prim_random(args: &LispObject) -> ElispResult<LispObject> {
    let limit = args.first().and_then(|a| a.as_integer());
    let old = RANDOM_STATE.load(Ordering::Relaxed);
    let next = old.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    RANDOM_STATE.store(next, Ordering::Relaxed);
    let raw = (next >> 17) & 0x7fff_ffff;
    match limit {
        Some(l) if l > 0 => Ok(LispObject::integer(raw % l)),
        _ => Ok(LispObject::integer(raw)),
    }
}

fn prim_evenp(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    Ok(LispObject::from(n % 2 == 0))
}

fn prim_oddp(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    Ok(LispObject::from(n % 2 != 0))
}

fn prim_plusp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().unwrap_or_else(LispObject::nil);
    let p = match arg {
        LispObject::Integer(i) => i > 0,
        LispObject::Float(f) => f > 0.0,
        _ => return Err(ElispError::WrongTypeArgument("number".to_string())),
    };
    Ok(LispObject::from(p))
}

fn prim_minusp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().unwrap_or_else(LispObject::nil);
    let n = match arg {
        LispObject::Integer(i) => i < 0,
        LispObject::Float(f) => f < 0.0,
        _ => return Err(ElispError::WrongTypeArgument("number".to_string())),
    };
    Ok(LispObject::from(n))
}

fn prim_isnan(args: &LispObject) -> ElispResult<LispObject> {
    let x = args.first().unwrap_or_else(LispObject::nil);
    Ok(LispObject::from(
        matches!(x, LispObject::Float(f) if f.is_nan()),
    ))
}

fn prim_copysign(args: &LispObject) -> ElispResult<LispObject> {
    let x = get_float_arg(args)?;
    let y = args
        .nth(1)
        .and_then(|a| match a {
            LispObject::Integer(i) => Some(i as f64),
            LispObject::Float(f) => Some(f),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::float(x.copysign(y)))
}

trait LdexpSafe {
    fn ldexp_safe(self, exp: i64) -> f64;
}
impl LdexpSafe for f64 {
    fn ldexp_safe(self, exp: i64) -> f64 {
        if exp >= 0 {
            self * (1u64 << exp.min(62)) as f64
        } else {
            self / (1u64 << (-exp).min(62)) as f64
        }
    }
}

fn prim_frexp(args: &LispObject) -> ElispResult<LispObject> {
    let x = get_float_arg(args)?;
    if x == 0.0 {
        return Ok(LispObject::cons(
            LispObject::float(0.0),
            LispObject::integer(0),
        ));
    }
    let bits = x.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i64 - 1022;
    let mantissa = x / (1f64.ldexp_safe(exp));
    Ok(LispObject::cons(
        LispObject::float(mantissa),
        LispObject::integer(exp),
    ))
}

fn prim_ldexp(args: &LispObject) -> ElispResult<LispObject> {
    let m = get_float_arg(args)?;
    let e = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    Ok(LispObject::float(m.ldexp_safe(e)))
}

// -- Transcendental functions --

fn prim_math_1(args: &LispObject, f: fn(f64) -> f64) -> ElispResult<LispObject> {
    let x = get_float_arg(args)?;
    Ok(LispObject::float(f(x)))
}

fn prim_atan(args: &LispObject) -> ElispResult<LispObject> {
    let y = get_float_arg(args)?;
    if let Some(x) = args.nth(1) {
        let x = match x {
            LispObject::Integer(i) => i as f64,
            LispObject::Float(f) => f,
            _ => return Err(ElispError::WrongTypeArgument("number".to_string())),
        };
        Ok(LispObject::float(y.atan2(x)))
    } else {
        Ok(LispObject::float(y.atan()))
    }
}

fn prim_log(args: &LispObject) -> ElispResult<LispObject> {
    let x = get_float_arg(args)?;
    if let Some(base) = args.nth(1) {
        let base = match base {
            LispObject::Integer(i) => i as f64,
            LispObject::Float(f) => f,
            _ => return Err(ElispError::WrongTypeArgument("number".to_string())),
        };
        Ok(LispObject::float(x.log(base)))
    } else {
        Ok(LispObject::float(x.ln()))
    }
}

fn prim_expt(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match (&a, &b) {
        (LispObject::Integer(base), LispObject::Integer(exp)) => {
            if *exp >= 0 && *exp <= 62 {
                Ok(LispObject::integer(base.wrapping_pow(*exp as u32)))
            } else {
                Ok(LispObject::float((*base as f64).powf(*exp as f64)))
            }
        }
        _ => {
            let base = get_number(&a)
                .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
            let exp = get_number(&b)
                .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
            Ok(LispObject::float(base.powf(exp)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_number_nil_as_zero() {
        assert_eq!(get_number(&LispObject::nil()), Some(0.0));
    }

    #[test]
    fn test_get_number_t_as_one() {
        assert_eq!(get_number(&LispObject::t()), Some(1.0));
    }

    #[test]
    fn test_add_with_nil() {
        let args = LispObject::cons(
            LispObject::nil(),
            LispObject::cons(LispObject::integer(5), LispObject::nil()),
        );
        let result = prim_add(&args).unwrap();
        assert_eq!(result.as_float(), Some(5.0));
    }

    #[test]
    fn test_add_t_plus_5() {
        let args = LispObject::cons(
            LispObject::t(),
            LispObject::cons(LispObject::integer(5), LispObject::nil()),
        );
        let result = prim_add(&args).unwrap();
        assert_eq!(result.as_float(), Some(6.0));
    }

    #[test]
    fn test_gt_5_gt_nil() {
        let args = LispObject::cons(
            LispObject::integer(5),
            LispObject::cons(LispObject::nil(), LispObject::nil()),
        );
        assert_eq!(prim_gt(&args).unwrap(), LispObject::t());
    }
}
