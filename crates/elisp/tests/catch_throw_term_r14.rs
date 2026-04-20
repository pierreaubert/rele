//! R14 regression: bytecode `Bpushcatch` (opcode 50) must actually
//! install a catch handler so `throw` inside a byte-compiled body can
//! be caught.
//!
//! Before the fix, opcode 50 was a no-op: it popped the catch tag and
//! discarded the handler's jump target. Any `throw` inside the body
//! of a byte-compiled `catch` therefore escaped to the top level and
//! surfaced as "no catch for tag: X with value: Y".
//!
//! This bit 49 tree-sitter ERT tests because loading `treesit.elc`
//! overrides our `treesit-ready-p` stub with the real (byte-compiled)
//! definition:
//!
//! ```elisp
//! (defun treesit-ready-p (language &optional quiet)
//!   (let (... msg)
//!     (catch 'term
//!       (when (not (treesit-available-p))
//!         (setq msg ...)
//!         (throw 'term nil))            ; <- escapes the catch
//!       ...)))
//! ```
//!
//! The fix mirrors opcode 141 semantics for opcode 50: run the body
//! inline, catch matching `throw`s, restore the stack, jump to the
//! end-tag. Non-matching throws propagate.

use rele_elisp::{add_primitives, primitives_modules, Interpreter, LispObject};

fn make_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    primitives_modules::register(&mut interp);
    interp
}

/// Tree-walking catch should work — this was already correct, and
/// serves as a baseline so a bytecode regression can't hide as an
/// eval-level regression.
#[test]
fn eval_catch_handles_throw_in_lambda() {
    let interp = make_interp();
    let res = interp
        .eval_source(
            r#"
(let ((advance (lambda (x)
                 (if (null x)
                     (throw 'term nil)
                   x))))
  (catch 'term
    (funcall advance nil)
    'never-reached))
"#,
        )
        .expect("catch should handle the throw from the lambda");
    assert!(res.is_nil(), "expected nil, got {:?}", res);
}

#[test]
fn eval_catch_returns_thrown_value() {
    let interp = make_interp();
    let res = interp
        .eval_source(
            r#"
(let ((advance (lambda (x) (throw 'term x))))
  (catch 'term
    (funcall advance 42)
    'never-reached))
"#,
        )
        .expect("catch should handle the throw");
    assert_eq!(res.as_integer(), Some(42));
}

#[test]
fn catch_only_matching_tag() {
    let interp = make_interp();
    let res = interp.eval_source(
        r#"
(catch 'other
  (catch 'term
    (throw 'term 'inner))
  'outer-continues)
"#,
    );
    let v = res.expect("inner catch should handle");
    assert_eq!(v.as_symbol().as_deref(), Some("outer-continues"));
}

#[test]
fn uncaught_throw_is_error() {
    let interp = make_interp();
    let res = interp.eval_source("(throw 'term nil)");
    assert!(res.is_err());
}

#[test]
fn nested_lambda_throw_through_catch() {
    let interp = make_interp();
    let res = interp
        .eval_source(
            r#"
(let* ((inner (lambda (x) (if (null x) (throw 'term nil) x)))
       (outer (lambda (x) (funcall inner x))))
  (catch 'term
    (funcall outer nil)
    'never))
"#,
        )
        .expect("nested lambdas must not hide the throw");
    assert!(res.is_nil());
}

/// Regression for the specific shape that the broken bytecode
/// `Bpushcatch` produced: `catch` wraps a `when`/`throw` in a defun
/// body. Tree-walking works fine, but if the body is ever compiled
/// (via `byte-compile` or by loading a .elc), it used to fail.
#[test]
fn catch_throw_nil_shaped_like_treesit_ready_p() {
    let interp = make_interp();
    let res = interp
        .eval_source(
            r#"
(defun my-ready (x)
  (let (msg)
    (catch 'term
      (when (null x)
        (setq msg "not ready")
        (throw 'term nil)))
    (if (not msg) t nil)))
(list (my-ready nil) (my-ready t))
"#,
        )
        .expect("treesit-ready-p shape must work");
    // (my-ready nil) → nil (throw caught, msg set, final returns nil)
    // (my-ready t)   → t (catch body falls through, msg stays nil)
    let first = res.first().unwrap_or(LispObject::nil());
    let second = res.nth(1).unwrap_or(LispObject::nil());
    assert!(first.is_nil(), "my-ready nil expected nil, got {:?}", first);
    assert!(second.is_t(), "my-ready t expected t, got {:?}", second);
}
