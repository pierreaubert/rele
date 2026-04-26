use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "symbol-name" => Some(prim_symbol_name(args)),
        "indirect-variable" => Some(prim_indirect_variable(args)),
        "subr-arity" => Some(prim_subr_arity(args)),
        "subr-name" => Some(prim_subr_name(args)),
        "position-symbol" => Some(prim_position_symbol(args)),
        "remove-pos-from-symbol" => Some(prim_remove_pos_from_symbol(args)),
        "symbol-with-pos-p" => Some(prim_symbol_with_pos_p(args)),
        "symbol-with-pos-pos" => Some(prim_symbol_with_pos_pos(args)),
        "setplist" => Some(prim_setplist(args)),
        "generate-new-buffer-name" => Some(prim_generate_new_buffer_name(args)),
        _ => None,
    }
}

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for &name in SYMBOL_PRIMITIVE_NAMES {
        interp.define(name, LispObject::primitive(name));
    }
}

pub const SYMBOL_PRIMITIVE_NAMES: &[&str] = &[
    "symbol-name",
    "intern-soft",
    "indirect-function",
    "indirect-variable",
    "subr-arity",
    "subr-name",
    "position-symbol",
    "remove-pos-from-symbol",
    "symbol-with-pos-p",
    "symbol-with-pos-pos",
    "setplist",
    "add-variable-watcher",
    "remove-variable-watcher",
    "get-variable-watchers",
    "command-modes",
    "default-toplevel-value",
    "buffer-local-value",
    "generate-new-buffer-name",
];

pub fn prim_symbol_name(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Symbol(id) => Ok(LispObject::string(&crate::obarray::symbol_name(*id))),
        LispObject::Nil => Ok(LispObject::string("nil")),
        LispObject::T => Ok(LispObject::string("t")),
        _ => Err(ElispError::WrongTypeArgument("symbol".to_string())),
    }
}

fn prim_indirect_variable(args: &LispObject) -> ElispResult<LispObject> {
    Ok(args.first().unwrap_or_else(LispObject::nil))
}

fn prim_subr_arity(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &obj {
        LispObject::Primitive(name) => {
            if name == "if" {
                return Ok(LispObject::cons(
                    LispObject::integer(2),
                    LispObject::symbol("unevalled"),
                ));
            }
            let arity = crate::emacs::data::subr_arity(name);
            let min = arity.map_or(0, |a| a.min_args) as i64;
            let max = match arity.and_then(|a| a.max_args) {
                Some(max) => LispObject::integer(max as i64),
                None => LispObject::symbol("many"),
            };
            Ok(LispObject::cons(LispObject::integer(min), max))
        }
        _ => Err(ElispError::WrongTypeArgument("subr".to_string())),
    }
}

fn prim_subr_name(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &obj {
        LispObject::Primitive(name) => Ok(LispObject::string(name)),
        _ => Err(ElispError::WrongTypeArgument("subr".to_string())),
    }
}

fn prim_setplist(args: &LispObject) -> ElispResult<LispObject> {
    let _sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let plist = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    // rele doesn't implement plist storage on symbols directly;
    // return the plist so callers that chain on the return value work.
    Ok(plist)
}

fn prim_position_symbol(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let pos = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if !matches!(sym, LispObject::Symbol(_) | LispObject::Nil | LispObject::T) {
        return Err(ElispError::WrongTypeArgument("symbol".to_string()));
    }
    if pos.as_integer().is_none() {
        return Err(ElispError::WrongTypeArgument("integer".to_string()));
    }
    Ok(LispObject::cons(sym, pos))
}

fn position_symbol_parts(obj: &LispObject) -> Option<(LispObject, LispObject)> {
    let (sym, pos) = obj.destructure_cons()?;
    if !matches!(sym, LispObject::Symbol(_) | LispObject::Nil | LispObject::T) {
        return None;
    }
    pos.as_integer()?;
    Some((sym, pos))
}

fn prim_remove_pos_from_symbol(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(position_symbol_parts(&obj)
        .map(|(sym, _)| sym)
        .unwrap_or(obj))
}

fn prim_symbol_with_pos_p(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(position_symbol_parts(&obj).is_some()))
}

fn prim_symbol_with_pos_pos(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    position_symbol_parts(&obj)
        .map(|(_, pos)| pos)
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol-with-pos".to_string()))
}

fn prim_generate_new_buffer_name(args: &LispObject) -> ElispResult<LispObject> {
    let starting = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match starting {
        LispObject::String(_) => Ok(starting),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}
