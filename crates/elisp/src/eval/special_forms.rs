// Special form evaluation functions: if, setq, defun, let, progn, cond, etc.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{obj_to_value, value_to_obj, Value};
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::sync::Arc;

use super::dynamic::unwind_specpdl;
use super::{eval, Environment, InterpreterState, Macro, MacroTable};

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
        eval_progn(obj_to_value(else_forms), env, editor, macros, state)
    } else {
        eval(obj_to_value(then_branch), env, editor, macros, state)
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
    _state: &InterpreterState,
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
    crate::obarray::set_function_cell(id, lambda);
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
    let mut form = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;

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

    let result = eval_progn(obj_to_value(body.clone()), &temp_env, editor, macros, state)?;
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
    let mut mode = 0u8; // 0=positional, 1=optional, 2=rest
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
                    _ => unreachable!(),
                }
            }
        }
    }
    Ok(())
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

    let result = eval_progn(obj_to_value(body), &new_env, editor, macros, state);

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
    let body_obj = value_to_obj(body);
    let mut result = Value::nil();
    let mut current = Some(body_obj);
    while let Some(curr) = current {
        if let Some((expr, rest)) = curr.destructure_cons() {
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
                    return eval_progn(obj_to_value(exprs), env, editor, macros, state);
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
        // Charge an op per iteration so an unbounded `(loop ...)` can be
        // killed by the eval-ops budget. Without this, a malformed body
        // that signals nothing would hang forever.
        state.charge(1)?;
        eval_progn(body, env, editor, macros, state)?;
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
        let cond_val = eval(obj_to_value(cond.clone()), env, editor, macros, state)?;
        if cond_val.is_nil() {
            return Ok(Value::nil());
        }
        eval_progn(body_val, env, editor, macros, state)?;
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

    let result = eval_progn(obj_to_value(body), &new_env, editor, macros, state);

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

    let result = eval_progn(obj_to_value(body), env, editor, macros, state);

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
        eval_progn(obj_to_value(body), env, editor, macros, state)
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
        eval_progn(obj_to_value(body), env, editor, macros, state)
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
    crate::obarray::mark_special(id);

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
            crate::obarray::set_value_cell(id, value);
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
    crate::obarray::mark_special(id);

    if let Some(value_expr) = args_obj.nth(1) {
        let value = value_to_obj(eval(obj_to_value(value_expr), env, editor, macros, state)?);
        env.write().define_id(id, value.clone());
        crate::obarray::set_value_cell(id, value);
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
    crate::obarray::set_function_cell(id, value);
    Ok(obj_to_value(LispObject::Symbol(id)))
}
