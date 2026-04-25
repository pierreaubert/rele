//! R23 regression — `ert-deftest` must register its test under
//! `ert--test` with recoverable tags/body slots, and the runner must
//! publish the current `ert-test` struct for `(ert-running-test)`.
//!
//! Round-2 baseline: 61 errors `signal wrong-type-argument: (ert-test
//! nil)`, concentrated in tramp-tests.el (20) and erc-scenarios-*.el
//! (~40). Both file families use macros that expand to:
//!
//! ```elisp
//! (ert-deftest NAME-with-stat ()
//!   :tags '(:expensive-test)
//!   ...
//!   (if-let ((ert-test (ert-get-test 'NAME))
//!            (result (ert-test-most-recent-result ert-test)))
//!       ...)
//!   (funcall (ert-test-body ert-test)))
//!
//! ;; erc-scenarios-common--make-bindings:
//! (memq :erc--graphical (ert-test-tags (ert-running-test)))
//! ```
//!
//! Real Emacs's `cl-defstruct ert-test` accessors signal
//! `wrong-type-argument: (ert-test nil)` whenever the chain starts with
//! a nil lookup. Our headless harness didn't record tests under the
//! `ert--test` property, so `(ert-get-test 'X)` resolved to nil and the
//! cl-accessor blew up downstream.
//!
//! Fix: `ert-deftest` now stores a `(rele-ert-test NAME TAGS DOC BODY
//! FILE)` tagged list on the symbol's `ert--test` plist entry, and
//! Rust-side stubs for `ert-get-test` / `ert-test-*` / `ert-running-test`
//! understand both that shape and nil. The runner publishes the
//! current test object before each body so `(ert-running-test)` never
//! returns nil while BODY is executing.

use rele_elisp::eval::bootstrap::run_rele_ert_tests_detailed;
use rele_elisp::{Interpreter, add_primitives, read};

fn fresh_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
}

#[test]
fn r23_ert_registrations_are_interpreter_local() {
    let interp_a = fresh_interp();
    let interp_b = fresh_interp();

    interp_a
        .eval(read("(ert-deftest r23-isolated-a () (should t))").unwrap())
        .expect("register test in interp A");
    interp_b
        .eval(read("(ert-deftest r23-isolated-b () (should t))").unwrap())
        .expect("register test in interp B");

    assert!(
        interp_a
            .eval(read("(ert-get-test 'r23-isolated-b)").unwrap())
            .expect("A should not see B")
            .is_nil(),
        "ERT registration from interpreter B leaked into interpreter A",
    );
    assert!(
        interp_b
            .eval(read("(ert-get-test 'r23-isolated-a)").unwrap())
            .expect("B should not see A")
            .is_nil(),
        "ERT registration from interpreter A leaked into interpreter B",
    );

    let (stats_a, results_a) = run_rele_ert_tests_detailed(&interp_a);
    let names_a: Vec<&str> = results_a
        .iter()
        .map(|result| result.name.as_str())
        .collect();
    assert_eq!(stats_a.passed, 1);
    assert_eq!(names_a, vec!["r23-isolated-a"]);

    let (stats_b, results_b) = run_rele_ert_tests_detailed(&interp_b);
    let names_b: Vec<&str> = results_b
        .iter()
        .map(|result| result.name.as_str())
        .collect();
    assert_eq!(stats_b.passed, 1);
    assert_eq!(names_b, vec!["r23-isolated-b"]);
}

