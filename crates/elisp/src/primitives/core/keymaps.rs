use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use std::collections::HashMap;
use std::sync::Arc;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "make-sparse-keymap" => Some(prim_make_sparse_keymap(args)),
        "make-keymap" => Some(prim_make_keymap(args)),
        "keymapp" => Some(prim_keymapp(args)),
        "kbd" | "key-parse" => Some(prim_kbd(args)),
        "define-key" | "keymap-set" => Some(prim_define_key(args)),
        "define-key-after" => Some(prim_define_key(args)),
        "substitute-key-definition" => Some(prim_substitute_key_definition(args)),
        "define-keymap" => Some(prim_define_keymap(args)),
        "key-valid-p" => Some(prim_key_valid_p(args)),
        "make-composed-keymap" => Some(prim_make_composed_keymap(args)),
        "set-keymap-parent" => Some(prim_set_keymap_parent(args)),
        "keymap-parent" => Some(prim_keymap_parent(args)),
        "lookup-key" | "keymap-lookup" | "key-binding" => Some(prim_lookup_key(args)),
        "where-is-internal" => Some(prim_where_is_internal(args)),
        "copy-keymap" => Some(prim_copy_keymap(args)),
        "global-unset-key" | "local-unset-key" => Some(prim_unset_key(args)),
        "map-keymap" | "map-keymap-internal" | "keyboard-translate" | "set-quit-char" => {
            Some(Ok(LispObject::nil()))
        }
        "global-set-key" | "local-set-key" => Some(prim_set_current_map_key(name, args)),
        "use-global-map" => Some(prim_use_global_map(args)),
        "use-local-map" => Some(prim_use_local_map(args)),
        "current-global-map" => Some(prim_current_global_map(args)),
        "current-local-map" => Some(prim_current_local_map(args)),
        "current-active-maps" => Some(prim_current_active_maps(args)),
        "type-of" => Some(prim_type_of(args)),
        _ => None,
    }
}

fn parent_marker() -> LispObject {
    LispObject::symbol(":parent")
}

fn prompt_tail(prompt: Option<LispObject>) -> LispObject {
    match prompt {
        Some(prompt) if !prompt.is_nil() => LispObject::cons(prompt, LispObject::nil()),
        _ => LispObject::nil(),
    }
}

fn make_sparse_keymap_with_prompt(prompt: Option<LispObject>) -> LispObject {
    LispObject::cons(LispObject::symbol("keymap"), prompt_tail(prompt))
}

fn make_sparse_keymap() -> LispObject {
    make_sparse_keymap_with_prompt(None)
}

fn make_full_keymap_with_prompt(prompt: Option<LispObject>) -> LispObject {
    let char_table = super::make_char_table(LispObject::symbol("keymap"), LispObject::nil());
    LispObject::cons(
        LispObject::symbol("keymap"),
        LispObject::cons(char_table, prompt_tail(prompt)),
    )
}

fn vector(items: Vec<LispObject>) -> LispObject {
    LispObject::Vector(std::sync::Arc::new(crate::eval::SyncRefCell::new(items)))
}

fn key_equal(a: &LispObject, b: &LispObject) -> bool {
    a == b || {
        let a_events = key_events(a);
        let b_events = key_events(b);
        !a_events.is_empty() && a_events == b_events
    }
}

fn canonical_key_token(token: &str) -> String {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut parts: Vec<&str> = trimmed.split('-').collect();
    let Some(base) = parts.pop() else {
        return trimmed.to_string();
    };
    let mut mods = Vec::new();
    for part in parts {
        let canonical = match part.to_ascii_lowercase().as_str() {
            "control" | "ctrl" | "c" => "C",
            "meta" | "m" => "M",
            "shift" | "s" => "S",
            "super" | "sup" => "s",
            "hyper" | "h" => "H",
            "alt" | "a" => "A",
            _ => part,
        };
        if !mods.contains(&canonical) {
            mods.push(canonical);
        }
    }
    let base = match base.to_ascii_lowercase().as_str() {
        "return" => "RET".to_string(),
        "escape" => "ESC".to_string(),
        "delete" => "DEL".to_string(),
        "backspace" => "BS".to_string(),
        "tab" => "TAB".to_string(),
        "space" | "spc" => "SPC".to_string(),
        other if other.len() == 1 => other.to_string(),
        _ => base.to_string(),
    };
    if mods.is_empty() {
        base
    } else {
        format!("{}-{}", mods.join("-"), base)
    }
}

