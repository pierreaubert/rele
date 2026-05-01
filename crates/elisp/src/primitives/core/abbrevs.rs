//! Abbrev table primitives backed by a process-wide registry.
//!
//! In Emacs, abbrev tables are obarrays of symbols where each symbol's
//! value is the expansion. Headless rele-elisp models tables as a
//! `HashMap<name, AbbrevEntry>` plus a per-table property alist.
//! Lisp references tables via `(abbrev-table . id)` cons cells.

use std::collections::HashMap;
use std::sync::LazyLock;

use parking_lot::Mutex;

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

#[derive(Debug, Clone)]
struct AbbrevEntry {
    expansion: LispObject,
    /// Optional hook function. Stored for retrieval via `abbrev-get`,
    /// but not invoked by the headless interpreter (no pre/post-expansion
    /// hook chain).
    #[allow(dead_code)]
    hook: LispObject,
    count: i64,
    system: bool,
    properties: Vec<(String, LispObject)>,
}

impl Default for AbbrevEntry {
    fn default() -> Self {
        Self {
            expansion: LispObject::nil(),
            hook: LispObject::nil(),
            count: 0,
            system: false,
            properties: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Clone)]
struct AbbrevTable {
    entries: HashMap<String, AbbrevEntry>,
    properties: Vec<(String, LispObject)>,
}

#[derive(Debug, Default)]
struct Registry {
    tables: HashMap<usize, AbbrevTable>,
    next_id: usize,
}

static REGISTRY: LazyLock<Mutex<Registry>> =
    LazyLock::new(|| Mutex::new(Registry::default()));

fn make_table_object(id: usize) -> LispObject {
    LispObject::cons(
        LispObject::symbol("abbrev-table"),
        LispObject::integer(id as i64),
    )
}

fn table_id(obj: &LispObject) -> Option<usize> {
    let (car, cdr) = obj.destructure_cons()?;
    if car.as_symbol().as_deref() != Some("abbrev-table") {
        return None;
    }
    cdr.as_integer().map(|n| n as usize)
}

fn make_abbrev_object(table: usize, name: &str) -> LispObject {
    LispObject::cons(
        LispObject::symbol("abbrev"),
        LispObject::cons(
            LispObject::integer(table as i64),
            LispObject::string(name),
        ),
    )
}

fn abbrev_handle(obj: &LispObject) -> Option<(usize, String)> {
    let (car, rest) = obj.destructure_cons()?;
    if car.as_symbol().as_deref() != Some("abbrev") {
        return None;
    }
    let (table_id, name_obj) = rest.destructure_cons()?;
    let id = table_id.as_integer()? as usize;
    let name = name_obj.as_string()?.to_string();
    Some((id, name))
}

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "make-abbrev-table",
        "abbrev-table-p",
        "abbrev-table-empty-p",
        "clear-abbrev-table",
        "copy-abbrev-table",
        "define-abbrev",
        "abbrev-expansion",
        "abbrev-symbol",
        "abbrev-get",
        "abbrev-put",
        "abbrev-insert",
        "abbrev-table-get",
        "abbrev-table-put",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        "make-abbrev-table" => Ok(prim_make_abbrev_table(args)),
        "abbrev-table-p" => Ok(prim_abbrev_table_p(args)),
        "abbrev-table-empty-p" => Ok(prim_abbrev_table_empty_p(args)),
        "clear-abbrev-table" => Ok(prim_clear_abbrev_table(args)),
        "copy-abbrev-table" => Ok(prim_copy_abbrev_table(args)),
        "define-abbrev" => prim_define_abbrev(args),
        "abbrev-expansion" => Ok(prim_abbrev_expansion(args)),
        "abbrev-symbol" => Ok(prim_abbrev_symbol(args)),
        "abbrev-get" => Ok(prim_abbrev_get(args)),
        "abbrev-put" => Ok(prim_abbrev_put(args)),
        "abbrev-insert" => Ok(prim_abbrev_insert(args)),
        "abbrev-table-get" => Ok(prim_abbrev_table_get(args)),
        "abbrev-table-put" => Ok(prim_abbrev_table_put(args)),
        _ => return None,
    };
    Some(result)
}