#[test]
fn r23_same_ert_symbol_has_per_interpreter_test_body() {
    let interp_pass = fresh_interp();
    let interp_fail = fresh_interp();

    interp_pass
        .eval(read("(ert-deftest r23-same-name () (should t))").unwrap())
        .expect("register passing test");
    interp_fail
        .eval(read("(ert-deftest r23-same-name () (should nil))").unwrap())
        .expect("register failing test");

    let (pass_stats, pass_results) = run_rele_ert_tests_detailed(&interp_pass);
    assert_eq!(pass_stats.passed, 1);
    assert_eq!(pass_stats.failed, 0);
    assert_eq!(pass_results.len(), 1);
    assert_eq!(pass_results[0].name, "r23-same-name");
    assert_eq!(pass_results[0].result, "pass");

    let (fail_stats, fail_results) = run_rele_ert_tests_detailed(&interp_fail);
    assert_eq!(fail_stats.passed, 0);
    assert_eq!(fail_stats.failed, 1);
    assert_eq!(fail_results.len(), 1);
    assert_eq!(fail_results[0].name, "r23-same-name");
    assert_eq!(fail_results[0].result, "fail");
}

/// `(ert-get-test 'X)` must return a non-nil value once X is
/// registered via `ert-deftest`, so `if-let` bindings proceed to the
/// then-branch and wrapper macros can read the tags/body slots.
#[test]
fn r23_ert_get_test_returns_non_nil_for_registered_test() {
    let interp = fresh_interp();
    interp
        .eval(read("(ert-deftest r23-plain () (should t))").unwrap())
        .expect("registration");

    let result = interp
        .eval(read("(ert-get-test 'r23-plain)").unwrap())
        .expect("ert-get-test");
    assert!(
        !result.is_nil(),
        "ert-get-test returned nil for registered test"
    );
    assert!(
        result.is_cons(),
        "expected a rele-ert-test cons, got {result:?}",
    );

    // Round-trip: (ert-test-name (ert-get-test 'X)) must match X.
    let name = interp
        .eval(read("(ert-test-name (ert-get-test 'r23-plain))").unwrap())
        .expect("ert-test-name");
    assert_eq!(
        name.as_symbol().as_deref(),
        Some("r23-plain"),
        "ert-test-name chain returned {name:?}",
    );
}

/// Unregistered symbols pass through as nil — callers use `if-let` to
/// branch, which requires a non-signalling nil here.
#[test]
fn r23_ert_get_test_nil_for_unknown_symbol() {
    let interp = fresh_interp();
    let result = interp
        .eval(read("(ert-get-test 'r23-no-such-test)").unwrap())
        .expect("ert-get-test on unknown");
    assert!(result.is_nil(), "expected nil, got {result:?}");

    // `ert-test-boundp` mirrors this: nil for unknown, t for known.
    let bound = interp
        .eval(read("(ert-test-boundp 'r23-no-such-test)").unwrap())
        .expect("ert-test-boundp");
    assert!(bound.is_nil(), "boundp should be nil for unknown test");
}

/// Accessors must NOT signal `wrong-type-argument: (ert-test nil)`
/// when called with nil — this is the core round-2 baseline regression.
#[test]
fn r23_accessors_return_nil_on_nil_input_without_signalling() {
    let interp = fresh_interp();
    // All of these would signal `wrong-type-argument: (ert-test nil)`
    // in real Emacs. We want nil.
    for expr in [
        "(ert-test-name nil)",
        "(ert-test-tags nil)",
        "(ert-test-body nil)",
        "(ert-test-most-recent-result nil)",
        "(ert-test-documentation nil)",
        "(ert-test-file-name nil)",
    ] {
        let result = interp
            .eval(read(expr).unwrap())
            .unwrap_or_else(|e| panic!("{expr} signalled {e:?}"));
        assert!(result.is_nil(), "{expr} returned {result:?}, expected nil",);
    }
    // `ert-test-result-duration` returns 0, not nil, to keep
    // (skip-unless (< (ert-test-result-duration r) 300)) workable.
    let dur = interp
        .eval(read("(ert-test-result-duration nil)").unwrap())
        .expect("ert-test-result-duration");
    assert_eq!(dur.as_integer(), Some(0));
}

