use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "make-sparse-keymap" => Some(prim_make_sparse_keymap(args)),
        "make-keymap" => Some(prim_make_keymap(args)),
        "keymapp" => Some(prim_keymapp(args)),
        "define-key" | "keymap-set" => Some(prim_define_key(args)),
        "define-keymap" => Some(prim_define_keymap(args)),
        "key-valid-p" => Some(prim_key_valid_p(args)),
        "make-composed-keymap" => Some(prim_make_composed_keymap(args)),
        "set-keymap-parent" => Some(prim_set_keymap_parent(args)),
        "keymap-parent" => Some(prim_keymap_parent(args)),
        "lookup-key" | "keymap-lookup" | "key-binding" => Some(prim_lookup_key(args)),
        "copy-keymap" => Some(prim_copy_keymap(args)),
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

fn make_sparse_keymap() -> LispObject {
    LispObject::cons(LispObject::symbol("keymap"), LispObject::nil())
}

fn make_full_keymap() -> LispObject {
    let char_table = super::make_char_table(LispObject::symbol("keymap"), LispObject::nil());
    LispObject::cons(
        LispObject::symbol("keymap"),
        LispObject::cons(char_table, LispObject::nil()),
    )
}

fn key_equal(a: &LispObject, b: &LispObject) -> bool {
    a == b
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

pub fn canonical_key_object(key: &LispObject) -> LispObject {
    match key {
        LispObject::String(s) => LispObject::string(&canonical_key_string(s)),
        LispObject::Vector(items) => {
            let normalized: Vec<LispObject> =
                items.lock().iter().map(canonical_key_object).collect();
            LispObject::Vector(std::sync::Arc::new(crate::eval::SyncRefCell::new(
                normalized,
            )))
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

fn define_key_in_map(map: &LispObject, key: LispObject, def: LispObject) -> ElispResult<()> {
    if !is_keymap(map) {
        return Ok(());
    }

    let mut cur = map.cdr().unwrap_or_else(LispObject::nil);
    while let Some((entry, rest)) = cur.destructure_cons() {
        if let Some((entry_key, _)) = entry.destructure_cons()
            && entry_key != parent_marker()
            && key_equal(&entry_key, &key)
        {
            entry.set_cdr(def);
            return Ok(());
        }
        cur = rest;
    }

    let old_tail = map.cdr().unwrap_or_else(LispObject::nil);
    map.set_cdr(LispObject::cons(LispObject::cons(key, def), old_tail));
    Ok(())
}

fn lookup_in_map(map: &LispObject, key: &LispObject) -> LispObject {
    if !is_keymap(map) {
        return LispObject::nil();
    }

    let mut cur = map.cdr().unwrap_or_else(LispObject::nil);
    while let Some((entry, rest)) = cur.destructure_cons() {
        if is_keymap(&entry) {
            let found = lookup_in_map(&entry, key);
            if !found.is_nil() {
                return found;
            }
        } else if let Some((entry_key, value)) = entry.destructure_cons()
            && entry_key != parent_marker()
            && key_equal(&entry_key, key)
        {
            return value;
        }
        cur = rest;
    }

    let parent = keymap_parent(map);
    if parent.is_nil() {
        LispObject::nil()
    } else {
        lookup_in_map(&parent, key)
    }
}

fn deep_copy(obj: &LispObject) -> LispObject {
    match obj {
        LispObject::Cons(_) => {
            let car = obj.car().unwrap_or_else(LispObject::nil);
            let cdr = obj.cdr().unwrap_or_else(LispObject::nil);
            LispObject::cons(deep_copy(&car), deep_copy(&cdr))
        }
        LispObject::Vector(v) => LispObject::Vector(std::sync::Arc::new(
            crate::eval::SyncRefCell::new(v.lock().iter().map(deep_copy).collect()),
        )),
        _ => obj.clone(),
    }
}

pub fn prim_make_sparse_keymap(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args;
    Ok(make_sparse_keymap())
}

pub fn prim_make_keymap(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args;
    Ok(make_full_keymap())
}

pub fn prim_keymapp(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().unwrap_or_else(LispObject::nil);
    Ok(LispObject::from(is_keymap(&obj)))
}

pub fn prim_define_key(args: &LispObject) -> ElispResult<LispObject> {
    let map = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let key = canonical_key_object(&args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?);
    let def = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    define_key_in_map(&map, key, def.clone())?;
    Ok(def)
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
    Ok(lookup_in_map(&map, &key))
}

pub fn prim_copy_keymap(args: &LispObject) -> ElispResult<LispObject> {
    let map = args.first().unwrap_or_else(LispObject::nil);
    Ok(deep_copy(&map))
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
}
