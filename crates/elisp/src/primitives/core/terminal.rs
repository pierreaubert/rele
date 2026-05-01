//! Headless terminal-parameter store. Emacs uses these for per-terminal
//! attributes (e.g. background-mode, geometry) that some Lisp code reads
//! at startup. We model them as a single shared alist since the
//! interpreter only ever has one (virtual) terminal.

use std::collections::HashMap;
use std::sync::LazyLock;

use parking_lot::Mutex;

use crate::error::ElispResult;
use crate::object::LispObject;

static PARAMS: LazyLock<Mutex<HashMap<String, LispObject>>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert("background-mode".to_string(), LispObject::symbol("light"));
    map.insert(
        "tty-color-mode".to_string(),
        LispObject::integer(8),
    );
    Mutex::new(map)
});

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in ["terminal-parameter", "set-terminal-parameter", "tty-type"] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        "terminal-parameter" => Ok(get_param(args)),
        "set-terminal-parameter" => Ok(set_param(args)),
        "tty-type" => Ok(LispObject::symbol("ansi")),
        _ => return None,
    };
    Some(result)
}

fn param_key(value: &LispObject) -> Option<String> {
    if let Some(name) = value.as_symbol() {
        return Some(name.to_string());
    }
    value.as_string().map(|s| s.to_string())
}

fn get_param(args: &LispObject) -> LispObject {
    // (terminal-parameter &optional TERMINAL PARAMETER) — first arg is
    // the terminal (we only have one), second is the parameter name.
    let key = args
        .nth(1)
        .or_else(|| args.first())
        .and_then(|a| param_key(&a));
    let Some(key) = key else {
        return LispObject::nil();
    };
    PARAMS
        .lock()
        .get(&key)
        .cloned()
        .unwrap_or_else(LispObject::nil)
}

fn set_param(args: &LispObject) -> LispObject {
    // (set-terminal-parameter TERMINAL PARAMETER VALUE)
    let Some(key) = args.nth(1).and_then(|a| param_key(&a)) else {
        return LispObject::nil();
    };
    let value = args.nth(2).unwrap_or_else(LispObject::nil);
    let prev = PARAMS.lock().insert(key, value);
    prev.unwrap_or_else(LispObject::nil)
}
