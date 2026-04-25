//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use super::InterpreterState;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

/// Return f64 view of any numeric LispObject; non-numeric → 0.0.
pub(super) fn as_f64(v: &LispObject) -> f64 {
    match v {
        LispObject::Integer(n) => *n as f64,
        LispObject::Float(f) => *f,
        _ => 0.0,
    }
}
/// Is this token a cl-loop keyword? Used to stop greedy `do` / `initially`
/// / `finally` body collection.
pub(super) fn is_loop_keyword(obj: &LispObject) -> bool {
    let Some(n) = obj.as_symbol() else {
        return false;
    };
    matches!(
        n.as_str(),
        "for"
            | "as"
            | "with"
            | "repeat"
            | "while"
            | "until"
            | "always"
            | "never"
            | "thereis"
            | "do"
            | "doing"
            | "collect"
            | "collecting"
            | "append"
            | "appending"
            | "nconc"
            | "nconcing"
            | "sum"
            | "summing"
            | "count"
            | "counting"
            | "maximize"
            | "maximizing"
            | "minimize"
            | "minimizing"
            | "return"
            | "initially"
            | "finally"
            | "named"
            | "if"
            | "when"
            | "unless"
            | "and"
            | "else"
            | "end"
    )
}
pub(super) fn stateful_indirect_function(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &obj {
        LispObject::Symbol(id) => Ok(state.get_function_cell(*id).unwrap_or_else(LispObject::nil)),
        _ => Ok(obj),
    }
}
pub(super) fn stateful_default_toplevel_value(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    Ok(state.get_value_cell(sym).unwrap_or_else(LispObject::nil))
}
pub(super) fn stateful_buffer_local_value(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    Ok(state.get_value_cell(sym).unwrap_or_else(LispObject::nil))
}
pub(super) fn stateful_ert_get_test(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = match args.first().and_then(|a| a.as_symbol_id()) {
        Some(s) => s,
        None => return Ok(LispObject::nil()),
    };
    let key = crate::obarray::intern("ert--test");
    let v = state.get_plist(sym, key);
    if v.is_nil() {
        Ok(LispObject::nil())
    } else {
        Ok(v)
    }
}
pub(super) fn stateful_ert_test_boundp(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = match args.first().and_then(|a| a.as_symbol_id()) {
        Some(s) => s,
        None => return Ok(LispObject::nil()),
    };
    let key = crate::obarray::intern("ert--test");
    if state.get_plist(sym, key).is_nil() {
        Ok(LispObject::nil())
    } else {
        Ok(LispObject::t())
    }
}
pub(super) fn stateful_autoload_do_load(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let _ = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let funname = args.nth(1);
    if let Some(sym_obj) = funname {
        if let Some(sym_name) = sym_obj.as_symbol() {
            if let Some(sym_id) = crate::obarray::GLOBAL_OBARRAY.read().find(&sym_name) {
                if let Some(def) = state.get_function_cell(sym_id) {
                    let is_autoload_stub = def
                        .first()
                        .and_then(|head| head.as_symbol())
                        .is_some_and(|s| s == "autoload");
                    if !def.is_nil() && !is_autoload_stub {
                        return Ok(def);
                    }
                }
            }
        }
    }
    Ok(LispObject::nil())
}
pub(super) fn stateful_load_history_filename_element(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let filename_arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let filename = match filename_arg.as_string() {
        Some(s) => s.clone(),
        None => return Ok(LispObject::nil()),
    };
    let load_history_id = crate::obarray::intern("load-history");
    let load_history = state
        .get_value_cell(load_history_id)
        .unwrap_or(LispObject::nil());
    let mut current = load_history;
    while let Some((entry, rest)) = current.destructure_cons() {
        if let Some((key, _)) = entry.destructure_cons() {
            if let Some(key_str) = key.as_string() {
                if *key_str == filename {
                    return Ok(entry);
                }
            }
        }
        current = rest;
    }
    Ok(LispObject::nil())
}