pub fn canonical_key_string(input: &str) -> String {
    input
        .split_whitespace()
        .map(canonical_key_token)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn key_token_event(token: &str) -> LispObject {
    let mut chars = token.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => LispObject::integer(i64::from(u32::from(ch))),
        _ => LispObject::string(token),
    }
}

fn key_events(key: &LispObject) -> Vec<LispObject> {
    match key {
        LispObject::String(s) => canonical_key_string(s)
            .split_whitespace()
            .map(key_token_event)
            .collect(),
        LispObject::Vector(items) => items.lock().iter().cloned().collect(),
        LispObject::Cons(_) => {
            let mut out = Vec::new();
            let mut cur = key.clone();
            while let Some((item, rest)) = cur.destructure_cons() {
                out.push(item);
                cur = rest;
            }
            if out.is_empty() {
                vec![key.clone()]
            } else {
                out
            }
        }
        _ => vec![key.clone()],
    }
}

fn key_output_object(key: &LispObject) -> LispObject {
    match key {
        LispObject::String(_) => {
            let events = key_events(key);
            if events
                .iter()
                .all(|event| matches!(event, LispObject::Integer(_)))
            {
                vector(events)
            } else {
                canonical_key_object(key)
            }
        }
        _ => key.clone(),
    }
}

pub fn canonical_key_object(key: &LispObject) -> LispObject {
    match key {
        LispObject::String(s) => LispObject::string(&canonical_key_string(s)),
        LispObject::Vector(items) => {
            let guard = items.lock();
            let menu_bar =
                guard.first().and_then(|item| item.as_symbol()).as_deref() == Some("menu-bar");
            let normalized: Vec<LispObject> = guard
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    if menu_bar
                        && index > 0
                        && let Some(name) = item.as_symbol()
                    {
                        return LispObject::symbol(&name.replace(' ', "-").to_lowercase());
                    }
                    canonical_key_object(item)
                })
                .collect();
            vector(normalized)
        }
        _ => key.clone(),
    }
}

fn is_keymap(obj: &LispObject) -> bool {
    obj.destructure_cons()
        .is_some_and(|(head, _)| head.as_symbol().as_deref() == Some("keymap"))
}

fn keymap_parent(map: &LispObject) -> LispObject {
    let mut cur = map.cdr().unwrap_or_else(LispObject::nil);
    while let Some((entry, rest)) = cur.destructure_cons() {
        if let Some((key, value)) = entry.destructure_cons()
            && key == parent_marker()
        {
            return value;
        }
        cur = rest;
    }
    LispObject::nil()
}

fn list_from_objects(items: Vec<LispObject>) -> LispObject {
    items
        .into_iter()
        .rev()
        .fold(LispObject::nil(), |tail, item| LispObject::cons(item, tail))
}

fn menu_item_command(def: &LispObject) -> Option<LispObject> {
    if def.car()?.as_symbol().as_deref() == Some("menu-item") {
        return def.nth(2);
    }
    None
}

fn binding_command(def: &LispObject) -> LispObject {
    menu_item_command(def).unwrap_or_else(|| def.clone())
}

fn binding_keymap(def: &LispObject) -> Option<LispObject> {
    if is_keymap(def) {
        return Some(def.clone());
    }
    def.destructure_cons()
        .and_then(|(_, tail)| is_keymap(&tail).then_some(tail))
}

fn is_remap_key(key: &LispObject) -> bool {
    match key {
        LispObject::Vector(items) => items
            .lock()
            .first()
            .is_some_and(|item| item.as_symbol().as_deref() == Some("remap")),
        _ => false,
    }
}

fn define_key_in_map(map: &LispObject, key: LispObject, def: LispObject) -> ElispResult<()> {
    if !is_keymap(map) {
        return Ok(());
    }

    // Single-event keys (like "a" / [?a] / 97) are stored as the bare
    // event itself so the keymap matches Emacs's `(keymap (97 . foo))`
    // shape. Multi-event sequences fall back to the original object
    // for now (full nested-keymap support is not yet implemented).
    let events = key_events(&key);
    let stored_key = if events.len() == 1 {
        events.into_iter().next().unwrap_or(key.clone())
    } else {
        key.clone()
    };

    let mut cur = map.cdr().unwrap_or_else(LispObject::nil);
    while let Some((entry, rest)) = cur.destructure_cons() {
        if let Some((entry_key, _)) = entry.destructure_cons()
            && entry_key != parent_marker()
            && key_equal(&entry_key, &stored_key)
        {
            entry.set_cdr(def);
            return Ok(());
        }
        cur = rest;
    }

    let old_tail = map.cdr().unwrap_or_else(LispObject::nil);
    map.set_cdr(LispObject::cons(
        LispObject::cons(stored_key, def),
        old_tail,
    ));
    Ok(())
}

