//! Headless face / font / color registry. The interpreter has no
//! display, so we maintain an in-memory map of face name → attribute
//! plist, plus a small palette of named X11 colors. This is enough
//! for tests that round-trip face attributes or look up named colors.

use std::collections::HashMap;
use std::sync::LazyLock;

use parking_lot::Mutex;

use crate::error::ElispResult;
use crate::object::LispObject;

#[derive(Debug, Default)]
struct Registry {
    faces: HashMap<String, Vec<(String, LispObject)>>,
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| {
    let mut r = Registry::default();
    // Predefine `default` so face-list / face-name works out of the box.
    r.faces
        .insert("default".to_string(), Vec::new());
    Mutex::new(r)
});

const NAMED_COLORS: &[(&str, (u16, u16, u16))] = &[
    ("black", (0, 0, 0)),
    ("white", (65535, 65535, 65535)),
    ("red", (65535, 0, 0)),
    ("green", (0, 65535, 0)),
    ("blue", (0, 0, 65535)),
    ("yellow", (65535, 65535, 0)),
    ("cyan", (0, 65535, 65535)),
    ("magenta", (65535, 0, 65535)),
    ("gray", (49344, 49344, 49344)),
    ("grey", (49344, 49344, 49344)),
];

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "make-face",
        "copy-face",
        "face-list",
        "face-name",
        "face-id",
        "face-documentation",
        "face-attribute",
        "face-all-attributes",
        "face-bold-p",
        "face-italic-p",
        "face-font",
        "face-background",
        "face-foreground",
        "face-spec-recalc",
        "set-face-attribute",
        "set-face-background",
        "set-face-foreground",
        "set-face-underline",
        "set-face-strike-through",
        "internal-lisp-face-p",
        "internal-lisp-face-equal-p",
        "internal-make-lisp-face",
        "font-family-list",
        "font-info",
        "font-face-attributes",
        "color-defined-p",
        "color-name-to-rgb",
        "color-values",
        "color-values-from-color-spec",
        "tty-color-alist",
        "tty-color-approximate",
        "tty-color-by-index",
        "tty-color-define",
        "tty-color-standard-values",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        "make-face" | "internal-make-lisp-face" => Ok(make_face(args)),
        "copy-face" => Ok(copy_face(args)),
        "face-list" => Ok(face_list()),
        "face-name" => Ok(args.first().unwrap_or_else(LispObject::nil)),
        "face-id" => Ok(LispObject::integer(0)),
        "face-documentation" => Ok(LispObject::nil()),
        "face-attribute" | "face-all-attributes" => Ok(face_attribute(args)),
        "face-bold-p" | "face-italic-p" => Ok(LispObject::nil()),
        "face-font" => Ok(LispObject::string("monospace")),
        "face-background" | "face-foreground" => Ok(face_color(args)),
        "face-spec-recalc" => Ok(LispObject::nil()),
        "set-face-attribute" => Ok(set_face_attribute(args)),
        "set-face-background" | "set-face-foreground" => Ok(set_face_color(name, args)),
        "set-face-underline" | "set-face-strike-through" => Ok(LispObject::nil()),
        "internal-lisp-face-p" => Ok(internal_lisp_face_p(args)),
        "internal-lisp-face-equal-p" => Ok(LispObject::t()),
        "font-family-list" => Ok(font_family_list()),
        "font-info" => Ok(LispObject::nil()),
        "font-face-attributes" => Ok(LispObject::nil()),
        "color-defined-p" => Ok(color_defined_p(args)),
        "color-name-to-rgb" => Ok(color_name_to_rgb(args)),
        "color-values" => Ok(color_values(args)),
        "color-values-from-color-spec" => {
            return Some(crate::primitives::window::prim_color_values_from_color_spec(args));
        }
        "tty-color-alist" => Ok(tty_color_alist()),
        "tty-color-approximate" | "tty-color-by-index" => {
            Ok(args.first().unwrap_or_else(LispObject::nil))
        }
        "tty-color-define" => Ok(LispObject::nil()),
        "tty-color-standard-values" => Ok(color_values(args)),
        _ => return None,
    };
    Some(result)
}

fn face_name_arg(value: &LispObject) -> Option<String> {
    if let Some(sym) = value.as_symbol() {
        return Some(sym.to_string());
    }
    value.as_string().map(|s| s.to_string())
}