/// `:tags` on `ert-deftest` must survive into `(ert-test-tags
/// (ert-get-test 'X))` so `(memq :expensive-test ...)` works.
#[test]
fn r23_ert_deftest_tags_roundtrip() {
    let interp = fresh_interp();
    interp
        .eval(
            read(
                "(ert-deftest r23-tagged () \
                   :tags '(:expensive-test :tramp-asynchronous-processes) \
                   (should t))",
            )
            .unwrap(),
        )
        .expect("registration");

    let got_tags = interp
        .eval(read("(ert-test-tags (ert-get-test 'r23-tagged))").unwrap())
        .expect("ert-test-tags");
    let memq_result = interp
        .eval(
            read(
                "(memq :expensive-test \
                       (ert-test-tags (ert-get-test 'r23-tagged)))",
            )
            .unwrap(),
        )
        .expect("memq tag lookup");
    assert!(
        !memq_result.is_nil(),
        ":expensive-test should be in ert-test-tags; got {memq_result:?} (tags: {})",
        got_tags.prin1_to_string(),
    );
}

/// Docstring-before-body must still register the test AND populate the
/// documentation slot. Many ERT files put their docstring first.
#[test]
fn r23_ert_deftest_docstring_before_body() {
    let interp = fresh_interp();
    interp
        .eval(
            read(
                "(ert-deftest r23-documented () \
                   \"This test checks something important.\" \
                   (should (= 1 1)))",
            )
            .unwrap(),
        )
        .expect("registration");

    // Registration must not have consumed the docstring as the whole
    // body — the test must run its `should` form.
    let got = interp
        .eval(read("(ert-get-test 'r23-documented)").unwrap())
        .expect("ert-get-test");
    assert!(!got.is_nil(), "documented test should be registered");

    let doc = interp
        .eval(read("(ert-test-documentation (ert-get-test 'r23-documented))").unwrap())
        .expect("ert-test-documentation");
    let doc_str = doc.as_string();
    assert_eq!(
        doc_str.as_deref().map(|s| s.as_str()),
        Some("This test checks something important."),
        "docstring did not survive to documentation slot: {doc:?}",
    );
}

/// tramp-tests.el reduction: a wrapper macro defines a NEW test whose
/// body calls `(ert-get-test 'ORIG)` and funcalls its body. This must
/// not signal; the lookup returns the rele-ert-test struct and
/// `ert-test-body` hands back the closure, which we can funcall.
///
/// Uses plain `let` + `when` rather than `if-let` because the fresh
/// interpreter doesn't preload subr-x; the production harness does,
/// and the tramp wrapper's `if-let` shape would go through the same
/// `ert-get-test` / `ert-test-body` machinery we exercise here.
#[test]
fn r23_tramp_wrapper_deftest_shape_does_not_signal() {
    let interp = fresh_interp();
    interp
        .eval(read("(ert-deftest r23-orig () (should t))").unwrap())
        .expect("register orig");
    interp
        .eval(
            read(
                "(ert-deftest r23-wrapper () \
                   :tags '(:expensive-test) \
                   (let ((ert-test (ert-get-test 'r23-orig))) \
                     (when ert-test \
                       (funcall (ert-test-body ert-test)))))",
            )
            .unwrap(),
        )
        .expect("register wrapper");

    // Sanity-check lookups used by the wrapper body.
    let struct_obj = interp
        .eval(read("(ert-get-test 'r23-orig)").unwrap())
        .expect("ert-get-test 'r23-orig");
    assert!(
        !struct_obj.is_nil(),
        "(ert-get-test 'r23-orig) returned nil — r23-orig not registered?",
    );
    let body = interp
        .eval(read("(ert-test-body (ert-get-test 'r23-orig))").unwrap())
        .expect("ert-test-body");
    assert!(
        !body.is_nil(),
        "(ert-test-body ...) returned nil — struct has empty body slot",
    );

    // Execute the body form directly — this is the inner chain
    // `(ert-get-test ...)` → `(ert-test-body ...)` → `funcall` that
    // tramp wrapper macros rely on. If any step signals
    // `wrong-type-argument: (ert-test ...)` we fail; if it returns
    // cleanly we pass. We inline the exact shape here (rather than
    // funcalling `r23-wrapper`'s stored closure) because the global
    // obarray is shared across `#[test]` functions and a concurrent
    // test calling `run_rele_ert_tests_detailed` can clear the
    // `ert--rele-test` slot we'd otherwise read.
    let outcome = interp.eval(
        read(
            "(let ((ert-test (ert-get-test 'r23-orig))) \
               (when ert-test \
                 (funcall (ert-test-body ert-test))))",
        )
        .unwrap(),
    );
    match outcome {
        Ok(_) => {}
        Err(rele_elisp::ElispError::Signal(sig)) => {
            let data_str = sig.data.prin1_to_string();
            assert!(
                !data_str.contains("ert-test"),
                "wrapper signalled (ert-test ...): {data_str}",
            );
            panic!(
                "unexpected signal: {} {}",
                sig.symbol.prin1_to_string(),
                data_str
            );
        }
        Err(e) => panic!("wrapper errored: {e:?}"),
    }
}