/// Look up KEY directly in MAP (not following the parent chain).
/// Returns `Some(value)` when an entry is present (even if its value
/// is nil — that distinguishes an explicit `(define-key map key nil)`
/// shadow from a missing binding) and `None` when no entry exists.
fn lookup_raw_direct_in_map(map: &LispObject, key: &LispObject) -> Option<LispObject> {
    if !is_keymap(map) {
        return None;
    }

    let mut cur = map.cdr().unwrap_or_else(LispObject::nil);
    while let Some((entry, rest)) = cur.destructure_cons() {
        if is_keymap(&entry) {
            let found = lookup_in_map(&entry, key);
            if !found.is_nil() {
                return Some(found);
            }
        } else if let Some((entry_key, value)) = entry.destructure_cons()
            && entry_key != parent_marker()
            && key_equal(&entry_key, key)
        {
            return Some(value);
        }
        cur = rest;
    }
    None
}

fn key_from_events(events: &[LispObject]) -> LispObject {
    match events {
        [single] => single.clone(),
        _ => vector(events.to_vec()),
    }
}

fn lookup_sequence_in_map(map: &LispObject, events: &[LispObject]) -> LispObject {
    if events.is_empty() {
        return LispObject::nil();
    }

    for len in (1..=events.len()).rev() {
        let prefix = key_from_events(&events[..len]);
        let Some(raw) = lookup_raw_direct_in_map(map, &prefix) else {
            continue;
        };
        if len == events.len() {
            return binding_command(&raw);
        }
        if let Some(next_map) = binding_keymap(&raw) {
            let found = lookup_sequence_in_map(&next_map, &events[len..]);
            if !found.is_nil() {
                return found;
            }
        }
    }

    LispObject::nil()
}

fn lookup_in_map(map: &LispObject, key: &LispObject) -> LispObject {
    if !is_keymap(map) {
        return LispObject::nil();
    }

    // An explicit (key . nil) entry SHADOWS the parent — we return nil
    // without falling through. Only a missing entry consults the
    // parent.
    if let Some(raw) = lookup_raw_direct_in_map(map, key) {
        return binding_command(&raw);
    }

    let events = key_events(key);
    if events.len() > 1 {
        let found = lookup_sequence_in_map(map, &events);
        if !found.is_nil() {
            return found;
        }
    }

    let parent = keymap_parent(map);
    if parent.is_nil() {
        LispObject::nil()
    } else {
        lookup_in_map(&parent, key)
    }
}

pub(crate) fn lookup_in_keymaps(map_or_maps: &LispObject, key: &LispObject) -> LispObject {
    if is_keymap(map_or_maps) {
        return lookup_in_map(map_or_maps, key);
    }

    let mut cur = map_or_maps.clone();
    while let Some((map, rest)) = cur.destructure_cons() {
        let found = lookup_in_keymaps(&map, key);
        if !found.is_nil() {
            return found;
        }
        cur = rest;
    }
    LispObject::nil()
}

fn combine_key(prefix: Option<&LispObject>, key: &LispObject) -> LispObject {
    let key = key_output_object(key);
    let Some(prefix) = prefix else {
        // No prefix: a stand-alone key sequence. Bare events (integers
        // or symbols) get wrapped as a 1-element vector so callers like
        // `where-is-internal` see Emacs's standard sequence shape.
        return match key {
            LispObject::String(_) | LispObject::Vector(_) => key,
            _ => vector(vec![key]),
        };
    };

    match (prefix, &key) {
        (LispObject::String(a), LispObject::String(b)) => LispObject::string(&format!("{a} {b}")),
        _ => {
            let mut events = key_events(prefix);
            events.extend(key_events(&key));
            vector(events)
        }
    }
}

