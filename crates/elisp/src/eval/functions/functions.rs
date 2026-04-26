//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use super::SyncRefCell as RwLock;
use super::{Environment, InterpreterState, Macro, MacroTable, eval};
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
    stateful_put, stateful_require, stateful_setplist, symbol_id_including_constants,
    symbol_name_including_constants,
};
use super::functions_3::apply_lambda;
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
        "setplist" => Some(stateful_setplist(args, state)),
        "def-edebug-elem-spec" => Some(stateful_def_edebug_elem_spec(args, state)),
        "defvar-1" => Some(stateful_defvar_1(args, env, state)),
        "cl-generic-generalizers" => Some(stateful_cl_generic_generalizers(args, state)),
        "cl-generic-define" => Some(stateful_cl_generic_define(args)),
        "cl-generic-define-method" => Some(stateful_cl_generic_define_method(args, state)),
        "add-to-list" | "add-to-ordered-list" => Some(stateful_add_to_list(args, env, state)),
        "boundp" => Some(stateful_boundp(args, env, state)),
        "fboundp" => Some(stateful_fboundp(args, env, state, macros)),
        "intern" => Some(stateful_intern(args)),
        "intern-soft" => Some(stateful_intern_soft(args, env, state)),
        "set" => Some(stateful_set(args, env, editor, macros, state)),
        "symbol-value" => Some(stateful_symbol_value(args, env, state)),
        "symbol-function" => Some(stateful_symbol_function(args, env, macros, state)),
        "default-value" => Some(stateful_default_value(args, state)),
        "default-boundp" => Some(stateful_default_boundp(args, state)),
        "set-default" => Some(stateful_set_default(args, env, editor, macros, state)),
        "make-local-variable" => Some(stateful_make_local_variable(args, env, state)),
        "make-variable-buffer-local" => Some(stateful_make_variable_buffer_local(args, state)),
        "local-variable-p" | "local-variable-if-set-p" => Some(stateful_local_variable_p(args)),
        "variable-binding-locus" => Some(stateful_variable_binding_locus(args)),
        "kill-local-variable" => Some(stateful_kill_local_variable(
            args, env, editor, macros, state,
        )),
        "kill-all-local-variables" => Some(stateful_kill_all_local_variables(
            args, env, editor, macros, state,
        )),
        "special-variable-p" => Some(stateful_special_variable_p(args, state)),
        "type-of" => Some(stateful_type_of(args, state)),
        "cl-type-of" => Some(stateful_cl_type_of(args, state)),
        "makunbound" => Some(stateful_makunbound(args, env, editor, macros, state)),
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
        "command-modes" => Some(stateful_command_modes(args, state)),
        "add-variable-watcher" => Some(stateful_add_variable_watcher(args, state)),
        "remove-variable-watcher" => Some(stateful_remove_variable_watcher(args, state)),
        "get-variable-watchers" => Some(stateful_get_variable_watchers(args, state)),
        "default-toplevel-value" => Some(stateful_default_toplevel_value(args, state)),
        "set-default-toplevel-value" => Some(stateful_set_default_toplevel_value(args, state)),
        "buffer-local-toplevel-value" => Some(stateful_buffer_local_toplevel_value(args)),
        "set-buffer-local-toplevel-value" => Some(stateful_set_buffer_local_toplevel_value(args)),
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
            if let Some(r) = super::super::metadata::call_metadata_primitive(name, args, state) {
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
            if let Some(r) = crate::primitives_eieio::call_eieio_primitive(name, args, Some(state))
            {
                return Some(r);
            }
            if let Some(r) =
                super::state_cl::call_stateful_cl(name, args, env, editor, macros, state)
            {
                return Some(r.map(|v| value_to_obj(v)));
            }
            if let Some(r) = crate::primitives_cl::call_cl_primitive(name, args) {
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

fn stateful_type_of(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(class_name) = crate::primitives_eieio::object_class_name(&arg)
        && crate::primitives_eieio::get_class_for_state(state, &class_name).is_some()
    {
        return Ok(LispObject::symbol(&class_name));
    }
    if let LispObject::Vector(v) = &arg {
        let guard = v.lock();
        if let Some(first) = guard.first()
            && let Some(sym) = first.as_symbol()
            && crate::primitives_eieio::get_class_for_state(state, &sym).is_some()
        {
            return Ok(LispObject::symbol(&sym));
        }
    }
    crate::primitives::call_primitive("type-of", args)
}

fn stateful_cl_type_of(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(class_name) = crate::primitives_eieio::object_class_name(&arg)
        && crate::primitives_eieio::get_class_for_state(state, &class_name).is_some()
    {
        return Ok(LispObject::symbol(&class_name));
    }
    if let LispObject::Vector(v) = &arg {
        let guard = v.lock();
        if let Some(first) = guard.first()
            && let Some(sym) = first.as_symbol()
            && crate::primitives_eieio::get_class_for_state(state, &sym).is_some()
        {
            return Ok(LispObject::symbol(&sym));
        }
    }
    crate::primitives_cl::prim_cl_type_of(args)
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
    if let Some(val) = env.read().get_id_local(sym_id) {
        return Ok(LispObject::from(!val.is_unbound_marker()));
    }
    if let Some(local) = current_buffer_local(&name) {
        return Ok(LispObject::from(!local.is_unbound_marker()));
    }
    Ok(LispObject::from(
        matches!(sym, LispObject::Nil | LispObject::T)
            || state
                .get_value_cell(sym_id)
                .is_some_and(|val| !val.is_unbound_marker())
            || env
                .read()
                .get(&name)
                .is_some_and(|val| !val.is_unbound_marker()),
    ))
}
fn stateful_fboundp(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
    macros: &MacroTable,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if matches!(sym, LispObject::Nil | LispObject::T) {
        return Ok(LispObject::nil());
    }
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = symbol_name_including_constants(&sym)?;
    let found = state.get_function_cell(sym_id).is_some()
        || env.read().get_function(&name).is_some()
        || macros.read().contains_key(&name);
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
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    assign_symbol_value(
        sym_id,
        val.clone(),
        env,
        editor,
        macros,
        state,
        SetOperation::Set,
    )?;
    Ok(val)
}

#[derive(Clone, Copy)]
pub(crate) enum SetOperation {
    Set,
    Setq,
    SetqLocal,
    SetDefault,
    Makunbound,
    Let,
    Unlet,
    Defvaralias,
}

impl SetOperation {
    fn symbol_name(self) -> &'static str {
        match self {
            SetOperation::Set | SetOperation::Setq => "set",
            SetOperation::SetqLocal => "set",
            SetOperation::SetDefault => "set",
            SetOperation::Makunbound => "makunbound",
            SetOperation::Let => "let",
            SetOperation::Unlet => "unlet",
            SetOperation::Defvaralias => "defvaralias",
        }
    }
}

pub(crate) fn resolve_variable_alias(
    sym_id: crate::obarray::SymbolId,
    state: &InterpreterState,
) -> crate::obarray::SymbolId {
    let alias_key = crate::obarray::intern("variable-alias");
    let mut current = sym_id;
    let mut seen = Vec::new();
    loop {
        if seen.contains(&current) {
            return current;
        }
        seen.push(current);
        let target = state.get_plist(current, alias_key);
        let Some(next) = target.as_symbol_id() else {
            return current;
        };
        current = next;
    }
}

pub(crate) fn assign_symbol_value(
    mut sym_id: crate::obarray::SymbolId,
    mut value: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
    operation: SetOperation,
) -> ElispResult<LispObject> {
    if !matches!(operation, SetOperation::Defvaralias) {
        sym_id = resolve_variable_alias(sym_id, state);
    }
    let name = crate::obarray::symbol_name(sym_id);
    validate_symbol_value_assignment(sym_id, &name, &value)?;
    if name == "display-hourglass" {
        value = LispObject::from(!value.is_nil());
    }

    match operation {
        SetOperation::SetqLocal => {
            if state.specpdl.read().iter().any(|(id, _)| *id == sym_id) {
                if env.read().get_id_local(sym_id).is_some() && !Arc::ptr_eq(env, &state.global_env)
                {
                    env.write().set_id(sym_id, value.clone());
                }
                state.global_env.write().set_id(sym_id, value.clone());
                state.set_value_cell(sym_id, value.clone());
            } else {
                set_current_buffer_local(&name, value.clone());
            }
        }
        SetOperation::SetDefault => {
            env.write().set_id(sym_id, value.clone());
            state.global_env.write().set_id(sym_id, value.clone());
            state.set_value_cell(sym_id, value.clone());
        }
        SetOperation::Set | SetOperation::Setq => {
            if state.special_vars.read().contains(&sym_id) {
                if matches!(operation, SetOperation::Set) && has_current_buffer_local(&name) {
                    set_current_buffer_local(&name, value.clone());
                    if state.specpdl.read().iter().any(|(id, _)| *id == sym_id) {
                        set_previous_buffer_local(&name, value.clone());
                    }
                } else if is_automatic_buffer_local(sym_id, state)
                    && state.specpdl.read().iter().any(|(id, _)| *id == sym_id)
                {
                    if dynamic_binding_buffer(sym_id, state).is_some_and(|id| {
                        crate::buffer::with_registry(|registry| registry.current_id()) != id
                    }) {
                        set_current_buffer_local(&name, value.clone());
                    } else {
                        if env.read().get_id_local(sym_id).is_some()
                            && !Arc::ptr_eq(env, &state.global_env)
                        {
                            env.write().set_id(sym_id, value.clone());
                        }
                        state.global_env.write().set_id(sym_id, value.clone());
                        state.set_value_cell(sym_id, value.clone());
                    }
                } else if is_automatic_buffer_local(sym_id, state) {
                    set_current_buffer_local(&name, value.clone());
                } else if has_current_buffer_local(&name)
                    && !state.specpdl.read().iter().any(|(id, _)| *id == sym_id)
                {
                    set_current_buffer_local(&name, value.clone());
                } else {
                    if env.read().get_id_local(sym_id).is_some()
                        && !Arc::ptr_eq(env, &state.global_env)
                    {
                        env.write().set_id(sym_id, value.clone());
                    }
                    state.global_env.write().set_id(sym_id, value.clone());
                    state.set_value_cell(sym_id, value.clone());
                }
            } else if env.read().get_id_local(sym_id).is_some()
                && !Arc::ptr_eq(env, &state.global_env)
            {
                env.write().set_id(sym_id, value.clone());
            } else if has_current_buffer_local(&name) || is_automatic_buffer_local(sym_id, state) {
                set_current_buffer_local(&name, value.clone());
            } else {
                env.write().set_id(sym_id, value.clone());
                state.global_env.write().set_id(sym_id, value.clone());
                state.set_value_cell(sym_id, value.clone());
            }
        }
        SetOperation::Makunbound => {
            deliver_variable_watchers(
                sym_id,
                &LispObject::nil(),
                operation,
                env,
                editor,
                macros,
                state,
            )?;
            if state.specpdl.read().iter().any(|(id, _)| *id == sym_id) {
                if env.read().get_id_local(sym_id).is_some() && !Arc::ptr_eq(env, &state.global_env)
                {
                    env.write().set_id(sym_id, LispObject::unbound_marker());
                }
                state
                    .global_env
                    .write()
                    .set_id(sym_id, LispObject::unbound_marker());
                state.set_value_cell(sym_id, LispObject::unbound_marker());
            } else if has_current_buffer_local(&name) {
                if has_current_buffer_local(&local_toplevel_key(&name)) {
                    set_current_buffer_local(&name, LispObject::unbound_marker());
                } else {
                    remove_current_buffer_local(&name);
                }
            } else {
                state.clear_value_cell(sym_id);
                state.global_env.write().unset_id(sym_id);
            }
        }
        SetOperation::Let | SetOperation::Unlet | SetOperation::Defvaralias => {}
    }

    if !matches!(operation, SetOperation::Makunbound) {
        deliver_variable_watchers(sym_id, &value, operation, env, editor, macros, state)?;
    }
    Ok(value)
}

fn dynamic_binding_buffer(
    id: crate::obarray::SymbolId,
    state: &InterpreterState,
) -> Option<crate::buffer::BufferId> {
    let key = crate::obarray::intern("rele-dynamic-binding-buffer");
    state.get_plist(id, key).as_integer().map(|id| id as usize)
}

fn validate_symbol_value_assignment(
    sym_id: crate::obarray::SymbolId,
    name: &str,
    value: &LispObject,
) -> ElispResult<()> {
    if is_constant_symbol_name(name) && value != &LispObject::Symbol(sym_id) {
        return Err(setting_constant_signal(sym_id));
    }
    if name == "gc-cons-threshold"
        && !matches!(value, LispObject::Integer(_) | LispObject::BigInt(_))
    {
        return Err(ElispError::WrongTypeArgument("integer".to_string()));
    }
    if name == "scroll-up-aggressively" {
        match value {
            LispObject::Nil => {}
            LispObject::Integer(n) if (0..=1).contains(n) => {}
            LispObject::Float(f) if (0.0..=1.0).contains(f) => {}
            _ => return Err(ElispError::WrongTypeArgument("fraction".to_string())),
        }
    }
    if name == "vertical-scroll-bar" && !value.is_nil() {
        match value.as_symbol().as_deref() {
            Some("left" | "right") => {}
            _ => {
                return Err(ElispError::WrongTypeArgument(
                    "vertical-scroll-bar".to_string(),
                ));
            }
        }
    }
    if name == "overwrite-mode" && !value.is_nil() {
        match value.as_symbol().as_deref() {
            Some("overwrite-mode-textual" | "overwrite-mode-binary") => {}
            _ => return Err(ElispError::WrongTypeArgument("overwrite-mode".to_string())),
        }
    }
    Ok(())
}

pub(crate) fn deliver_variable_watchers(
    sym_id: crate::obarray::SymbolId,
    value: &LispObject,
    operation: SetOperation,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<()> {
    let sym_id = if matches!(operation, SetOperation::Defvaralias) {
        sym_id
    } else {
        resolve_variable_alias(sym_id, state)
    };
    let key = crate::obarray::intern("variable-watchers");
    let watchers = list_to_vec(state.get_plist(sym_id, key));
    if watchers.is_empty() {
        return Ok(());
    }
    let name = crate::obarray::symbol_name(sym_id);
    let is_local_operation = !matches!(
        operation,
        SetOperation::SetDefault | SetOperation::Defvaralias
    ) && has_current_buffer_local(&name);
    let where_arg = if is_local_operation && editor.read().is_some() {
        crate::buffer::with_registry(|registry| {
            registry
                .get(registry.current_id())
                .map(|buffer| LispObject::string(&buffer.name))
                .unwrap_or_else(LispObject::nil)
        })
    } else {
        LispObject::nil()
    };
    for watcher in watchers {
        if watcher.is_nil() {
            continue;
        }
        let callback_args = vec_to_list(vec![
            LispObject::Symbol(sym_id),
            value.clone(),
            LispObject::symbol(operation.symbol_name()),
            where_arg.clone(),
        ]);
        call_variable_watcher(watcher, callback_args, env, editor, macros, state)?;
    }
    Ok(())
}

fn call_variable_watcher(
    watcher: LispObject,
    callback_args: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<()> {
    if let Some((head, rest)) = watcher.clone().destructure_cons()
        && let Some(kind) = head.as_symbol()
    {
        match kind.as_str() {
            "lambda" => {
                let params = rest.first().unwrap_or_else(LispObject::nil);
                let body = rest.rest().unwrap_or_else(LispObject::nil);
                if apply_watcher_body_directly(
                    &params,
                    &body,
                    &callback_args,
                    env,
                    editor,
                    macros,
                    state,
                )? {
                    return Ok(());
                }
                apply_lambda(
                    &params,
                    &body,
                    obj_to_value(callback_args),
                    env,
                    editor,
                    macros,
                    state,
                )?;
                return Ok(());
            }
            "closure" => {
                let rest = rest.rest().ok_or(ElispError::WrongNumberOfArguments)?;
                let params = rest.first().unwrap_or_else(LispObject::nil);
                let body = rest.rest().unwrap_or_else(LispObject::nil);
                if apply_watcher_body_directly(
                    &params,
                    &body,
                    &callback_args,
                    env,
                    editor,
                    macros,
                    state,
                )? {
                    return Ok(());
                }
                apply_lambda(
                    &params,
                    &body,
                    obj_to_value(callback_args),
                    env,
                    editor,
                    macros,
                    state,
                )?;
                return Ok(());
            }
            _ => {}
        }
    }
    let args = LispObject::cons(watcher, callback_args);
    let _ = stateful_funcall(&args, env, editor, macros, state)?;
    Ok(())
}

fn apply_watcher_body_directly(
    params: &LispObject,
    body: &LispObject,
    callback_args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<bool> {
    let first_form = body.first().unwrap_or_else(LispObject::nil);
    if !body.rest().unwrap_or_else(LispObject::nil).is_nil() {
        return Ok(false);
    }
    if let Some((head, rest)) = first_form.destructure_cons()
        && let Some(head_name) = head.as_symbol()
    {
        if head_name == "push" {
            let pushed = rest.first().unwrap_or_else(LispObject::nil);
            let place = rest.nth(1).unwrap_or_else(LispObject::nil);
            if let (Some(rest_param), Some(place_id)) =
                (rest_param_name(params), place.as_symbol_id())
                && pushed.as_symbol_id() == Some(rest_param)
            {
                let old = env.read().get_id(place_id).unwrap_or_else(LispObject::nil);
                env.write()
                    .set_id(place_id, LispObject::cons(callback_args.clone(), old));
                return Ok(true);
            }
        }
        if head_name == "setq" && only_rest_params(params) {
            let place = rest.first().unwrap_or_else(LispObject::nil);
            let value_expr = rest.nth(1).unwrap_or_else(LispObject::nil);
            if let Some(place_id) = place.as_symbol_id() {
                let value =
                    value_to_obj(eval(obj_to_value(value_expr), env, editor, macros, state)?);
                env.write().set_id(place_id, value);
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn rest_param_name(params: &LispObject) -> Option<crate::obarray::SymbolId> {
    let mut cur = params.clone();
    while let Some((param, rest)) = cur.destructure_cons() {
        if param.as_symbol().as_deref() == Some("&rest") {
            return rest.first().and_then(|arg| arg.as_symbol_id());
        }
        cur = rest;
    }
    None
}

fn only_rest_params(params: &LispObject) -> bool {
    let cur = params.clone();
    while let Some((param, rest)) = cur.destructure_cons() {
        if param.as_symbol().as_deref() == Some("&rest") {
            return rest.rest().map(|tail| tail.is_nil()).unwrap_or(true);
        }
        return false;
    }
    true
}

pub(crate) fn set_function_cell_checked(
    sym_id: crate::obarray::SymbolId,
    def: LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = crate::obarray::symbol_name(sym_id);
    if is_constant_symbol_name(&name) {
        return Err(setting_constant_signal(sym_id));
    }
    reject_cyclic_function_indirection(sym_id, &def, state)?;
    state.set_function_cell(sym_id, def.clone());
    Ok(def)
}

fn is_constant_symbol_name(name: &str) -> bool {
    matches!(
        name,
        "nil" | "t" | "initial-window-system" | "most-positive-fixnum" | "most-negative-fixnum"
    ) || name.starts_with(':')
}

fn setting_constant_signal(sym_id: crate::obarray::SymbolId) -> ElispError {
    ElispError::Signal(Box::new(crate::error::SignalData {
        symbol: LispObject::symbol("setting-constant"),
        data: LispObject::cons(LispObject::Symbol(sym_id), LispObject::nil()),
    }))
}

fn current_buffer_local(name: &str) -> Option<LispObject> {
    crate::buffer::with_registry(|registry| {
        registry
            .get(registry.current_id())
            .and_then(|buffer| buffer.locals.get(name).cloned())
    })
}

fn has_current_buffer_local(name: &str) -> bool {
    crate::buffer::with_registry(|registry| {
        registry
            .get(registry.current_id())
            .is_some_and(|buffer| buffer.locals.contains_key(name))
    })
}

fn set_current_buffer_local(name: &str, value: LispObject) {
    crate::buffer::with_registry_mut(|registry| {
        let current = registry.current_id();
        if let Some(buffer) = registry.get_mut(current) {
            buffer.locals.insert(name.to_string(), value);
        }
    });
}

fn local_toplevel_key(name: &str) -> String {
    format!("__rele_buffer_local_toplevel::{name}")
}

fn set_previous_buffer_local(name: &str, value: LispObject) {
    crate::buffer::with_registry_mut(|registry| {
        let Some(&previous) = registry.stack.iter().rev().nth(1) else {
            return;
        };
        if let Some(buffer) = registry.get_mut(previous) {
            buffer.locals.insert(name.to_string(), value);
        }
    });
}

fn remove_current_buffer_local(name: &str) -> Option<LispObject> {
    crate::buffer::with_registry_mut(|registry| {
        let current = registry.current_id();
        registry
            .get_mut(current)
            .and_then(|buffer| buffer.locals.remove(name))
    })
}

fn clear_current_buffer_locals() -> Vec<(String, LispObject)> {
    crate::buffer::with_registry_mut(|registry| {
        let current = registry.current_id();
        registry
            .get_mut(current)
            .map(|buffer| buffer.locals.drain().collect())
            .unwrap_or_default()
    })
}

fn is_automatic_buffer_local(id: crate::obarray::SymbolId, state: &InterpreterState) -> bool {
    let key = crate::obarray::intern("variable-buffer-local");
    !state.get_plist(id, key).is_nil()
}

fn stateful_make_local_variable(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = crate::obarray::symbol_name(sym_id);
    let value = current_buffer_local(&name)
        .or_else(|| env.read().get_id_local(sym_id))
        .or_else(|| state.get_value_cell(sym_id))
        .or_else(|| env.read().get(&name))
        .unwrap_or_else(LispObject::nil);
    set_current_buffer_local(&name, value);
    if name == "last-coding-system-used" {
        state.set_value_cell(
            sym_id,
            current_buffer_local(&name).unwrap_or_else(LispObject::nil),
        );
        state.global_env.write().set_id(
            sym_id,
            current_buffer_local(&name).unwrap_or_else(LispObject::nil),
        );
        env.write().unset_id(sym_id);
    }
    Ok(LispObject::Symbol(sym_id))
}

fn stateful_make_variable_buffer_local(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let key = crate::obarray::intern("variable-buffer-local");
    state.put_plist(sym_id, key, LispObject::t());
    Ok(LispObject::Symbol(sym_id))
}

fn stateful_local_variable_p(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = crate::obarray::symbol_name(sym_id);
    Ok(LispObject::from(has_current_buffer_local(&name)))
}

fn stateful_variable_binding_locus(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = crate::obarray::symbol_name(sym_id);
    if has_current_buffer_local(&name) {
        let buffer_name = crate::buffer::with_registry(|registry| {
            registry
                .get(registry.current_id())
                .map(|buffer| buffer.name.clone())
        });
        return Ok(buffer_name
            .map(|name| LispObject::string(&name))
            .unwrap_or_else(LispObject::nil));
    }
    Ok(LispObject::nil())
}

fn stateful_buffer_local_toplevel_value(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = crate::obarray::symbol_name(sym_id);
    current_buffer_local(&local_toplevel_key(&name))
        .or_else(|| current_buffer_local(&name))
        .ok_or_else(|| ElispError::VoidVariable(name))
}

fn stateful_set_buffer_local_toplevel_value(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = crate::obarray::symbol_name(sym_id);
    if !has_current_buffer_local(&name) {
        return Err(ElispError::VoidVariable(name));
    }
    let key = local_toplevel_key(&name);
    if has_current_buffer_local(&key) {
        set_current_buffer_local(&key, val.clone());
    } else {
        set_current_buffer_local(&name, val.clone());
    }
    Ok(val)
}

fn stateful_kill_local_variable(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    let name = crate::obarray::symbol_name(sym_id);
    if has_current_buffer_local(&name) {
        deliver_variable_watchers(
            sym_id,
            &LispObject::nil(),
            SetOperation::Makunbound,
            env,
            editor,
            macros,
            state,
        )?;
    }
    remove_current_buffer_local(&name);
    Ok(LispObject::Symbol(sym_id))
}

fn stateful_kill_all_local_variables(
    _args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let locals = crate::buffer::with_registry(|registry| {
        registry
            .get(registry.current_id())
            .map(|buffer| buffer.locals.clone())
            .unwrap_or_default()
    });
    for name in locals.keys() {
        let sym_id = crate::obarray::intern(name);
        deliver_variable_watchers(
            sym_id,
            &LispObject::nil(),
            SetOperation::Makunbound,
            env,
            editor,
            macros,
            state,
        )?;
    }
    let _removed = clear_current_buffer_locals();
    Ok(LispObject::nil())
}

fn stateful_set_default(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    assign_symbol_value(
        sym_id,
        val.clone(),
        env,
        editor,
        macros,
        state,
        SetOperation::SetDefault,
    )
}

fn stateful_default_value(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    Ok(state.get_value_cell(sym_id).unwrap_or_else(LispObject::nil))
}

fn stateful_default_boundp(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    Ok(LispObject::from(
        state
            .get_value_cell(sym_id)
            .is_some_and(|val| !val.is_unbound_marker()),
    ))
}

fn stateful_set_default_toplevel_value(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let key = crate::obarray::intern("default-toplevel-value");
    state.put_plist(sym_id, key, val.clone());
    Ok(val)
}

fn stateful_symbol_value(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    let name = crate::obarray::symbol_name(sym_id);
    if state.special_vars.read().contains(&sym_id)
        && state.specpdl.read().iter().any(|(id, _)| *id == sym_id)
    {
        return match state.global_env.read().get_id(sym_id) {
            Some(val) if val.is_unbound_marker() => Err(ElispError::VoidVariable(name)),
            Some(val) => Ok(val),
            None => Err(ElispError::VoidVariable(name)),
        };
    }
    if state.special_vars.read().contains(&sym_id) {
        return match current_buffer_local(&name).or_else(|| state.global_env.read().get_id(sym_id))
        {
            Some(val) if val.is_unbound_marker() => Err(ElispError::VoidVariable(name)),
            Some(val) => Ok(val),
            None => Err(ElispError::VoidVariable(name)),
        };
    }
    if let Some(val) = env.read().get_id_local(sym_id) {
        if val.is_unbound_marker() {
            return Err(ElispError::VoidVariable(name));
        }
        return Ok(val);
    }
    match current_buffer_local(&name).or_else(|| state.get_value_cell(sym_id)) {
        Some(val) if val.is_unbound_marker() => Err(ElispError::VoidVariable(name)),
        Some(val) => Ok(val),
        None => Ok(LispObject::nil()),
    }
}
fn stateful_symbol_function(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    let name = symbol_name_including_constants(&sym)?;
    if name == "if" {
        return Ok(LispObject::primitive("if"));
    }
    if let Some(val) = state.get_function_cell(sym_id) {
        return Ok(val);
    }
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
fn stateful_special_variable_p(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    Ok(LispObject::from(
        state.special_vars.read().contains(&sym_id),
    ))
}
fn stateful_makunbound(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(id) = sym.as_symbol_id() {
        let name = crate::obarray::symbol_name(id);
        if is_makunbound_forbidden(&name) {
            return Err(setting_constant_signal(id));
        }
        assign_symbol_value(
            id,
            LispObject::unbound_marker(),
            env,
            editor,
            macros,
            state,
            SetOperation::Makunbound,
        )?;
    }
    Ok(sym)
}
fn stateful_fmakunbound(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(id) = sym.as_symbol_id() {
        let name = crate::obarray::symbol_name(id);
        if is_constant_symbol_name(&name) {
            return Err(setting_constant_signal(id));
        }
        state.clear_function_cell(id);
    }
    Ok(sym)
}

fn is_makunbound_forbidden(name: &str) -> bool {
    matches!(
        name,
        "nil" | "t" | "initial-window-system" | "most-positive-fixnum" | "most-negative-fixnum"
    ) || name.starts_with(':')
}

fn list_to_vec(list: LispObject) -> Vec<LispObject> {
    let mut out = Vec::new();
    let mut cur = list;
    while let Some((car, cdr)) = cur.destructure_cons() {
        out.push(car);
        cur = cdr;
    }
    out
}

fn vec_to_list(items: Vec<LispObject>) -> LispObject {
    let mut out = LispObject::nil();
    for item in items.into_iter().rev() {
        out = LispObject::cons(item, out);
    }
    out
}

fn stateful_command_modes(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = symbol_id_including_constants(&sym)?;
    Ok(state.get_plist(id, crate::obarray::intern("command-modes")))
}

fn stateful_get_variable_watchers(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    Ok(state.get_plist(id, crate::obarray::intern("variable-watchers")))
}

fn stateful_add_variable_watcher(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let watcher = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    let key = crate::obarray::intern("variable-watchers");
    let mut watchers = list_to_vec(state.get_plist(id, key));
    if !watchers.iter().any(|item| item == &watcher) {
        watchers.push(watcher.clone());
    }
    state.put_plist(id, key, vec_to_list(watchers));
    Ok(watcher)
}

fn stateful_remove_variable_watcher(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let watcher = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    let key = crate::obarray::intern("variable-watchers");
    let watchers = list_to_vec(state.get_plist(id, key))
        .into_iter()
        .filter(|item| item != &watcher)
        .collect();
    state.put_plist(id, key, vec_to_list(watchers));
    Ok(watcher)
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
    set_function_cell_checked(id, value, state)?;
    Ok(LispObject::symbol(&name_str))
}
/// `(fset SYMBOL DEFINITION)` — set SYMBOL's function cell. Like
/// `defalias` but doesn't check for the macro form. Args pre-evaluated.
fn stateful_fset(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let def = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&name)?;
    set_function_cell_checked(sym_id, def, state)
}

fn reject_cyclic_function_indirection(
    name: crate::obarray::SymbolId,
    def: &LispObject,
    state: &InterpreterState,
) -> ElispResult<()> {
    let mut current = match def {
        LispObject::Symbol(id) => *id,
        _ => return Ok(()),
    };
    let mut seen = std::collections::HashSet::new();
    while seen.insert(current) {
        if current == name {
            return Err(ElispError::Signal(Box::new(crate::error::SignalData {
                symbol: LispObject::symbol("cyclic-function-indirection"),
                data: LispObject::cons(LispObject::Symbol(name), LispObject::nil()),
            })));
        }
        match state.get_function_cell(current) {
            Some(LispObject::Symbol(next)) => current = next,
            _ => return Ok(()),
        }
    }
    Ok(())
}