/// erc-scenarios-common--make-bindings reduction: tests outside of a
/// running ert context see `(ert-running-test)` == nil and accessors
/// on nil return nil, so `(memq :X (ert-test-tags (ert-running-test)))`
/// resolves to nil cleanly.
#[test]
fn r23_ert_running_test_nil_outside_run() {
    let interp = fresh_interp();
    let running = interp
        .eval(read("(ert-running-test)").unwrap())
        .expect("ert-running-test");
    assert!(
        running.is_nil(),
        "running-test should be nil outside a run, got {running:?}",
    );

    // The chain that blew up in the baseline. Must return nil, not signal.
    let memq = interp
        .eval(read("(memq :erc--graphical (ert-test-tags (ert-running-test)))").unwrap())
        .expect("chained lookup must not signal");
    assert!(memq.is_nil(), "chain should be nil: got {memq:?}");
}

/// End-to-end through the real runner: register a test whose BODY
/// calls `(ert-running-test)`; the result must be a non-nil ert-test
/// whose name matches, and whose tags contain the declared tag.
///
/// The assertion lives inside the test body via `(should ...)` so we
/// don't have to worry about how setq/defvar interact with the
/// closure captured by R19.
#[test]
fn r23_running_test_visible_inside_body() {
    use rele_elisp::eval::bootstrap::run_rele_ert_tests_detailed;

    let interp = fresh_interp();
    interp
        .eval(
            read(
                "(ert-deftest r23-introspects () \
                   :tags '(:erc--graphical) \
                   (should (eq (ert-test-name (ert-running-test)) \
                               'r23-introspects)) \
                   (should (memq :erc--graphical \
                                 (ert-test-tags (ert-running-test)))))",
            )
            .unwrap(),
        )
        .expect("register");

    let (_stats, results) = run_rele_ert_tests_detailed(&interp);
    // The global obarray is shared across `#[test]` functions, so
    // results may include tests registered by parallel tests. Assert
    // only on our specific entry.
    let ours = results
        .iter()
        .find(|r| r.name == "r23-introspects")
        .unwrap_or_else(|| {
            panic!(
                "r23-introspects not in results: {:?}",
                results
                    .iter()
                    .map(|r| (r.name.clone(), r.result.to_string()))
                    .collect::<Vec<_>>(),
            )
        });
    assert_eq!(
        ours.result, "pass",
        "r23-introspects did not pass: result={} detail={}",
        ours.result, ours.detail,
    );
}

/// `(ert-running-test)` must return to nil after the runner finishes,
/// so that tests registered later don't see a stale struct if they
/// happen to introspect before their own body runs.
#[test]
fn r23_running_test_cleared_after_runner_finishes() {
    use rele_elisp::eval::bootstrap::run_rele_ert_tests_detailed;

    let interp = fresh_interp();
    interp
        .eval(read("(ert-deftest r23-clear () (should t))").unwrap())
        .expect("register");
    let _ = run_rele_ert_tests_detailed(&interp);

    let running = interp
        .eval(read("(ert-running-test)").unwrap())
        .expect("ert-running-test");
    assert!(
        running.is_nil(),
        "running-test should be nil after run, got {running:?}",
    );
}