fn command_keys_in_map(
    map: &LispObject,
    command: &LispObject,
    prefix: Option<&LispObject>,
    out: &mut Vec<LispObject>,
) {
    if !is_keymap(map) {
        return;
    }

    let mut cur = map.cdr().unwrap_or_else(LispObject::nil);
    while let Some((entry, rest)) = cur.destructure_cons() {
        if is_keymap(&entry) {
            command_keys_in_map(&entry, command, prefix, out);
        } else if let Some((key, value)) = entry.destructure_cons() {
            if key == parent_marker() {
                command_keys_in_map(&value, command, prefix, out);
            } else {
                if let Some(submap) = binding_keymap(&value) {
                    let next_prefix = combine_key(prefix, &key);
                    command_keys_in_map(&submap, command, Some(&next_prefix), out);
                }
                if !is_remap_key(&key) && binding_command(&value) == *command {
                    out.push(combine_key(prefix, &key));
                }
            }
        }
        cur = rest;
    }
}

pub(crate) fn command_keys_in_keymaps(
    map_or_maps: &LispObject,
    command: &LispObject,
) -> Vec<LispObject> {
    let mut out = Vec::new();
    if is_keymap(map_or_maps) {
        command_keys_in_map(map_or_maps, command, None, &mut out);
        return out;
    }

    let mut cur = map_or_maps.clone();
    while let Some((map, rest)) = cur.destructure_cons() {
        command_keys_in_map(&map, command, None, &mut out);
        cur = rest;
    }
    out
}

pub(crate) fn remap_key(command: LispObject) -> LispObject {
    vector(vec![LispObject::symbol("remap"), command])
}

pub(crate) fn command_remapping_in_keymaps(
    map_or_maps: &LispObject,
    command: &LispObject,
) -> LispObject {
    lookup_in_keymaps(map_or_maps, &remap_key(command.clone()))
}

fn substitute_definition_in_map(map: &LispObject, olddef: &LispObject, newdef: &LispObject) {
    if !is_keymap(map) {
        return;
    }

    let mut cur = map.cdr().unwrap_or_else(LispObject::nil);
    while let Some((entry, rest)) = cur.destructure_cons() {
        if is_keymap(&entry) {
            substitute_definition_in_map(&entry, olddef, newdef);
        } else if let Some((key, value)) = entry.destructure_cons() {
            if key == parent_marker() {
                substitute_definition_in_map(&value, olddef, newdef);
            } else if &binding_command(&value) == olddef {
                entry.set_cdr(newdef.clone());
            } else if let Some(submap) = binding_keymap(&value) {
                substitute_definition_in_map(&submap, olddef, newdef);
            }
        }
        cur = rest;
    }
}

fn deep_copy_seen(obj: &LispObject, seen: &mut HashMap<usize, LispObject>) -> LispObject {
    match obj {
        LispObject::Cons(cell) => {
            let ptr = Arc::as_ptr(cell) as usize;
            if let Some(existing) = seen.get(&ptr) {
                return existing.clone();
            }
            let copy = LispObject::cons(LispObject::nil(), LispObject::nil());
            seen.insert(ptr, copy.clone());
            let (car, cdr) = cell.lock().clone();
            copy.set_car(deep_copy_seen(&car, seen));
            copy.set_cdr(deep_copy_seen(&cdr, seen));
            copy
        }
        LispObject::Vector(v) => {
            LispObject::Vector(std::sync::Arc::new(crate::eval::SyncRefCell::new(
                v.lock()
                    .iter()
                    .map(|item| deep_copy_seen(item, seen))
                    .collect(),
            )))
        }
        _ => obj.clone(),
    }
}

fn deep_copy(obj: &LispObject) -> LispObject {
    let mut seen = HashMap::new();
    deep_copy_seen(obj, &mut seen)
}

pub fn prim_make_sparse_keymap(args: &LispObject) -> ElispResult<LispObject> {
    Ok(make_sparse_keymap_with_prompt(args.first()))
}

pub fn prim_make_keymap(args: &LispObject) -> ElispResult<LispObject> {
    Ok(make_full_keymap_with_prompt(args.first()))
}

pub fn prim_keymapp(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().unwrap_or_else(LispObject::nil);
    Ok(LispObject::from(is_keymap(&obj)))
}

pub fn prim_kbd(args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(canonical_key_object(&key))
}

