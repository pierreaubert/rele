//! Headless sqlite primitives. Without linking against libsqlite,
//! we can't run real queries, so each primitive returns nil. Tests
//! that only need `sqlitep`, `sqlite-version`, or `sqlite-available-p`
//! to resolve will succeed; tests that actually depend on persistent
//! query results will not.

use crate::error::ElispResult;
use crate::object::LispObject;

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "sqlite-available-p",
        "sqlite-open",
        "sqlite-close",
        "sqlite-execute",
        "sqlite-select",
        "sqlite-transaction",
        "sqlite-commit",
        "sqlite-rollback",
        "sqlitep",
        "sqlite-pragma",
        "sqlite-load-extension",
        "sqlite-version",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let _ = args;
    let result = match name {
        "sqlite-available-p" => Ok(LispObject::nil()),
        "sqlite-version" => Ok(LispObject::string("0.0-headless")),
        "sqlite-open"
        | "sqlite-close"
        | "sqlite-execute"
        | "sqlite-select"
        | "sqlite-transaction"
        | "sqlite-commit"
        | "sqlite-rollback"
        | "sqlitep"
        | "sqlite-pragma"
        | "sqlite-load-extension" => Ok(LispObject::nil()),
        _ => return None,
    };
    Some(result)
}
