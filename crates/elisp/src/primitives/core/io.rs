use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "prin1-to-string" | "pp-to-string" => Some(prim_prin1_to_string(args)),
        "terpri" => Some(prim_terpri(args)),
        "print" => Some(prim_print(args)),
        "pp" => Some(prim_pp(args)),
        "write-char" => Some(prim_write_char(args)),
        "read" => Some(prim_read(args)),
        _ => None,
    }
}

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for &name in IO_PRIMITIVE_NAMES {
        interp.define(name, LispObject::primitive(name));
    }
}

pub const IO_PRIMITIVE_NAMES: &[&str] = &[
    "prin1-to-string",
    "pp-to-string",
    "terpri",
    "print",
    "pp",
    "write-char",
    "read",
];

pub fn prim_prin1_to_string(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::string(&arg.prin1_to_string()))
}

fn prim_terpri(_args: &LispObject) -> ElispResult<LispObject> {
    println!();
    Ok(LispObject::nil())
}

fn prim_print(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().unwrap_or(LispObject::nil());
    println!("{}", arg.prin1_to_string());
    Ok(arg)
}

fn prim_pp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().unwrap_or(LispObject::nil());
    println!("{}", arg.prin1_to_string());
    Ok(arg)
}

fn prim_write_char(args: &LispObject) -> ElispResult<LispObject> {
    Ok(args.first().unwrap_or(LispObject::nil()))
}

fn prim_read(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().unwrap_or_else(LispObject::nil);
    let s = match &arg {
        LispObject::String(s) => s.clone(),
        _ => return Ok(LispObject::nil()),
    };
    crate::reader::read(&s).map_err(|e| {
        ElispError::Signal(Box::new(crate::error::SignalData {
            symbol: LispObject::symbol("invalid-read-syntax"),
            data: LispObject::cons(LispObject::string(&e.to_string()), LispObject::nil()),
        }))
    })
}