pub fn prim_define_key(args: &LispObject) -> ElispResult<LispObject> {
    let map = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let key = canonical_key_object(&args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?);
    let def = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let remove = args.nth(3).is_some_and(|v| !v.is_nil());
    if remove {
        remove_key_in_map(&map, &key);
    } else {
        define_key_in_map(&map, key, def.clone())?;
    }
    Ok(def)
}

fn remove_key_in_map(map: &LispObject, key: &LispObject) {
    if !is_keymap(map) {
        return;
    }
    let events = key_events(key);
    let canonical = if events.len() == 1 {
        events.into_iter().next().unwrap_or(key.clone())
    } else {
        key.clone()
    };
    let mut prev: Option<LispObject> = None;
    let mut cur = map.cdr().unwrap_or_else(LispObject::nil);
    while let Some((entry, rest)) = cur.destructure_cons() {
        if let Some((entry_key, _)) = entry.destructure_cons()
            && entry_key != parent_marker()
            && key_equal(&entry_key, &canonical)
        {
            match &prev {
                Some(p) => p.set_cdr(rest),
                None => map.set_cdr(rest),
            }
            return;
        }
        prev = Some(cur.clone());
        cur = rest;
    }
}

pub fn prim_substitute_key_definition(args: &LispObject) -> ElispResult<LispObject> {
    let olddef = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let newdef = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let keymap = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    substitute_definition_in_map(&keymap, &olddef, &newdef);
    Ok(LispObject::nil())
}

pub fn prim_define_keymap(args: &LispObject) -> ElispResult<LispObject> {
    let map = make_sparse_keymap();
    let mut cur = args.clone();
    while let Some((key, rest)) = cur.destructure_cons() {
        let Some((def, next)) = rest.destructure_cons() else {
            break;
        };
        define_key_in_map(&map, canonical_key_object(&key), def)?;
        cur = next;
    }
    Ok(map)
}

pub fn prim_key_valid_p(args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().unwrap_or_else(LispObject::nil);
    Ok(LispObject::from(!key.is_nil()))
}

pub fn prim_make_composed_keymap(args: &LispObject) -> ElispResult<LispObject> {
    let maps = args.first().unwrap_or_else(LispObject::nil);
    let parent = args.nth(1).unwrap_or_else(LispObject::nil);
    let result = make_sparse_keymap();

    if !maps.is_nil() {
        if is_keymap(&maps) {
            let old_tail = result.cdr().unwrap_or_else(LispObject::nil);
            result.set_cdr(LispObject::cons(maps, old_tail));
        } else {
            let mut cur = maps;
            while let Some((map, rest)) = cur.destructure_cons() {
                let old_tail = result.cdr().unwrap_or_else(LispObject::nil);
                result.set_cdr(LispObject::cons(map, old_tail));
                cur = rest;
            }
        }
    }

    if !parent.is_nil() {
        set_parent(&result, parent);
    }

    Ok(result)
}

fn set_parent(map: &LispObject, parent: LispObject) {
    let marker = parent_marker();
    let mut cur = map.cdr().unwrap_or_else(LispObject::nil);
    while let Some((entry, rest)) = cur.destructure_cons() {
        if let Some((key, _)) = entry.destructure_cons()
            && key == marker
        {
            entry.set_cdr(parent);
            return;
        }
        cur = rest;
    }
    let old_tail = map.cdr().unwrap_or_else(LispObject::nil);
    map.set_cdr(LispObject::cons(LispObject::cons(marker, parent), old_tail));
}

pub fn prim_set_keymap_parent(args: &LispObject) -> ElispResult<LispObject> {
    let map = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let parent = args.nth(1).unwrap_or_else(LispObject::nil);
    set_parent(&map, parent.clone());
    Ok(parent)
}

pub fn prim_keymap_parent(args: &LispObject) -> ElispResult<LispObject> {
    let map = args.first().unwrap_or_else(LispObject::nil);
    Ok(keymap_parent(&map))
}

pub fn prim_lookup_key(args: &LispObject) -> ElispResult<LispObject> {
    let map = args.first().unwrap_or_else(LispObject::nil);
    let key = canonical_key_object(&args.nth(1).unwrap_or_else(LispObject::nil));
    Ok(lookup_in_keymaps(&map, &key))
}

