// Special form evaluation functions: if, setq, defun, let, progn, cond, etc.

use super::SyncRefCell as RwLock;
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::sync::Arc;

use super::dynamic::unwind_specpdl;
use super::{Environment, InterpreterState, Macro, MacroTable, eval};

pub(super) fn eval_if(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let cond = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let then_branch = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let cond_val = eval(obj_to_value(cond), env, editor, macros, state)?;
    if cond_val.is_nil() {
        let else_forms = args_obj
            .rest()
            .and_then(|r| r.rest())
            .unwrap_or(LispObject::nil());
        // Tail position: else branch
        eval_progn_tco(obj_to_value(else_forms), env, editor, macros, state)
    } else {
        // Tail position: then branch
        super::tail_call(obj_to_value(then_branch))
    }
}
pub(super) fn eval_setq(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let mut result = Value::nil();
    let mut current = args_obj;
    while let Some((name_obj, rest)) = current.destructure_cons() {
        let id = name_obj
            .as_symbol_id()
            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
        let val_expr = rest.first().ok_or(ElispError::WrongNumberOfArguments)?;
        let value_val = eval(obj_to_value(val_expr), env, editor, macros, state)?;
        let value = value_to_obj(value_val);
        if state.special_vars.read().contains(&id) {
            state.global_env.write().set_id(id, value);
        } else {
            env.write().set_id(id, value);
        }
        result = value_val;
        current = rest.rest().unwrap_or(LispObject::nil());
    }
    Ok(result)
}
pub(super) fn eval_defun(
    args: Value,
    _env: &Arc<RwLock<Environment>>,
    _editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    _macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name_obj = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let rest = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = name_obj
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    let lambda = LispObject::lambda_expr(
        rest.first().unwrap_or(LispObject::nil()),
        rest.rest().unwrap_or(LispObject::nil()),
    );
    // defun writes the function cell directly.
    state.set_function_cell(id, lambda);
    Ok(obj_to_value(LispObject::Symbol(id)))
}
pub(super) fn eval_defmacro(args: Value, macros: &MacroTable) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let rest = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    let macro_args = rest.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let macro_body = rest.rest().unwrap_or(LispObject::nil());

    let macro_def = Macro {
        args: macro_args,
        body: macro_body,
    };

    macros.write().insert(name.clone(), macro_def);
    Ok(obj_to_value(LispObject::symbol(&name)))
}
pub(super) fn eval_macroexpand(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let form_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut form = value_to_obj(eval(obj_to_value(form_arg), env, editor, macros, state)?);

    // Keep expanding until the outermost form is no longer a macro call,
    // matching Emacs's `macroexpand` semantics.
    loop {
        if let LispObject::Cons(_) = form {
            let car = form.first().unwrap_or(LispObject::nil());
            if let Some(s) = car.as_symbol() {
                let macro_table = macros.read();
                if let Some(macro_) = macro_table.get(&s) {
                    let macro_ = macro_.clone();
                    drop(macro_table);
                    let cdr_obj = form.rest().unwrap_or(LispObject::nil());
                    form = expand_macro(&macro_, cdr_obj, env, editor, macros, state)?;
                    continue;
                }
            }
        }
        break;
    }

    Ok(obj_to_value(form))
}
pub(super) fn expand_macro(
    macro_: &Macro,
    args: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let params = &macro_.args;
    let body = &macro_.body;

    // Bind macro parameters to the call-site args, supporting `cl-defmacro`-style
    // destructuring (e.g. `((name other &optional flag) &rest body)`).
    //
    // Top-level params is a lambda list with &optional / &rest / &body markers.
    // A non-symbol positional slot is a SUB-lambda-list that recursively matches
    // the shape of the corresponding call-site argument. This is essential for
    // macros like `files-tests--with-temp-non-special` whose first param is
    // `(name non-special-name &optional dir-flag)`: the outer positional slot
    // consumes one call-site arg (itself a list like `(tmpfile nospecial)`),
    // and the inner pattern binds `name=tmpfile`, `non-special-name=nospecial`.
    let mut bindings: Vec<(String, LispObject)> = Vec::new();
    bind_macro_params(params.clone(), args, &mut bindings)?;

    let parent_env = Arc::new(env.read().clone());
    let temp_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    for (name, value) in bindings {
        if name.is_empty() {
            continue; // skip the SKIP sentinel (unsupported shape)
        }
        temp_env.write().define(&name, value);
    }

    let result = eval_progn_no_tco(obj_to_value(body.clone()), &temp_env, editor, macros, state)?;
    Ok(value_to_obj(result))
}

