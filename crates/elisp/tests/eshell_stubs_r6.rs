//! R6 eshell primitive stubs — integration tests.
//!
//! These tests verify that the eshell-* stubs added in stream R6
//! (see `crates/elisp/src/primitives_modules.rs`) are registered and
//! callable. They don't test behaviour of real eshell — the goal is
//! merely to flip void-function errors into sensible default returns
//! so that ERT tests that call these entry points can continue running
//! and fail with assertion errors instead of aborting during setup.
//!
//! Hit counts below are taken from
//! `/tmp/emacs-results-round2-baseline.jsonl` (round-2 ERT baseline).
//!
//! See also: `tests/void_vars_r5.rs` and `tests/autoload_r2.rs` for the
//! integration-test conventions in this crate.

use rele_elisp::{add_primitives, primitives_modules, read, Interpreter, LispObject};

/// Build an interpreter that mirrors the inside-crate `make_stdlib_interp`
/// helper closely enough for calling module stubs: primitives registered,
/// plus `primitives_modules::register`.
fn make_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    primitives_modules::register(&mut interp);
    interp
}

// ---- hits->=5 bucket -------------------------------------------------------

/// `eshell-glob-convert` (6 hits) — identity pass-through.
#[test]
fn test_eshell_glob_convert_identity() {
    let interp = make_interp();
    let expr = read("(eshell-glob-convert \"*.el\")").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(
        result,
        LispObject::string("*.el"),
        "eshell-glob-convert should return its argument unchanged"
    );
}

/// `eshell-glob-convert` with no args — still safe (returns nil car).
#[test]
fn test_eshell_glob_convert_no_args() {
    let interp = make_interp();
    let expr = read("(eshell-glob-convert)").unwrap();
    // Should not signal void-function; the stub returns (car nil) = nil.
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::nil());
}

/// `eshell-printable-size` (5 hits) — empty string stub.
#[test]
fn test_eshell_printable_size_returns_string() {
    let interp = make_interp();
    let expr = read("(eshell-printable-size 1024)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(
        result,
        LispObject::string(""),
        "eshell-printable-size stub should return empty string"
    );
}

// ---- explicit API-surface bucket -------------------------------------------

/// `eshell-command` — nil stub.
#[test]
fn test_eshell_command_returns_nil() {
    let interp = make_interp();
    let expr = read("(eshell-command \"ls\")").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::nil());
}

/// `eshell-parse-arguments` — structural stub. We don't assert shape
/// beyond "returns non-nil list" because callers mostly just care that
/// it doesn't signal.
#[test]
fn test_eshell_parse_arguments_returns_list() {
    let interp = make_interp();
    let expr = read("(eshell-parse-arguments \"foo\" \"bar\")").unwrap();
    let result = interp.eval(expr).unwrap();
    assert!(
        !result.is_nil(),
        "eshell-parse-arguments should return a non-nil list"
    );
}

/// `eshell-parse-arguments` with zero args — still non-error.
#[test]
fn test_eshell_parse_arguments_no_args_ok() {
    let interp = make_interp();
    let expr = read("(eshell-parse-arguments)").unwrap();
    let result = interp.eval(expr);
    assert!(
        result.is_ok(),
        "eshell-parse-arguments with no args should not signal"
    );
}

/// `eshell-evaluate-predicate` — predicate stub returns t.
#[test]
fn test_eshell_evaluate_predicate_returns_t() {
    let interp = make_interp();
    let expr = read("(eshell-evaluate-predicate nil)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::t());
}

/// `eshell-eval-command` — nil stub.
#[test]
fn test_eshell_eval_command_returns_nil() {
    let interp = make_interp();
    let expr = read("(eshell-eval-command '(pwd))").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::nil());
}

/// `eshell-evaluate-gpg` — nil stub.
#[test]
fn test_eshell_evaluate_gpg_returns_nil() {
    let interp = make_interp();
    let expr = read("(eshell-evaluate-gpg \"--help\")").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::nil());
}

// ---- regression: make sure R6 didn't break pre-existing stubs --------------

/// Sanity: still works after R6 additions.
#[test]
fn test_prior_eshell_stub_still_works() {
    let interp = make_interp();
    let expr = read("(eshell-extended-glob \"*.md\")").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::string("*.md"));
}
