//! In-memory selection registry. The headless interpreter has no real
//! GUI clipboard or X11 selection store, so we model PRIMARY /
//! SECONDARY / CLIPBOARD as a process-wide map. This is enough for
//! tests that round-trip a value through `gui-set-selection` /
//! `gui-get-selection`.

use std::collections::HashMap;
use std::sync::LazyLock;

use parking_lot::Mutex;

use crate::error::ElispResult;
use crate::object::LispObject;

static SELECTIONS: LazyLock<Mutex<HashMap<String, LispObject>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "x-set-selection",
        "x-get-selection",
        "x-get-selection-internal",
        "x-own-selection",
        "x-own-selection-internal",
        "x-disown-selection",
        "x-selection-exists-p",
        "x-selection-owner-p",
        "x-selection-value",
        "gui-set-selection",
        "gui-get-selection",
        "gui-selection-exists-p",
        "gui-selection-owner-p",
        "clipboard-yank",
        "clipboard-kill-ring-save",
        "clipboard-kill-region",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        "x-set-selection"
        | "x-own-selection"
        | "x-own-selection-internal"
        | "gui-set-selection" => Ok(set_selection(args)),
        "x-get-selection" | "x-get-selection-internal" | "x-selection-value"
        | "gui-get-selection" => Ok(get_selection(args)),
        "x-disown-selection" => Ok(disown(args)),
        "x-selection-exists-p" | "gui-selection-exists-p" => Ok(exists(args)),
        "x-selection-owner-p" | "gui-selection-owner-p" => Ok(exists(args)),
        // Clipboard helpers — without a real clipboard these mostly
        // mirror their kill-ring counterparts. We keep them as no-ops
        // so callers don't crash.
        "clipboard-yank" | "clipboard-kill-ring-save" | "clipboard-kill-region" => {
            Ok(LispObject::nil())
        }
        _ => return None,
    };
    Some(result)
}

fn type_key(value: &LispObject) -> String {
    if let Some(sym) = value.as_symbol() {
        return sym.to_string();
    }
    "PRIMARY".to_string()
}

fn set_selection(args: &LispObject) -> LispObject {
    let kind = type_key(&args.first().unwrap_or_else(LispObject::nil));
    let value = args.nth(1).unwrap_or_else(LispObject::nil);
    SELECTIONS.lock().insert(kind, value.clone());
    value
}

fn get_selection(args: &LispObject) -> LispObject {
    let kind = type_key(&args.first().unwrap_or_else(LispObject::nil));
    SELECTIONS
        .lock()
        .get(&kind)
        .cloned()
        .unwrap_or_else(LispObject::nil)
}

fn disown(args: &LispObject) -> LispObject {
    let kind = type_key(&args.first().unwrap_or_else(LispObject::nil));
    SELECTIONS.lock().remove(&kind);
    LispObject::nil()
}

fn exists(args: &LispObject) -> LispObject {
    let kind = type_key(&args.first().unwrap_or_else(LispObject::nil));
    LispObject::from(SELECTIONS.lock().contains_key(&kind))
}