fn prim_make_abbrev_table(args: &LispObject) -> LispObject {
    let mut reg = REGISTRY.lock();
    let id = reg.next_id;
    reg.next_id += 1;
    let mut table = AbbrevTable::default();
    let mut cur = args.first().unwrap_or_else(LispObject::nil);
    while let Some((key, rest)) = cur.destructure_cons() {
        if let (Some(prop), Some(value)) = (key.as_symbol(), rest.first()) {
            table.properties.push((prop.to_string(), value));
        }
        cur = rest.rest().unwrap_or_else(LispObject::nil);
    }
    reg.tables.insert(id, table);
    make_table_object(id)
}

fn prim_abbrev_table_p(args: &LispObject) -> LispObject {
    let value = args.first().unwrap_or_else(LispObject::nil);
    LispObject::from(table_id(&value).is_some())
}

fn prim_abbrev_table_empty_p(args: &LispObject) -> LispObject {
    let Some(id) = args.first().and_then(|a| table_id(&a)) else {
        return LispObject::t();
    };
    let reg = REGISTRY.lock();
    let empty = reg
        .tables
        .get(&id)
        .map(|t| t.entries.is_empty())
        .unwrap_or(true);
    LispObject::from(empty)
}

fn prim_clear_abbrev_table(args: &LispObject) -> LispObject {
    if let Some(id) = args.first().and_then(|a| table_id(&a)) {
        let mut reg = REGISTRY.lock();
        if let Some(table) = reg.tables.get_mut(&id) {
            table.entries.clear();
        }
    }
    LispObject::nil()
}

fn prim_copy_abbrev_table(args: &LispObject) -> LispObject {
    let Some(src_id) = args.first().and_then(|a| table_id(&a)) else {
        return prim_make_abbrev_table(&LispObject::nil());
    };
    let mut reg = REGISTRY.lock();
    let cloned = reg.tables.get(&src_id).cloned().unwrap_or_default();
    let id = reg.next_id;
    reg.next_id += 1;
    reg.tables.insert(id, cloned);
    make_table_object(id)
}

fn prim_define_abbrev(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name_obj = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let expansion = args.nth(2).unwrap_or_else(LispObject::nil);
    let hook = args.nth(3).unwrap_or_else(LispObject::nil);
    let count = args
        .nth(4)
        .and_then(|v| v.as_integer())
        .unwrap_or(0);
    let system = args.nth(5).is_some_and(|v| !v.is_nil());

    let id = table_id(&table)
        .ok_or_else(|| ElispError::WrongTypeArgument("abbrev-table".into()))?;
    let name = name_obj
        .as_string()
        .ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?
        .to_string();

    let mut reg = REGISTRY.lock();
    let table = reg
        .tables
        .get_mut(&id)
        .ok_or_else(|| ElispError::WrongTypeArgument("abbrev-table".into()))?;
    table.entries.insert(
        name.clone(),
        AbbrevEntry {
            expansion,
            hook,
            count,
            system,
            properties: Vec::new(),
        },
    );
    drop(reg);
    Ok(make_abbrev_object(id, &name))
}

fn prim_abbrev_expansion(args: &LispObject) -> LispObject {
    let Some(name) = args.first().and_then(|a| a.as_string().cloned()) else {
        return LispObject::nil();
    };
    let table_obj = args.nth(1).unwrap_or_else(LispObject::nil);
    let Some(id) = table_id(&table_obj) else {
        return LispObject::nil();
    };
    let reg = REGISTRY.lock();
    reg.tables
        .get(&id)
        .and_then(|t| t.entries.get(&name).map(|e| e.expansion.clone()))
        .unwrap_or_else(LispObject::nil)
}

fn prim_abbrev_symbol(args: &LispObject) -> LispObject {
    let Some(name) = args.first().and_then(|a| a.as_string().cloned()) else {
        return LispObject::nil();
    };
    let table_obj = args.nth(1).unwrap_or_else(LispObject::nil);
    let Some(id) = table_id(&table_obj) else {
        return LispObject::nil();
    };
    let reg = REGISTRY.lock();
    if reg
        .tables
        .get(&id)
        .is_some_and(|t| t.entries.contains_key(&name))
    {
        make_abbrev_object(id, &name)
    } else {
        LispObject::integer(0) // Emacs returns 0 for "no abbrev".
    }
}

