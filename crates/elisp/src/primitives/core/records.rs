use std::cell::RefCell;
use std::collections::HashSet;

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

thread_local! {
    /// Symbols seen as record tags via `record` / `make-record`. Used by
    /// `type-of` to return the tag (not `vector`) for records. Distinct
    /// from the eieio class registry so creating a raw record doesn't
    /// pollute it.
    static RECORD_TAGS: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

pub fn register_record_tag(tag: &str) {
    RECORD_TAGS.with(|t| {
        t.borrow_mut().insert(tag.to_string());
    });
}

pub fn is_record_tag(tag: &str) -> bool {
    RECORD_TAGS.with(|t| t.borrow().contains(tag))
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "record" => Some(prim_record(args)),
        "recordp" => Some(prim_recordp(args)),
        "record-type" => Some(prim_record_type(args)),
        _ => None,
    }
}

pub fn prim_record(args: &LispObject) -> ElispResult<LispObject> {
    let mut items = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        items.push(arg);
        current = rest;
    }
    if let Some(tag) = items.first().and_then(|t| t.as_symbol()) {
        register_record_tag(&tag);
    }
    Ok(LispObject::Vector(std::sync::Arc::new(
        crate::eval::SyncRefCell::new(items),
    )))
}

pub fn prim_recordp(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(matches!(&obj, LispObject::Vector(v) if
        matches!(v.lock().first(), Some(LispObject::Symbol(_))))))
}

pub fn prim_record_type(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &obj {
        LispObject::Vector(v) => {
            let guard = v.lock();
            if let Some(first) = guard.first() {
                Ok(first.clone())
            } else {
                Ok(LispObject::nil())
            }
        }
        _ => Err(ElispError::WrongTypeArgument("recordp".to_string())),
    }
}
