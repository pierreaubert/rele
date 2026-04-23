use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "eq" => Some(prim_eq(args)),
        "eql" => Some(prim_eql(args)),
        "equal" => Some(prim_equal(args)),
        "not" => Some(prim_not(args)),
        "null" => Some(prim_null(args)),
        "symbolp" => Some(prim_symbolp(args)),
        "numberp" => Some(prim_numberp(args)),
        "listp" => Some(prim_listp(args)),
        "consp" => Some(prim_consp(args)),
        "stringp" => Some(prim_stringp(args)),
        "atom" => Some(prim_atom(args)),
        "integerp" => Some(prim_integerp(args)),
        "floatp" => Some(prim_floatp(args)),
        "zerop" => Some(prim_zerop(args)),
        "natnump" => Some(prim_natnump(args)),
        "functionp" => Some(prim_functionp(args)),
        "subrp" => Some(prim_subrp(args)),
        "sequencep" => Some(prim_sequencep(args)),
        "char-or-string-p" => Some(prim_char_or_string_p(args)),
        "booleanp" => Some(prim_booleanp(args)),
        "keywordp" => Some(prim_keywordp(args)),
        "obarrayp" => Some(prim_obarrayp(args)),
        "characterp" => Some(prim_characterp(args)),
        "arrayp" => Some(prim_arrayp(args)),
        "nlistp" => Some(prim_nlistp(args)),
        "byte-code-function-p" => Some(prim_byte_code_function_p(args)),
        "hash-table-p" => Some(prim_hash_table_p(args)),
        _ => None,
    }
}

pub fn prim_eq(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match (&a, &b) {
        (LispObject::Nil, LispObject::Nil) => true,
        (LispObject::T, LispObject::T) => true,
        (LispObject::Integer(x), LispObject::Integer(y)) => x == y,
        (LispObject::Symbol(x), LispObject::Symbol(y)) => x == y,
        _ => false,
    };
    Ok(LispObject::from(result))
}

pub fn prim_eql(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match (&a, &b) {
        (LispObject::Nil, LispObject::Nil) => true,
        (LispObject::T, LispObject::T) => true,
        (LispObject::Symbol(x), LispObject::Symbol(y)) => x == y,
        (LispObject::Integer(x), LispObject::Integer(y)) => x == y,
        (LispObject::Float(x), LispObject::Float(y)) => x.to_bits() == y.to_bits(),
        _ => false,
    };
    Ok(LispObject::from(result))
}

pub fn prim_equal(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(a == b))
}

pub fn prim_not(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_nil()))
}

pub fn prim_null(args: &LispObject) -> ElispResult<LispObject> {
    prim_not(args)
}

pub fn prim_symbolp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(
        arg.is_symbol() || arg.is_nil() || arg.is_t(),
    ))
}

pub fn prim_numberp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_integer() || arg.is_float()))
}

pub fn prim_listp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_nil() || arg.is_cons()))
}

pub fn prim_consp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_cons()))
}

pub fn prim_stringp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_string()))
}

pub fn prim_atom(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(!arg.is_cons()))
}

pub fn prim_integerp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_integer()))
}

pub fn prim_floatp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_float()))
}

pub fn prim_zerop(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n =
        super::math::get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::from(n == 0.0))
}

pub fn prim_natnump(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match &arg {
        LispObject::Integer(i) => *i >= 0,
        _ => false,
    };
    Ok(LispObject::from(result))
}

pub fn prim_functionp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match &arg {
        LispObject::Primitive(_) => true,
        LispObject::Cons(cell) => {
            let b = cell.lock();
            if let LispObject::Symbol(id) = &b.0 {
                crate::obarray::symbol_name(*id) == "lambda"
            } else {
                false
            }
        }
        _ => false,
    };
    Ok(LispObject::from(result))
}

pub fn prim_subrp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_primitive()))
}

pub fn prim_sequencep(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = matches!(
        arg,
        LispObject::Nil | LispObject::Cons(_) | LispObject::Vector(_) | LispObject::String(_)
    );
    Ok(LispObject::from(result))
}

pub fn prim_char_or_string_p(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = matches!(arg, LispObject::String(_) | LispObject::Integer(_));
    Ok(LispObject::from(result))
}

pub fn prim_booleanp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = matches!(arg, LispObject::Nil | LispObject::T);
    Ok(LispObject::from(result))
}

pub fn prim_keywordp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match &arg {
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id).starts_with(':'),
        _ => false,
    };
    Ok(LispObject::from(result))
}

pub fn prim_obarrayp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = matches!(arg, LispObject::Vector(_) | LispObject::HashTable(_));
    Ok(LispObject::from(result))
}

pub fn prim_characterp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_integer()))
}

pub fn prim_arrayp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(matches!(
        arg,
        LispObject::Vector(_) | LispObject::String(_)
    )))
}

pub fn prim_nlistp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(!arg.is_nil() && !arg.is_cons()))
}

pub fn prim_byte_code_function_p(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(matches!(arg, LispObject::BytecodeFn(_))))
}

pub fn prim_hash_table_p(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(matches!(arg, LispObject::HashTable(_))))
}
