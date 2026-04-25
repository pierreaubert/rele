//! P6 tests for keyword slot accessor dispatch in EIEIO.

use rele_elisp::primitives_eieio::{
    prim_make_instance, register_class, try_keyword_slot_call, Class, Slot,
};
use rele_elisp::LispObject;

fn symbol(s: &str) -> LispObject {
    LispObject::symbol(s)
}

fn list(items: &[LispObject]) -> LispObject {
    let mut out = LispObject::nil();
    for item in items.iter().rev() {
        out = LispObject::cons(item.clone(), out.clone());
    }
    out
}

/// Keyword slot accessor by initarg.
#[test]
fn keyword_slot_accessor_by_initarg() {
    register_class(Class {
        name: "p6-test-printer-class".into(),
        parent: None,
        slots: vec![Slot {
            name: "printer".into(),
            initarg: Some("printer".into()),
            default: LispObject::nil(),
        }],
    });

    let args = list(&[
        symbol("p6-test-printer-class"),
        symbol(":printer"),
        LispObject::string("my-printer"),
    ]);
    let instance = prim_make_instance(&args).expect("make-instance ok");

    let kw_call = LispObject::cons(instance, LispObject::nil());
    let result = try_keyword_slot_call(":printer", &kw_call)
        .expect("keyword slot lookup should return Some")
        .expect("keyword slot lookup should succeed");

    match result.as_string() {
        Some(s) => assert_eq!(s, "my-printer"),
        _ => panic!("expected string result"),
    }
}

/// Keyword slot accessor by slot name.
#[test]
fn keyword_slot_accessor_by_slot_name() {
    register_class(Class {
        name: "p6-test-name-class".into(),
        parent: None,
        slots: vec![Slot {
            name: "my-name".into(),
            initarg: None,
            default: LispObject::string("default-name"),
        }],
    });

    let args = list(&[symbol("p6-test-name-class")]);
    let instance = prim_make_instance(&args).expect("make-instance ok");

    let kw_call = LispObject::cons(instance, LispObject::nil());
    let result = try_keyword_slot_call(":my-name", &kw_call)
        .expect("keyword slot lookup should return Some")
        .expect("keyword slot lookup should succeed");

    match result.as_string() {
        Some(s) => assert_eq!(s, "default-name"),
        _ => panic!("expected string result"),
    }
}

/// Keyword slot accessor returns None when keyword doesn't match.
#[test]
fn keyword_slot_accessor_no_match() {
    register_class(Class {
        name: "p6-test-nomatch".into(),
        parent: None,
        slots: vec![Slot {
            name: "x".into(),
            initarg: None,
            default: LispObject::integer(0),
        }],
    });

    let args = list(&[symbol("p6-test-nomatch")]);
    let instance = prim_make_instance(&args).expect("make-instance ok");

    let kw_call = LispObject::cons(instance, LispObject::nil());
    let result = try_keyword_slot_call(":unknown", &kw_call);

    assert!(result.is_none());
}

/// Non-keyword symbols don't trigger keyword slot accessor.
#[test]
fn keyword_slot_accessor_non_keyword() {
    register_class(Class {
        name: "p6-test-nonkw".into(),
        parent: None,
        slots: vec![],
    });

    let args = list(&[symbol("p6-test-nonkw")]);
    let instance = prim_make_instance(&args).expect("make-instance ok");

    let kw_call = LispObject::cons(instance, LispObject::nil());
    let result = try_keyword_slot_call("regular-symbol", &kw_call);

    assert!(result.is_none());
}

/// Keyword slot accessor returns None when no instance is provided.
#[test]
fn keyword_slot_accessor_no_instance() {
    let kw_call = LispObject::nil();
    let result = try_keyword_slot_call(":printer", &kw_call);

    assert!(result.is_none());
}
