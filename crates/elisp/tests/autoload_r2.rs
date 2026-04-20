//! R2: autoload-do-load machinery tests.
//!
//! Tests for the new `autoload-do-load`, `autoload-compute-prefixes`, and
//! `load-history-filename-element` primitives added in stream R2.

use rele_elisp::{add_primitives, Interpreter, LispObject};

/// Helper: create a fresh interpreter with all primitives loaded.
fn make_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
}

/// Helper: evaluate a string expression in an interpreter and return the result.
fn eval_str(interp: &Interpreter, s: &str) -> Result<LispObject, rele_elisp::ElispError> {
    let form = rele_elisp::read(s).unwrap();
    interp.eval(form)
}

// ---- autoload-do-load -------------------------------------------------------

/// (a) `autoload-do-load` when FUNNAME is already fboundp returns its definition.
///
/// We define a function `r2-test-defined-fn`, then call `autoload-do-load`
/// with a dummy loaddef and that symbol name. It should return the function
/// definition rather than nil.
#[test]
fn test_autoload_do_load_fboundp_returns_def() {
    let interp = make_interp();
    // Define a function so it is fboundp.
    eval_str(&interp, "(defun r2-test-defined-fn (x) (* x 2))").unwrap();
    // Build a dummy loaddef: (autoload "some-file.el" "docstring" nil nil)
    // The actual file doesn't matter — we shouldn't load anything.
    let result = eval_str(
        &interp,
        "(autoload-do-load
            '(autoload \"some-file.el\" \"doc\" nil nil)
            'r2-test-defined-fn
            nil)",
    )
    .unwrap();
    // The result should be non-nil (the lambda/closure for r2-test-defined-fn).
    assert!(
        !result.is_nil(),
        "autoload-do-load should return the existing definition, got nil"
    );
}

/// (b) `autoload-do-load` when FUNNAME is NOT fboundp returns nil.
#[test]
fn test_autoload_do_load_not_fboundp_returns_nil() {
    let interp = make_interp();
    // Use a symbol name that is definitely not defined.
    let result = eval_str(
        &interp,
        "(autoload-do-load
            '(autoload \"phantom-file.el\" \"doc\" nil nil)
            'r2-test-phantom-fn-that-does-not-exist
            nil)",
    )
    .unwrap();
    assert_eq!(
        result,
        LispObject::nil(),
        "autoload-do-load should return nil for unbound symbol"
    );
}

/// (b-2) `autoload-do-load` with only LOADDEF (FUNNAME omitted) returns nil.
#[test]
fn test_autoload_do_load_no_funname_returns_nil() {
    let interp = make_interp();
    let result = eval_str(
        &interp,
        "(autoload-do-load '(autoload \"file.el\" \"doc\" nil nil))",
    )
    .unwrap();
    assert_eq!(result, LispObject::nil());
}

/// (c) Arity check: calling `autoload-do-load` with zero arguments signals
/// `WrongNumberOfArguments`.
#[test]
fn test_autoload_do_load_no_args_signals_wrong_arity() {
    let interp = make_interp();
    let err = eval_str(&interp, "(autoload-do-load)");
    assert!(
        err.is_err(),
        "autoload-do-load with no args should signal an error"
    );
    match err.unwrap_err() {
        rele_elisp::ElispError::WrongNumberOfArguments => {}
        other => panic!(
            "expected WrongNumberOfArguments, got {:?}",
            other
        ),
    }
}

// ---- autoload-compute-prefixes ----------------------------------------------

/// `autoload-compute-prefixes` is a no-op stub returning nil.
#[test]
fn test_autoload_compute_prefixes_returns_nil() {
    let interp = make_interp();
    let result =
        eval_str(&interp, "(autoload-compute-prefixes \"some-file.el\")").unwrap();
    assert_eq!(result, LispObject::nil());
}

// ---- load-history-filename-element -----------------------------------------

/// When `load-history` is nil (fresh interpreter), returns nil.
#[test]
fn test_load_history_filename_element_empty_history() {
    let interp = make_interp();
    // Use `set` (not `setq`) to write directly to the global symbol value
    // cell, which is where `prim_load_history_filename_element` reads from.
    eval_str(&interp, "(set 'load-history nil)").unwrap();
    let result =
        eval_str(&interp, "(load-history-filename-element \"nonexistent.el\")").unwrap();
    assert_eq!(result, LispObject::nil());
}

/// When `load-history` contains the filename, returns the matching element.
///
/// NOTE: We use `(set 'load-history ...)` rather than `(setq load-history ...)`
/// because `setq` stores to the lexical/dynamic env HashMap, whereas
/// `prim_load_history_filename_element` reads from the obarray value cell
/// (the global symbol-value slot), which is what `(set 'sym val)` writes to.
/// This matches real Emacs semantics: `load-history` is a global variable.
#[test]
fn test_load_history_filename_element_found() {
    let interp = make_interp();
    // Build a fake load-history alist: (("foo.el" . (some-fn)) ("bar.el" . (other-fn)))
    eval_str(
        &interp,
        "(set 'load-history '((\"foo.el\" . (some-fn)) (\"bar.el\" . (other-fn))))",
    )
    .unwrap();
    let result =
        eval_str(&interp, "(load-history-filename-element \"foo.el\")").unwrap();
    // Should return the cons ("foo.el" . (some-fn)), so car is "foo.el".
    let car = result.first().expect("result should be a cons");
    assert_eq!(
        car.as_string().map(|s| s.as_str()),
        Some("foo.el"),
        "returned element car should be \"foo.el\""
    );
}

/// Returns nil when filename is not in `load-history`.
#[test]
fn test_load_history_filename_element_not_found() {
    let interp = make_interp();
    eval_str(
        &interp,
        "(set 'load-history '((\"foo.el\" . (some-fn))))",
    )
    .unwrap();
    let result =
        eval_str(&interp, "(load-history-filename-element \"bar.el\")").unwrap();
    assert_eq!(result, LispObject::nil());
}

/// Zero-argument call should signal WrongNumberOfArguments.
#[test]
fn test_load_history_filename_element_no_args() {
    let interp = make_interp();
    let err = eval_str(&interp, "(load-history-filename-element)");
    assert!(
        err.is_err(),
        "load-history-filename-element with no args should signal an error"
    );
    match err.unwrap_err() {
        rele_elisp::ElispError::WrongNumberOfArguments => {}
        other => panic!("expected WrongNumberOfArguments, got {:?}", other),
    }
}
