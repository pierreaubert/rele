use crate::error::{ElispError, ElispResult, SignalData};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "error" => Some(prim_error(args)),
        "user-error" => Some(prim_user_error(args)),
        "signal" => Some(prim_signal(args)),
        _ => None,
    }
}

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for &name in &["error", "user-error", "signal"] {
        interp.define(name, LispObject::primitive(name));
    }
}

fn prim_error(args: &LispObject) -> ElispResult<LispObject> {
    let msg = args
        .first()
        .map(|a| match a {
            LispObject::String(s) => s,
            _ => a.princ_to_string(),
        })
        .unwrap_or_default();
    Err(ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("error"),
        data: LispObject::cons(LispObject::string(&msg), LispObject::nil()),
    })))
}

fn prim_user_error(args: &LispObject) -> ElispResult<LispObject> {
    let msg = args
        .first()
        .map(|a| match a {
            LispObject::String(s) => s,
            _ => a.princ_to_string(),
        })
        .unwrap_or_default();
    Err(ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("user-error"),
        data: LispObject::cons(LispObject::string(&msg), LispObject::nil()),
    })))
}

fn prim_signal(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().unwrap_or_else(LispObject::nil);
    let data = args.nth(1).unwrap_or_else(LispObject::nil);
    Err(ElispError::Signal(Box::new(SignalData {
        symbol: sym,
        data,
    })))
}
