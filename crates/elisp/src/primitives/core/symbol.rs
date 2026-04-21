use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "symbol-name" => Some(prim_symbol_name(args)),
        "intern-soft" => Some(prim_intern_soft(args)),
        "indirect-function" => Some(prim_indirect_function(args)),
        "indirect-variable" => Some(prim_indirect_variable(args)),
        "subr-arity" => Some(prim_subr_arity(args)),
        "subr-name" => Some(prim_subr_name(args)),
        "setplist" => Some(prim_setplist(args)),
        "default-toplevel-value" => Some(prim_default_toplevel_value(args)),
        "buffer-local-value" => Some(prim_buffer_local_value(args)),
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
    "setplist",
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

fn prim_intern_soft(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = match &arg {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(_) => arg.as_symbol().unwrap_or_default(),
        _ => {
            return Err(ElispError::WrongTypeArgument(
                "string or symbol".to_string(),
            ));
        }
    };
    let sym_table = crate::obarray::GLOBAL_OBARRAY.read();
    for id in 0..sym_table.symbol_count() as u32 {
        let sid = crate::obarray::SymbolId(id);
        if sym_table.name(sid) == name {
            return Ok(LispObject::Symbol(sid));
        }
    }
    Ok(LispObject::nil())
}

fn prim_indirect_function(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &obj {
        LispObject::Symbol(id) => {
            Ok(crate::obarray::get_function_cell(*id).unwrap_or_else(LispObject::nil))
        }
        _ => Ok(obj),
    }
}

fn prim_indirect_variable(args: &LispObject) -> ElispResult<LispObject> {
    Ok(args.first().unwrap_or_else(LispObject::nil))
}

fn prim_subr_arity(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &obj {
        LispObject::Primitive(_) => Ok(LispObject::cons(
            LispObject::integer(0),
            LispObject::symbol("many"),
        )),
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

fn prim_default_toplevel_value(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    Ok(crate::obarray::get_value_cell(sym).unwrap_or_else(LispObject::nil))
}

fn prim_buffer_local_value(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    Ok(crate::obarray::get_value_cell(sym).unwrap_or_else(LispObject::nil))
}

fn prim_generate_new_buffer_name(args: &LispObject) -> ElispResult<LispObject> {
    let starting = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match starting {
        LispObject::String(_) => Ok(starting),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}