fn make_face(args: &LispObject) -> LispObject {
    let Some(name_obj) = args.first() else {
        return LispObject::nil();
    };
    let Some(name) = face_name_arg(&name_obj) else {
        return LispObject::nil();
    };
    REGISTRY
        .lock()
        .faces
        .entry(name)
        .or_default();
    name_obj
}

fn copy_face(args: &LispObject) -> LispObject {
    let Some(src_name) = args.first().and_then(|a| face_name_arg(&a)) else {
        return LispObject::nil();
    };
    let Some(dst_name) = args.nth(1).and_then(|a| face_name_arg(&a)) else {
        return LispObject::nil();
    };
    let mut reg = REGISTRY.lock();
    let attrs = reg.faces.get(&src_name).cloned().unwrap_or_default();
    reg.faces.insert(dst_name.clone(), attrs);
    LispObject::symbol(&dst_name)
}

fn face_list() -> LispObject {
    let reg = REGISTRY.lock();
    let names: Vec<String> = reg.faces.keys().cloned().collect();
    drop(reg);
    names
        .into_iter()
        .rev()
        .fold(LispObject::nil(), |tail, name| {
            LispObject::cons(LispObject::symbol(&name), tail)
        })
}

fn face_attribute(args: &LispObject) -> LispObject {
    let Some(name) = args.first().and_then(|a| face_name_arg(&a)) else {
        return LispObject::nil();
    };
    let Some(attr) = args.nth(1).and_then(|a| a.as_symbol().map(|s| s.to_string())) else {
        return LispObject::nil();
    };
    let reg = REGISTRY.lock();
    reg.faces
        .get(&name)
        .and_then(|attrs| attrs.iter().find(|(k, _)| *k == attr).map(|(_, v)| v.clone()))
        .unwrap_or_else(|| LispObject::symbol("unspecified"))
}

fn face_color(args: &LispObject) -> LispObject {
    let Some(name) = args.first().and_then(|a| face_name_arg(&a)) else {
        return LispObject::nil();
    };
    let reg = REGISTRY.lock();
    let attrs = match reg.faces.get(&name) {
        Some(a) => a,
        None => return LispObject::nil(),
    };
    for key in [":foreground", ":background"] {
        if let Some((_, value)) = attrs.iter().find(|(k, _)| k == key) {
            return value.clone();
        }
    }
    LispObject::nil()
}

fn set_face_attribute(args: &LispObject) -> LispObject {
    let Some(name) = args.first().and_then(|a| face_name_arg(&a)) else {
        return LispObject::nil();
    };
    // skip FRAME at args.nth(1); pairs start at nth(2).
    let mut idx = 2;
    let mut reg = REGISTRY.lock();
    let attrs = reg.faces.entry(name).or_default();
    while let (Some(key_obj), Some(value)) = (args.nth(idx), args.nth(idx + 1)) {
        let Some(key) = key_obj.as_symbol().map(|s| s.to_string()) else {
            break;
        };
        if let Some(slot) = attrs.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            attrs.push((key, value));
        }
        idx += 2;
    }
    LispObject::nil()
}

fn set_face_color(prim_name: &str, args: &LispObject) -> LispObject {
    let Some(name) = args.first().and_then(|a| face_name_arg(&a)) else {
        return LispObject::nil();
    };
    let value = args.nth(1).unwrap_or_else(LispObject::nil);
    let key = if prim_name == "set-face-background" {
        ":background"
    } else {
        ":foreground"
    }
    .to_string();
    let mut reg = REGISTRY.lock();
    let attrs = reg.faces.entry(name).or_default();
    if let Some(slot) = attrs.iter_mut().find(|(k, _)| *k == key) {
        slot.1 = value.clone();
    } else {
        attrs.push((key, value.clone()));
    }
    value
}

fn internal_lisp_face_p(args: &LispObject) -> LispObject {
    let Some(name) = args.first().and_then(|a| face_name_arg(&a)) else {
        return LispObject::nil();
    };
    LispObject::from(REGISTRY.lock().faces.contains_key(&name))
}

fn font_family_list() -> LispObject {
    LispObject::cons(
        LispObject::string("monospace"),
        LispObject::cons(LispObject::string("sans-serif"), LispObject::nil()),
    )
}

