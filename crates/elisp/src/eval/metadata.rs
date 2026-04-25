use super::{Environment, InterpreterState, MacroTable, RwLock, eval};
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::sync::Arc;

fn list_obj(items: impl IntoIterator<Item = LispObject>) -> LispObject {
    let mut out = LispObject::nil();
    let mut values: Vec<_> = items.into_iter().collect();
    while let Some(item) = values.pop() {
        out = LispObject::cons(item, out);
    }
    out
}

fn symbol_name(obj: &LispObject) -> Option<String> {
    obj.as_symbol()
        .or_else(|| obj.as_string().map(ToString::to_string))
}

fn eval_args(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Vec<LispObject>> {
    let mut out = Vec::new();
    let mut cur = args.clone();
    while let Some((arg, rest)) = cur.destructure_cons() {
        out.push(value_to_obj(eval(
            obj_to_value(arg),
            env,
            editor,
            macros,
            state,
        )?));
        cur = rest;
    }
    Ok(out)
}

fn builtin_coding_system(name: &str) -> bool {
    matches!(
        name,
        "utf-8"
            | "utf-8-unix"
            | "utf-8-dos"
            | "utf-8-mac"
            | "undecided"
            | "raw-text"
            | "no-conversion"
            | "emacs-mule"
    )
}

fn resolve_coding_name(state: &InterpreterState, obj: &LispObject) -> Option<String> {
    let name = symbol_name(obj)?;
    let aliases = state.coding_aliases.read();
    Some(aliases.get(&name).cloned().unwrap_or(name))
}

fn coding_known(state: &InterpreterState, obj: &LispObject) -> bool {
    let Some(name) = resolve_coding_name(state, obj) else {
        return false;
    };
    builtin_coding_system(&name) || state.coding_systems.read().contains_key(&name)
}

fn coding_system_list(state: &InterpreterState) -> Vec<LispObject> {
    let mut names = vec![
        "utf-8".to_string(),
        "utf-8-unix".to_string(),
        "undecided".to_string(),
        "raw-text".to_string(),
        "no-conversion".to_string(),
    ];
    for name in state.coding_systems.read().keys() {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    names.sort();
    names
        .into_iter()
        .map(|name| LispObject::symbol(&name))
        .collect()
}

fn coding_plist(state: &InterpreterState, obj: &LispObject) -> LispObject {
    let Some(name) = resolve_coding_name(state, obj) else {
        return LispObject::nil();
    };
    state
        .coding_systems
        .read()
        .get(&name)
        .cloned()
        .unwrap_or_else(LispObject::nil)
}

fn plist_get(plist: LispObject, prop: LispObject) -> LispObject {
    let mut cur = plist;
    while let Some((key, rest)) = cur.destructure_cons() {
        if let Some((value, next)) = rest.destructure_cons() {
            if key == prop {
                return value;
            }
            cur = next;
        } else {
            break;
        }
    }
    LispObject::nil()
}

fn plist_put(plist: LispObject, prop: LispObject, value: LispObject) -> LispObject {
    let mut items = Vec::new();
    let mut replaced = false;
    let mut cur = plist;
    while let Some((key, rest)) = cur.destructure_cons() {
        if let Some((old_value, next)) = rest.destructure_cons() {
            items.push(key.clone());
            if key == prop {
                items.push(value.clone());
                replaced = true;
            } else {
                items.push(old_value);
            }
            cur = next;
        } else {
            break;
        }
    }
    if !replaced {
        items.push(prop);
        items.push(value);
    }
    list_obj(items)
}

fn define_coding_system(state: &InterpreterState, args: &[LispObject]) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(name_str) = symbol_name(name) else {
        return Err(ElispError::WrongTypeArgument("symbol".to_string()));
    };
    let plist = list_obj(args.iter().skip(1).cloned());
    state.coding_systems.write().insert(name_str, plist);
    Ok(name.clone())
}

fn define_coding_alias(state: &InterpreterState, args: &[LispObject]) -> ElispResult<LispObject> {
    let alias = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let target = args.get(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(alias_name) = symbol_name(alias) else {
        return Err(ElispError::WrongTypeArgument("symbol".to_string()));
    };
    let Some(target_name) = symbol_name(target) else {
        return Err(ElispError::WrongTypeArgument("symbol".to_string()));
    };
    state.coding_aliases.write().insert(alias_name, target_name);
    Ok(alias.clone())
}

fn coding_put(state: &InterpreterState, args: &[LispObject]) -> ElispResult<LispObject> {
    let coding = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.get(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.get(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(name) = resolve_coding_name(state, coding) else {
        return Ok(LispObject::nil());
    };
    let old = state
        .coding_systems
        .read()
        .get(&name)
        .cloned()
        .unwrap_or_else(LispObject::nil);
    let updated = plist_put(old, prop.clone(), value.clone());
    state.coding_systems.write().insert(name, updated);
    Ok(value.clone())
}

fn coding_get(state: &InterpreterState, args: &[LispObject]) -> ElispResult<LispObject> {
    let coding = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.get(1).ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(plist_get(coding_plist(state, coding), prop.clone()))
}

fn set_coding_priority(state: &InterpreterState, args: &[LispObject]) -> LispObject {
    let names: Vec<String> = args.iter().filter_map(|arg| symbol_name(arg)).collect();
    if !names.is_empty() {
        *state.coding_priority.write() = names;
    }
    LispObject::nil()
}

fn coding_priority_list(state: &InterpreterState) -> LispObject {
    let names = state.coding_priority.read();
    let items: Vec<LispObject> = if names.is_empty() {
        vec![LispObject::symbol("utf-8")]
    } else {
        names.iter().map(|name| LispObject::symbol(name)).collect()
    };
    list_obj(items)
}

fn define_translation_table(
    state: &InterpreterState,
    args: &[LispObject],
) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let table = args.get(1).cloned().unwrap_or_else(LispObject::nil);
    let Some(name_str) = symbol_name(name) else {
        return Err(ElispError::WrongTypeArgument("symbol".to_string()));
    };
    state
        .translation_tables
        .write()
        .insert(name_str.clone(), table.clone());
    state.set_value_cell(crate::obarray::intern(&name_str), table);
    Ok(name.clone())
}

fn make_translation_table_from_alist(args: &[LispObject]) -> LispObject {
    let alist = args.first().cloned().unwrap_or_else(LispObject::nil);
    LispObject::cons(LispObject::symbol("translation-table"), alist)
}

fn translation_table_id(state: &InterpreterState, args: &[LispObject]) -> LispObject {
    let Some(name) = args.first().and_then(symbol_name) else {
        return LispObject::nil();
    };
    if state.translation_tables.read().contains_key(&name) {
        LispObject::symbol(&name)
    } else {
        LispObject::nil()
    }
}

fn custom_store(
    state: &InterpreterState,
    args: &[LispObject],
    kind: &str,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(id) = sym.as_symbol_id() else {
        return Err(ElispError::WrongTypeArgument("symbol".to_string()));
    };
    let payload = LispObject::cons(
        LispObject::symbol(kind),
        list_obj(args.iter().skip(1).cloned()),
    );
    state.custom_metadata.write().insert(id, payload.clone());
    state.put_plist(id, crate::obarray::intern("custom-metadata"), payload);
    Ok(sym.clone())
}

fn custom_add_metadata(
    state: &InterpreterState,
    args: &[LispObject],
    prop: &str,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(id) = sym.as_symbol_id() else {
        return Err(ElispError::WrongTypeArgument("symbol".to_string()));
    };
    let value = args.get(1).cloned().unwrap_or_else(LispObject::nil);
    state.put_plist(id, crate::obarray::intern(prop), value);
    Ok(sym.clone())
}

fn advice_add(state: &InterpreterState, args: &[LispObject]) -> ElispResult<LispObject> {
    let target = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(id) = target.as_symbol_id() else {
        return Ok(LispObject::nil());
    };
    let advice = args.get(2).cloned().unwrap_or_else(LispObject::nil);
    state
        .advice_metadata
        .write()
        .entry(id)
        .or_default()
        .push(advice);
    Ok(LispObject::nil())
}

fn advice_remove(state: &InterpreterState, args: &[LispObject]) -> ElispResult<LispObject> {
    let target = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(id) = target.as_symbol_id() else {
        return Ok(LispObject::nil());
    };
    let advice = args.get(1).cloned().unwrap_or_else(LispObject::nil);
    if let Some(list) = state.advice_metadata.write().get_mut(&id) {
        list.retain(|item| *item != advice);
    }
    Ok(LispObject::nil())
}

fn advice_member(state: &InterpreterState, args: &[LispObject]) -> ElispResult<LispObject> {
    let advice = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let target = args.get(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(id) = target.as_symbol_id() else {
        return Ok(LispObject::nil());
    };
    let found = state
        .advice_metadata
        .read()
        .get(&id)
        .is_some_and(|items| items.iter().any(|item| item == advice));
    Ok(LispObject::from(found))
}

fn advice_p(state: &InterpreterState, args: &[LispObject]) -> LispObject {
    let Some(target) = args.first().and_then(LispObject::as_symbol_id) else {
        return LispObject::nil();
    };
    LispObject::from(
        state
            .advice_metadata
            .read()
            .get(&target)
            .is_some_and(|items| !items.is_empty()),
    )
}

fn advice_mapc(state: &InterpreterState, args: &[LispObject]) -> ElispResult<LispObject> {
    let func = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let target = args.get(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(id) = target.as_symbol_id() else {
        return Ok(LispObject::nil());
    };
    let Some(items) = state.advice_metadata.read().get(&id).cloned() else {
        return Ok(LispObject::nil());
    };
    // Metadata gate only: full callback invocation belongs with full advice dispatch.
    let _ = func;
    Ok(list_obj(items))
}

pub(super) fn call_metadata_primitive(
    name: &str,
    args: &LispObject,
    state: &InterpreterState,
) -> Option<ElispResult<LispObject>> {
    let mut values = Vec::new();
    let mut cur = args.clone();
    while let Some((arg, rest)) = cur.destructure_cons() {
        values.push(arg);
        cur = rest;
    }
    Some(match name {
        "kbd" | "key-parse" => Ok(values
            .first()
            .map(crate::primitives::core::keymaps::canonical_key_object)
            .unwrap_or_else(|| LispObject::string(""))),
        "define-coding-system" | "define-coding-system-internal" => {
            define_coding_system(state, &values)
        }
        "define-coding-system-alias" => define_coding_alias(state, &values),
        "coding-system-p" => Ok(LispObject::from(
            values.first().is_some_and(|obj| coding_known(state, obj)),
        )),
        "check-coding-system" => Ok(values.first().cloned().unwrap_or_else(LispObject::nil)),
        "coding-system-list" => Ok(list_obj(coding_system_list(state))),
        "coding-system-plist" => Ok(values
            .first()
            .map(|obj| coding_plist(state, obj))
            .unwrap_or_else(LispObject::nil)),
        "coding-system-put" => coding_put(state, &values),
        "coding-system-get" => coding_get(state, &values),
        "coding-system-base" | "coding-system-change-eol-conversion" => Ok(values
            .first()
            .and_then(|obj| resolve_coding_name(state, obj))
            .map(|name| LispObject::symbol(&name))
            .unwrap_or_else(LispObject::nil)),
        "coding-system-priority-list" => Ok(coding_priority_list(state)),
        "set-coding-system-priority" | "prefer-coding-system" => {
            Ok(set_coding_priority(state, &values))
        }
        "find-coding-systems-string"
        | "find-coding-systems-region"
        | "find-coding-systems-for-charsets" => Ok(list_obj([LispObject::symbol("utf-8")])),
        "define-translation-table" => define_translation_table(state, &values),
        "make-translation-table-from-alist" => Ok(make_translation_table_from_alist(&values)),
        "translation-table-id" => Ok(translation_table_id(state, &values)),
        "custom-declare-variable" => custom_store(state, &values, "variable"),
        "custom-declare-face" => custom_store(state, &values, "face"),
        "custom-declare-group" => custom_store(state, &values, "group"),
        "custom-add-option" => custom_add_metadata(state, &values, "custom-options"),
        "custom-add-version" => custom_add_metadata(state, &values, "custom-version"),
        "advice-add" => advice_add(state, &values),
        "advice-remove" => advice_remove(state, &values),
        "advice-member-p" | "advice-function-member-p" => advice_member(state, &values),
        "advice--p" | "advice-p" | "ad-is-advised" => Ok(advice_p(state, &values)),
        "advice-function-mapc" => advice_mapc(state, &values),
        _ => return None,
    })
}

pub(super) fn eval_metadata_form(
    name: &str,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> Option<ElispResult<Value>> {
    let evaled = match name {
        "kbd"
        | "key-parse"
        | "define-coding-system"
        | "define-coding-system-internal"
        | "define-coding-system-alias"
        | "coding-system-p"
        | "check-coding-system"
        | "coding-system-list"
        | "coding-system-plist"
        | "coding-system-put"
        | "coding-system-get"
        | "coding-system-base"
        | "coding-system-change-eol-conversion"
        | "coding-system-priority-list"
        | "set-coding-system-priority"
        | "prefer-coding-system"
        | "find-coding-systems-string"
        | "find-coding-systems-region"
        | "find-coding-systems-for-charsets"
        | "define-translation-table"
        | "make-translation-table-from-alist"
        | "translation-table-id"
        | "custom-declare-variable"
        | "custom-declare-face"
        | "custom-declare-group"
        | "custom-add-option"
        | "custom-add-version"
        | "advice-add"
        | "advice-remove"
        | "advice-member-p"
        | "advice-function-member-p"
        | "advice--p"
        | "advice-p"
        | "ad-is-advised"
        | "advice-function-mapc" => eval_args(args, env, editor, macros, state),
        _ => return None,
    };
    Some(evaled.and_then(|values| {
        let args = list_obj(values);
        call_metadata_primitive(name, &args, state)
            .unwrap_or_else(|| Err(ElispError::VoidFunction(name.to_string())))
            .map(obj_to_value)
    }))
}