fn prim_abbrev_get(args: &LispObject) -> LispObject {
    let Some((id, name)) = args.first().and_then(|a| abbrev_handle(&a)) else {
        return LispObject::nil();
    };
    let Some(prop) = args.nth(1).and_then(|a| a.as_symbol().map(|s| s.to_string())) else {
        return LispObject::nil();
    };
    let reg = REGISTRY.lock();
    let Some(entry) = reg.tables.get(&id).and_then(|t| t.entries.get(&name)) else {
        return LispObject::nil();
    };
    match prop.as_str() {
        "count" => LispObject::integer(entry.count),
        "system" => LispObject::from(entry.system),
        _ => entry
            .properties
            .iter()
            .find(|(k, _)| *k == prop)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(LispObject::nil),
    }
}

fn prim_abbrev_put(args: &LispObject) -> LispObject {
    let Some((id, name)) = args.first().and_then(|a| abbrev_handle(&a)) else {
        return LispObject::nil();
    };
    let Some(prop) = args.nth(1).and_then(|a| a.as_symbol().map(|s| s.to_string())) else {
        return LispObject::nil();
    };
    let value = args.nth(2).unwrap_or_else(LispObject::nil);
    let mut reg = REGISTRY.lock();
    let Some(entry) = reg.tables.get_mut(&id).and_then(|t| t.entries.get_mut(&name)) else {
        return LispObject::nil();
    };
    match prop.as_str() {
        "count" => {
            if let Some(n) = value.as_integer() {
                entry.count = n;
            }
        }
        "system" => entry.system = !value.is_nil(),
        _ => {
            if let Some(slot) = entry.properties.iter_mut().find(|(k, _)| *k == prop) {
                slot.1 = value.clone();
            } else {
                entry.properties.push((prop, value.clone()));
            }
        }
    }
    value
}

fn prim_abbrev_insert(args: &LispObject) -> LispObject {
    // Inserting the expansion at point. Headless can do this.
    let Some((id, name)) = args.first().and_then(|a| abbrev_handle(&a)) else {
        return LispObject::nil();
    };
    let reg = REGISTRY.lock();
    let Some(entry) = reg.tables.get(&id).and_then(|t| t.entries.get(&name)) else {
        return LispObject::nil();
    };
    let Some(text) = entry.expansion.as_string().cloned() else {
        return LispObject::nil();
    };
    drop(reg);
    crate::buffer::with_registry_mut(|r| r.insert_current(&text, false));
    LispObject::nil()
}

fn prim_abbrev_table_get(args: &LispObject) -> LispObject {
    let Some(id) = args.first().and_then(|a| table_id(&a)) else {
        return LispObject::nil();
    };
    let Some(prop) = args.nth(1).and_then(|a| a.as_symbol().map(|s| s.to_string())) else {
        return LispObject::nil();
    };
    let reg = REGISTRY.lock();
    reg.tables
        .get(&id)
        .and_then(|t| t.properties.iter().find(|(k, _)| *k == prop).map(|(_, v)| v.clone()))
        .unwrap_or_else(LispObject::nil)
}

fn prim_abbrev_table_put(args: &LispObject) -> LispObject {
    let Some(id) = args.first().and_then(|a| table_id(&a)) else {
        return LispObject::nil();
    };
    let Some(prop) = args.nth(1).and_then(|a| a.as_symbol().map(|s| s.to_string())) else {
        return LispObject::nil();
    };
    let value = args.nth(2).unwrap_or_else(LispObject::nil);
    let mut reg = REGISTRY.lock();
    if let Some(table) = reg.tables.get_mut(&id) {
        if let Some(slot) = table.properties.iter_mut().find(|(k, _)| *k == prop) {
            slot.1 = value.clone();
        } else {
            table.properties.push((prop, value.clone()));
        }
    }
    value
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
    fn define_and_lookup_round_trip() {
        let table = prim_make_abbrev_table(&LispObject::nil());
        prim_define_abbrev(&args(vec![
            table.clone(),
            LispObject::string("foo"),
            LispObject::string("foobar"),
        ]))
        .expect("define-abbrev ok");
        let exp = prim_abbrev_expansion(&args(vec![LispObject::string("foo"), table.clone()]));
        assert_eq!(exp.as_string().map(String::as_str), Some("foobar"));
        let empty = prim_abbrev_table_empty_p(&args(vec![table.clone()]));
        assert_eq!(empty, LispObject::nil());
        prim_clear_abbrev_table(&args(vec![table.clone()]));
        let empty = prim_abbrev_table_empty_p(&args(vec![table]));
        assert_eq!(empty, LispObject::t());
    }
}
