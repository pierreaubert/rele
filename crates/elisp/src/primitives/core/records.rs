use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

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
