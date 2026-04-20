//! R4: EIEIO keyword dispatch wiring + `eql` predicate tests.
//!
//! Verifies that:
//! 1. `(:slot instance)` syntax reaches `try_keyword_slot_call` through the
//!    normal function-call dispatch path (the wiring in `functions.rs`).
//! 2. `(eql A B)` implements Emacs/CL structural-equality semantics.

use rele_elisp::add_primitives;
use rele_elisp::eval::Interpreter;
use rele_elisp::LispObject;

// ---- helper ----------------------------------------------------------

fn make_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
}

fn eval(src: &str) -> LispObject {
    let interp = make_interp();
    interp
        .eval_source(src)
        .unwrap_or_else(|(_i, e)| panic!("eval error in {:?}: {:?}", src, e))
}

fn eval_err(src: &str) -> String {
    let interp = make_interp();
    match interp.eval_source(src) {
        Ok(v) => panic!("expected error, got {:?}", v),
        Err((_i, e)) => format!("{:?}", e),
    }
}

// ---- keyword slot accessor wiring ------------------------------------

/// `(:name instance)` must return the value of the `name` slot via
/// the keyword-dispatch hook wired in `functions.rs`.
#[test]
fn keyword_call_returns_slot_value() {
    let result = eval(
        r#"
        (defclass r4-animal ()
          ((name :initarg :name :initform "unknown")))
        (let ((obj (make-instance 'r4-animal :name "Cat")))
          (:name obj))
        "#,
    );
    assert_eq!(result.as_string().map(|s| s.as_str()), Some("Cat"), "expected slot 'name' to be 'Cat'");
}

/// When multiple slots exist, only the matching keyword is returned.
#[test]
fn keyword_call_matches_correct_slot() {
    let result = eval(
        r#"
        (defclass r4-point ()
          ((x :initarg :x :initform 0)
           (y :initarg :y :initform 0)))
        (let ((pt (make-instance 'r4-point :x 3 :y 7)))
          (:y pt))
        "#,
    );
    assert_eq!(result.as_integer(), Some(7));
}

/// The keyword accessor falls back to the slot's `:initform` when the
/// initarg was not passed to `make-instance`.
#[test]
fn keyword_call_default_initform() {
    let result = eval(
        r#"
        (defclass r4-defaulted ()
          ((speed :initarg :speed :initform 42)))
        (let ((obj (make-instance 'r4-defaulted)))
          (:speed obj))
        "#,
    );
    assert_eq!(result.as_integer(), Some(42));
}

/// A keyword that doesn't match any slot should still signal void-function
/// (the keyword was never bound as a function and has no slot match).
#[test]
fn keyword_call_unknown_slot_is_error() {
    let err = eval_err(
        r#"
        (defclass r4-nofield ()
          ((present :initarg :present :initform 0)))
        (let ((obj (make-instance 'r4-nofield)))
          (:absent obj))
        "#,
    );
    // The error must be VoidFunction or similar; not a successful value.
    assert!(
        err.contains("VoidFunction") || err.contains("void"),
        "expected VoidFunction error, got: {err}"
    );
}

// ---- eql predicate ---------------------------------------------------

/// `(eql x x)` — same fixnum is always `t`.
#[test]
fn eql_same_integer() {
    let result = eval("(eql 1 1)");
    assert_eq!(result, LispObject::t());
}

/// `(eql 1.0 1.0)` — same float value is `t` (eql extends eq for floats).
#[test]
fn eql_same_float() {
    let result = eval("(eql 1.0 1.0)");
    assert_eq!(result, LispObject::t());
}

/// `(eql 1 1.0)` — integer and float with the same numeric value are
/// different types and therefore NOT eql.
#[test]
fn eql_integer_float_different_types() {
    let result = eval("(eql 1 1.0)");
    assert_eq!(result, LispObject::nil());
}

/// `(eql "a" "a")` — strings need `equal`, not eql.
#[test]
fn eql_strings_are_nil() {
    let result = eval(r#"(eql "a" "a")"#);
    assert_eq!(result, LispObject::nil());
}

/// `(eql 'x 'x)` — same interned symbol is `t` (eq ⇒ eql).
#[test]
fn eql_same_symbol() {
    let result = eval("(eql 'x 'x)");
    assert_eq!(result, LispObject::t());
}

/// `(eql 'a 'b)` — different symbols are nil.
#[test]
fn eql_different_symbols() {
    let result = eval("(eql 'a 'b)");
    assert_eq!(result, LispObject::nil());
}

/// `(eql nil nil)` → t  (nil is eq to itself).
#[test]
fn eql_nil_nil() {
    let result = eval("(eql nil nil)");
    assert_eq!(result, LispObject::t());
}

/// `(eql 2 3)` — different integers are nil.
#[test]
fn eql_different_integers() {
    let result = eval("(eql 2 3)");
    assert_eq!(result, LispObject::nil());
}