fn lookup_color(name: &str) -> Option<(u16, u16, u16)> {
    NAMED_COLORS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, rgb)| *rgb)
        .or_else(|| parse_hex_color(name))
}

fn parse_hex_color(name: &str) -> Option<(u16, u16, u16)> {
    let stripped = name.strip_prefix('#')?;
    match stripped.len() {
        3 => {
            let r = u16::from_str_radix(&stripped[0..1], 16).ok()? * 0x1111;
            let g = u16::from_str_radix(&stripped[1..2], 16).ok()? * 0x1111;
            let b = u16::from_str_radix(&stripped[2..3], 16).ok()? * 0x1111;
            Some((r, g, b))
        }
        6 => {
            let r = u16::from_str_radix(&stripped[0..2], 16).ok()? << 8;
            let g = u16::from_str_radix(&stripped[2..4], 16).ok()? << 8;
            let b = u16::from_str_radix(&stripped[4..6], 16).ok()? << 8;
            Some((r, g, b))
        }
        12 => {
            let r = u16::from_str_radix(&stripped[0..4], 16).ok()?;
            let g = u16::from_str_radix(&stripped[4..8], 16).ok()?;
            let b = u16::from_str_radix(&stripped[8..12], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}

fn color_defined_p(args: &LispObject) -> LispObject {
    let Some(name) = args.first().and_then(|a| a.as_string().cloned()) else {
        return LispObject::nil();
    };
    LispObject::from(lookup_color(&name).is_some())
}

fn color_name_to_rgb(args: &LispObject) -> LispObject {
    let Some(name) = args.first().and_then(|a| a.as_string().cloned()) else {
        return LispObject::nil();
    };
    let Some((r, g, b)) = lookup_color(&name) else {
        return LispObject::nil();
    };
    let r = f64::from(r) / 65535.0;
    let g = f64::from(g) / 65535.0;
    let b = f64::from(b) / 65535.0;
    LispObject::cons(
        LispObject::Float(r),
        LispObject::cons(
            LispObject::Float(g),
            LispObject::cons(LispObject::Float(b), LispObject::nil()),
        ),
    )
}

fn color_values(args: &LispObject) -> LispObject {
    let Some(name) = args.first().and_then(|a| a.as_string().cloned()) else {
        return LispObject::nil();
    };
    let Some((r, g, b)) = lookup_color(&name) else {
        return LispObject::nil();
    };
    LispObject::cons(
        LispObject::integer(r as i64),
        LispObject::cons(
            LispObject::integer(g as i64),
            LispObject::cons(LispObject::integer(b as i64), LispObject::nil()),
        ),
    )
}

fn tty_color_alist() -> LispObject {
    NAMED_COLORS
        .iter()
        .rev()
        .fold(LispObject::nil(), |tail, (name, (r, g, b))| {
            let entry = LispObject::cons(
                LispObject::string(name),
                LispObject::cons(
                    LispObject::integer(0),
                    LispObject::cons(
                        LispObject::integer(*r as i64),
                        LispObject::cons(
                            LispObject::integer(*g as i64),
                            LispObject::cons(LispObject::integer(*b as i64), LispObject::nil()),
                        ),
                    ),
                ),
            );
            LispObject::cons(entry, tail)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: Vec<LispObject>) -> LispObject {
        items
            .into_iter()
            .rev()
            .fold(LispObject::nil(), |tail, item| LispObject::cons(item, tail))
    }

    #[test]
    fn round_trip_face_attribute() {
        make_face(&args(vec![LispObject::symbol("test-face")]));
        set_face_attribute(&args(vec![
            LispObject::symbol("test-face"),
            LispObject::nil(),
            LispObject::symbol(":foreground"),
            LispObject::string("red"),
        ]));
        let value = face_attribute(&args(vec![
            LispObject::symbol("test-face"),
            LispObject::symbol(":foreground"),
        ]));
        assert_eq!(value.as_string().map(String::as_str), Some("red"));
    }

    #[test]
    fn color_name_lookup_known_and_unknown() {
        let known = color_name_to_rgb(&args(vec![LispObject::string("red")]));
        assert!(matches!(known, LispObject::Cons(_)));
        let unknown = color_name_to_rgb(&args(vec![LispObject::string("frobozz")]));
        assert_eq!(unknown, LispObject::nil());
    }
}
