//! Headless timer primitives. There is no event loop or wall-clock
//! deadline scheduler in the rele-elisp build, so timers exist only
//! as inert handles. `run-at-time` and friends accept the call but
//! do not actually fire later.

use crate::error::ElispResult;
use crate::object::LispObject;

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "run-at-time",
        "run-with-timer",
        "run-with-idle-timer",
        "cancel-timer",
        "cancel-function-timers",
        "timer-activate",
        "timer-activate-when-idle",
        "timer-event-handler",
        "timer-create",
        "timer-set-time",
        "timer-set-function",
        "timer-set-idle-time",
        "timer-inc-time",
        "timerp",
        "current-idle-time",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
    // `timer-list` is a defvar in Emacs (lisp/emacs-lisp/timer.el),
    // not a function. Initialize the value cell so `(let ((timer-list
    // ...)) ...)` and `(symbol-value 'timer-list)` resolve.
    interp.define("timer-list", LispObject::nil());
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        "run-at-time"
        | "run-with-timer"
        | "run-with-idle-timer"
        | "cancel-timer"
        | "cancel-function-timers"
        | "timer-activate"
        | "timer-activate-when-idle"
        | "timer-event-handler"
        | "timer-create"
        | "timer-set-time"
        | "timer-set-function"
        | "timer-set-idle-time"
        | "timer-inc-time"
        | "timerp"
        | "current-idle-time" => {
            let _ = args;
            Ok(LispObject::nil())
        }
        _ => return None,
    };
    Some(result)
}
