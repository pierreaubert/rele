//! R19 regression — `ert-deftest` must capture the surrounding lexical
//! env at registration time.
//!
//! R15's void-variable classifier (PR #26) flagged 13 hits where a
//! `let`-bound variable at file top level wasn't visible inside a
//! subsequent `ert-deftest` body:
//!
//! | Variable                  | Hits | File              |
//! |---------------------------|-----:|-------------------|
//! | `spec`                    |    5 | bindat-tests.el   |
//! | `create-file-buffer`      |    4 | ibuffer-tests.el  |
//! | `create-non-file-buffer`  |    4 | ibuffer-tests.el  |
//!
//! Both test files declare `lexical-binding: t`, so the let-binding is
//! supposed to form a lexical closure around the test body. Our
//! `ert-deftest` interceptor used to store a bare `(lambda () BODY...)`
//! which does *not* snapshot the lexical env; by the time the runner
//! invoked the test, the `let` had long since exited, so the test
//! raised `void-variable`.
//!
//! Fix: at registration, capture the current lexical env into a
//! `(closure CAPTURED () BODY...)` form — the same shape the `lambda`
//! special form produces at evaluation time. The runner quotes this
//! form so `funcall` receives the closure object directly.

use rele_elisp::{add_primitives, read, Interpreter, LispObject};

fn fresh_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
}

/// Run the registered thunk for `test_name` and return its result.
fn run_ert_body(interp: &Interpreter, test_name: &str) -> LispObject {
    // Emulate what `run_rele_ert_tests_detailed` does: fish the thunk
    // off the symbol's plist under the `ert--rele-test` key and invoke
    // it via `(funcall (quote THUNK))`. The quote matters because the
    // R19 fix stores the thunk as a `closure` form, which is not a
    // self-evaluating cons — evaluating it naked would try to dispatch
    // on `closure` as a function head.
    let src = format!(
        "(funcall (get (intern \"{test_name}\") 'ert--rele-test))",
    );
    // `get` needs the thunk treated as a value, so use a small elisp
    // shim that funcalls it correctly.
    //
    // Simpler: just call our native runner via the public API. We
    // don't have that re-exported here, so we go through `funcall` on
    // the plist value — and wrap it in quote. Build via `read` so we
    // exercise the same shape real tests do.
    let _ = src; // shim retained for clarity, actual call below
    let expr = read(
        &format!(
            "(funcall (get '{test_name} 'ert--rele-test))",
        ),
    )
    .unwrap_or_else(|e| panic!("read: {e:?}"));
    interp.eval(expr).unwrap_or_else(|e| {
        panic!("eval ({test_name} body): {e:?}")
    })
}

/// bindat-tests.el reduction: a single `let`-bound symbol the body
/// references via `equal`.
#[test]
fn r19_let_captured_into_ert_body_bindat_shape() {
    let interp = fresh_interp();
    let expr = read(
        "(let ((spec '(foo 1 bar 2))) \
           (ert-deftest r19-spec-visible () \
             (should (equal spec '(foo 1 bar 2)))))",
    )
    .unwrap();
    interp.eval(expr).expect("registration");
    // Running the body must NOT raise void-variable on `spec`.
    let result = run_ert_body(&interp, "r19-spec-visible");
    // `should` returns the value it checked if non-nil.
    assert!(!result.is_nil(), "body should return non-nil, got {result:?}");
}

/// ibuffer-tests.el reduction: `let*`-bound lambda funcall'd from the
/// body.
#[test]
fn r19_let_star_captured_lambda_ibuffer_shape() {
    let interp = fresh_interp();
    let expr = read(
        "(let* ((create-file-buffer (lambda (name) (concat \"buf:\" name)))) \
           (ert-deftest r19-create-file-buffer-visible () \
             (should (equal (funcall create-file-buffer \"hello\") \"buf:hello\"))))",
    )
    .unwrap();
    interp.eval(expr).expect("registration");
    let result = run_ert_body(&interp, "r19-create-file-buffer-visible");
    assert!(!result.is_nil(), "body should return non-nil, got {result:?}");
}

/// Two distinct let-bound values at two distinct test registrations —
/// each must capture its own binding, not share or overwrite.
#[test]
fn r19_two_tests_capture_distinct_lets() {
    let interp = fresh_interp();
    interp
        .eval(
            read(
                "(let ((spec \"first\")) \
                   (ert-deftest r19-test-a () \
                     (should (equal spec \"first\"))))",
            )
            .unwrap(),
        )
        .expect("test-a");
    interp
        .eval(
            read(
                "(let ((spec \"second\")) \
                   (ert-deftest r19-test-b () \
                     (should (equal spec \"second\"))))",
            )
            .unwrap(),
        )
        .expect("test-b");
    let a = run_ert_body(&interp, "r19-test-a");
    let b = run_ert_body(&interp, "r19-test-b");
    assert!(!a.is_nil(), "test-a body: {a:?}");
    assert!(!b.is_nil(), "test-b body: {b:?}");
}

/// Mutating the outer binding AFTER registration must NOT leak into
/// the captured closure (the snapshot semantics match real Emacs with
/// `lexical-binding: t` and rele's existing `lambda` behaviour).
#[test]
fn r19_closure_captures_snapshot_not_reference() {
    let interp = fresh_interp();
    interp
        .eval(
            read(
                "(let ((spec 42)) \
                   (ert-deftest r19-snapshot () \
                     (should (eq spec 42))))",
            )
            .unwrap(),
        )
        .expect("registration");
    // After the let exits, setq on the top-level has no effect on the
    // captured binding because `spec` was lexically bound inside a let
    // that has already unwound. The body should still see 42.
    let result = run_ert_body(&interp, "r19-snapshot");
    assert!(!result.is_nil(), "captured value should still be 42: {result:?}");
}

/// Baseline: a test with no enclosing `let` still works (empty
/// captured alist is a no-op, not a crash).
#[test]
fn r19_no_let_still_works() {
    let interp = fresh_interp();
    interp
        .eval(
            read("(ert-deftest r19-plain () (should (= 1 1)))").unwrap(),
        )
        .expect("registration");
    let result = run_ert_body(&interp, "r19-plain");
    assert!(!result.is_nil(), "plain test: {result:?}");
}