pub fn prim_where_is_internal(args: &LispObject) -> ElispResult<LispObject> {
    let command = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let map = args.nth(1).unwrap_or_else(LispObject::nil);
    let first_only = args.nth(2).is_some_and(|value| !value.is_nil());
    let keys = command_keys_in_keymaps(&map, &command);
    if first_only {
        Ok(keys.first().cloned().unwrap_or_else(LispObject::nil))
    } else {
        Ok(list_from_objects(keys))
    }
}

pub fn prim_copy_keymap(args: &LispObject) -> ElispResult<LispObject> {
    let map = args.first().unwrap_or_else(LispObject::nil);
    Ok(deep_copy(&map))
}

pub fn prim_unset_key(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args.first().unwrap_or_else(LispObject::nil);
    Ok(LispObject::nil())
}

pub fn prim_set_current_map_key(name: &str, args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let def = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    // Only `(global-set-key KEY 'cmd)` populates the user keybinding
    // table that client key handlers consult — `local-set-key` is
    // per-buffer in Emacs and we don't model that today.
    if name == "global-set-key"
        && let LispObject::String(key_str) = &key
        && let Some(cmd_name) = def.as_symbol()
    {
        crate::primitives_window::record_global_keybinding(key_str.clone(), cmd_name);
    }
    Ok(def)
}

pub fn prim_use_global_map(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args.first().unwrap_or_else(LispObject::nil);
    Ok(LispObject::nil())
}

pub fn prim_use_local_map(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args.first().unwrap_or_else(LispObject::nil);
    Ok(LispObject::nil())
}

pub fn prim_current_global_map(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args;
    Ok(make_sparse_keymap())
}

pub fn prim_current_local_map(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args;
    Ok(LispObject::nil())
}

pub fn prim_current_active_maps(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args;
    Ok(LispObject::cons(make_sparse_keymap(), LispObject::nil()))
}

