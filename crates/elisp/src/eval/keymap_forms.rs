use super::{Environment, InterpreterState, MacroTable, RwLock, eval};
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::obarray;
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::sync::Arc;

const CURRENT_GLOBAL_MAP_SYMBOL: &str = "rele--current-global-map";
const CURRENT_LOCAL_MAP_SYMBOL: &str = "rele--current-local-map";

pub(super) fn eval_keymap_form(
    name: &str,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> Option<ElispResult<Value>> {
    match name {
        "make-keymap" => Some(call_keymap_primitive("make-keymap", LispObject::nil())),
        "define-keymap" => Some(eval_define_keymap(args, env, editor, macros, state)),
        "key-valid-p" => Some(eval_key_valid_p(args, env, editor, macros, state)),
        "defvar-keymap" => Some(eval_defvar_keymap(args, env, editor, macros, state)),
        "global-set-key" => Some(eval_set_current_map_key(
            CURRENT_GLOBAL_MAP_SYMBOL,
            args,
            env,
            editor,
            macros,
            state,
        )),
        "local-set-key" => Some(eval_set_current_map_key(
            CURRENT_LOCAL_MAP_SYMBOL,
            args,
            env,
            editor,
            macros,
            state,
        )),
        "use-global-map" => Some(eval_use_current_map(
            CURRENT_GLOBAL_MAP_SYMBOL,
            args,
            env,
            editor,
            macros,
            state,
        )),
        "use-local-map" => Some(eval_use_current_map(
            CURRENT_LOCAL_MAP_SYMBOL,
            args,
            env,
            editor,
            macros,
            state,
        )),
        "current-global-map" => Some(Ok(obj_to_value(current_global_map(state)))),
        "current-local-map" => Some(Ok(obj_to_value(current_local_map(state)))),
        "current-active-maps" => Some(Ok(obj_to_value(current_active_maps(state)))),
        "key-binding" => Some(eval_key_binding(args, env, editor, macros, state)),
        "command-remapping" => Some(eval_command_remapping(args, env, editor, macros, state)),
        "where-is-internal" => Some(eval_where_is_internal(args, env, editor, macros, state)),
        _ => None,
    }
}

fn call_keymap_primitive(name: &str, args: LispObject) -> ElispResult<Value> {
    Ok(obj_to_value(crate::primitives::call_primitive(
        name, &args,
    )?))
}

fn make_sparse_keymap() -> ElispResult<LispObject> {
    Ok(value_to_obj(call_keymap_primitive(
        "make-sparse-keymap",
        LispObject::nil(),
    )?))
}

fn state_map(state: &InterpreterState, name: &str, create: bool) -> LispObject {
    let id = obarray::intern(name);
    if let Some(map) = state.get_value_cell(id) {
        return map;
    }
    if !create {
        return LispObject::nil();
    }
    let map = match make_sparse_keymap() {
        Ok(map) => map,
        Err(_) => LispObject::nil(),
    };
    state.set_value_cell(id, map.clone());
    map
}

fn set_state_map(state: &InterpreterState, name: &str, map: LispObject) {
    state.set_value_cell(obarray::intern(name), map);
}

fn current_global_map(state: &InterpreterState) -> LispObject {
    state_map(state, CURRENT_GLOBAL_MAP_SYMBOL, true)
}

fn current_local_map(state: &InterpreterState) -> LispObject {
    state_map(state, CURRENT_LOCAL_MAP_SYMBOL, false)
}

fn current_active_maps(state: &InterpreterState) -> LispObject {
    let global = current_global_map(state);
    let local = current_local_map(state);
    let mut maps = LispObject::cons(global, LispObject::nil());
    if !local.is_nil() {
        maps = LispObject::cons(local, maps);
    }
    maps
}

fn eval_arg(
    form: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    eval(obj_to_value(form), env, editor, macros, state).map(value_to_obj)
}

fn eval_use_current_map(
    slot_name: &str,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let map_form = args.first().unwrap_or_else(LispObject::nil);
    let map = eval_arg(map_form, env, editor, macros, state)?;
    set_state_map(state, slot_name, map);
    Ok(obj_to_value(LispObject::nil()))
}

fn eval_set_current_map_key(
    slot_name: &str,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let key_form = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let def_form = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let key = eval_arg(key_form, env, editor, macros, state)?;
    let def = eval_arg(def_form, env, editor, macros, state)?;
    let target = state_map(state, slot_name, true);
    let primitive_args = LispObject::cons(
        target,
        LispObject::cons(key, LispObject::cons(def.clone(), LispObject::nil())),
    );
    crate::primitives::call_primitive("define-key", &primitive_args)?;
    Ok(obj_to_value(def))
}

fn lookup_in_map(map: LispObject, key: LispObject) -> ElispResult<LispObject> {
    let args = LispObject::cons(map, LispObject::cons(key, LispObject::nil()));
    crate::primitives::call_primitive("lookup-key", &args)
}

fn eval_key_binding(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let key_form = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let key = eval_arg(key_form, env, editor, macros, state)?;
    let local = current_local_map(state);
    if !local.is_nil() {
        let found = lookup_in_map(local, key.clone())?;
        if !found.is_nil() {
            return Ok(obj_to_value(found));
        }
    }
    Ok(obj_to_value(lookup_in_map(current_global_map(state), key)?))
}

fn remap_key(command: LispObject) -> LispObject {
    LispObject::Vector(std::sync::Arc::new(crate::eval::SyncRefCell::new(vec![
        LispObject::symbol("remap"),
        command,
    ])))
}

fn command_keys_in_map(map: &LispObject, command: &LispObject, out: &mut Vec<LispObject>) {
    let Some((head, tail)) = map.destructure_cons() else {
        return;
    };
    if head.as_symbol().as_deref() != Some("keymap") {
        return;
    }
    let mut cur = tail;
    while let Some((entry, rest)) = cur.destructure_cons() {
        if entry
            .destructure_cons()
            .is_some_and(|(head, _)| head.as_symbol().as_deref() == Some("keymap"))
        {
            command_keys_in_map(&entry, command, out);
        } else if let Some((key, value)) = entry.destructure_cons() {
            if key.as_symbol().as_deref() == Some(":parent") {
                command_keys_in_map(&value, command, out);
            } else if &value == command {
                out.push(key);
            }
        }
        cur = rest;
    }
}

fn command_keys(state: &InterpreterState, command: LispObject) -> Vec<LispObject> {
    let mut out = Vec::new();
    let local = current_local_map(state);
    if !local.is_nil() {
        command_keys_in_map(&local, &command, &mut out);
    }
    command_keys_in_map(&current_global_map(state), &command, &mut out);
    out
}

fn eval_command_remapping(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let command_form = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let command = eval_arg(command_form, env, editor, macros, state)?;
    let key = remap_key(command);
    let local = current_local_map(state);
    if !local.is_nil() {
        let found = lookup_in_map(local, key.clone())?;
        if !found.is_nil() {
            return Ok(obj_to_value(found));
        }
    }
    Ok(obj_to_value(lookup_in_map(current_global_map(state), key)?))
}

fn eval_where_is_internal(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let command_form = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let command = eval_arg(command_form, env, editor, macros, state)?;
    Ok(state.list_from_objects(command_keys(state, command)))
}

pub(crate) fn call_stateful_keymap_primitive(
    name: &str,
    args: &LispObject,
    state: &InterpreterState,
) -> Option<ElispResult<LispObject>> {
    match name {
        "make-sparse-keymap"
        | "make-keymap"
        | "keymapp"
        | "define-key"
        | "keymap-set"
        | "define-keymap"
        | "key-valid-p"
        | "make-composed-keymap"
        | "set-keymap-parent"
        | "keymap-parent"
        | "lookup-key"
        | "keymap-lookup"
        | "copy-keymap" => Some(crate::primitives::call_primitive(name, args)),
        "global-set-key" => Some(set_current_map_key_value(
            CURRENT_GLOBAL_MAP_SYMBOL,
            args,
            state,
        )),
        "local-set-key" => Some(set_current_map_key_value(
            CURRENT_LOCAL_MAP_SYMBOL,
            args,
            state,
        )),
        "use-global-map" => Some(use_current_map_value(
            CURRENT_GLOBAL_MAP_SYMBOL,
            args,
            state,
        )),
        "use-local-map" => Some(use_current_map_value(CURRENT_LOCAL_MAP_SYMBOL, args, state)),
        "current-global-map" => Some(Ok(current_global_map(state))),
        "current-local-map" => Some(Ok(current_local_map(state))),
        "current-active-maps" => Some(Ok(current_active_maps(state))),
        "key-binding" => Some(key_binding_value(args, state)),
        "command-remapping" => Some(command_remapping_value(args, state)),
        "where-is-internal" => Some(where_is_internal_value(args, state)),
        _ => None,
    }
}

fn set_current_map_key_value(
    slot_name: &str,
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let def = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let target = state_map(state, slot_name, true);
    let primitive_args = LispObject::cons(
        target,
        LispObject::cons(key, LispObject::cons(def.clone(), LispObject::nil())),
    );
    crate::primitives::call_primitive("define-key", &primitive_args)?;
    Ok(def)
}

fn use_current_map_value(
    slot_name: &str,
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let map = args.first().unwrap_or_else(LispObject::nil);
    set_state_map(state, slot_name, map);
    Ok(LispObject::nil())
}

fn key_binding_value(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let local = current_local_map(state);
    if !local.is_nil() {
        let found = lookup_in_map(local, key.clone())?;
        if !found.is_nil() {
            return Ok(found);
        }
    }
    lookup_in_map(current_global_map(state), key)
}

fn command_remapping_value(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let command = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let key = remap_key(command);
    let local = current_local_map(state);
    if !local.is_nil() {
        let found = lookup_in_map(local, key.clone())?;
        if !found.is_nil() {
            return Ok(found);
        }
    }
    lookup_in_map(current_global_map(state), key)
}

fn where_is_internal_value(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let command = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(value_to_obj(
        state.list_from_objects(command_keys(state, command)),
    ))
}

fn eval_key_valid_p(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let key = if let Some(arg) = args.first() {
        value_to_obj(eval(obj_to_value(arg), env, editor, macros, state)?)
    } else {
        LispObject::nil()
    };
    Ok(obj_to_value(LispObject::from(!key.is_nil())))
}

fn eval_define_keymap(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let keymap = value_to_obj(call_keymap_primitive(
        "make-sparse-keymap",
        LispObject::nil(),
    )?);
    apply_keymap_pairs(&keymap, args, env, editor, macros, state)?;
    Ok(obj_to_value(keymap))
}

fn eval_defvar_keymap(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = name
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let keymap = value_to_obj(call_keymap_primitive(
        "make-sparse-keymap",
        LispObject::nil(),
    )?);
    let rest = args.rest().unwrap_or_else(LispObject::nil);
    apply_keymap_pairs(&keymap, &rest, env, editor, macros, state)?;
    env.write().define_id(id, keymap.clone());
    state.set_value_cell(id, keymap);
    Ok(obj_to_value(name))
}

fn apply_keymap_pairs(
    keymap: &LispObject,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<()> {
    let mut cur = args.clone();
    while let Some((key_or_kw, rest)) = cur.destructure_cons() {
        if let Some(keyword) = key_or_kw.as_symbol().filter(|s| s.starts_with(':')) {
            let Some((value_form, next)) = rest.destructure_cons() else {
                break;
            };
            if keyword == ":parent" {
                let parent = match eval(obj_to_value(value_form), env, editor, macros, state) {
                    Ok(value) => value_to_obj(value),
                    Err(ElispError::VoidVariable(_)) => LispObject::nil(),
                    Err(err) => return Err(err),
                };
                let primitive_args =
                    LispObject::cons(keymap.clone(), LispObject::cons(parent, LispObject::nil()));
                crate::primitives::call_primitive("set-keymap-parent", &primitive_args)?;
            }
            cur = next;
            continue;
        }

        let Some((def_form, next)) = rest.destructure_cons() else {
            break;
        };
        let def = value_to_obj(eval(obj_to_value(def_form), env, editor, macros, state)?);
        let primitive_args = LispObject::cons(
            keymap.clone(),
            LispObject::cons(key_or_kw, LispObject::cons(def, LispObject::nil())),
        );
        crate::primitives::call_primitive("define-key", &primitive_args)?;
        cur = next;
    }
    Ok(())
}
