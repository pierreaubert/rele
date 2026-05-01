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
        "timer-list",
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
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        "run-at-time"
        | "run-with-timer"
        | "run-with-idle-timer"
        | "cancel-timer"
        | "cancel-function-timers"
        | "timer-list"
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
