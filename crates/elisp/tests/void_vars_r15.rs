//! R15 void-variable fixtures round 3 — tests for legitimate Emacs globals.
//!
//! This test module verifies that every void-variable fixture added in R15
//! is properly initialised in `make_stdlib_interp()`. Each test builds a
//! fresh interpreter through the shared runtime bootstrap helper, then asserts
//! both `(boundp 'VAR)` and `(symbol-value 'VAR)`.
//!
//! Only the (a)-class variables from the round-2 baseline shortlist are
//! covered here. (b) (macro/let-bound locals) and (c) (reader/gensym) cases
//! are documented in the PR body and deferred to a follow-up stream.

use rele_elisp::eval::bootstrap::make_stdlib_interp;
use rele_elisp::{Interpreter, LispObject, read};

fn assert_boundp(interp: &Interpreter, name: &str) {
    let src = format!("(boundp '{name})");
    let expr = read(&src).unwrap_or_else(|e| panic!("read {src:?}: {e:?}"));
    let value = interp
        .eval(expr)
        .unwrap_or_else(|e| panic!("eval (boundp '{name}): {e:?}"));
    assert_eq!(
        value,
        LispObject::t(),
        "(boundp '{name}) should return t but returned {value:?}"
    );
}

#[test]
fn r15_internet_is_working_is_bound() {
    let interp = make_stdlib_interp();
    assert_boundp(&interp, "internet-is-working");
}

#[test]
fn r15_internet_is_working_value_is_nil() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'internet-is-working)").unwrap();
    let value = interp.eval(expr).unwrap();
    assert_eq!(
        value,
        LispObject::nil(),
        "internet-is-working should default to nil (no internet in CI)"
    );
}

#[test]
fn r15_regex_tests_resources_dir_is_bound() {
    let interp = make_stdlib_interp();
    assert_boundp(&interp, "regex-tests--resources-dir");
}

#[test]
fn r15_regex_tests_resources_dir_value_is_empty_string() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'regex-tests--resources-dir)").unwrap();
    let value = interp.eval(expr).unwrap();
    assert_eq!(
        value,
        LispObject::string(""),
        "regex-tests--resources-dir should default to \"\" so that \
         (file-directory-p regex-tests--resources-dir) returns nil and the \
         guarded tests skip cleanly"
    );
}
