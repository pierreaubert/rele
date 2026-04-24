//! R11 regression: calling a major/minor mode installed by
//! `define-derived-mode` / `define-minor-mode` / `define-globalized-minor-mode`
//! used to raise `void function: nil`.
//!
//! Cause: our stubs bound the mode name as a variable = nil but never
//! installed anything in the function cell. `Environment::get_function_id`
//! falls back to the variable slot when the function cell is unset, and
//! returned `Nil`. Invoking that `Nil` went to `call_function`'s
//! `LispObject::Nil => VoidFunction("nil")` arm.
//!
//! This broke every ERT test that ran `(with-temp-buffer (X-mode) ...)`
//! for a mode defined via the stub path — 100+ tests across
//! autoconf-tests, buff-menu-tests, buffer-tests, checkdoc-tests,
//! derived-tests, easy-mmode-tests, f90-tests, help-mode-tests,
//! lisp-mode-tests, simple-tests, tcl-tests, text-property-search-tests,
//! undo-tests, tabulated-list-tests, and others (134 total occurrences
//! of "void function: nil" in the baseline ERT run).
//!
//! Fix: set the symbol's function cell to the built-in `ignore`
//! primitive so the call resolves as a no-op that returns nil. The
//! variable slot is still `nil` (real Emacs also has a defvar for
//! minor modes, and tests that check `(boundp 'the-mode)` still see
//! t), but the function-position lookup now finds the ignore primitive
//! first via the `is_callable_value` check in `get_function_id`.

use rele_elisp::LispObject;
use rele_elisp::eval::bootstrap::make_stdlib_interp;
use rele_elisp::read;

fn eval_str(src: &str) -> Result<LispObject, String> {
    let interp = make_stdlib_interp();
    interp.reset_eval_ops();
    interp.set_eval_ops_limit(2_000_000);
    let form = read(src).map_err(|e| format!("read: {e}"))?;
    interp.eval(form).map_err(|e| format!("{e}"))
}

/// `define-derived-mode` must install a callable function cell so
/// calling `(the-mode)` later returns nil instead of void-function: nil.
#[test]
fn define_derived_mode_is_callable() {
    let r = eval_str(
        r#"(progn
             (define-derived-mode my-mode-r11 fundamental-mode "MyR11"
               "Test mode.")
             (my-mode-r11))"#,
    );
    assert!(r.is_ok(), "expected success, got {r:?}");
}

/// `define-minor-mode` must install a callable function cell too.
#[test]
fn define_minor_mode_is_callable() {
    let r = eval_str(
        r#"(progn
             (define-minor-mode my-minor-r11
               "Test minor mode."
               :init-value nil)
             (my-minor-r11))"#,
    );
    assert!(r.is_ok(), "expected success, got {r:?}");
}

/// `define-globalized-minor-mode` must install a callable function cell.
#[test]
fn define_globalized_minor_mode_is_callable() {
    let r = eval_str(
        r#"(progn
             (define-globalized-minor-mode my-global-r11
               my-minor-r11 (lambda () nil))
             (my-global-r11 1))"#,
    );
    assert!(r.is_ok(), "expected success, got {r:?}");
}

/// `fboundp` on a freshly defined mode must return t (consistency with
/// real Emacs and with the previous fboundp behaviour).
#[test]
fn fboundp_sees_defined_modes() {
    let r = eval_str(
        r#"(progn
             (define-derived-mode my-mode-r11-fb fundamental-mode "X")
             (and (fboundp 'my-mode-r11-fb) t))"#,
    );
    assert!(matches!(r, Ok(ref v) if !v.is_nil()), "{r:?}");
}

/// The mode's *variable* cell should still be nil (tests that rely on
/// `(boundp 'the-mode)` or `(symbol-value 'the-mode)` would break if
/// we accidentally replaced it).
#[test]
fn mode_variable_cell_still_nil() {
    let r = eval_str(
        r#"(progn
             (define-minor-mode my-minor-r11-var "X" :init-value nil)
             (symbol-value 'my-minor-r11-var))"#,
    );
    assert!(matches!(r, Ok(ref v) if v.is_nil()), "{r:?}");
}

/// Literal `(nil)` form still correctly signals void-function: nil.
/// (We're fixing the STUB path, not the underlying call-nil semantics.)
#[test]
fn literal_nil_head_still_errors() {
    let r = eval_str("(nil)");
    assert!(r.is_err());
    assert!(
        r.as_ref().err().unwrap().contains("void") && r.as_ref().err().unwrap().contains("nil"),
        "{r:?}"
    );
}

/// End-to-end: the exact shape that autoconf-tests, checkdoc-tests,
/// f90-tests all share — `(with-temp-buffer (some-mode) ...)` — now
/// runs without the void-function: nil signal.
#[test]
fn with_temp_buffer_mode_call_is_silent() {
    let r = eval_str(
        r#"(progn
             (define-derived-mode my-mode-r11-wtb fundamental-mode "X")
             (with-temp-buffer
               (my-mode-r11-wtb)
               t))"#,
    );
    assert!(matches!(r, Ok(ref v) if !v.is_nil()), "{r:?}");
}