/// Walk a macro lambda list `params` and the corresponding call-site args,
/// pushing (name, unevaluated-value) pairs into `bindings`.
///
/// Handles `&optional`, `&rest`, `&body`, and recursive list destructuring
/// (cl-defmacro style). Non-symbol positional slots are treated as a sub
/// lambda list matching the structure of the corresponding argument.
fn bind_macro_params(
    params: LispObject,
    args: LispObject,
    bindings: &mut Vec<(String, LispObject)>,
) -> ElispResult<()> {
    let mut arg_list = args;
    // 0=positional, 1=optional, 2=rest, 3=key
    let mut mode = 0u8;
    // Collect &key param specs so we can scan the arg list after
    // positional/optional params are consumed.
    let mut key_params: Vec<(String, LispObject)> = Vec::new();
    let mut cur = Some(params);
    while let Some(curr) = cur {
        match curr.destructure_cons() {
            None => break,
            Some((param, tail)) => {
                if let Some(s) = param.as_symbol() {
                    match s.as_str() {
                        "&optional" => {
                            mode = 1;
                            cur = Some(tail);
                            continue;
                        }
                        // `&body` is a CL extension synonym for `&rest`.
                        // Used heavily in `cl-defmacro` (e.g. ert-deftest).
                        "&rest" | "&body" => {
                            mode = 2;
                            cur = Some(tail);
                            continue;
                        }
                        "&key" => {
                            mode = 3;
                            cur = Some(tail);
                            continue;
                        }
                        "&allow-other-keys" => {
                            cur = Some(tail);
                            continue;
                        }
                        _ => {}
                    }
                }

                match mode {
                    0 => {
                        // Positional slot: consume one call-site arg.
                        let arg = arg_list.first().unwrap_or(LispObject::nil());
                        arg_list = arg_list.rest().unwrap_or(LispObject::nil());
                        bind_one_param(param, arg, bindings)?;
                        cur = Some(tail);
                    }
                    1 => {
                        // Optional slot: default to nil if call-site exhausted.
                        let arg = if arg_list.is_nil() {
                            LispObject::nil()
                        } else {
                            let a = arg_list.first().unwrap_or(LispObject::nil());
                            arg_list = arg_list.rest().unwrap_or(LispObject::nil());
                            a
                        };
                        bind_one_param(param, arg, bindings)?;
                        cur = Some(tail);
                    }
                    2 => {
                        // Rest slot: the whole remainder, bound to one name.
                        // A destructuring pattern after &rest is unusual; just skip.
                        if let Some(s) = param.as_symbol() {
                            bindings.push((s, arg_list.clone()));
                        }
                        break;
                    }
                    3 => {
                        // &key slot: collect the param name and default value.
                        // Formats:   foo          -> name="foo", default=nil
                        //            (foo DEFAULT) -> name="foo", default=DEFAULT
                        let (name, default) = extract_key_param(param);
                        key_params.push((name, default));
                        cur = Some(tail);
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    // Process &key params: scan the remaining arg_list for :keyword value pairs.
    if !key_params.is_empty() {
        bind_keyword_args(&key_params, &arg_list, bindings);
    }

    Ok(())
}

/// Extract the parameter name and default value from a `&key` spec.
///
/// Accepted shapes:
///   `foo`              -> ("foo", nil)
///   `(foo DEFAULT)`    -> ("foo", DEFAULT)
///   `(foo)`            -> ("foo", nil)
///   `((:keyword foo))` -> ("foo", nil)       (full form, rare)
///   `((:keyword foo) DEFAULT)` -> ("foo", DEFAULT)
pub(super) fn extract_key_param(param: LispObject) -> (String, LispObject) {
    // Simple symbol
    if let Some(s) = param.as_symbol() {
        return (s, LispObject::nil());
    }
    // List: (NAME DEFAULT?) or ((:KEYWORD NAME) DEFAULT?)
    if let Some((first, rest)) = param.destructure_cons() {
        // Check for ((:keyword name) default?)
        if let Some((inner_first, inner_rest)) = first.destructure_cons() {
            // inner_first is the keyword, inner_rest has the actual name
            let _keyword = inner_first; // e.g. :foo
            let name = inner_rest
                .first()
                .and_then(|n| n.as_symbol())
                .unwrap_or_default();
            let default = rest.first().unwrap_or(LispObject::nil());
            return (name, default);
        }
        // Simple form: (name default?)
        let name = first.as_symbol().unwrap_or_default();
        let default = rest.first().unwrap_or(LispObject::nil());
        return (name, default);
    }
    (String::new(), LispObject::nil())
}

/// Walk the argument list looking for `:keyword value` pairs and bind
/// the corresponding param names.  Unmatched keywords are ignored.
fn bind_keyword_args(
    key_params: &[(String, LispObject)],
    arg_list: &LispObject,
    bindings: &mut Vec<(String, LispObject)>,
) {
    // Build a quick lookup of which keywords we've found values for.
    let mut found: Vec<Option<LispObject>> = vec![None; key_params.len()];

    // Walk the arg list looking for :keyword value pairs.
    let mut cursor = arg_list.clone();
    while !cursor.is_nil() {
        let key = match cursor.first() {
            Some(k) => k,
            None => break,
        };
        let rest = cursor.rest().unwrap_or(LispObject::nil());

        if let Some(key_name) = key.as_symbol()
            && key_name.starts_with(':')
        {
            // Strip the leading colon to match the param name.
            let param_name = &key_name[1..];
            for (i, (name, _)) in key_params.iter().enumerate() {
                if name == param_name {
                    let value = rest.first().unwrap_or(LispObject::nil());
                    found[i] = Some(value);
                    break;
                }
            }
            // Skip past the value.
            cursor = rest.rest().unwrap_or(LispObject::nil());
            continue;
        }

        // Not a keyword — skip one element.
        cursor = rest;
    }

    // Bind all key params: use found value or default.
    for (i, (name, default)) in key_params.iter().enumerate() {
        if name.is_empty() {
            continue;
        }
        let value = found[i].clone().unwrap_or_else(|| default.clone());
        bindings.push((name.clone(), value));
    }
}

/// Bind a single positional/optional parameter slot — either a plain symbol
/// or a destructuring sub-pattern (itself a lambda list).
fn bind_one_param(
    param: LispObject,
    arg: LispObject,
    bindings: &mut Vec<(String, LispObject)>,
) -> ElispResult<()> {
    if let Some(s) = param.as_symbol() {
        bindings.push((s, arg));
        return Ok(());
    }
    // Non-symbol: must be a list (destructuring pattern). Recurse with the
    // arg as the inner "arg list". Non-list args against a sub-pattern are
    // permissively treated as nil (Emacs would signal; we stay lenient to
    // match existing behaviour for unsupported shapes).
    if matches!(param, LispObject::Cons(_)) {
        let inner_args = if matches!(arg, LispObject::Cons(_)) || arg.is_nil() {
            arg
        } else {
            LispObject::nil()
        };
        bind_macro_params(param, inner_args, bindings)?;
    }
    // Any other shape (vector, literal) — skip, matches old SKIP behaviour.
    Ok(())
}
pub(super) fn eval_let(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let bindings = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;

    let parent_env = Arc::new(env.read().clone());
    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    let specpdl_depth = state.specpdl.read().len();

    let mut bindings_list = bindings;
    while let Some((binding, rest)) = bindings_list.destructure_cons() {
        let (id, value) = if let Some(id) = binding.as_symbol_id() {
            (id, LispObject::nil())
        } else if let Some((binding_name, binding_val_wrapper)) = binding.destructure_cons() {
            let binding_id = binding_name
                .as_symbol_id()
                .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
            let binding_val = binding_val_wrapper.first().unwrap_or(LispObject::nil());
            let binding_val =
                value_to_obj(eval(obj_to_value(binding_val), env, editor, macros, state)?);
            (binding_id, binding_val)
        } else {
            return Err(ElispError::WrongTypeArgument("symbol or list".to_string()));
        };

        if state.special_vars.read().contains(&id) {
            let global = &state.global_env;
            let old = global.read().get_id(id);
            state.specpdl.write().push((id, old));
            global.write().set_id(id, value);
        } else {
            new_env.write().define_id(id, value);
        }

        bindings_list = rest;
    }

    let result = eval_progn_no_tco(obj_to_value(body), &new_env, editor, macros, state);

    unwind_specpdl(state, specpdl_depth);

    result
}
pub(super) fn eval_progn(
    body: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    eval_progn_impl(body, env, editor, macros, state, false)
}

/// Like `eval_progn` but tail-calls the last form. Only safe to call
/// when no scope cleanup (specpdl, env, temp buffer) is needed after.
pub(super) fn eval_progn_tco(
    body: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    eval_progn_impl(body, env, editor, macros, state, true)
}

/// Like `eval_progn` but never tail-calls. Used when the caller needs
/// to run cleanup after the body (let, unwind-protect, catch, etc.).
pub(super) fn eval_progn_no_tco(
    body: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    eval_progn_impl(body, env, editor, macros, state, false)
}

fn eval_progn_impl(
    body: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
    allow_tco: bool,
) -> ElispResult<Value> {
    let body_obj = value_to_obj(body);
    let mut result = Value::nil();
    let mut current = Some(body_obj);
    while let Some(curr) = current {
        if let Some((expr, rest)) = curr.destructure_cons() {
            // Re-check the limit here in case a watchdog thread lowered
            // it while we were inside a non-eval code path.
            let limit = state
                .eval_ops_limit
                .load(std::sync::atomic::Ordering::Relaxed);
            if limit > 0 {
                let ops = state.eval_ops.load(std::sync::atomic::Ordering::Relaxed);
                if ops >= limit {
                    return Err(ElispError::EvalError(
                        "eval operation limit exceeded".into(),
                    ));
                }
            }
            if allow_tco && rest.is_nil() {
                // Last form in tail position: bounce back to eval's
                // loop instead of recursing.
                return super::tail_call(obj_to_value(expr));
            }
            result = eval(obj_to_value(expr), env, editor, macros, state)?;
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(result)
}
pub(super) fn eval_cond(
    clauses: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let clauses_obj = value_to_obj(clauses);
    let mut current = Some(clauses_obj);
    while let Some(curr) = current {
        if let Some((clause, rest)) = curr.destructure_cons() {
            let (cond_expr, then_exprs) = if let Some((c, r)) = clause.destructure_cons() {
                (c, Some(r))
            } else {
                (clause, None)
            };

            let cond_val = eval(obj_to_value(cond_expr), env, editor, macros, state)?;
            if !cond_val.is_nil() {
                if let Some(exprs) = then_exprs {
                    // Tail position: matching clause body
                    return eval_progn_tco(obj_to_value(exprs), env, editor, macros, state);
                }
                return Ok(cond_val);
            }
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(Value::nil())
}
pub(super) fn eval_loop(
    body: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    loop {
        state.charge(1)?;
        eval_progn_no_tco(body, env, editor, macros, state)?;
    }
}
pub(super) fn eval_while(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let cond = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().unwrap_or(LispObject::nil());
    let body_val = obj_to_value(body);
    loop {
        state.charge(1)?;
        let cond_val = eval(obj_to_value(cond.clone()), env, editor, macros, state)?;
        if cond_val.is_nil() {
            return Ok(Value::nil());
        }
        eval_progn_no_tco(body_val, env, editor, macros, state)?;
    }
}
pub(super) fn eval_let_star(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let bindings = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;

    let parent_env = Arc::new(env.read().clone());
    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    let specpdl_depth = state.specpdl.read().len();

    let mut bindings_list = bindings;
    while let Some((binding, rest)) = bindings_list.destructure_cons() {
        let (id, value) = if let Some(id) = binding.as_symbol_id() {
            (id, LispObject::nil())
        } else if let Some((binding_name, binding_val_wrapper)) = binding.destructure_cons() {
            let binding_id = binding_name
                .as_symbol_id()
                .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
            let binding_val = binding_val_wrapper.first().unwrap_or(LispObject::nil());
            let binding_val = value_to_obj(eval(
                obj_to_value(binding_val),
                &new_env,
                editor,
                macros,
                state,
            )?);
            (binding_id, binding_val)
        } else {
            return Err(ElispError::WrongTypeArgument("symbol or list".to_string()));
        };

        if state.special_vars.read().contains(&id) {
            let global = &state.global_env;
            let old = global.read().get_id(id);
            state.specpdl.write().push((id, old));
            global.write().set_id(id, value);
        } else {
            new_env.write().define_id(id, value);
        }

        bindings_list = rest;
    }

    let result = eval_progn_no_tco(obj_to_value(body), &new_env, editor, macros, state);

    unwind_specpdl(state, specpdl_depth);

    result
}
/// `(dlet BINDINGS BODY...)` — always-dynamic let, regardless of
/// `lexical-binding`. Mirrors Emacs's definition `(progn (defvar x) ... (let
/// ((x ...) ...) body))`: each bound symbol is promoted to a special variable
/// for the duration of the body, its prior global value saved and restored on
/// exit. Binding values are evaluated in the *outer* environment (parallel
/// semantics, like `let`).
pub(super) fn eval_dlet(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    use crate::obarray::SymbolId;

    let args_obj = value_to_obj(args);
    let bindings = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;

    // Evaluate every binding value in the outer env *before* we install any
    // of them — `dlet` uses parallel semantics.
    let mut pairs: Vec<(SymbolId, LispObject)> = Vec::new();
    let mut bindings_list = bindings;
    while let Some((binding, rest)) = bindings_list.destructure_cons() {
        let (id, value) = if let Some(id) = binding.as_symbol_id() {
            (id, LispObject::nil())
        } else if let Some((binding_name, binding_val_wrapper)) = binding.destructure_cons() {
            let binding_id = binding_name
                .as_symbol_id()
                .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
            let binding_val = binding_val_wrapper.first().unwrap_or(LispObject::nil());
            let binding_val =
                value_to_obj(eval(obj_to_value(binding_val), env, editor, macros, state)?);
            (binding_id, binding_val)
        } else {
            return Err(ElispError::WrongTypeArgument("symbol or list".to_string()));
        };
        pairs.push((id, value));
        bindings_list = rest;
    }

    // Snapshot old globals & special-ness so we can restore on exit, then
    // promote each binding to special + write its new global value.
    let mut saved: Vec<(SymbolId, Option<LispObject>, bool)> = Vec::with_capacity(pairs.len());
    {
        let mut special = state.special_vars.write();
        let mut global = state.global_env.write();
        for (id, val) in &pairs {
            let was_special = special.contains(id);
            let old = global.get_id_local(*id);
            saved.push((*id, old, was_special));
            if !was_special {
                special.insert(*id);
            }
            global.set_id(*id, val.clone());
        }
    }

    let result = eval_progn_no_tco(obj_to_value(body), env, editor, macros, state);

    // Restore in reverse order so shadowed re-bindings of the same symbol
    // within `pairs` are unwound correctly.
    {
        let mut special = state.special_vars.write();
        let mut global = state.global_env.write();
        for (id, old, was_special) in saved.into_iter().rev() {
            match old {
                Some(v) => global.set_id(id, v),
                None => global.unset_id(id),
            }
            if !was_special {
                special.remove(&id);
            }
        }
    }

    result
}
pub(super) fn eval_when(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let cond = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let cond_val = eval(obj_to_value(cond), env, editor, macros, state)?;
    if cond_val.is_nil() {
        Ok(Value::nil())
    } else {
        // Tail position
        eval_progn_tco(obj_to_value(body), env, editor, macros, state)
    }
}
pub(super) fn eval_unless(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let cond = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let cond_val = eval(obj_to_value(cond), env, editor, macros, state)?;
    if cond_val.is_nil() {
        // Tail position
        eval_progn_tco(obj_to_value(body), env, editor, macros, state)
    } else {
        Ok(Value::nil())
    }
}
pub(super) fn eval_and(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    if args_obj.is_nil() {
        return Ok(Value::t());
    }
    let mut current = Some(args_obj);
    let mut result = Value::t();
    while let Some(curr) = current {
        if let Some((expr, rest)) = curr.destructure_cons() {
            result = eval(obj_to_value(expr), env, editor, macros, state)?;
            if result.is_nil() {
                return Ok(result);
            }
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(result)
}
pub(super) fn eval_or(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    if args_obj.is_nil() {
        return Ok(Value::nil());
    }
    let mut current = Some(args_obj);
    while let Some(curr) = current {
        if let Some((expr, rest)) = curr.destructure_cons() {
            let result = eval(obj_to_value(expr), env, editor, macros, state)?;
            if !result.is_nil() {
                return Ok(result);
            }
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(Value::nil())
}
pub(super) fn eval_prog1(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let first = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = eval(obj_to_value(first), env, editor, macros, state)?;
    let _ = eval_progn(
        obj_to_value(args_obj.rest().unwrap_or(LispObject::nil())),
        env,
        editor,
        macros,
        state,
    )?;
    Ok(result)
}
pub(super) fn eval_prog2(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let first = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let second = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let _ = eval(obj_to_value(first), env, editor, macros, state)?;
    let result = eval(obj_to_value(second), env, editor, macros, state)?;
    let rest = args_obj
        .rest()
        .and_then(|r| r.rest())
        .unwrap_or(LispObject::nil());
    let _ = eval_progn(obj_to_value(rest), env, editor, macros, state)?;
    Ok(result)
}
pub(super) fn eval_defvar(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name_obj = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = name_obj
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    state.special_vars.write().insert(id);

    // defvar only initializes when the variable has no existing local
    // binding. We use get_id_local (env-only, no value-cell fallback)
    // because process-global value cells may hold a concurrent test's
    // value — that must NOT block initialization of this interpreter's
    // binding.
    let is_bound = env.read().get_id_local(id).is_some();
    if !is_bound {
        if let Some(value_expr) = args_obj.nth(1) {
            let value = value_to_obj(eval(obj_to_value(value_expr), env, editor, macros, state)?);
            env.write().define_id(id, value.clone());
            // Mirror to the value cell so fresh envs elsewhere can see it.
            state.set_value_cell(id, value);
        }
    }
    Ok(obj_to_value(LispObject::Symbol(id)))
}
pub(super) fn eval_defconst(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name_obj = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = name_obj
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    state.special_vars.write().insert(id);

    if let Some(value_expr) = args_obj.nth(1) {
        let value = value_to_obj(eval(obj_to_value(value_expr), env, editor, macros, state)?);
        env.write().define_id(id, value.clone());
        state.set_value_cell(id, value);
    }
    Ok(obj_to_value(LispObject::Symbol(id)))
}
pub(super) fn eval_defalias(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let definition = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let name = value_to_obj(eval(obj_to_value(name_expr), env, editor, macros, state)?);
    let name = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let value = value_to_obj(eval(obj_to_value(definition), env, editor, macros, state)?);

    if let Some((car, rest)) = value.destructure_cons() {
        if car.as_symbol().as_deref() == Some("macro") {
            if let Some((lambda_sym, lambda_rest)) = rest.destructure_cons() {
                if lambda_sym.as_symbol().as_deref() == Some("lambda") {
                    let macro_args = lambda_rest.first().unwrap_or(LispObject::nil());
                    let macro_body = lambda_rest.rest().unwrap_or(LispObject::nil());
                    macros.write().insert(
                        name.clone(),
                        Macro {
                            args: macro_args,
                            body: macro_body,
                        },
                    );
                    return Ok(obj_to_value(LispObject::symbol(&name)));
                }
            }
        }
    }

    // defalias writes the function cell — value is a function definition.
    let id = crate::obarray::intern(&name);
    state.set_function_cell(id, value);
    Ok(obj_to_value(LispObject::Symbol(id)))
}
