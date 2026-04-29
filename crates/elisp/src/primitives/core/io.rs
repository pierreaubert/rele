use crate::error::{ElispError, ElispResult, SignalData};
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

fn invalid_read_syntax(data: LispObject) -> ElispError {
    ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("invalid-read-syntax"),
        data,
    }))
}

fn invalid_read_syntax_message(message: String) -> ElispError {
    invalid_read_syntax(LispObject::cons(
        LispObject::string(&message),
        LispObject::nil(),
    ))
}

fn invalid_read_syntax_unreadable_object() -> ElispError {
    invalid_read_syntax(LispObject::cons(
        LispObject::string("#<"),
        LispObject::cons(
            LispObject::integer(1),
            LispObject::cons(LispObject::integer(2), LispObject::nil()),
        ),
    ))
}

fn starts_with_unreadable_object(text: &str) -> bool {
    text.trim_start().starts_with("#<")
}

fn prim_read(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().unwrap_or_else(LispObject::nil);
    let (s, buffer_start) = match &arg {
        LispObject::String(s) => (s.clone(), None),
        LispObject::Cons(_) => {
            let Some(id) = crate::primitives::buffer::buffer_object_id(&arg) else {
                return Ok(LispObject::nil());
            };
            let Some((text, point)) = crate::buffer::with_registry(|r| {
                r.get(id)
                    .map(|b| (b.substring(b.point, b.point_max()), b.point))
            }) else {
                return Ok(LispObject::nil());
            };
            (text, Some((id, point)))
        }
        _ => return Ok(LispObject::nil()),
    };
    if starts_with_unreadable_object(&s) {
        return Err(invalid_read_syntax_unreadable_object());
    }
    let mut reader = crate::reader::Reader::new(&s);
    match reader.read() {
        Ok(obj) => {
            if let Some((id, point)) = buffer_start {
                crate::buffer::with_registry_mut(|r| {
                    if let Some(buffer) = r.get_mut(id) {
                        buffer.goto_char(point + reader.position());
                    }
                });
            }
            Ok(obj)
        }
        Err(ElispError::Signal(signal)) => Err(ElispError::Signal(signal)),
        Err(e) => Err(invalid_read_syntax_message(e.to_string())),
    }
}
