//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use super::SyncRefCell as RwLock;
use super::{Environment, InterpreterState, Macro, MacroTable, eval, eval_progn};
use super::{builtins, state_cl};
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::sync::Arc;

use super::functions_2::{
    stateful_add_to_list, stateful_apply, stateful_cl_generic_define,
    stateful_cl_generic_define_method, stateful_cl_generic_generalizers,
    stateful_def_edebug_elem_spec, stateful_defvar_1, stateful_eval, stateful_featurep,
    stateful_funcall, stateful_function_get, stateful_function_put, stateful_get, stateful_provide,
    stateful_put, stateful_require, symbol_id_including_constants, symbol_name_including_constants,
};
use super::functions_5::{
    stateful_autoload_do_load, stateful_buffer_local_value, stateful_default_toplevel_value,
    stateful_ert_get_test, stateful_ert_test_boundp, stateful_indirect_function,
    stateful_load_history_filename_element,
};
use super::types::FallbackFrame;

/// Phase 7a: **stateful primitives** — functions that are traditionally
/// implemented as special forms in the source-level dispatch (because
/// they need env/macros/state access) but are *semantically* regular
/// functions with evaluated arguments. When a bytecode function from
/// the VM (or any other caller that resolves through the symbol's
/// function cell) invokes `defalias`, `fset`, `eval`, `funcall`,
/// `apply`, etc., we route through this table instead of the regular
/// stateless `primitives::call_primitive` — the caller has already
/// evaluated the arguments, so we must not re-evaluate.
///
/// Returns `Some(result)` when `name` matches; `None` when the caller
/// should fall through to the regular primitive dispatch.
pub(crate) fn call_stateful_primitive(
    name: &str,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> Option<ElispResult<LispObject>> {
    match name {
        "defalias" => Some(stateful_defalias(args, macros, state)),
        "fset" => Some(stateful_fset(args, state)),
        "eval" => Some(stateful_eval(args, env, editor, macros, state)),
        "funcall" => Some(stateful_funcall(args, env, editor, macros, state)),
        "apply" => Some(stateful_apply(args, env, editor, macros, state)),
        "put" => Some(stateful_put(args, state)),
        "get" => Some(stateful_get(args, state)),
        "provide" => Some(stateful_provide(args, state)),
        "featurep" => Some(stateful_featurep(args, state)),
        "require" => Some(stateful_require(args, env, editor, macros, state)),
        "function-put" => Some(stateful_function_put(args, state)),
        "function-get" => Some(stateful_function_get(args, state)),
        "def-edebug-elem-spec" => Some(stateful_def_edebug_elem_spec(args, state)),
        "defvar-1" => Some(stateful_defvar_1(args, env, state)),
        "cl-generic-generalizers" => Some(stateful_cl_generic_generalizers(args, state)),
        "cl-generic-define" => Some(stateful_cl_generic_define(args)),
        "cl-generic-define-method" => Some(stateful_cl_generic_define_method(args, state)),
        "add-to-list" | "add-to-ordered-list" => Some(stateful_add_to_list(args, env, state)),
        "boundp" => Some(stateful_boundp(args, env, state)),
        "fboundp" => Some(stateful_fboundp(args, env, macros)),
        "intern" => Some(stateful_intern(args)),
        "intern-soft" => Some(stateful_intern_soft(args, env, state)),
        "set" => Some(stateful_set(args, env, state)),
        "symbol-value" => Some(stateful_symbol_value(args, env, state)),
        "symbol-function" => Some(stateful_symbol_function(args, env, macros, state)),
        "default-value" => Some(stateful_symbol_value(args, env, state)),
        "default-boundp" => Some(stateful_boundp(args, env, state)),
        "set-default" => Some(stateful_set_default(args, env)),
        "makunbound" => Some(stateful_makunbound(args, state)),
        "fmakunbound" => Some(stateful_fmakunbound(args, state)),
        "make-hash-table" => Some(stateful_make_hash_table(args, state)),
        "make-closure" => Some(stateful_make_closure(args)),
        "vector" => Some(stateful_vector(args, state)),
        "format" | "format-message" | "message" => {
            Some(stateful_format(args, env, editor, macros, state))
        }
        "throw" => Some(stateful_throw(args)),
        "signal" => Some(stateful_signal(args)),
        "error" => Some(stateful_error(args, env, editor, macros, state)),
        "indirect-function" => Some(stateful_indirect_function(args, state)),
        "default-toplevel-value" => Some(stateful_default_toplevel_value(args, state)),
        "buffer-local-value" => Some(stateful_buffer_local_value(args, state)),
        "ert-get-test" => Some(stateful_ert_get_test(args, state)),
        "ert-test-boundp" => Some(stateful_ert_test_boundp(args, state)),
        "autoload-do-load" => Some(stateful_autoload_do_load(args, state)),
        "load-history-filename-element" => {
            Some(stateful_load_history_filename_element(args, state))
        }
        _ => {
            if let Some(r) =
                super::super::keymap_forms::call_stateful_keymap_primitive(name, args, state)
            {
                return Some(r);
            }
            if let Some(r) = crate::primitives_buffer::call_buffer_primitive(name, args) {
                return Some(r);
            }
            if let Some(r) = crate::primitives_window::call_window_primitive(name, args) {
                return Some(r);
            }
            if let Some(r) = crate::primitives_file::call_file_primitive(name, args, state) {
                return Some(r);
            }
            if let Some(r) = crate::primitives_eieio::call_eieio_primitive(name, args) {
                return Some(r);
            }
            if let Some(r) = crate::primitives_cl::call_cl_primitive(name, args) {
                return Some(r);
            }
            if let Some(r) =
                super::state_cl::call_stateful_cl(name, args, env, editor, macros, state)
            {
                return Some(r.map(|v| value_to_obj(v)));
            }
            None
        }
    }
}
/// `(throw TAG VALUE)` via funcall. Args pre-evaluated.
fn stateful_throw(args: &LispObject) -> ElispResult<LispObject> {
    let tag = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    Err(ElispError::Throw(Box::new(crate::error::ThrowData {
        tag,
        value,
    })))
}
/// `(signal ERROR-SYMBOL DATA)` via funcall. Args pre-evaluated.
fn stateful_signal(args: &LispObject) -> ElispResult<LispObject> {
    let symbol = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let data = args.nth(1).unwrap_or(LispObject::nil());
    Err(ElispError::Signal(Box::new(crate::error::SignalData {
        symbol,
        data,
    })))
}
/// `(error FMT &rest ARGS)` via funcall. Format the message with
/// pre-evaluated args, then signal `error` with it.
fn stateful_error(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let formatted = stateful_format(args, env, editor, macros, state)?;
    let msg = formatted.princ_to_string();
    Err(ElispError::Signal(Box::new(crate::error::SignalData {
        symbol: LispObject::symbol("error"),
        data: LispObject::cons(LispObject::string(&msg), LispObject::nil()),
    })))
}
fn stateful_vector(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let mut items: Vec<LispObject> = Vec::new();
    let mut cur = args.clone();
    while let Some((car, cdr)) = cur.destructure_cons() {
        items.push(car);
        cur = cdr;
    }
    let v = state.heap_vector_from_objects(&items);
    Ok(value_to_obj(v))
}
/// `(format FMT &rest ARGS)` via funcall path. Args are pre-evaluated;
/// we rewrap each as `(quote ARG)` so the existing source-level
/// `eval_format` can treat them as forms to eval — `quote` just
/// returns the wrapped value unchanged.
fn stateful_format(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let quote_sym = LispObject::symbol("quote");
    let mut wrapped: Vec<LispObject> = Vec::new();
    let mut cur = args.clone();
    while let Some((car, cdr)) = cur.destructure_cons() {
        if wrapped.is_empty() {
            wrapped.push(car);
        } else {
            let quoted =
                LispObject::cons(quote_sym.clone(), LispObject::cons(car, LispObject::nil()));
            wrapped.push(quoted);
        }
        cur = cdr;
    }
    let mut form = LispObject::nil();
    for item in wrapped.into_iter().rev() {
        form = LispObject::cons(item, form);
    }
    let result = super::builtins::eval_format(obj_to_value(form), env, editor, macros, state)?;
    Ok(value_to_obj(result))
}
fn stateful_boundp(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = crate::obarray::symbol_name(sym_id);
    Ok(LispObject::from(
        matches!(sym, LispObject::Nil | LispObject::T)
            || state.get_value_cell(sym_id).is_some()
            || env.read().get(&name).is_some(),
    ))
}
fn stateful_fboundp(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    macros: &MacroTable,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if matches!(sym, LispObject::Nil | LispObject::T) {
        return Ok(LispObject::nil());
    }
    let name = symbol_name_including_constants(&sym)?;
    let found = env.read().get_function(&name).is_some() || macros.read().contains_key(&name);
    Ok(LispObject::from(found))
}
fn stateful_intern(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::String(s) => Ok(LispObject::symbol(&s)),
        LispObject::Symbol(_) => Ok(arg),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}
fn stateful_intern_soft(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = match &arg {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Ok(LispObject::nil()),
    };
    if env.read().get(&name).is_some() {
        return Ok(LispObject::symbol(&name));
    }
    let table = crate::obarray::GLOBAL_OBARRAY.read();
    for id in 0..table.symbol_count() as u32 {
        let sid = crate::obarray::SymbolId(id);
        if table.name(sid) == name {
            drop(table);
            let has_value = state.get_value_cell(sid).is_some();
            let has_fn = state.get_function_cell(sid).is_some();
            if has_value || has_fn {
                return Ok(LispObject::Symbol(sid));
            }
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::nil())
}
fn stateful_set(
    args: &LispObject,
    _env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = sym
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    state.set_value_cell(sym_id, val.clone());
    state.global_env.write().set_id(sym_id, val.clone());
    Ok(val)
}
fn stateful_set_default(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let name = sym
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    env.write().set(&name, val.clone());
    Ok(val)
}
fn stateful_symbol_value(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = crate::obarray::symbol_name(sym_id);
    Ok(state
        .get_value_cell(sym_id)
        .or_else(|| env.read().get(&name))
        .unwrap_or(LispObject::nil()))
}
fn stateful_symbol_function(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    macros: &MacroTable,
    _state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = symbol_name_including_constants(&sym)?;
    let func_val = env.read().get_function(&name);
    if let Some(val) = func_val {
        return Ok(val);
    }
    let macro_val = macros.read().get(&name).cloned();
    if let Some(m) = macro_val {
        let args_body = LispObject::cons(m.args, m.body);
        let lambda_form = LispObject::cons(LispObject::symbol("lambda"), args_body);
        return Ok(LispObject::cons(LispObject::symbol("macro"), lambda_form));
    }
    Ok(LispObject::nil())
}
fn stateful_makunbound(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(id) = sym.as_symbol_id() {
        state.clear_value_cell(id);
    }
    Ok(sym)
}
fn stateful_fmakunbound(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(id) = sym.as_symbol_id() {
        state.clear_function_cell(id);
    }
    Ok(sym)
}
/// `(make-hash-table &rest KEYWORDS)` when invoked via funcall path.
/// Args are pre-evaluated; we only read `:test`.
fn stateful_make_hash_table(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let mut test = crate::object::HashTableTest::Eql;
    let mut cur = args.clone();
    while let Some((key, rest)) = cur.destructure_cons() {
        if let Some(s) = key.as_symbol() {
            if s == ":test" {
                if let Some((val, rest2)) = rest.destructure_cons() {
                    if let Some(t) = val.as_symbol() {
                        test = match t.as_str() {
                            "eq" => crate::object::HashTableTest::Eq,
                            "eql" => crate::object::HashTableTest::Eql,
                            "equal" => crate::object::HashTableTest::Equal,
                            _ => crate::object::HashTableTest::Eql,
                        };
                    }
                    cur = rest2;
                    continue;
                }
            }
        }
        cur = rest;
    }
    let v = state.heap_hashtable(crate::object::LispHashTable::new(test));
    Ok(value_to_obj(v))
}
thread_local! {
    #[doc = " Names currently inside `source_level_fallback`. If the same"] #[doc =
    " name re-enters, it means the source-level eval dispatch also"] #[doc =
    " funcalls through this name — definitely no implementation; bail"] #[doc =
    " with VoidFunction rather than infinite-looping."] pub(super) static FALLBACK_STACK :
    std::cell::RefCell < Vec < String >> = const { std::cell::RefCell::new(Vec::new()) };
}
pub(super) fn source_level_fallback(
    name: &str,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let already_active = FALLBACK_STACK.with(|st| st.borrow().iter().any(|n| n == name));
    if already_active {
        return Err(ElispError::VoidFunction(name.to_string()));
    }
    let quote_sym = LispObject::symbol("quote");
    let mut quoted_args: Vec<LispObject> = Vec::new();
    let mut cur = args.clone();
    while let Some((arg, rest)) = cur.destructure_cons() {
        let q = LispObject::cons(quote_sym.clone(), LispObject::cons(arg, LispObject::nil()));
        quoted_args.push(q);
        cur = rest;
    }
    let mut arg_list = LispObject::nil();
    for q in quoted_args.into_iter().rev() {
        arg_list = LispObject::cons(q, arg_list);
    }
    let form = LispObject::cons(LispObject::symbol(name), arg_list);
    let _guard = FallbackFrame::new(name);
    eval(obj_to_value(form), env, editor, macros, state)
}
/// `(make-closure PROTOTYPE &rest CLOSURE-VARS)` — Emacs byte-compiled
/// code uses this to construct closures at runtime. We don't have a
/// bytecode prototype model that matches Emacs's; for interpreted code,
/// the PROTOTYPE is usually a lambda already bound to the env that
/// produced it. Simplest correct-enough stub: return PROTOTYPE. Tests
/// that depend on the captured-vars mechanism will fail but no longer
/// "void function".
fn stateful_make_closure(args: &LispObject) -> ElispResult<LispObject> {
    let proto = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(proto)
}
/// `(defalias SYMBOL DEFINITION)` — set SYMBOL's function definition.
/// When DEFINITION is `(macro lambda ARGS . BODY)`, register as a
/// macro in the macros table; otherwise write the obarray function
/// cell. Args are pre-evaluated.
fn stateful_defalias(
    args: &LispObject,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let name_str = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    if let Some((car, rest)) = value.destructure_cons() {
        if car.as_symbol().as_deref() == Some("macro") {
            if let Some((lambda_sym, lambda_rest)) = rest.destructure_cons() {
                if lambda_sym.as_symbol().as_deref() == Some("lambda") {
                    let macro_args = lambda_rest.first().unwrap_or(LispObject::nil());
                    let macro_body = lambda_rest.rest().unwrap_or(LispObject::nil());
                    macros.write().insert(
                        name_str.clone(),
                        Macro {
                            args: macro_args,
                            body: macro_body,
                        },
                    );
                    return Ok(LispObject::symbol(&name_str));
                }
            }
        }
    }
    let id = crate::obarray::intern(&name_str);
    state.set_function_cell(id, value);
    Ok(LispObject::symbol(&name_str))
}
/// `(fset SYMBOL DEFINITION)` — set SYMBOL's function cell. Like
/// `defalias` but doesn't check for the macro form. Args pre-evaluated.
fn stateful_fset(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let def = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = name
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    state.set_function_cell(sym_id, def.clone());
    Ok(def)
}
