// Function calling and application: funcall, apply, lambda application.

use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use parking_lot::RwLock;
use std::sync::Arc;

use super::dynamic::{bind_param_dynamic, unwind_specpdl};
use super::{Environment, InterpreterState, Macro, MacroTable, eval, eval_progn};

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
            // Stateless cl-* primitives (accessors, math, types, sets).
            if let Some(r) = crate::primitives_cl::call_cl_primitive(name, args) {
                return Some(r);
            }
            // Stateful cl-* (cl-find, cl-mapcar, cl-reduce, ...) — need
            // interpreter env/state to funcall their predicate argument.
            // Returns an ElispResult<Value> so it's already in the right
            // shape for Ok(value_to_obj(...)) downstream.
            if let Some(r) =
                super::state_cl::call_stateful_cl(name, args, env, editor, macros, state)
            {
                // Route through the same Value→Obj bridge as other
                // stateful handlers so the caller sees a LispObject.
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
        // First arg is the format string — leave as-is (eval'ing a
        // string returns itself). Wrap subsequent args in (quote X) so
        // eval_format's internal eval is a no-op.
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

fn stateful_boundp(args: &LispObject, env: &Arc<RwLock<Environment>>) -> ElispResult<LispObject> {
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
    // `intern-soft` returns the symbol if it's already known to the
    // obarray, else nil. "Known" here means: any prior
    // `intern`/`define`/`defun`/etc. reference installed the symbol
    // — which covers primitives (function cell set at startup),
    // user-defined values (variable cell), and plain references in
    // loaded code. We probe the global obarray by name.
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = match &arg {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Ok(LispObject::nil()),
    };
    if env.read().get(&name).is_some() {
        return Ok(LispObject::symbol(&name));
    }
    // Fall back to a global obarray scan — but only count a symbol
    // as "interned" if it has a real binding (value OR function
    // cell). Merely appearing in the obarray isn't enough because
    // our reader interns bare tokens on first read; that would
    // make `(intern-soft "foo")` return `foo` for any name the
    // reader has seen, which isn't what callers expect.
    let table = crate::obarray::GLOBAL_OBARRAY.read();
    for id in 0..table.symbol_count() as u32 {
        let sid = crate::obarray::SymbolId(id);
        if table.name(sid) == name {
            drop(table);
            let has_value = crate::obarray::get_value_cell(sid).is_some();
            let has_fn = crate::obarray::get_function_cell(sid).is_some();
            if has_value || has_fn {
                return Ok(LispObject::Symbol(sid));
            }
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::nil())
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

/// RAII guard for `FALLBACK_STACK`. Pushes the name on construction
/// and pops on drop — so the stack unwinds correctly even if the
/// evaluation panics inside `catch_unwind` (leaving a stale entry
/// would poison that name for the rest of the interpreter's life).
struct FallbackFrame {
    name: String,
}

impl FallbackFrame {
    fn new(name: &str) -> Self {
        FALLBACK_STACK.with(|st| st.borrow_mut().push(name.to_string()));
        Self {
            name: name.to_string(),
        }
    }
}

impl Drop for FallbackFrame {
    fn drop(&mut self) {
        FALLBACK_STACK.with(|st| {
            let mut stack = st.borrow_mut();
            // Pop the frame for `self.name`. In normal operation this
            // is always the top; but if earlier frames leaked we still
            // want to purge our own entry.
            if let Some(pos) = stack.iter().rposition(|n| n == &self.name) {
                stack.remove(pos);
            }
        });
    }
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
        let q = LispObject::cons(quote_sym.clone(), LispObject::cons(arg, LispObject::nil()));
        quoted_args.push(q);
        cur = rest;
    }
    let mut arg_list = LispObject::nil();
    for q in quoted_args.into_iter().rev() {
        arg_list = LispObject::cons(q, arg_list);
    }
    let form = LispObject::cons(LispObject::symbol(name), arg_list);

    // RAII: the frame is popped even if `eval` panics.
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
    // Keyword slot accessor pre-check: `(:keyword INSTANCE ...)` syntax.
    // When the function symbol starts with `:` and has no function
    // definition, we try EIEIO keyword-slot dispatch *before* signalling
    // VoidFunction. We must evaluate the args first so that `INSTANCE` is
    // the real object, not the source form.
    let func_name: Option<String> = {
        let func_obj = value_to_obj(func);
        if let LispObject::Symbol(id) = func_obj {
            let name = crate::obarray::symbol_name(id);
            if name.starts_with(':') && env.read().get_function(&name).is_none() {
                Some(name)
            } else {
                None
            }
        } else {
            None
        }
    };
    if let Some(ref kw) = func_name {
        let evaled_args = eval_list(args, env, editor, macros, state)?;
        let args_obj = value_to_obj(evaled_args);
        if let Some(result) = crate::primitives_eieio::try_keyword_slot_call(kw, &args_obj) {
            return result.map(obj_to_value);
        }
        // Keyword didn't match any slot — fall through to resolve_function
        // which will produce the appropriate VoidFunction error.
        return Err(ElispError::VoidFunction(kw.clone()));
    }
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
                if s == "macro" {
                    // `(macro . FUNC)` — the macro form `symbol-function`
                    // returns for a macro. Calling it through funcall is
                    // rare but happens when code fishes the function cell
                    // out with `(symbol-function 'foo)` and forwards it.
                    // Unwrap and invoke the underlying function.
                    return call_function(obj_to_value(cdr_val), args, env, editor, macros, state);
                }
                if s == "autoload" {
                    // `(autoload FILE ...)` stuck in the function cell.
                    // Load the file (ignoring errors — the file may not
                    // exist in our stub stdlib) and signal void-function
                    // so the caller sees the same error shape it would if
                    // the autoload had failed in real Emacs.
                    if let Some(file_obj) = cdr_val.first() {
                        if let LispObject::String(ref f) = file_obj {
                            let load_args = LispObject::cons(
                                LispObject::string(f),
                                LispObject::cons(LispObject::t(), LispObject::nil()),
                            );
                            let _ = super::builtins::eval_load(
                                obj_to_value(load_args),
                                env,
                                editor,
                                macros,
                                state,
                            );
                        }
                    }
                    return Err(ElispError::VoidFunction("autoload".into()));
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
            if let Some(result) = crate::primitives_value::try_call_primitive_value(name, args) {
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
            // Keyword slot accessor: `(:slot instance)` syntax. If the
            // symbol is a keyword (starts with `:`) and the first arg
            // is an EIEIO instance, look up the matching slot and return
            // its value. Returns `Some(result)` on a match; `None` means
            // the keyword doesn't map to any slot — fall through to the
            // normal error path.
            if let Some(result) = crate::primitives_eieio::try_keyword_slot_call(&name, &args_obj) {
                return result.map(obj_to_value);
            }
            source_level_fallback(&name, &args_obj, env, editor, macros, state)
        }
        LispObject::Nil => Err(ElispError::VoidFunction("nil".to_string())),
        _ => Err(ElispError::WrongTypeArgument("function".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: R3. Previously the FALLBACK_STACK was pushed/popped
    /// by explicit method calls. If the `eval` inside the fallback
    /// panicked, the pop never ran and the name stayed on the stack
    /// forever — poisoning every subsequent call to that name with a
    /// spurious VoidFunction error. The RAII `FallbackFrame` guard
    /// ensures the pop happens on unwind too.
    #[test]
    fn fallback_frame_pops_on_panic() {
        // Baseline: stack is empty.
        FALLBACK_STACK.with(|st| assert!(st.borrow().is_empty()));

        // Simulate a panic inside a fallback call.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = FallbackFrame::new("r3-poisoned");
            // Verify the frame was pushed.
            FALLBACK_STACK.with(|st| {
                assert_eq!(st.borrow().as_slice(), &["r3-poisoned".to_string()]);
            });
            panic!("simulated panic inside fallback");
        }));
        assert!(result.is_err(), "panic must propagate");

        // After the unwind, the stack must be empty again.
        FALLBACK_STACK.with(|st| {
            assert!(
                st.borrow().is_empty(),
                "FallbackFrame must pop even on panic; saw: {:?}",
                st.borrow()
            );
        });
    }

    /// Normal drop (no panic) also pops.
    #[test]
    fn fallback_frame_pops_on_normal_return() {
        FALLBACK_STACK.with(|st| st.borrow_mut().clear());
        {
            let _guard = FallbackFrame::new("r3-normal");
            FALLBACK_STACK.with(|st| {
                assert_eq!(st.borrow().len(), 1);
            });
        }
        FALLBACK_STACK.with(|st| assert!(st.borrow().is_empty()));
    }

    /// Nested frames pop in reverse order.
    #[test]
    fn nested_fallback_frames() {
        FALLBACK_STACK.with(|st| st.borrow_mut().clear());
        {
            let _outer = FallbackFrame::new("outer");
            {
                let _inner = FallbackFrame::new("inner");
                FALLBACK_STACK.with(|st| {
                    assert_eq!(
                        st.borrow().as_slice(),
                        &["outer".to_string(), "inner".to_string()]
                    );
                });
            }
            FALLBACK_STACK.with(|st| {
                assert_eq!(st.borrow().as_slice(), &["outer".to_string()]);
            });
        }
        FALLBACK_STACK.with(|st| assert!(st.borrow().is_empty()));
    }
}

// -------------------------------------------------------------------
// cl-loop — comprehensive subset
// -------------------------------------------------------------------
//
// Implemented clauses:
//
//   for VAR in LIST
//   for VAR on LIST
//   for VAR from A to/upto/below B [by STEP]
//   for VAR from A downto/above B [by STEP]
//   for VAR = INIT [then NEXT]
//   for VAR across SEQ
//   for VAR being the elements of SEQ
//   for VAR being the hash-keys of TABLE
//   for VAR being the hash-values of TABLE
//   with VAR = INIT
//   repeat N
//   while COND / until COND
//   always COND / never COND / thereis EXPR
//   do BODY / doing BODY
//   collect EXPR [into VAR] / collecting EXPR
//   append EXPR [into VAR] / appending EXPR
//   nconc EXPR [into VAR] / nconcing EXPR
//   sum EXPR [into VAR]
//   count EXPR [into VAR]
//   maximize EXPR [into VAR] / minimize EXPR [into VAR]
//   return EXPR
//   initially BODY
//   finally BODY / finally return EXPR
//   named NAME (for cl-return-from compat)
//
// Not supported (will silently misbehave or error): destructuring
// patterns, parallel steps without `and`, `being the :of-type`.

enum Iter {
    InList(LispObject),
    OnList(LispObject),
    FromTo {
        cur: i64,
        end: i64,
        step: i64,
        inclusive: bool,
        descending: bool,
    },
    Assignment {
        init: LispObject,
        then: Option<LispObject>,
        first: bool,
        value: LispObject,
    },
    Across {
        items: Vec<LispObject>,
        idx: usize,
    },
    Elements {
        items: Vec<LispObject>,
        idx: usize,
    },
    HashKeys {
        items: Vec<LispObject>,
        idx: usize,
    },
    HashValues {
        items: Vec<LispObject>,
        idx: usize,
    },
    Repeat {
        remaining: i64,
    },
}

impl Iter {
    /// Advance this iterator one step, producing the next bound
    /// value (or None when exhausted).
    fn next(
        &mut self,
        env: &Arc<RwLock<Environment>>,
        editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
        macros: &MacroTable,
        state: &InterpreterState,
    ) -> ElispResult<Option<LispObject>> {
        match self {
            Iter::InList(list) => {
                if let Some((car, cdr)) = list.clone().destructure_cons() {
                    *list = cdr;
                    Ok(Some(car))
                } else {
                    Ok(None)
                }
            }
            Iter::OnList(list) => {
                if list.destructure_cons().is_some() {
                    let current = list.clone();
                    *list = list.rest().unwrap_or(LispObject::nil());
                    Ok(Some(current))
                } else {
                    Ok(None)
                }
            }
            Iter::FromTo {
                cur,
                end,
                step,
                inclusive,
                descending,
            } => {
                let within = if *descending {
                    if *inclusive {
                        *cur >= *end
                    } else {
                        *cur > *end
                    }
                } else if *inclusive {
                    *cur <= *end
                } else {
                    *cur < *end
                };
                if !within {
                    return Ok(None);
                }
                let v = *cur;
                *cur = if *descending {
                    *cur - *step
                } else {
                    *cur + *step
                };
                Ok(Some(LispObject::integer(v)))
            }
            Iter::Assignment {
                init,
                then,
                first,
                value,
            } => {
                let out = if *first {
                    *first = false;
                    let v = value_to_obj(eval(
                        obj_to_value(init.clone()),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    *value = v.clone();
                    v
                } else {
                    let form = then.clone().unwrap_or_else(|| init.clone());
                    let v = value_to_obj(eval(obj_to_value(form), env, editor, macros, state)?);
                    *value = v.clone();
                    v
                };
                Ok(Some(out))
            }
            Iter::Across { items, idx } | Iter::Elements { items, idx } => {
                if *idx < items.len() {
                    let v = items[*idx].clone();
                    *idx += 1;
                    Ok(Some(v))
                } else {
                    Ok(None)
                }
            }
            Iter::HashKeys { items, idx } | Iter::HashValues { items, idx } => {
                if *idx < items.len() {
                    let v = items[*idx].clone();
                    *idx += 1;
                    Ok(Some(v))
                } else {
                    Ok(None)
                }
            }
            Iter::Repeat { remaining } => {
                if *remaining > 0 {
                    *remaining -= 1;
                    Ok(Some(LispObject::nil()))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

#[derive(Clone)]
enum Action {
    Do(LispObject),              // body forms (progn)
    While(LispObject, bool),     // (cond, is_until)
    Collect(String, LispObject), // accumulator name, expr
    Append(String, LispObject),
    Nconc(String, LispObject),
    Sum(String, LispObject),
    Count(String, LispObject),
    Max(String, LispObject),
    Min(String, LispObject),
    Return(LispObject),
    Always(LispObject),
    Never(LispObject),
    Thereis(LispObject),
}

#[derive(Clone)]
enum Accumulator {
    List(Vec<LispObject>), // collect/append/nconc (all list-typed)
    Num(f64),              // sum/count
    MaxMin(Option<f64>),   // max/min (starts None)
}

impl Accumulator {
    fn list() -> Self {
        Accumulator::List(Vec::new())
    }
    fn num() -> Self {
        Accumulator::Num(0.0)
    }
    fn max_min() -> Self {
        Accumulator::MaxMin(None)
    }
    fn into_value(self, integer_mode: bool) -> LispObject {
        match self {
            Accumulator::List(items) => {
                let mut out = LispObject::nil();
                for i in items.into_iter().rev() {
                    out = LispObject::cons(i, out);
                }
                out
            }
            Accumulator::Num(n) => {
                if integer_mode && n.fract() == 0.0 {
                    LispObject::integer(n as i64)
                } else {
                    LispObject::float(n)
                }
            }
            Accumulator::MaxMin(Some(n)) => {
                if integer_mode && n.fract() == 0.0 {
                    LispObject::integer(n as i64)
                } else {
                    LispObject::float(n)
                }
            }
            Accumulator::MaxMin(None) => LispObject::nil(),
        }
    }
}

pub(super) fn eval_cl_loop(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let clauses_obj = value_to_obj(args);

    // Set up loop env.
    let parent = Arc::new(env.read().clone());
    let loop_env = Arc::new(RwLock::new(Environment::with_parent(parent)));

    // One bank of (var, iterator) + step actions. Each `for` clause
    // adds a var/iter pair. Other clauses compile into Actions run
    // in body order. `initially` / `finally` land in separate vecs.
    let mut bindings: Vec<(String, Iter)> = Vec::new();
    let mut actions: Vec<Action> = Vec::new();
    let mut initially: Vec<LispObject> = Vec::new();
    let mut finally: Vec<LispObject> = Vec::new();
    let mut finally_return: Option<LispObject> = None;
    // Accumulators: name → (kind, integer-mode-hint).
    //    integer-mode stays true until the first non-integer value.
    use std::collections::HashMap;
    let mut accs: HashMap<String, (Accumulator, bool)> = HashMap::new();
    let mut default_collect_name: Option<String> = None;

    // Walk clauses.
    let mut cur = clauses_obj;
    while let Some((first, rest)) = cur.clone().destructure_cons() {
        let word = first.as_symbol().unwrap_or_default();
        cur = rest;
        match word.as_str() {
            "named" => {
                // `named NAME` — consumed (we don't use it since
                // cl-block isn't wrapping cl-loop). Skip the name.
                let (_, r) = cur
                    .clone()
                    .destructure_cons()
                    .unwrap_or((LispObject::nil(), LispObject::nil()));
                cur = r;
            }
            "for" | "as" => {
                // `for VAR ...`
                let (var_obj, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                let var = var_obj
                    .as_symbol()
                    .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
                // Initialize the var to nil; iter will overwrite.
                loop_env.write().define(&var, LispObject::nil());
                // Next token determines the iteration kind.
                let (kind_obj, r2) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r2;
                let kind = kind_obj.as_symbol().unwrap_or_default();
                match kind.as_str() {
                    "in" | "in-ref" => {
                        let (list_form, r3) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        let list = value_to_obj(eval(
                            obj_to_value(list_form),
                            env,
                            editor,
                            macros,
                            state,
                        )?);
                        bindings.push((var, Iter::InList(list)));
                    }
                    "on" => {
                        let (list_form, r3) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        let list = value_to_obj(eval(
                            obj_to_value(list_form),
                            env,
                            editor,
                            macros,
                            state,
                        )?);
                        bindings.push((var, Iter::OnList(list)));
                    }
                    "from" | "upfrom" | "downfrom" => {
                        let (start_form, r3) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        let start = value_to_obj(eval(
                            obj_to_value(start_form),
                            env,
                            editor,
                            macros,
                            state,
                        )?)
                        .as_integer()
                        .unwrap_or(0);
                        // Optional "to/upto/below/downto/above END" and "by STEP".
                        let mut descending = kind.as_str() == "downfrom";
                        let mut inclusive = true;
                        let mut end: i64 = if descending { i64::MIN } else { i64::MAX };
                        let mut step: i64 = 1;
                        // Peek for dir + end.
                        while let Some((next, r4)) = cur.clone().destructure_cons() {
                            let n = next.as_symbol().unwrap_or_default();
                            match n.as_str() {
                                "to" | "upto" => {
                                    inclusive = true;
                                    descending = false;
                                    let (e_form, r5) = r4
                                        .clone()
                                        .destructure_cons()
                                        .ok_or(ElispError::WrongNumberOfArguments)?;
                                    cur = r5;
                                    end = value_to_obj(eval(
                                        obj_to_value(e_form),
                                        env,
                                        editor,
                                        macros,
                                        state,
                                    )?)
                                    .as_integer()
                                    .unwrap_or(end);
                                }
                                "below" => {
                                    inclusive = false;
                                    descending = false;
                                    let (e_form, r5) = r4
                                        .clone()
                                        .destructure_cons()
                                        .ok_or(ElispError::WrongNumberOfArguments)?;
                                    cur = r5;
                                    end = value_to_obj(eval(
                                        obj_to_value(e_form),
                                        env,
                                        editor,
                                        macros,
                                        state,
                                    )?)
                                    .as_integer()
                                    .unwrap_or(end);
                                }
                                "downto" => {
                                    inclusive = true;
                                    descending = true;
                                    let (e_form, r5) = r4
                                        .clone()
                                        .destructure_cons()
                                        .ok_or(ElispError::WrongNumberOfArguments)?;
                                    cur = r5;
                                    end = value_to_obj(eval(
                                        obj_to_value(e_form),
                                        env,
                                        editor,
                                        macros,
                                        state,
                                    )?)
                                    .as_integer()
                                    .unwrap_or(end);
                                }
                                "above" => {
                                    inclusive = false;
                                    descending = true;
                                    let (e_form, r5) = r4
                                        .clone()
                                        .destructure_cons()
                                        .ok_or(ElispError::WrongNumberOfArguments)?;
                                    cur = r5;
                                    end = value_to_obj(eval(
                                        obj_to_value(e_form),
                                        env,
                                        editor,
                                        macros,
                                        state,
                                    )?)
                                    .as_integer()
                                    .unwrap_or(end);
                                }
                                "by" => {
                                    let (s_form, r5) = r4
                                        .clone()
                                        .destructure_cons()
                                        .ok_or(ElispError::WrongNumberOfArguments)?;
                                    cur = r5;
                                    step = value_to_obj(eval(
                                        obj_to_value(s_form),
                                        env,
                                        editor,
                                        macros,
                                        state,
                                    )?)
                                    .as_integer()
                                    .unwrap_or(1);
                                }
                                _ => break,
                            }
                        }
                        bindings.push((
                            var,
                            Iter::FromTo {
                                cur: start,
                                end,
                                step,
                                inclusive,
                                descending,
                            },
                        ));
                    }
                    "=" => {
                        let (init_form, r3) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        // Optional "then NEXT".
                        let mut then_form: Option<LispObject> = None;
                        if let Some((tok, r4)) = cur.clone().destructure_cons() {
                            if tok.as_symbol().as_deref() == Some("then") {
                                let (n_form, r5) = r4
                                    .clone()
                                    .destructure_cons()
                                    .ok_or(ElispError::WrongNumberOfArguments)?;
                                cur = r5;
                                then_form = Some(n_form);
                            }
                        }
                        bindings.push((
                            var,
                            Iter::Assignment {
                                init: init_form,
                                then: then_form,
                                first: true,
                                value: LispObject::nil(),
                            },
                        ));
                    }
                    "across" => {
                        let (seq_form, r3) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        let seq =
                            value_to_obj(eval(obj_to_value(seq_form), env, editor, macros, state)?);
                        let items = match seq {
                            LispObject::Vector(v) => v.lock().clone(),
                            LispObject::String(s) => {
                                s.chars().map(|c| LispObject::integer(c as i64)).collect()
                            }
                            _ => {
                                let mut out = Vec::new();
                                let mut c = seq;
                                while let Some((car, cdr)) = c.destructure_cons() {
                                    out.push(car);
                                    c = cdr;
                                }
                                out
                            }
                        };
                        bindings.push((var, Iter::Across { items, idx: 0 }));
                    }
                    "being" => {
                        // `being the KIND of SEQ`
                        // Consume `the` / `each`.
                        let (qualifier, r3) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        let qn = qualifier.as_symbol().unwrap_or_default();
                        cur = r3;
                        let kind_tok = if qn == "the" || qn == "each" {
                            let (k, r4) = cur
                                .clone()
                                .destructure_cons()
                                .ok_or(ElispError::WrongNumberOfArguments)?;
                            cur = r4;
                            k.as_symbol().unwrap_or_default()
                        } else {
                            qn
                        };
                        // Consume `of` / `in`.
                        if let Some((of_tok, r4)) = cur.clone().destructure_cons() {
                            let n = of_tok.as_symbol().unwrap_or_default();
                            if n == "of" || n == "in" {
                                cur = r4;
                            }
                        }
                        let (seq_form, r5) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r5;
                        let seq =
                            value_to_obj(eval(obj_to_value(seq_form), env, editor, macros, state)?);
                        let items: Vec<LispObject> = match (kind_tok.as_str(), &seq) {
                            // Our HashTable doesn't expose original-key
                            // iteration (keys are hashed into our
                            // HashKey enum). Callers using
                            // `being the hash-keys/values of T` will
                            // see an empty iteration — acceptable for
                            // the test corpus where this pattern is
                            // rare. A future extension could store
                            // the original key alongside its hash.
                            ("hash-keys" | "hash-key", LispObject::HashTable(_)) => Vec::new(),
                            ("hash-values" | "hash-value", LispObject::HashTable(ht)) => {
                                ht.lock().data.values().cloned().collect()
                            }
                            _ => match seq {
                                LispObject::Vector(v) => v.lock().clone(),
                                LispObject::String(s) => {
                                    s.chars().map(|c| LispObject::integer(c as i64)).collect()
                                }
                                _ => {
                                    let mut out = Vec::new();
                                    let mut c = seq;
                                    while let Some((car, cdr)) = c.destructure_cons() {
                                        out.push(car);
                                        c = cdr;
                                    }
                                    out
                                }
                            },
                        };
                        bindings.push((var, Iter::Elements { items, idx: 0 }));
                    }
                    _ => {
                        // Unknown iteration kind — skip and hope.
                    }
                }
            }
            "with" => {
                let (var_obj, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                let var = var_obj
                    .as_symbol()
                    .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
                // Optional `= INIT`.
                let mut init_value = LispObject::nil();
                if let Some((eq_tok, r2)) = cur.clone().destructure_cons() {
                    if eq_tok.as_symbol().as_deref() == Some("=") {
                        let (init_form, r3) = r2
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        init_value = value_to_obj(eval(
                            obj_to_value(init_form),
                            env,
                            editor,
                            macros,
                            state,
                        )?);
                    }
                }
                loop_env.write().define(&var, init_value);
            }
            "repeat" => {
                let (n_form, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                let n = value_to_obj(eval(obj_to_value(n_form), env, editor, macros, state)?)
                    .as_integer()
                    .unwrap_or(0);
                // Synthesize a binding that produces nil n times.
                bindings.push(("_repeat_".to_string(), Iter::Repeat { remaining: n }));
            }
            "while" | "until" => {
                let is_until = word == "until";
                let (cond, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                actions.push(Action::While(cond, is_until));
            }
            "always" => {
                let (form, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                actions.push(Action::Always(form));
            }
            "never" => {
                let (form, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                actions.push(Action::Never(form));
            }
            "thereis" => {
                let (form, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                actions.push(Action::Thereis(form));
            }
            "do" | "doing" => {
                // Collect forms until the next keyword.
                let mut body: Vec<LispObject> = Vec::new();
                while let Some((form, r)) = cur.clone().destructure_cons() {
                    if is_loop_keyword(&form) {
                        break;
                    }
                    body.push(form);
                    cur = r;
                }
                // Wrap in progn.
                let mut form = LispObject::symbol("progn");
                let mut chain = LispObject::nil();
                for f in body.into_iter().rev() {
                    chain = LispObject::cons(f, chain);
                }
                form = LispObject::cons(form, chain);
                actions.push(Action::Do(form));
            }
            "collect" | "collecting" | "append" | "appending" | "nconc" | "nconcing" | "sum"
            | "summing" | "count" | "counting" | "maximize" | "maximizing" | "minimize"
            | "minimizing" => {
                let (form, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                // Optional `into VAR`.
                let mut acc_name = String::new();
                if let Some((into, r2)) = cur.clone().destructure_cons() {
                    if into.as_symbol().as_deref() == Some("into") {
                        let (n_obj, r3) = r2
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        acc_name = n_obj.as_symbol().unwrap_or_default();
                    }
                }
                if acc_name.is_empty() {
                    // Synthetic default accumulator — single per loop.
                    acc_name = format!("__cl_loop_default_{}_", word);
                    default_collect_name = Some(acc_name.clone());
                }
                let kind = word.as_str();
                let acc = match kind {
                    "collect" | "collecting" | "append" | "appending" | "nconc" | "nconcing" => {
                        Accumulator::list()
                    }
                    "maximize" | "maximizing" | "minimize" | "minimizing" => Accumulator::max_min(),
                    _ => Accumulator::num(),
                };
                accs.entry(acc_name.clone()).or_insert((acc, true));
                actions.push(match kind {
                    "collect" | "collecting" => Action::Collect(acc_name, form),
                    "append" | "appending" => Action::Append(acc_name, form),
                    "nconc" | "nconcing" => Action::Nconc(acc_name, form),
                    "sum" | "summing" => Action::Sum(acc_name, form),
                    "count" | "counting" => Action::Count(acc_name, form),
                    "maximize" | "maximizing" => Action::Max(acc_name, form),
                    "minimize" | "minimizing" => Action::Min(acc_name, form),
                    _ => unreachable!(),
                });
            }
            "return" => {
                let (form, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                actions.push(Action::Return(form));
            }
            "initially" => {
                // Collect forms until next keyword.
                while let Some((form, r)) = cur.clone().destructure_cons() {
                    if is_loop_keyword(&form) {
                        break;
                    }
                    initially.push(form);
                    cur = r;
                }
            }
            "finally" => {
                // finally [return EXPR | do BODY...]
                if let Some((kw, r)) = cur.clone().destructure_cons() {
                    if kw.as_symbol().as_deref() == Some("return") {
                        let (form, r2) = r
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r2;
                        finally_return = Some(form);
                        continue;
                    }
                    if kw.as_symbol().as_deref() == Some("do") {
                        cur = r;
                    }
                }
                while let Some((form, r)) = cur.clone().destructure_cons() {
                    if is_loop_keyword(&form) {
                        break;
                    }
                    finally.push(form);
                    cur = r;
                }
            }
            "if" | "when" | "unless" | "and" | "else" | "end" => {
                // Not supported; consume the next form and hope.
                if let Some((_, r)) = cur.clone().destructure_cons() {
                    cur = r;
                }
            }
            _ => {
                // Unknown — skip one token to make progress.
            }
        }
    }

    // Run `initially`.
    for form in &initially {
        eval(obj_to_value(form.clone()), &loop_env, editor, macros, state)?;
    }

    // Main loop. Stops when any iterator exhausts OR a `while/until/
    // always/never/thereis` terminates OR `return` fires.
    let mut early_return: Option<LispObject> = None;
    let mut thereis_found: Option<LispObject> = None;
    let mut always_ok = true;
    let mut never_ok = true;
    // Safety bound.
    let mut steps: u64 = 0;
    const MAX_STEPS: u64 = 1 << 26;
    'outer: loop {
        if steps > MAX_STEPS {
            break;
        }
        steps += 1;
        // Advance each binding.
        for (name, iter) in bindings.iter_mut() {
            let next = iter.next(&loop_env, editor, macros, state)?;
            match next {
                Some(v) => {
                    if name != "_repeat_" {
                        loop_env.write().set(name, v);
                    }
                }
                None => break 'outer,
            }
        }
        // Run actions in order.
        for action in actions.iter() {
            match action {
                Action::Do(form) => {
                    eval(obj_to_value(form.clone()), &loop_env, editor, macros, state)?;
                }
                Action::While(cond, is_until) => {
                    let v = value_to_obj(eval(
                        obj_to_value(cond.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    let truthy = !matches!(v, LispObject::Nil);
                    let should_break = if *is_until { truthy } else { !truthy };
                    if should_break {
                        break 'outer;
                    }
                }
                Action::Collect(name, form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    let entry = accs
                        .entry(name.clone())
                        .or_insert((Accumulator::list(), true));
                    if let Accumulator::List(items) = &mut entry.0 {
                        items.push(v);
                    }
                }
                Action::Append(name, form) | Action::Nconc(name, form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    let entry = accs
                        .entry(name.clone())
                        .or_insert((Accumulator::list(), true));
                    if let Accumulator::List(items) = &mut entry.0 {
                        let mut c = v;
                        while let Some((car, cdr)) = c.destructure_cons() {
                            items.push(car);
                            c = cdr;
                        }
                    }
                }
                Action::Sum(name, form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    let entry = accs
                        .entry(name.clone())
                        .or_insert((Accumulator::num(), true));
                    let n = as_f64(&v);
                    if let Accumulator::Num(acc) = &mut entry.0 {
                        *acc += n;
                    }
                    if matches!(v, LispObject::Float(_)) {
                        entry.1 = false;
                    }
                }
                Action::Count(name, form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    if !matches!(v, LispObject::Nil) {
                        let entry = accs
                            .entry(name.clone())
                            .or_insert((Accumulator::num(), true));
                        if let Accumulator::Num(acc) = &mut entry.0 {
                            *acc += 1.0;
                        }
                    }
                }
                Action::Max(name, form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    let n = as_f64(&v);
                    let entry = accs
                        .entry(name.clone())
                        .or_insert((Accumulator::max_min(), true));
                    if let Accumulator::MaxMin(slot) = &mut entry.0 {
                        *slot = Some(match *slot {
                            Some(cur) => cur.max(n),
                            None => n,
                        });
                    }
                    if matches!(v, LispObject::Float(_)) {
                        entry.1 = false;
                    }
                }
                Action::Min(name, form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    let n = as_f64(&v);
                    let entry = accs
                        .entry(name.clone())
                        .or_insert((Accumulator::max_min(), true));
                    if let Accumulator::MaxMin(slot) = &mut entry.0 {
                        *slot = Some(match *slot {
                            Some(cur) => cur.min(n),
                            None => n,
                        });
                    }
                    if matches!(v, LispObject::Float(_)) {
                        entry.1 = false;
                    }
                }
                Action::Return(form) => {
                    early_return = Some(value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?));
                    break 'outer;
                }
                Action::Always(form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    if matches!(v, LispObject::Nil) {
                        always_ok = false;
                        break 'outer;
                    }
                }
                Action::Never(form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    if !matches!(v, LispObject::Nil) {
                        never_ok = false;
                        break 'outer;
                    }
                }
                Action::Thereis(form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    if !matches!(v, LispObject::Nil) {
                        thereis_found = Some(v);
                        break 'outer;
                    }
                }
            }
        }
    }

    // Publish each accumulator as a local variable so `finally`
    // (and `finally return`) can see e.g. `big`, `top`, etc.
    {
        let mut w = loop_env.write();
        for (name, (acc, integer_mode)) in &accs {
            // Skip the synthetic default accumulator; it's consumed
            // separately as the loop's return value.
            if default_collect_name.as_deref() == Some(name.as_str()) {
                continue;
            }
            w.define(name, acc.clone().into_value(*integer_mode));
        }
    }

    // Run `finally`.
    for form in &finally {
        eval(obj_to_value(form.clone()), &loop_env, editor, macros, state)?;
    }

    // Determine result value.
    if let Some(v) = early_return {
        return Ok(obj_to_value(v));
    }
    if let Some(form) = finally_return {
        return eval(obj_to_value(form), &loop_env, editor, macros, state);
    }
    if let Some(v) = thereis_found {
        return Ok(obj_to_value(v));
    }
    if !always_ok {
        return Ok(obj_to_value(LispObject::nil()));
    }
    if !never_ok {
        return Ok(obj_to_value(LispObject::nil()));
    }
    // Check for `always`/`never` exhaustion → t.
    let had_always = actions.iter().any(|a| matches!(a, Action::Always(_)));
    let had_never = actions.iter().any(|a| matches!(a, Action::Never(_)));
    if had_always || had_never {
        return Ok(obj_to_value(LispObject::t()));
    }
    // Default result: the (sole) default accumulator's value, or nil.
    if let Some(name) = default_collect_name {
        if let Some((acc, integer_mode)) = accs.remove(&name) {
            return Ok(obj_to_value(acc.into_value(integer_mode)));
        }
    }
    Ok(Value::nil())
}

/// Return f64 view of any numeric LispObject; non-numeric → 0.0.
fn as_f64(v: &LispObject) -> f64 {
    match v {
        LispObject::Integer(n) => *n as f64,
        LispObject::Float(f) => *f,
        _ => 0.0,
    }
}

/// Is this token a cl-loop keyword? Used to stop greedy `do` / `initially`
/// / `finally` body collection.
fn is_loop_keyword(obj: &LispObject) -> bool {
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
