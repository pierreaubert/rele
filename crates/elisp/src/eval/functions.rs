// Function calling and application: funcall, apply, lambda application.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{obj_to_value, value_to_obj, Value};
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::sync::Arc;

use super::dynamic::{bind_param_dynamic, unwind_specpdl};
use super::{eval, eval_progn, Environment, InterpreterState, Macro, MacroTable};

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
        "defalias" => Some(stateful_defalias(args, macros)),
        "fset" => Some(stateful_fset(args)),
        "eval" => Some(stateful_eval(args, env, editor, macros, state)),
        "funcall" => Some(stateful_funcall(args, env, editor, macros, state)),
        "apply" => Some(stateful_apply(args, env, editor, macros, state)),
        "put" => Some(stateful_put(args)),
        "get" => Some(stateful_get(args)),
        "boundp" => Some(stateful_boundp(args, env)),
        "fboundp" => Some(stateful_fboundp(args, env, macros)),
        "intern" => Some(stateful_intern(args)),
        "intern-soft" => Some(stateful_intern_soft(args, env)),
        "set" => Some(stateful_set(args)),
        "symbol-value" => Some(stateful_symbol_value(args, env)),
        "symbol-function" => Some(stateful_symbol_function(args, env, macros, state)),
        "default-value" => Some(stateful_symbol_value(args, env)),
        "default-boundp" => Some(stateful_boundp(args, env)),
        "set-default" => Some(stateful_set_default(args, env)),
        "makunbound" => Some(stateful_makunbound(args)),
        "fmakunbound" => Some(stateful_fmakunbound(args)),
        "make-hash-table" => Some(stateful_make_hash_table(args, state)),
        "make-closure" => Some(stateful_make_closure(args)),
        "vector" => Some(stateful_vector(args, state)),
        "format" | "format-message" | "message" => {
            Some(stateful_format(args, env, editor, macros, state))
        }
        "throw" => Some(stateful_throw(args)),
        "signal" => Some(stateful_signal(args)),
        "error" => Some(stateful_error(args, env, editor, macros, state)),
        _ => {
            // Buffer / marker / narrow / regex / match-data.
            if let Some(r) = crate::primitives_buffer::call_buffer_primitive(name, args) {
                return Some(r);
            }
            // Window / frame / keymap.
            if let Some(r) = crate::primitives_window::call_window_primitive(name, args) {
                return Some(r);
            }
            // File / filename / regex / env.
            if let Some(r) = crate::primitives_file::call_file_primitive(name, args) {
                return Some(r);
            }
            // EIEIO / advice / widgets.
            if let Some(r) = crate::primitives_eieio::call_eieio_primitive(name, args) {
                return Some(r);
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
        // First arg is the format string — leave as-is (eval'ing a
        // string returns itself). Wrap subsequent args in (quote X) so
        // eval_format's internal eval is a no-op.
        if wrapped.is_empty() {
            wrapped.push(car);
        } else {
            let quoted = LispObject::cons(
                quote_sym.clone(),
                LispObject::cons(car, LispObject::nil()),
            );
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
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = sym
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    Ok(LispObject::from(env.read().get(&name).is_some()))
}

fn stateful_fboundp(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    macros: &MacroTable,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = sym
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
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
) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = match &arg {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Ok(LispObject::nil()),
    };
    if env.read().get(&name).is_some() {
        Ok(LispObject::symbol(&name))
    } else {
        Ok(LispObject::nil())
    }
}

fn stateful_set(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = sym
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    crate::obarray::set_value_cell(sym_id, val.clone());
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
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = sym
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    Ok(env.read().get(&name).unwrap_or(LispObject::nil()))
}

fn stateful_symbol_function(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    macros: &MacroTable,
    _state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = sym
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    if let Some(val) = env.read().get_function(&name) {
        return Ok(val);
    }
    if let Some(m) = macros.read().get(&name).cloned() {
        // Build `(macro lambda ARGS . BODY)`.
        let args_body = LispObject::cons(m.args, m.body);
        let lambda_form = LispObject::cons(LispObject::symbol("lambda"), args_body);
        return Ok(LispObject::cons(LispObject::symbol("macro"), lambda_form));
    }
    Ok(LispObject::nil())
}

fn stateful_makunbound(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(id) = sym.as_symbol_id() {
        crate::obarray::set_value_cell(id, LispObject::nil());
    }
    Ok(sym)
}

fn stateful_fmakunbound(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(id) = sym.as_symbol_id() {
        crate::obarray::set_function_cell(id, LispObject::nil());
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
    /// Names currently inside `source_level_fallback`. If the same
    /// name re-enters, it means the source-level eval dispatch also
    /// funcalls through this name — definitely no implementation; bail
    /// with VoidFunction rather than infinite-looping.
    static FALLBACK_STACK: std::cell::RefCell<Vec<String>> = const {
        std::cell::RefCell::new(Vec::new())
    };
}

fn source_level_fallback(
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

    // Build `(quote X)` for each pre-evaluated arg; keeping original
    // cons shape for the arg list.
    let quote_sym = LispObject::symbol("quote");
    let mut quoted_args: Vec<LispObject> = Vec::new();
    let mut cur = args.clone();
    while let Some((arg, rest)) = cur.destructure_cons() {
        let q = LispObject::cons(
            quote_sym.clone(),
            LispObject::cons(arg, LispObject::nil()),
        );
        quoted_args.push(q);
        cur = rest;
    }
    let mut arg_list = LispObject::nil();
    for q in quoted_args.into_iter().rev() {
        arg_list = LispObject::cons(q, arg_list);
    }
    let form = LispObject::cons(LispObject::symbol(name), arg_list);

    FALLBACK_STACK.with(|st| st.borrow_mut().push(name.to_string()));
    let res = eval(obj_to_value(form), env, editor, macros, state);
    FALLBACK_STACK.with(|st| {
        st.borrow_mut().pop();
    });
    res
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
fn stateful_defalias(args: &LispObject, macros: &MacroTable) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let name_str = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    // Macro case: DEFINITION = (macro lambda ARGS . BODY)
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

    // Function case: write the obarray function cell directly.
    let id = crate::obarray::intern(&name_str);
    crate::obarray::set_function_cell(id, value);
    Ok(LispObject::symbol(&name_str))
}

/// `(fset SYMBOL DEFINITION)` — set SYMBOL's function cell. Like
/// `defalias` but doesn't check for the macro form. Args pre-evaluated.
fn stateful_fset(args: &LispObject) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let def = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = name
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    crate::obarray::set_function_cell(sym_id, def.clone());
    Ok(def)
}

/// `(eval FORM)` — evaluate FORM. Args pre-evaluated means FORM is the
/// ACTUAL form to evaluate (not wrapped in another eval).
fn stateful_eval(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let form = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = eval(obj_to_value(form), env, editor, macros, state)?;
    Ok(value_to_obj(result))
}

/// `(funcall FUNC &rest ARGS)` — call FUNC with ARGS. All args
/// pre-evaluated.
fn stateful_funcall(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let func = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let rest = args.rest().unwrap_or(LispObject::nil());
    let result = call_function(
        obj_to_value(func),
        obj_to_value(rest),
        env,
        editor,
        macros,
        state,
    )?;
    Ok(value_to_obj(result))
}

/// `(put SYMBOL PROPERTY VALUE)` — set SYMBOL's plist PROPERTY to
/// VALUE. All args pre-evaluated.
fn stateful_put(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = sym
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let prop_id = prop
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    crate::obarray::put_plist(sym_id, prop_id, val.clone());
    Ok(val)
}

/// `(get SYMBOL PROPERTY)` — get SYMBOL's plist PROPERTY. Args
/// pre-evaluated.
fn stateful_get(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = sym
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let prop_id = prop
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    Ok(crate::obarray::get_plist(sym_id, prop_id))
}

/// `(apply FUNC &rest ARGS)` — last ARG is a list; splice its contents
/// as trailing arguments. All args pre-evaluated.
fn stateful_apply(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let func = args.first().ok_or(ElispError::WrongNumberOfArguments)?;

    // Walk args[1..], collecting into `collected`; the final element
    // stays in `last` until we know whether more arguments follow.
    // When the loop exits, `last` is the list to splice.
    let mut collected: Vec<LispObject> = Vec::new();
    let mut last: Option<LispObject> = None;
    let mut current = args.rest().unwrap_or(LispObject::nil());
    while let Some((car, cdr)) = current.destructure_cons() {
        if let Some(prev) = last.take() {
            collected.push(prev);
        }
        last = Some(car);
        current = cdr;
    }
    if let Some(list) = last {
        let mut cur = list;
        while let Some((car, cdr)) = cur.destructure_cons() {
            collected.push(car);
            cur = cdr;
        }
    }

    // Rebuild a proper Lisp argument list (nil-terminated cons chain).
    let mut arg_list = LispObject::nil();
    for item in collected.into_iter().rev() {
        arg_list = LispObject::cons(item, arg_list);
    }

    let result = call_function(
        obj_to_value(func),
        obj_to_value(arg_list),
        env,
        editor,
        macros,
        state,
    )?;
    Ok(value_to_obj(result))
}

pub(super) fn eval_funcall(
    func: Value,
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let func = resolve_function(func, env, editor, macros, state)?;
    let args = eval_list(args, env, editor, macros, state)?;

    call_function(func, args, env, editor, macros, state)
}
pub(super) fn resolve_function(
    func: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let func_obj = value_to_obj(func);
    if let LispObject::Symbol(id) = func_obj {
        let name = crate::obarray::symbol_name(id);
        if let Some(val) = env.read().get_function(&name) {
            return Ok(obj_to_value(val));
        }
        // Check autoloads — load the file and retry
        if let Some(file) = state.autoloads.read().get(&name).cloned() {
            let load_args = LispObject::cons(
                LispObject::string(&file),
                LispObject::cons(LispObject::t(), LispObject::nil()),
            );
            let _ = super::builtins::eval_load(obj_to_value(load_args), env, editor, macros, state);
            if let Some(val) = env.read().get_function(&name) {
                return Ok(obj_to_value(val));
            }
        }
        Err(ElispError::VoidFunction(name))
    } else {
        eval(func, env, editor, macros, state)
    }
}
pub(super) fn eval_apply(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let func = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let func_val = eval(obj_to_value(func), env, editor, macros, state)?;

    let mut raw_args: Vec<LispObject> = Vec::new();
    let mut cur = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    while let Some((car, rest)) = cur.destructure_cons() {
        raw_args.push(car);
        cur = rest;
    }
    if raw_args.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }

    let mut evaled: Vec<LispObject> = Vec::new();
    for a in &raw_args {
        evaled.push(value_to_obj(eval(
            obj_to_value(a.clone()),
            env,
            editor,
            macros,
            state,
        )?));
    }

    let last = evaled.pop().unwrap();
    let mut combined: Vec<LispObject> = evaled;

    let mut tail = last;
    while let Some((car, rest)) = tail.destructure_cons() {
        combined.push(car);
        tail = rest;
    }

    let mut all_args = LispObject::nil();
    for arg in combined.iter().rev() {
        all_args = LispObject::cons(arg.clone(), all_args);
    }

    call_function(func_val, obj_to_value(all_args), env, editor, macros, state)
}
pub(super) fn eval_funcall_form(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let func = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let rest_args = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let func_val = eval(obj_to_value(func), env, editor, macros, state)?;
    let evaled_args = eval_list(obj_to_value(rest_args), env, editor, macros, state)?;

    call_function(func_val, evaled_args, env, editor, macros, state)
}
pub(super) fn eval_list(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    // Value-native arg-list evaluation.
    //
    // We walk the source-form cons chain (which may be ConsCell or
    // ConsArcCell) and collect each evaluated arg into a Vec<Value>,
    // then build the output list from raw ConsCells via
    // `state.list_from_values`. This matters because the Value-native
    // primitive fast path walks ConsCells without taking any mutexes,
    // but can't walk ConsArcCells efficiently — so building the arg
    // list as raw ConsCells lets all numeric / comparison / predicate
    // primitives hit the fast path.
    //
    // Previously this function round-tripped through `LispObject::cons`,
    // which always allocates ConsArcCells and defeats the fast path.
    let mut evaluated: Vec<Value> = Vec::new();
    let mut current = args;
    loop {
        let obj = value_to_obj(current);
        match obj {
            LispObject::Nil => break,
            LispObject::Cons(_) => {
                let (car, cdr) = obj.destructure();
                let v = eval(obj_to_value(car), env, editor, macros, state)?;
                evaluated.push(v);
                current = obj_to_value(cdr);
            }
            _ => return Err(ElispError::WrongTypeArgument("list".to_string())),
        }
    }
    Ok(state.list_from_values(evaluated))
}
pub(super) fn apply_lambda(
    params: &LispObject,
    body: &LispObject,
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let parent_env = Arc::new(env.read().clone());
    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    let specpdl_depth = state.specpdl.read().len();

    let mut params_list = params.clone();
    let mut args_list = value_to_obj(args);
    let mut optional = false;
    let mut rest = false;

    loop {
        if params_list.is_nil() {
            break;
        }
        let (param, params_rest) = match params_list.destructure_cons() {
            Some((p, r)) => (p, r),
            None => {
                if let Some(name) = params_list.as_symbol() {
                    bind_param_dynamic(&name, args_list, &new_env, state);
                }
                break;
            }
        };

        if let Some(name) = param.as_symbol() {
            match name.as_str() {
                "&optional" => {
                    optional = true;
                    params_list = params_rest;
                    continue;
                }
                "&rest" | "&body" => {
                    rest = true;
                    params_list = params_rest;
                    continue;
                }
                _ => {}
            }

            if rest {
                bind_param_dynamic(&name, args_list.clone(), &new_env, state);
                args_list = LispObject::nil();
                params_list = params_rest;
                continue;
            }

            let (arg, args_rest) = match args_list.destructure_cons() {
                Some((a, r)) => (a, r),
                None => {
                    if optional || rest {
                        (LispObject::nil(), LispObject::nil())
                    } else {
                        unwind_specpdl(state, specpdl_depth);
                        return Err(ElispError::WrongNumberOfArguments);
                    }
                }
            };
            bind_param_dynamic(&name, arg, &new_env, state);
            args_list = args_rest;
        }
        params_list = params_rest;
    }

    let result = eval_progn(obj_to_value(body.clone()), &new_env, editor, macros, state);
    unwind_specpdl(state, specpdl_depth);
    result
}
pub fn call_function(
    func: Value,
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let func_obj = value_to_obj(func);
    match func_obj {
        LispObject::Cons(ref cell) => {
            let (car_val, cdr_val) = {
                let b = cell.lock();
                (b.0.clone(), b.1.clone())
            };
            if let Some(s) = car_val.as_symbol() {
                if s == "lambda" {
                    // Bare lambda — no captured env. Used by `defun` (which
                    // stores in the function cell) and by any code that
                    // constructs a lambda form manually. Uses the caller's
                    // env as the enclosing scope for free variables.
                    let params = cdr_val.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr_val.rest().ok_or(ElispError::WrongNumberOfArguments)?;
                    return apply_lambda(&params, &body, args, env, editor, macros, state);
                }
                if s == "closure" {
                    // (closure CAPTURED-ALIST PARAMS . BODY) — re-hydrate
                    // the captured env and use it as the parent scope for
                    // arg bindings and body evaluation. Free variables are
                    // resolved against the captured alist, falling back to
                    // the global env for anything not captured (built-ins,
                    // top-level defvars, etc.).
                    let captured_alist =
                        cdr_val.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let rest = cdr_val.rest().ok_or(ElispError::WrongNumberOfArguments)?;
                    let params = rest.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = rest.rest().ok_or(ElispError::WrongNumberOfArguments)?;

                    // Rebuild env: parent = clone of global, bindings = alist entries.
                    let global_snapshot = Arc::new(state.global_env.read().clone());
                    let mut captured_env = Environment::with_parent(global_snapshot);
                    let mut cur = captured_alist;
                    while let Some((pair, rest_alist)) = cur.destructure_cons() {
                        if let Some((k, v)) = pair.destructure_cons() {
                            if let Some(id) = k.as_symbol_id() {
                                captured_env.define_id(id, v);
                            }
                        }
                        cur = rest_alist;
                    }
                    let closure_env = Arc::new(RwLock::new(captured_env));
                    return apply_lambda(&params, &body, args, &closure_env, editor, macros, state);
                }
            }
            Err(ElispError::WrongTypeArgument("function".to_string()))
        }
        LispObject::Primitive(ref name) => {
            // Fast path: try Value-native dispatch for hot primitives
            // (arithmetic, comparison, cons, null, eq, …). This avoids
            // the obj_to_value/value_to_obj round-trip and the per-cons
            // mutex lock that the LispObject path does. Falls through
            // to the slow path if the primitive isn't handled or args
            // aren't all in the fast-path-compatible shape.
            //
            // Measured win: ~30% speedup on `(+ 1 2 3)` — 515 ns/call
            // vs 725 ns/call with the fast path disabled. Prerequisite:
            // `eval_list` builds raw ConsCells (not ConsArcCells) so the
            // fast path's pointer-tag-based walk can actually walk them.
            if let Some(result) =
                crate::primitives_value::try_call_primitive_value(name, args)
            {
                crate::primitives_value::inc_fast_hits();
                return result;
            }
            crate::primitives_value::inc_slow_hits();

            let args_obj = value_to_obj(args);
            // Phase 7a: try state-aware primitives first (defalias,
            // fset, eval, funcall, apply…). These are registered as
            // `LispObject::Primitive` in the function cell so they're
            // reachable from the VM's function-call path, not just
            // the source-level special-form dispatch.
            if let Some(result) =
                call_stateful_primitive(name, &args_obj, env, editor, macros, state)
            {
                return Ok(obj_to_value(result?));
            }
            match crate::primitives::call_primitive(name, &args_obj) {
                Ok(v) => Ok(obj_to_value(v)),
                Err(ElispError::VoidFunction(_)) => {
                    // Fallback: many primitives are only implemented as
                    // source-level dispatch arms in eval/mod.rs (because
                    // they need env/state access) and aren't reachable
                    // via `call_primitive`. Synthesize a source form
                    // `(NAME (quote ARG1) (quote ARG2) ...)` and hand
                    // it to `eval`. The `(quote X)` wrapping ensures
                    // eval won't re-evaluate pre-evaluated args. A
                    // thread-local guard prevents infinite recursion
                    // when the same name has no handler anywhere.
                    source_level_fallback(name, &args_obj, env, editor, macros, state)
                }
                Err(e) => Err(e),
            }
        }
        LispObject::BytecodeFn(ref bc) => {
            let func_id = bc as *const _ as usize;
            #[allow(unused_variables)]
            let should_jit = state.profiler.write().record_call(func_id);

            let args_obj = value_to_obj(args);
            let mut arg_vec = Vec::new();
            let mut current = args_obj;
            while let Some((car, cdr)) = current.destructure_cons() {
                arg_vec.push(car);
                current = cdr;
            }

            #[cfg(feature = "jit")]
            {
                let mut jit = state.jit.write();
                if should_jit && !jit.is_compiled(func_id) {
                    jit.compile(func_id, bc);
                }
                if let Some(id) = jit.get_compiled(func_id) {
                    {
                        let jit_args: Vec<i64> = arg_vec
                            .iter()
                            .map(|a| crate::value::Value::from_lisp_object(a).raw() as i64)
                            .collect();
                        if let Some(native_result) = jit.call(id, &jit_args) {
                            match native_result {
                                crate::jit::NativeResult::Ok(raw) => {
                                    let val = crate::value::Value::from_raw(raw);
                                    return Ok(val);
                                }
                                crate::jit::NativeResult::Deoptimize => {
                                    // Fall through to VM
                                }
                            }
                        }
                    }
                }
            }

            let result = crate::vm::execute_bytecode(bc, &arg_vec, env, editor, macros, state)?;
            Ok(obj_to_value(result))
        }
        LispObject::Symbol(id) => {
            let name = crate::obarray::symbol_name(id);
            // Function-position lookup: env chain for callable, then
            // the symbol's function cell.
            if let Some(val) = env.read().get_function(&name) {
                return call_function(obj_to_value(val), args, env, editor, macros, state);
            }
            // Check autoloads — load the file and retry
            if let Some(file) = state.autoloads.read().get(&name).cloned() {
                let load_args = LispObject::cons(
                    LispObject::string(&file),
                    LispObject::cons(LispObject::t(), LispObject::nil()),
                );
                let _ =
                    super::builtins::eval_load(obj_to_value(load_args), env, editor, macros, state);
                if let Some(val) = env.read().get_function(&name) {
                    return call_function(obj_to_value(val), args, env, editor, macros, state);
                }
            }
            // Last-chance fallback: many core functions (e.g.
            // `expand-file-name`, `looking-at`, `symbol-value`,
            // `make-hash-table` with complex keyword args) are only
            // wired as source-level special-form dispatch arms in
            // eval/mod.rs. They're not in the function cell and
            // therefore not reachable via call_function. Synthesize
            // `(NAME (quote ARG1) (quote ARG2) ...)` and re-enter
            // `eval` — `quote` ensures we don't double-eval.
            let args_obj = value_to_obj(args);
            source_level_fallback(&name, &args_obj, env, editor, macros, state)
        }
        _ => Err(ElispError::WrongTypeArgument("function".to_string())),
    }
}
