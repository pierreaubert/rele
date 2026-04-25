//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use super::SyncRefCell as RwLock;
use super::{Environment, InterpreterState, MacroTable, eval, eval_progn};
use super::{bind_param_dynamic, unwind_specpdl};
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::sync::Arc;

use super::functions::{call_stateful_primitive, source_level_fallback};

pub(super) fn eval_list(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
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
pub(crate) fn apply_lambda(
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
    let mut aux = false;
    let mut key_mode = false;
    let mut key_params: Vec<(String, LispObject)> = Vec::new();
    loop {
        if params_list.is_nil() {
            break;
        }
        let (param, params_rest) = match params_list.destructure_cons() {
            Some((p, r)) => (p, r),
            None => {
                if let Some(name) = params_list.as_symbol() {
                    bind_param_dynamic(&name, args_list.clone(), &new_env, state);
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
                "&key" => {
                    key_mode = true;
                    params_list = params_rest;
                    continue;
                }
                "&allow-other-keys" => {
                    params_list = params_rest;
                    continue;
                }
                "&aux" => {
                    aux = true;
                    key_mode = false;
                    rest = false;
                    params_list = params_rest;
                    continue;
                }
                _ => {}
            }
            if aux {
                let value = LispObject::nil();
                bind_param_dynamic(&name, value, &new_env, state);
                params_list = params_rest;
                continue;
            }
            if key_mode {
                key_params.push((name, LispObject::nil()));
                params_list = params_rest;
                continue;
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
        } else if aux {
            if let Some((name_obj, init_rest)) = param.destructure_cons()
                && let Some(name) = name_obj.as_symbol()
            {
                let init = init_rest.first().unwrap_or(LispObject::nil());
                let value =
                    value_to_obj(eval(obj_to_value(init), &new_env, editor, macros, state)?);
                bind_param_dynamic(&name, value, &new_env, state);
            }
        } else if key_mode {
            let (name, default) = super::special_forms::extract_key_param(param);
            key_params.push((name, default));
            params_list = params_rest;
            continue;
        }
        params_list = params_rest;
    }
    if !key_params.is_empty() {
        let mut found: Vec<Option<LispObject>> = vec![None; key_params.len()];
        let mut cursor = args_list.clone();
        while !cursor.is_nil() {
            let key = match cursor.first() {
                Some(k) => k,
                None => break,
            };
            let krest = cursor.rest().unwrap_or(LispObject::nil());
            if let Some(key_name) = key.as_symbol()
                && key_name.starts_with(':')
            {
                let param_name = &key_name[1..];
                for (i, (name, _)) in key_params.iter().enumerate() {
                    if name == param_name {
                        found[i] = Some(krest.first().unwrap_or(LispObject::nil()));
                        break;
                    }
                }
                cursor = krest.rest().unwrap_or(LispObject::nil());
                continue;
            }
            cursor = krest;
        }
        for (i, (name, default)) in key_params.iter().enumerate() {
            if name.is_empty() {
                continue;
            }
            let value = match found[i].clone() {
                Some(v) => v,
                None => {
                    if default.is_nil() {
                        LispObject::nil()
                    } else {
                        value_to_obj(
                            eval(
                                obj_to_value(default.clone()),
                                &new_env,
                                editor,
                                macros,
                                state,
                            )
                            .unwrap_or(obj_to_value(default.clone())),
                        )
                    }
                }
            };
            bind_param_dynamic(name, value, &new_env, state);
        }
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
    call_function_inner(func, args, env, editor, macros, state, None)
}
/// Inner dispatch with an optional caller symbol for JIT version tracking.
///
/// When a function is called through a named symbol (`(foo ...)`), the
/// `Symbol(id)` arm passes `Some(id)` so the `BytecodeFn` arm can
/// look up the symbol's `def_version` and use the version-checked JIT
/// APIs (`compile_with_version` / `get_compiled_checked`).  Direct
/// bytecode calls (e.g. `(funcall #[...] ...)`) arrive with `None`
/// and fall back to un-versioned compilation.
fn call_function_inner(
    func: Value,
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
    #[allow(unused_variables)] caller_sym: Option<crate::obarray::SymbolId>,
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
                    let params = cdr_val.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr_val.rest().ok_or(ElispError::WrongNumberOfArguments)?;
                    return apply_lambda(&params, &body, args, env, editor, macros, state);
                }
                if s == "macro" {
                    return call_function(obj_to_value(cdr_val), args, env, editor, macros, state);
                }
                if s == "autoload" {
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
                    let captured_alist =
                        cdr_val.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let rest = cdr_val.rest().ok_or(ElispError::WrongNumberOfArguments)?;
                    let params = rest.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = rest.rest().ok_or(ElispError::WrongNumberOfArguments)?;
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
            if let Some(result) = crate::primitives_value::try_call_primitive_value(name, args) {
                crate::primitives_value::inc_fast_hits();
                return result;
            }
            crate::primitives_value::inc_slow_hits();
            let args_obj = value_to_obj(args);
            if let Some(result) =
                call_stateful_primitive(name, &args_obj, env, editor, macros, state)
            {
                return Ok(obj_to_value(result?));
            }
            match crate::primitives::call_primitive(name, &args_obj) {
                Ok(v) => Ok(obj_to_value(v)),
                Err(ElispError::VoidFunction(_)) => {
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
                let version = caller_sym.map(|s| state.def_version(s)).unwrap_or(0);
                let mut jit = state.jit.write();
                if should_jit && !jit.is_compiled(func_id) {
                    jit.compile_with_version(func_id, bc, version);
                }
                if let Some(id) = jit.get_compiled_checked(func_id, version) {
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
                                crate::jit::NativeResult::Deoptimize => {}
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
            let resolved = env.read().get_function(&name);
            if let Some(val) = resolved {
                return call_function_inner(
                    obj_to_value(val),
                    args,
                    env,
                    editor,
                    macros,
                    state,
                    Some(id),
                );
            }
            let autoload_file = state.autoloads.read().get(&name).cloned();
            if let Some(file) = autoload_file {
                let load_args = LispObject::cons(
                    LispObject::string(&file),
                    LispObject::cons(LispObject::t(), LispObject::nil()),
                );
                let _ =
                    super::builtins::eval_load(obj_to_value(load_args), env, editor, macros, state);
                let resolved2 = env.read().get_function(&name);
                if let Some(val) = resolved2 {
                    return call_function_inner(
                        obj_to_value(val),
                        args,
                        env,
                        editor,
                        macros,
                        state,
                        Some(id),
                    );
                }
            }
            let args_obj = value_to_obj(args);
            if let Some(result) = crate::primitives_eieio::try_keyword_slot_call_with_state(
                &name,
                &args_obj,
                Some(state),
            ) {
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
    use super::super::functions::FALLBACK_STACK;
    use super::super::types::FallbackFrame;
    /// Regression: R3. Previously the FALLBACK_STACK was pushed/popped
    /// by explicit method calls. If the `eval` inside the fallback
    /// panicked, the pop never ran and the name stayed on the stack
    /// forever — poisoning every subsequent call to that name with a
    /// spurious VoidFunction error. The RAII `FallbackFrame` guard
    /// ensures the pop happens on unwind too.
    #[test]
    fn fallback_frame_pops_on_panic() {
        FALLBACK_STACK.with(|st| assert!(st.borrow().is_empty()));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = FallbackFrame::new("r3-poisoned");
            FALLBACK_STACK.with(|st| {
                assert_eq!(st.borrow().as_slice(), &["r3-poisoned".to_string()]);
            });
            panic!("simulated panic inside fallback");
        }));
        assert!(result.is_err(), "panic must propagate");
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
