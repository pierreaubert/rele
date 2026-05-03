//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use super::SyncRefCell as RwLock;
use super::call_function;
use super::functions_3::eval_list;
use super::{Environment, InterpreterState, MacroTable, eval};
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::sync::Arc;

/// `(cl-generic-define NAME ARGS OPTIONS)` from byte-compiled cl-generic.
///
/// Real Emacs returns a dispatcher closure here; the subsequent
/// `cl-generic-define-method` calls attach primary/default methods. For
/// bootstrap loading, an ignore-backed function keeps the generic callable
/// until a primary method replaces it.
pub(super) fn stateful_cl_generic_define(args: &LispObject) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if name.as_symbol().is_none() {
        return Ok(LispObject::primitive("ignore"));
    }
    Ok(LispObject::primitive("ignore"))
}
/// Bootstrap implementation of `cl-generic-generalizers`.
///
/// `cl-generic.el` defines the real generic function in terms of methods
/// that themselves ask `cl-generic-generalizers` about `(head ...)` and
/// `(eql ...)` specializers. Until the full dispatcher exists, keep this
/// narrow Rust-backed version available so those self-hosting definitions can
/// load and build their metadata.
pub(super) fn stateful_cl_generic_generalizers(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let specializer = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let generalizer_name = cl_generic_generalizer_symbol(&specializer).unwrap_or_else(|| {
        if specializer.is_nil() {
            "cl--generic-t-generalizer"
        } else {
            "cl--generic-typeof-generalizer"
        }
    });
    let generalizer_id = crate::obarray::intern(generalizer_name);
    let generalizer = state
        .get_value_cell(generalizer_id)
        .unwrap_or_else(|| LispObject::symbol(generalizer_name));
    Ok(LispObject::cons(generalizer, LispObject::nil()))
}
fn cl_generic_generalizer_symbol(specializer: &LispObject) -> Option<&'static str> {
    if matches!(specializer, LispObject::T) {
        return Some("cl--generic-t-generalizer");
    }
    let LispObject::Cons(_) = specializer else {
        return None;
    };
    match specializer
        .first()
        .and_then(|head| head.as_symbol())
        .as_deref()
    {
        Some("head") => Some("cl--generic-head-generalizer"),
        Some("eql") => Some("cl--generic-eql-generalizer"),
        Some("derived-mode") => Some("cl--generic-derived-generalizer"),
        Some("oclosure") => Some("cl--generic-oclosure-generalizer"),
        _ => None,
    }
}
/// `(cl-generic-define-method NAME QUALIFIERS ARGS CALL-CON FUNCTION)`.
///
/// This mirrors the source-level `cl-defmethod` approximation already in
/// eval/mod.rs: primary methods become the callable function cell, while
/// qualified and `(setf NAME)` methods are ignored until the runtime grows a
/// real generic dispatcher.
pub(super) fn stateful_cl_generic_define_method(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let qualifiers = args.nth(1).unwrap_or(LispObject::nil());
    let function = args.nth(4).ok_or(ElispError::WrongNumberOfArguments)?;
    if !qualifiers.is_nil() {
        return Ok(LispObject::nil());
    }
    let Some(sym_id) = name.as_symbol_id() else {
        return Ok(LispObject::nil());
    };
    state.set_function_cell(sym_id, function.clone());
    Ok(name.clone())
}
/// `(eval FORM)` — evaluate FORM. Args pre-evaluated means FORM is the
/// ACTUAL form to evaluate (not wrapped in another eval).
pub(super) fn stateful_eval(
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
pub(super) fn stateful_funcall(
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
pub(super) fn stateful_put(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = plist_symbol_id(&sym)?;
    let prop_id = plist_symbol_id(&prop)?;
    state.put_plist(sym_id, prop_id, val.clone());
    Ok(val)
}
/// `(get SYMBOL PROPERTY)` — get SYMBOL's plist PROPERTY. Args
/// pre-evaluated.
pub(super) fn stateful_get(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = plist_symbol_id(&sym)?;
    let prop_id = plist_symbol_id(&prop)?;
    Ok(state.get_plist(sym_id, prop_id))
}

pub(super) fn stateful_setplist(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let plist = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = plist_symbol_id(&sym)?;
    state.replace_plist(sym_id, plist.clone());
    Ok(plist)
}

fn plist_symbol_id(obj: &LispObject) -> ElispResult<crate::obarray::SymbolId> {
    symbol_id_including_constants(obj)
}
pub(super) fn symbol_id_including_constants(
    obj: &LispObject,
) -> ElispResult<crate::obarray::SymbolId> {
    match obj {
        LispObject::Symbol(id) => Ok(*id),
        LispObject::T => Ok(crate::obarray::intern("t")),
        LispObject::Nil => Ok(crate::obarray::intern("nil")),
        _ => Err(ElispError::WrongTypeArgument("symbol".to_string())),
    }
}
pub(super) fn symbol_name_including_constants(obj: &LispObject) -> ElispResult<String> {
    Ok(crate::obarray::symbol_name(symbol_id_including_constants(
        obj,
    )?))
}
pub(super) fn stateful_provide(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let feature = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = feature
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let mut features = state.features.write();
    if !features.contains(&name) {
        features.push(name);
    }
    Ok(feature)
}
pub(super) fn stateful_featurep(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let feature = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = feature
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    Ok(LispObject::from(state.features.read().contains(&name)))
}
pub(super) fn stateful_require(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let feature = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = feature
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    if state.features.read().contains(&name) {
        return Ok(feature);
    }
    let file = args
        .nth(1)
        .and_then(|f| f.as_string().map(ToString::to_string))
        .unwrap_or(name);
    let load_args = LispObject::cons(
        LispObject::string(&file),
        LispObject::cons(LispObject::nil(), LispObject::nil()),
    );
    super::builtins::eval_load(obj_to_value(load_args), env, editor, macros, state)?;
    Ok(feature)
}
/// `(function-put FUNCTION PROPERTY VALUE)` — Emacs currently stores
/// function properties on the function symbol's plist, just like `put`.
pub(super) fn stateful_function_put(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    stateful_put(args, state)
}
/// `(function-get FUNCTION PROPERTY &optional AUTOLOAD)` — like `get`,
/// but if FUNCTION is a simple symbol alias, read the aliased symbol's
/// plist. We ignore AUTOLOAD for now; `require`/autoload already has
/// separate bootstrap plumbing in this interpreter.
pub(super) fn stateful_function_get(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let function = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(mut sym_id) = function.as_symbol_id() else {
        return Ok(LispObject::nil());
    };
    let prop_id = prop
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    for _ in 0..32 {
        match state.get_function_cell(sym_id) {
            Some(LispObject::Symbol(target)) => sym_id = target,
            _ => break,
        }
    }
    Ok(state.get_plist(sym_id, prop_id))
}
/// `(def-edebug-elem-spec NAME SPEC)` records an Edebug shorthand spec.
/// We do not instrument code, but stdlib loaders expect the metadata to
/// exist on NAME's plist.
pub(super) fn stateful_def_edebug_elem_spec(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let spec = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let key = crate::obarray::intern("edebug-elem-spec");
    state.put_plist(name, key, spec.clone());
    Ok(spec)
}
/// `(defvar-1 SYMBOL INITVALUE DOCSTRING)` is emitted by bytecomp for
/// top-level defvars. Keep the interpreter-visible binding in sync with
/// the per-interpreter value cell and record the docstring when present.
pub(super) fn stateful_defvar_1(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    state.special_vars.write().insert(sym);
    if env.read().get_id_local(sym).is_none() && state.get_value_cell(sym).is_none() {
        let init = args.nth(1).unwrap_or_else(LispObject::nil);
        env.write().define_id(sym, init.clone());
        state.set_value_cell(sym, init);
    }
    if let Some(doc) = args.nth(2) {
        let key = crate::obarray::intern("variable-documentation");
        state.put_plist(sym, key, doc);
    }
    Ok(LispObject::Symbol(sym))
}

pub(super) fn stateful_defconst_1(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let value = args.nth(1).unwrap_or_else(LispObject::nil);
    state.special_vars.write().insert(sym);
    env.write().define_id(sym, value.clone());
    state.set_value_cell(sym, value);
    if let Some(doc) = args.nth(2) {
        let key = crate::obarray::intern("variable-documentation");
        state.put_plist(sym, key, doc);
    }
    state.put_plist(
        sym,
        crate::obarray::intern("risky-local-variable"),
        LispObject::t(),
    );
    Ok(LispObject::Symbol(sym))
}

pub(super) fn stateful_internal_define_uninitialized_variable(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    state.special_vars.write().insert(sym);
    if let Some(doc) = args.nth(1) {
        let key = crate::obarray::intern("variable-documentation");
        state.put_plist(sym, key, doc);
    }
    Ok(LispObject::Symbol(sym))
}

pub(super) fn stateful_internal_make_var_non_special(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    state.special_vars.write().remove(&sym);
    Ok(LispObject::nil())
}

/// `(internal-delete-indirect-variable SYM)` — undo a `defvaralias` on
/// SYM. Removes the `variable-alias` plist entry and the
/// `variable-documentation`. Signals if SYM is not currently aliased.
pub(super) fn stateful_internal_delete_indirect_variable(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let alias_key = crate::obarray::intern("variable-alias");
    let current = state.get_plist(sym, alias_key);
    if current.as_symbol_id().is_none() {
        return Err(ElispError::WrongTypeArgument(
            "indirect-variable".to_string(),
        ));
    }
    state.put_plist(sym, alias_key, LispObject::nil());
    let doc_key = crate::obarray::intern("variable-documentation");
    state.put_plist(sym, doc_key, LispObject::nil());
    Ok(LispObject::nil())
}

/// `(make-interpreted-closure ARGS BODY ENV &optional DOCSTRING IFORM)` —
/// build the `(closure ENV ARGS . BODY)` representation. DOCSTRING and
/// IFORM (an `(interactive ...)` form) are prepended to BODY in the
/// conventional order that `documentation` and `interactive-form` look
/// for them.
pub(super) fn stateful_make_interpreted_closure(args: &LispObject) -> ElispResult<LispObject> {
    let arglist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let env = args.nth(2).unwrap_or_else(LispObject::nil);
    let docstring = args.nth(3).unwrap_or_else(LispObject::nil);
    let iform = args.nth(4).unwrap_or_else(LispObject::nil);

    let mut full_body = body;
    if !iform.is_nil() {
        full_body = LispObject::cons(iform, full_body);
    }
    if !docstring.is_nil() {
        full_body = LispObject::cons(docstring, full_body);
    }
    Ok(LispObject::closure_expr(env, arglist, full_body))
}
/// `(add-to-list SYMBOL ELEMENT &optional APPEND COMPARE-FN)`.
///
/// This is stateful because SYMBOL names the variable to mutate. The
/// implementation intentionally ignores custom COMPARE-FN for now, but uses
/// structural equality instead of pointer identity so bootstrap tag lists and
/// ordinary symbol/string lists behave predictably.
pub(super) fn stateful_add_to_list(
    args: &LispObject,
    _env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let val = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let append = args.nth(2).unwrap_or_else(LispObject::nil);
    let name = crate::obarray::symbol_name(sym);
    let existing = state
        .get_value_cell(sym)
        .or_else(|| state.global_env.read().get(&name))
        .unwrap_or_else(LispObject::nil);
    let mut cur = existing.clone();
    while let Some((head, tail)) = cur.destructure_cons() {
        if head == val {
            return Ok(existing);
        }
        cur = tail;
    }
    let updated = if append.is_nil() {
        LispObject::cons(val, existing)
    } else {
        append_lisp_list(existing, val)
    };
    state.set_value_cell(sym, updated.clone());
    state.global_env.write().set_id(sym, updated.clone());
    Ok(updated)
}
fn append_lisp_list(list: LispObject, val: LispObject) -> LispObject {
    if list.is_nil() {
        return LispObject::cons(val, LispObject::nil());
    }
    let mut items = Vec::new();
    let mut cur = list;
    while let Some((head, tail)) = cur.destructure_cons() {
        items.push(head);
        cur = tail;
    }
    items.push(val);
    let mut out = LispObject::nil();
    for item in items.into_iter().rev() {
        out = LispObject::cons(item, out);
    }
    out
}
/// `(apply FUNC &rest ARGS)` — last ARG is a list; splice its contents
/// as trailing arguments. All args pre-evaluated.
pub(super) fn stateful_apply(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let func = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
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
pub(crate) fn eval_funcall(
    func: Value,
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let func_obj = value_to_obj(func);
    let caller_symbol = func_obj.as_symbol_id();
    let func_name: Option<String> = {
        if let LispObject::Symbol(id) = &func_obj {
            let name = crate::obarray::symbol_name(*id);
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
        if let Some(result) =
            crate::primitives_eieio::try_keyword_slot_call_with_state(kw, &args_obj, Some(state))
        {
            return result.map(obj_to_value);
        }
        return Err(ElispError::VoidFunction(kw.clone()));
    }
    let args = eval_list(args, env, editor, macros, state)?;
    if caller_symbol.is_some() {
        call_function(obj_to_value(func_obj), args, env, editor, macros, state)
    } else {
        let func = resolve_function(func, env, editor, macros, state)?;
        call_function(func, args, env, editor, macros, state)
    }
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
        let resolved = env.read().get_function(&name);
        if let Some(val) = resolved {
            return Ok(obj_to_value(val));
        }
        let autoload_file = state.autoloads.read().get(&name).cloned();
        if let Some(file) = autoload_file {
            let load_args = LispObject::cons(
                LispObject::string(&file),
                LispObject::cons(LispObject::t(), LispObject::nil()),
            );
            let _ = super::builtins::eval_load(obj_to_value(load_args), env, editor, macros, state);
            let resolved2 = env.read().get_function(&name);
            if let Some(val) = resolved2 {
                return Ok(obj_to_value(val));
            }
        }
        Err(ElispError::VoidFunction(name))
    } else {
        eval(func, env, editor, macros, state)
    }
}
pub(crate) fn eval_apply(
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
/// `(add-hook HOOK FUNCTION &optional APPEND LOCAL)` — push FUNCTION
/// onto HOOK's value cell. APPEND non-nil places it at the tail.
/// LOCAL is accepted for compatibility but ignored (no buffer-local
/// hooks in the headless model).
pub(super) fn stateful_add_hook(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let hook = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let func = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let append = args.nth(2).is_some_and(|v| !v.is_nil());
    let name = crate::obarray::symbol_name(hook);
    let existing = state
        .get_value_cell(hook)
        .or_else(|| state.global_env.read().get(&name))
        .unwrap_or_else(LispObject::nil);
    let mut cur = existing.clone();
    while let Some((head, tail)) = cur.destructure_cons() {
        if head == func {
            return Ok(LispObject::nil());
        }
        cur = tail;
    }
    let updated = if append {
        append_lisp_list(existing, func)
    } else {
        LispObject::cons(func, existing)
    };
    state.set_value_cell(hook, updated.clone());
    state.global_env.write().set_id(hook, updated);
    Ok(LispObject::nil())
}

/// `(remove-hook HOOK FUNCTION &optional LOCAL)` — drop FUNCTION
/// from HOOK's list of functions.
pub(super) fn stateful_remove_hook(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let hook = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let func = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let name = crate::obarray::symbol_name(hook);
    let existing = state
        .get_value_cell(hook)
        .or_else(|| state.global_env.read().get(&name))
        .unwrap_or_else(LispObject::nil);
    let mut items = Vec::new();
    let mut cur = existing;
    while let Some((head, tail)) = cur.destructure_cons() {
        if head != func {
            items.push(head);
        }
        cur = tail;
    }
    let updated = items
        .into_iter()
        .rev()
        .fold(LispObject::nil(), |tail, item| LispObject::cons(item, tail));
    state.set_value_cell(hook, updated.clone());
    state.global_env.write().set_id(hook, updated);
    Ok(LispObject::nil())
}

#[allow(clippy::too_many_arguments)]
fn run_hook_functions(
    hook: LispObject,
    extra_args: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
    stop_on_success: bool,
    stop_on_failure: bool,
) -> ElispResult<LispObject> {
    let Some(sym) = hook.as_symbol_id() else {
        return Ok(LispObject::nil());
    };
    let name = crate::obarray::symbol_name(sym);
    let funcs = state
        .get_value_cell(sym)
        .or_else(|| state.global_env.read().get(&name))
        .unwrap_or_else(LispObject::nil);
    // A hook value can be either a single function or a list of functions.
    // List form is recognised by a leading symbol of `lambda`/`closure` not
    // applying — Emacs special-cases this; we keep it simple: if the value
    // is a non-nil non-cons (e.g., symbol, primitive), treat as singleton.
    let mut cur = match funcs {
        LispObject::Nil => return Ok(LispObject::nil()),
        LispObject::Cons(_) => funcs,
        other => LispObject::cons(other, LispObject::nil()),
    };
    let mut last = LispObject::nil();
    while let Some((func, rest)) = cur.destructure_cons() {
        let result = call_function(
            obj_to_value(func),
            obj_to_value(extra_args.clone()),
            env,
            editor,
            macros,
            state,
        )?;
        last = value_to_obj(result);
        if stop_on_success && !last.is_nil() {
            return Ok(last);
        }
        if stop_on_failure && last.is_nil() {
            return Ok(last);
        }
        cur = rest;
    }
    Ok(last)
}

/// `(run-hooks &rest HOOKS)` — run each hook for side effects.
pub(super) fn stateful_run_hooks(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let mut cur = args.clone();
    while let Some((hook, rest)) = cur.destructure_cons() {
        run_hook_functions(
            hook,
            LispObject::nil(),
            env,
            editor,
            macros,
            state,
            false,
            false,
        )?;
        cur = rest;
    }
    Ok(LispObject::nil())
}

/// `(run-hook-with-args HOOK &rest ARGS)` — run HOOK passing ARGS.
pub(super) fn stateful_run_hook_with_args(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let hook = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let extra = args.rest().unwrap_or_else(LispObject::nil);
    run_hook_functions(hook, extra, env, editor, macros, state, false, false)
}

pub(super) fn stateful_run_hook_with_args_until_success(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let hook = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let extra = args.rest().unwrap_or_else(LispObject::nil);
    run_hook_functions(hook, extra, env, editor, macros, state, true, false)
}

pub(super) fn stateful_run_hook_with_args_until_failure(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let hook = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let extra = args.rest().unwrap_or_else(LispObject::nil);
    run_hook_functions(hook, extra, env, editor, macros, state, false, true)
}

/// `(run-hook-wrapped HOOK WRAP-FN &rest ARGS)` — call WRAP-FN on each
/// function in HOOK. Stops on first non-nil result.
pub(super) fn stateful_run_hook_wrapped(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let hook = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let wrap = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let extra = args.rest().and_then(|r| r.rest()).unwrap_or_else(LispObject::nil);
    let Some(sym) = hook.as_symbol_id() else {
        return Ok(LispObject::nil());
    };
    let name = crate::obarray::symbol_name(sym);
    let funcs = state
        .get_value_cell(sym)
        .or_else(|| state.global_env.read().get(&name))
        .unwrap_or_else(LispObject::nil);
    let mut cur = match funcs {
        LispObject::Nil => return Ok(LispObject::nil()),
        LispObject::Cons(_) => funcs,
        other => LispObject::cons(other, LispObject::nil()),
    };
    while let Some((func, rest)) = cur.destructure_cons() {
        let call_args = LispObject::cons(func, extra.clone());
        let result = call_function(
            obj_to_value(wrap.clone()),
            obj_to_value(call_args),
            env,
            editor,
            macros,
            state,
        )?;
        let v = value_to_obj(result);
        if !v.is_nil() {
            return Ok(v);
        }
        cur = rest;
    }
    Ok(LispObject::nil())
}

/// Advice family — rele-elisp does not yet model around/before/after
/// advice. `advice-add` and friends accept the call but leave the
/// underlying function untouched, so callers keep working without
/// crashing.
pub(super) fn stateful_advice_add(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args;
    Ok(LispObject::nil())
}

pub(super) fn stateful_advice_remove(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args;
    Ok(LispObject::nil())
}

/// `(kill-buffer &optional BUFFER)` with the modified-buffer prompt
/// and the unlock-error path Emacs implements in `kill-buffer`. We
/// reproduce just enough to support the filelock test corpus:
///
/// - If the target buffer is modified and visits a file, ask
///   `yes-or-no-p` first; bail out (return nil) on a "no" answer.
/// - On confirmation, attempt the unlock. If unlock raises
///   `file-error` (the spoiled-lock case), call
///   `userlock--handle-unlock-error` so user code can intervene
///   (bug#46397).
/// - Then perform the actual buffer removal.
pub(super) fn stateful_kill_buffer(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let target_arg = args.first().unwrap_or(LispObject::nil());
    let target_id = if matches!(target_arg, LispObject::Nil) {
        Some(crate::buffer::with_registry(|r| r.current_id()))
    } else {
        crate::primitives::buffer::resolve_buffer(&target_arg)
    };
    let Some(buffer_id) = target_id else {
        return Ok(LispObject::nil());
    };
    let (modified, file) = crate::buffer::with_registry(|r| {
        r.get(buffer_id)
            .map(|b| (b.modified, b.file_name.clone()))
            .unwrap_or((false, None))
    });

    if modified && file.is_some() {
        let prompt = LispObject::string(&format!(
            "Buffer {:?} modified; kill anyway? ",
            file.as_deref().unwrap_or("")
        ));
        let prompt_args = LispObject::cons(prompt, LispObject::nil());
        let yes_or_no_p = env
            .read()
            .get_function("yes-or-no-p")
            .unwrap_or_else(|| LispObject::primitive("yes-or-no-p"));
        let answer = super::functions_3::call_function(
            obj_to_value(yes_or_no_p),
            obj_to_value(prompt_args),
            env,
            editor,
            macros,
            state,
        )?;
        if value_to_obj(answer).is_nil() {
            return Ok(LispObject::nil());
        }
        // Try to unlock. On file-error route through
        // `userlock--handle-unlock-error` so the test (and user
        // configurations) can intercept.
        let prev_id = crate::buffer::with_registry(|r| r.current_id());
        let _ = crate::buffer::with_registry_mut(|r| r.set_current(buffer_id));
        let unlock_result = crate::primitives_buffer::prim_unlock_buffer(&LispObject::nil());
        let _ = crate::buffer::with_registry_mut(|r| r.set_current(prev_id));
        if let Err(err) = unlock_result {
            if let ElispError::Signal(sig) = &err
                && sig.symbol.as_symbol().as_deref() == Some("file-error")
            {
                let handler_args = LispObject::cons(
                    sig.symbol.clone(),
                    LispObject::cons(sig.data.clone(), LispObject::nil()),
                );
                let handler = env
                    .read()
                    .get_function("userlock--handle-unlock-error")
                    .unwrap_or_else(LispObject::nil);
                if !handler.is_nil() {
                    super::functions_3::call_function(
                        obj_to_value(handler),
                        obj_to_value(handler_args),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                }
            } else {
                return Err(err);
            }
        }
    }

    let killed = crate::buffer::with_registry_mut(|r| r.kill(buffer_id));
    Ok(LispObject::from(killed))
}

pub(crate) fn eval_funcall_form(
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
