//! Headless threads / mutexes / condition variables. The interpreter
//! is single-threaded by design; these primitives exist so Lisp code
//! that loads thread-aware libraries can resolve their names without
//! `void-function`. All return nil.

use crate::error::ElispResult;
use crate::object::LispObject;

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "make-thread",
        "current-thread",
        "thread-name",
        "thread-alive-p",
        "thread-join",
        "thread-signal",
        "thread-yield",
        "thread-last-error",
        "thread-live-p",
        "thread--blocker",
        "all-threads",
        "threadp",
        "make-mutex",
        "mutex-lock",
        "mutex-unlock",
        "mutex-name",
        "mutexp",
        "make-condition-variable",
        "condition-mutex",
        "condition-name",
        "condition-notify",
        "condition-wait",
        "condition-variable-p",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        "make-thread"
        | "current-thread"
        | "thread-name"
        | "thread-alive-p"
        | "thread-join"
        | "thread-signal"
        | "thread-yield"
        | "thread-last-error"
        | "thread-live-p"
        | "thread--blocker"
        | "all-threads"
        | "threadp"
        | "make-mutex"
        | "mutex-lock"
        | "mutex-unlock"
        | "mutex-name"
        | "mutexp"
        | "make-condition-variable"
        | "condition-mutex"
        | "condition-name"
        | "condition-notify"
        | "condition-wait"
        | "condition-variable-p" => {
            let _ = args;
            Ok(LispObject::nil())
        }
        _ => return None,
    };
    Some(result)
}
