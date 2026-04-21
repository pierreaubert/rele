use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use std::sync::Arc;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "make-sparse-keymap" => Some(prim_make_sparse_keymap(args)),
        "make-keymap" => Some(prim_make_keymap(args)),
        "keymapp" => Some(prim_keymapp(args)),
        "define-key" => Some(prim_define_key(args)),
        "type-of" => Some(prim_type_of(args)),
        _ => None,
    }
}

pub fn prim_make_sparse_keymap(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args;
    Ok(LispObject::Vector(Arc::new(parking_lot::Mutex::new(vec![
        LispObject::symbol("keymap"),
        LispObject::Vector(Arc::new(parking_lot::Mutex::new(vec![]))),
        LispObject::Vector(Arc::new(parking_lot::Mutex::new(vec![]))),
        LispObject::cons(LispObject::nil(), LispObject::nil()),
    ]))))
}

pub fn prim_make_keymap(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args;
    prim_make_sparse_keymap(args)
}

pub fn prim_keymapp(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some((car, _)) = obj.destructure_cons() {
        if car.as_symbol().as_deref() == Some("keymap") {
            return Ok(LispObject::t());
        }
    }
    Ok(LispObject::nil())
}

pub fn prim_define_key(args: &LispObject) -> ElispResult<LispObject> {
    let map = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let key = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let def = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let _ = (map, key, def);
    Ok(LispObject::nil())
}

pub fn prim_type_of(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    // Records are vectors with a struct-tag symbol in slot 0.
    // If the tag is a registered class, return that symbol.
    if let LispObject::Vector(v) = &arg {
        let guard = v.lock();
        if let Some(first) = guard.first() {
            if let Some(sym) = first.as_symbol() {
                if crate::primitives_eieio::get_class(&sym).is_some() {
                    return Ok(LispObject::symbol(&sym));
                }
            }
        }
    }
    let type_name = match &arg {
        LispObject::Nil => "symbol",
        LispObject::T => "symbol",
        LispObject::Symbol(_) => "symbol",
        LispObject::Integer(_) => "integer",
        LispObject::Float(_) => "float",
        LispObject::String(_) => "string",
        LispObject::Cons(_) => "cons",
        LispObject::Primitive(_) => "subr",
        LispObject::Vector(_) => "vector",
        LispObject::BytecodeFn(_) => "compiled-function",
        LispObject::HashTable(_) => "hash-table",
    };
    Ok(LispObject::symbol(type_name))
}