pub fn prim_type_of(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    // Records are vectors with a struct-tag symbol in slot 0.
    // If the tag is a registered class, return that symbol.
    if let LispObject::Vector(v) = &arg {
        let guard = v.lock();
        if let Some(first) = guard.first()
            && let Some(sym) = first.as_symbol()
            && crate::primitives_eieio::get_class(&sym).is_some()
        {
            return Ok(LispObject::symbol(&sym));
        }
    }
    // Tagged cons cells used as opaque handles. Each tag is the type
    // name `(sqlite . id)`, `(buffer . id)`, etc.
    if let Some((car, _)) = arg.destructure_cons()
        && let Some(tag) = car.as_symbol()
        && matches!(
            tag.as_str(),
            "sqlite" | "sqlite-set" | "buffer" | "marker" | "abbrev-table"
        )
    {
        return Ok(LispObject::symbol(&tag));
    }
    let type_name = match &arg {
        LispObject::Nil => "symbol",
        LispObject::T => "symbol",
        LispObject::Symbol(_) => "symbol",
        LispObject::Integer(_) => "integer",
        LispObject::BigInt(_) => "integer",
        LispObject::Float(_) => "float",
        LispObject::String(_) => "string",
        LispObject::Cons(_) => "cons",
        LispObject::Primitive(_) => "subr",
        LispObject::Vector(_) => "vector",
        LispObject::BytecodeFn(_) => "compiled-function",
        LispObject::HashTable(_) if crate::primitives::core::is_bool_vector(&arg) => "bool-vector",
        LispObject::HashTable(_) => "hash-table",
    };
    Ok(LispObject::symbol(type_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: Vec<LispObject>) -> LispObject {
        items
            .into_iter()
            .rev()
            .fold(LispObject::nil(), |tail, item| LispObject::cons(item, tail))
    }

    #[test]
    fn define_and_lookup_key_roundtrip() {
        let map = make_sparse_keymap();
        let key = LispObject::string("C-x C-s");
        let command = LispObject::symbol("save-buffer");
        prim_define_key(&args(vec![map.clone(), key.clone(), command.clone()])).unwrap();

        assert_eq!(prim_lookup_key(&args(vec![map, key])).unwrap(), command);
    }

    #[test]
    fn parent_lookup_falls_back() {
        let parent = make_sparse_keymap();
        let child = make_sparse_keymap();
        let key = LispObject::string("q");
        let command = LispObject::symbol("quit-window");

        prim_define_key(&args(vec![parent.clone(), key.clone(), command.clone()])).unwrap();
        prim_set_keymap_parent(&args(vec![child.clone(), parent.clone()])).unwrap();

        assert_eq!(
            prim_keymap_parent(&args(vec![child.clone()])).unwrap(),
            parent
        );
        assert_eq!(prim_lookup_key(&args(vec![child, key])).unwrap(), command);
    }

    #[test]
    fn copy_keymap_does_not_share_top_level_bindings() {
        let map = make_sparse_keymap();
        let key = LispObject::string("x");
        prim_define_key(&args(vec![
            map.clone(),
            key.clone(),
            LispObject::symbol("old"),
        ]))
        .unwrap();
        let copy = prim_copy_keymap(&args(vec![map.clone()])).unwrap();
        prim_define_key(&args(vec![
            copy.clone(),
            key.clone(),
            LispObject::symbol("new"),
        ]))
        .unwrap();

        assert_eq!(
            prim_lookup_key(&args(vec![map, key.clone()])).unwrap(),
            LispObject::symbol("old")
        );
        assert_eq!(
            prim_lookup_key(&args(vec![copy, key])).unwrap(),
            LispObject::symbol("new")
        );
    }

    #[test]
    fn lookup_traverses_prefix_maps_and_menu_items() {
        let map = make_sparse_keymap();
        let submap = make_sparse_keymap();
        let command = LispObject::symbol("save-buffer");
        prim_define_key(&args(vec![
            map.clone(),
            LispObject::string("C-x"),
            submap.clone(),
        ]))
        .unwrap();
        prim_define_key(&args(vec![
            submap,
            LispObject::string("C-s"),
            command.clone(),
        ]))
        .unwrap();

        assert_eq!(
            prim_lookup_key(&args(vec![map.clone(), LispObject::string("C-x C-s")])).unwrap(),
            command
        );

        let menu = make_sparse_keymap();
        let menu_command = LispObject::symbol("find-file");
        let menu_item = args(vec![
            LispObject::symbol("menu-item"),
            LispObject::string("Visit New File..."),
            menu_command.clone(),
        ]);
        prim_define_key(&args(vec![
            menu.clone(),
            vector(vec![LispObject::symbol("new-file")]),
            menu_item,
        ]))
        .unwrap();
        prim_define_key(&args(vec![
            map.clone(),
            vector(vec![
                LispObject::symbol("menu-bar"),
                LispObject::symbol("file"),
            ]),
            LispObject::cons(LispObject::string("File"), menu),
        ]))
        .unwrap();

        assert_eq!(
            prim_lookup_key(&args(vec![
                map,
                vector(vec![
                    LispObject::symbol("menu-bar"),
                    LispObject::symbol("file"),
                    LispObject::symbol("new-file"),
                ]),
            ]))
            .unwrap(),
            menu_command
        );
    }

    #[test]
    fn lookup_accepts_list_of_keymaps() {
        let first = make_sparse_keymap();
        let second = make_sparse_keymap();
        prim_define_key(&args(vec![
            first.clone(),
            vector(vec![LispObject::integer(97)]),
            LispObject::symbol("first-command"),
        ]))
        .unwrap();
        prim_define_key(&args(vec![
            second.clone(),
            vector(vec![LispObject::integer(98)]),
            LispObject::symbol("second-command"),
        ]))
        .unwrap();
        let maps = args(vec![first, second]);

        assert_eq!(
            prim_lookup_key(&args(vec![maps, vector(vec![LispObject::integer(98)])])).unwrap(),
            LispObject::symbol("second-command")
        );
    }

    #[test]
    fn where_is_internal_honors_explicit_map_and_first_only() {
        let map = make_sparse_keymap();
        let command = LispObject::symbol("keymap-test-command");
        prim_define_key(&args(vec![
            map.clone(),
            LispObject::string("x"),
            command.clone(),
        ]))
        .unwrap();
        prim_define_key(&args(vec![
            map.clone(),
            LispObject::string("y"),
            command.clone(),
        ]))
        .unwrap();

        assert_eq!(
            prim_where_is_internal(&args(vec![command.clone(), map.clone()])).unwrap(),
            args(vec![
                vector(vec![LispObject::integer(121)]),
                vector(vec![LispObject::integer(120)]),
            ])
        );
        assert_eq!(
            prim_where_is_internal(&args(vec![command, map, LispObject::t()])).unwrap(),
            vector(vec![LispObject::integer(121)])
        );
    }
}
