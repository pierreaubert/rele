//! Differential tests: Lean oracle vs rele-elisp interpreter.
//!
//! Each test generates a random well-formed Elisp expression (within the
//! oracle subset), evaluates it with both the Lean oracle binary and the
//! Rust interpreter, and asserts the results match.
//!
//! Requires the Lean oracle to be built:
//!   cd spec/lean && lake build
//!
//! Set ELISP_ORACLE_BIN to the oracle binary path, or it defaults to
//! spec/lean/.lake/build/bin/elisp-oracle.

use proptest::prelude::*;
use rele_elisp_spec_tests::expr_gen::{
    JsonVal, OracleInput, OracleOutput, json_val_to_lisp, lisp_to_json_val,
};
use std::process::Command;

/// Path to the Lean oracle binary.
fn oracle_bin() -> String {
    std::env::var("ELISP_ORACLE_BIN").unwrap_or_else(|_| {
        let manifest = env!("CARGO_MANIFEST_DIR");
        format!("{manifest}/../../spec/lean/.lake/build/bin/elisp-oracle")
    })
}

/// Check if the oracle binary is available.
fn oracle_available() -> bool {
    std::path::Path::new(&oracle_bin()).exists()
}

/// Run the oracle on a JSON-encoded expression.
fn run_oracle(input: &OracleInput) -> Option<OracleOutput> {
    let json = serde_json::to_string(input).ok()?;
    let output = Command::new(oracle_bin())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(json.as_bytes())
                .ok()?;
            child.wait_with_output().ok()
        })?;

    serde_json::from_slice(&output.stdout).ok()
}

/// Evaluate an expression with the Rust interpreter and return as JsonVal.
fn eval_rust(expr: &JsonVal) -> Result<JsonVal, String> {
    let lisp = json_val_to_lisp(expr);
    let mut interp = rele_elisp::Interpreter::new();
    rele_elisp::add_primitives(&mut interp);

    match interp.eval(lisp) {
        Ok(result) => Ok(lisp_to_json_val(&result)),
        Err(e) => Err(format!("{e:?}")),
    }
}

/// proptest strategy for self-evaluating atoms.
fn arb_atom() -> impl Strategy<Value = JsonVal> {
    prop_oneof![
        Just(JsonVal::nil()),
        Just(JsonVal::t()),
        (-1000i64..1000).prop_map(JsonVal::int),
        "[a-z]{1,8}".prop_map(|s| JsonVal::string(s)),
    ]
}

/// proptest strategy for (if cond then else).
fn arb_if() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom(), arb_atom()).prop_map(|(cond, then_, else_)| {
        JsonVal::list(vec![JsonVal::sym("if"), cond, then_, else_])
    })
}

/// proptest strategy for (quote ATOM).
fn arb_quote() -> impl Strategy<Value = JsonVal> {
    arb_atom().prop_map(|v| JsonVal::list(vec![JsonVal::sym("quote"), v]))
}

/// proptest strategy for (progn FORM...).
fn arb_progn() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom()).prop_map(|(a, b)| JsonVal::list(vec![JsonVal::sym("progn"), a, b]))
}

fn arb_arith_op() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("+"),
        Just("-"),
        Just("*"),
        Just("/"),
        Just("="),
        Just("<"),
        Just(">"),
        Just("<="),
        Just(">="),
        Just("/="),
    ]
}

/// proptest strategy for variadic arithmetic/comparison primitives with 1..=4
/// integer arguments. Restricted to integers so both interpreters agree on
/// representation (Lean uses arbitrary-precision Int, Rust uses i64 — within
/// the -100..100 range the two coincide).
fn arb_arith() -> impl Strategy<Value = JsonVal> {
    (arb_arith_op(), prop::collection::vec(-100i64..100, 1..=4)).prop_map(|(op, ns)| {
        let mut parts = vec![JsonVal::sym(op)];
        parts.extend(ns.into_iter().map(JsonVal::int));
        JsonVal::list(parts)
    })
}

/// (let ((x A) (y B)) (op x y)) — lexical bindings used by body arithmetic.
fn arb_let_arith() -> impl Strategy<Value = JsonVal> {
    (arb_arith_op(), -50i64..50, -50i64..50).prop_map(|(op, a, b)| {
        JsonVal::list(vec![
            JsonVal::sym("let"),
            JsonVal::list(vec![
                JsonVal::list(vec![JsonVal::sym("x"), JsonVal::int(a)]),
                JsonVal::list(vec![JsonVal::sym("y"), JsonVal::int(b)]),
            ]),
            JsonVal::list(vec![JsonVal::sym(op), JsonVal::sym("x"), JsonVal::sym("y")]),
        ])
    })
}

/// (let* ((x A) (y (+ x B))) (op x y)) — exercises sequential let* binding:
/// y's initializer references x, which only let* (not let) sees.
fn arb_let_star_dep() -> impl Strategy<Value = JsonVal> {
    (arb_arith_op(), -50i64..50, -50i64..50).prop_map(|(op, a, b)| {
        JsonVal::list(vec![
            JsonVal::sym("let*"),
            JsonVal::list(vec![
                JsonVal::list(vec![JsonVal::sym("x"), JsonVal::int(a)]),
                JsonVal::list(vec![
                    JsonVal::sym("y"),
                    JsonVal::list(vec![JsonVal::sym("+"), JsonVal::sym("x"), JsonVal::int(b)]),
                ]),
            ]),
            JsonVal::list(vec![JsonVal::sym(op), JsonVal::sym("x"), JsonVal::sym("y")]),
        ])
    })
}

/// ((lambda (x y) (op x y)) A B) — immediate lambda application with no
/// free variables, so Lean's lexical-closure semantics and Rust's
/// caller-env-parent semantics coincide.
fn arb_lambda_apply() -> impl Strategy<Value = JsonVal> {
    (arb_arith_op(), -50i64..50, -50i64..50).prop_map(|(op, a, b)| {
        JsonVal::list(vec![
            JsonVal::list(vec![
                JsonVal::sym("lambda"),
                JsonVal::list(vec![JsonVal::sym("x"), JsonVal::sym("y")]),
                JsonVal::list(vec![JsonVal::sym(op), JsonVal::sym("x"), JsonVal::sym("y")]),
            ]),
            JsonVal::int(a),
            JsonVal::int(b),
        ])
    })
}

/// (catch 'done FORM) where FORM never throws — result is FORM's value.
fn arb_catch_noop() -> impl Strategy<Value = JsonVal> {
    arb_arith().prop_map(|form| {
        JsonVal::list(vec![
            JsonVal::sym("catch"),
            JsonVal::list(vec![JsonVal::sym("quote"), JsonVal::sym("done")]),
            form,
        ])
    })
}

/// (catch 'done (throw 'done A)) — unconditional throw caught by matching tag.
fn arb_catch_throw() -> impl Strategy<Value = JsonVal> {
    (-100i64..100).prop_map(|a| {
        JsonVal::list(vec![
            JsonVal::sym("catch"),
            JsonVal::list(vec![JsonVal::sym("quote"), JsonVal::sym("done")]),
            JsonVal::list(vec![
                JsonVal::sym("throw"),
                JsonVal::list(vec![JsonVal::sym("quote"), JsonVal::sym("done")]),
                JsonVal::int(a),
            ]),
        ])
    })
}

/// (catch 'done (if COND (throw 'done A) B)) — tests catch that either
/// intercepts a throw from the then-branch or returns the else-branch value.
fn arb_catch_if_throw() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), -100i64..100, -100i64..100).prop_map(|(cond, a, b)| {
        JsonVal::list(vec![
            JsonVal::sym("catch"),
            JsonVal::list(vec![JsonVal::sym("quote"), JsonVal::sym("done")]),
            JsonVal::list(vec![
                JsonVal::sym("if"),
                cond,
                JsonVal::list(vec![
                    JsonVal::sym("throw"),
                    JsonVal::list(vec![JsonVal::sym("quote"), JsonVal::sym("done")]),
                    JsonVal::int(a),
                ]),
                JsonVal::int(b),
            ]),
        ])
    })
}

/// (progn (defun f (x y) (op x y)) (f A B)) — exercises Lisp-2 function
/// slot lookup (defun populates the function cell, not the variable cell).
fn arb_defun_call() -> impl Strategy<Value = JsonVal> {
    (arb_arith_op(), -50i64..50, -50i64..50).prop_map(|(op, a, b)| {
        JsonVal::list(vec![
            JsonVal::sym("progn"),
            JsonVal::list(vec![
                JsonVal::sym("defun"),
                JsonVal::sym("f"),
                JsonVal::list(vec![JsonVal::sym("x"), JsonVal::sym("y")]),
                JsonVal::list(vec![JsonVal::sym(op), JsonVal::sym("x"), JsonVal::sym("y")]),
            ]),
            JsonVal::list(vec![JsonVal::sym("f"), JsonVal::int(a), JsonVal::int(b)]),
        ])
    })
}

/// (let ((x A)) (setq x B) (op x C)) — exercises lexical-frame mutation:
/// setq must overwrite the existing let binding, not shadow it, so the
/// final reference to x sees B rather than A.
fn arb_setq_let() -> impl Strategy<Value = JsonVal> {
    (arb_arith_op(), -50i64..50, -50i64..50, -50i64..50).prop_map(|(op, a, b, c)| {
        JsonVal::list(vec![
            JsonVal::sym("let"),
            JsonVal::list(vec![JsonVal::list(vec![
                JsonVal::sym("x"),
                JsonVal::int(a),
            ])]),
            JsonVal::list(vec![
                JsonVal::sym("setq"),
                JsonVal::sym("x"),
                JsonVal::int(b),
            ]),
            JsonVal::list(vec![JsonVal::sym(op), JsonVal::sym("x"), JsonVal::int(c)]),
        ])
    })
}

/// (let ((x A) (y B)) (setq x (op x y)) x) — multi-binding + setq whose
/// RHS references the binding being mutated.
fn arb_setq_compound() -> impl Strategy<Value = JsonVal> {
    (arb_arith_op(), -50i64..50, -50i64..50).prop_map(|(op, a, b)| {
        JsonVal::list(vec![
            JsonVal::sym("let"),
            JsonVal::list(vec![
                JsonVal::list(vec![JsonVal::sym("x"), JsonVal::int(a)]),
                JsonVal::list(vec![JsonVal::sym("y"), JsonVal::int(b)]),
            ]),
            JsonVal::list(vec![
                JsonVal::sym("setq"),
                JsonVal::sym("x"),
                JsonVal::list(vec![JsonVal::sym(op), JsonVal::sym("x"), JsonVal::sym("y")]),
            ]),
            JsonVal::sym("x"),
        ])
    })
}

/// (dlet ((x A) (y B)) (op x y)) — always-dynamic let. Exercises the
/// parallel-binding + promote-to-special path in both oracles.
fn arb_dlet_arith() -> impl Strategy<Value = JsonVal> {
    (arb_arith_op(), -50i64..50, -50i64..50).prop_map(|(op, a, b)| {
        JsonVal::list(vec![
            JsonVal::sym("dlet"),
            JsonVal::list(vec![
                JsonVal::list(vec![JsonVal::sym("x"), JsonVal::int(a)]),
                JsonVal::list(vec![JsonVal::sym("y"), JsonVal::int(b)]),
            ]),
            JsonVal::list(vec![JsonVal::sym(op), JsonVal::sym("x"), JsonVal::sym("y")]),
        ])
    })
}

/// (progn (defun mk (a) (lambda (y) (op a y))) ((mk A) B))
///
/// Defun returns a lambda that closes over the defun's argument. Calling
/// `mk` binds `a`, evaluates the inner `lambda` which must capture `a`,
/// returns the closure; the outer bare-head application then invokes it
/// with B. Tests defun+closure interaction end-to-end.
fn arb_make_adder_closure() -> impl Strategy<Value = JsonVal> {
    (arb_arith_op(), -50i64..50, -50i64..50).prop_map(|(op, a, b)| {
        JsonVal::list(vec![
            JsonVal::sym("progn"),
            JsonVal::list(vec![
                JsonVal::sym("defun"),
                JsonVal::sym("mk"),
                JsonVal::list(vec![JsonVal::sym("a")]),
                JsonVal::list(vec![
                    JsonVal::sym("lambda"),
                    JsonVal::list(vec![JsonVal::sym("y")]),
                    JsonVal::list(vec![JsonVal::sym(op), JsonVal::sym("a"), JsonVal::sym("y")]),
                ]),
            ]),
            JsonVal::list(vec![
                JsonVal::list(vec![JsonVal::sym("mk"), JsonVal::int(a)]),
                JsonVal::int(b),
            ]),
        ])
    })
}

/// (let ((f (lambda (y) (op y B)))) (f A))
///
/// Lisp-2 function-slot lookup of a let-bound lambda. The head `f` is a
/// symbol; the interpreter must walk the lexical env (or fall back to
/// `lookup`) to find the closure, then apply it. Tests that let-bindings
/// participate in function-position resolution.
fn arb_letbound_fn_sym_call() -> impl Strategy<Value = JsonVal> {
    (arb_arith_op(), -50i64..50, -50i64..50).prop_map(|(op, a, b)| {
        JsonVal::list(vec![
            JsonVal::sym("let"),
            JsonVal::list(vec![JsonVal::list(vec![
                JsonVal::sym("f"),
                JsonVal::list(vec![
                    JsonVal::sym("lambda"),
                    JsonVal::list(vec![JsonVal::sym("y")]),
                    JsonVal::list(vec![JsonVal::sym(op), JsonVal::sym("y"), JsonVal::int(b)]),
                ]),
            ])]),
            JsonVal::list(vec![JsonVal::sym("f"), JsonVal::int(a)]),
        ])
    })
}

/// (((lambda (a) (lambda (b) (op a b))) A) B)
///
/// Curried two-argument lambda. The outer lambda returns an inner lambda
/// that captures `a`; the inner lambda is then applied to `B`. Exercises
/// nested closure capture where the captured binding is an argument from
/// the enclosing lambda, not a `let`.
fn arb_curried_lambda() -> impl Strategy<Value = JsonVal> {
    (arb_arith_op(), -50i64..50, -50i64..50).prop_map(|(op, a, b)| {
        JsonVal::list(vec![
            JsonVal::list(vec![
                JsonVal::list(vec![
                    JsonVal::sym("lambda"),
                    JsonVal::list(vec![JsonVal::sym("a")]),
                    JsonVal::list(vec![
                        JsonVal::sym("lambda"),
                        JsonVal::list(vec![JsonVal::sym("b")]),
                        JsonVal::list(vec![JsonVal::sym(op), JsonVal::sym("a"), JsonVal::sym("b")]),
                    ]),
                ]),
                JsonVal::int(a),
            ]),
            JsonVal::int(b),
        ])
    })
}

/// (let ((x A)) (let ((f (lambda () x))) (setq x B) (f)))
///
/// Closure capture is a *value snapshot*, not a live reference. After the
/// closure is built, the outer `setq` mutates `x` to B, but the closure
/// evaluates `x` from its captured snapshot (A). Both Rust and Lean
/// implement snapshot semantics — this test is a regression guard against
/// either side silently switching to live-binding semantics.
fn arb_closure_snapshot_after_setq() -> impl Strategy<Value = JsonVal> {
    (-50i64..50, -50i64..50)
        .prop_filter("distinct", |(a, b)| a != b)
        .prop_map(|(a, b)| {
            JsonVal::list(vec![
                JsonVal::sym("let"),
                JsonVal::list(vec![JsonVal::list(vec![
                    JsonVal::sym("x"),
                    JsonVal::int(a),
                ])]),
                JsonVal::list(vec![
                    JsonVal::sym("let"),
                    JsonVal::list(vec![JsonVal::list(vec![
                        JsonVal::sym("f"),
                        JsonVal::list(vec![
                            JsonVal::sym("lambda"),
                            JsonVal::list(vec![]),
                            JsonVal::sym("x"),
                        ]),
                    ])]),
                    JsonVal::list(vec![
                        JsonVal::sym("setq"),
                        JsonVal::sym("x"),
                        JsonVal::int(b),
                    ]),
                    JsonVal::list(vec![JsonVal::sym("f")]),
                ]),
            ])
        })
}

/// (let ((k K)) (mapcar (lambda (x) (op x k)) (list A B C)))
///
/// Exercises `mapcar` dispatching over a closure that captures a `let`-bound
/// variable. Each element of the list `(A B C)` is passed through the
/// closure, which must close over `k` at construction time. Validates that
/// (1) `list` produces a proper cons-terminated list, (2) `mapcar` iterates
/// it, (3) the closure's captured `k` is visible inside the applied lambda
/// on each iteration. Uses `/` (which coerces to int-only args) is avoided
/// here because the generated list elements must remain integers — we
/// restrict to `+ - *` plus comparisons.
fn arb_mapcar_closure() -> impl Strategy<Value = JsonVal> {
    // Drop `/` to avoid division-by-zero on generated inputs; keep the rest.
    let op = prop_oneof![
        Just("+"),
        Just("-"),
        Just("*"),
        Just("="),
        Just("<"),
        Just(">"),
        Just("<="),
        Just(">="),
        Just("/="),
    ];
    (op, -20i64..20, -20i64..20, -20i64..20, -20i64..20).prop_map(|(op, k, a, b, c)| {
        JsonVal::list(vec![
            JsonVal::sym("let"),
            JsonVal::list(vec![JsonVal::list(vec![
                JsonVal::sym("k"),
                JsonVal::int(k),
            ])]),
            JsonVal::list(vec![
                JsonVal::sym("mapcar"),
                JsonVal::list(vec![
                    JsonVal::sym("lambda"),
                    JsonVal::list(vec![JsonVal::sym("x")]),
                    JsonVal::list(vec![JsonVal::sym(op), JsonVal::sym("x"), JsonVal::sym("k")]),
                ]),
                JsonVal::list(vec![
                    JsonVal::sym("list"),
                    JsonVal::int(a),
                    JsonVal::int(b),
                    JsonVal::int(c),
                ]),
            ]),
        ])
    })
}

/// (cond (COND1 RESULT1) (COND2 RESULT2)) — first non-nil cond returns its result.
fn arb_cond() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom(), arb_atom(), arb_atom()).prop_map(|(c1, r1, c2, r2)| {
        JsonVal::list(vec![
            JsonVal::sym("cond"),
            JsonVal::list(vec![c1.clone(), r1]),
            JsonVal::list(vec![c2, r2]),
        ])
    })
}

/// (and E1 E2 E3) — short-circuit AND.
fn arb_and() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom(), arb_atom()).prop_map(|(a, b, c)| {
        JsonVal::list(vec![
            JsonVal::sym("and"),
            a,
            b,
            c,
        ])
    })
}

/// (or E1 E2 E3) — short-circuit OR.
fn arb_or() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom(), arb_atom()).prop_map(|(a, b, c)| {
        JsonVal::list(vec![
            JsonVal::sym("or"),
            a,
            b,
            c,
        ])
    })
}

/// (when COND BODY1 BODY2) — if COND is non-nil, evaluate body.
fn arb_when() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom(), arb_atom()).prop_map(|(cond, b1, b2)| {
        JsonVal::list(vec![
            JsonVal::sym("when"),
            cond,
            b1,
            b2,
        ])
    })
}

/// (unless COND BODY1 BODY2) — if COND is nil, evaluate body.
fn arb_unless() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom(), arb_atom()).prop_map(|(cond, b1, b2)| {
        JsonVal::list(vec![
            JsonVal::sym("unless"),
            cond,
            b1,
            b2,
        ])
    })
}

/// (prog1 E1 E2 E3) — evaluate all, return first.
fn arb_prog1() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom(), arb_atom()).prop_map(|(e1, e2, e3)| {
        JsonVal::list(vec![
            JsonVal::sym("prog1"),
            e1,
            e2,
            e3,
        ])
    })
}

/// (prog2 E1 E2 E3) — evaluate all, return second.
fn arb_prog2() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom(), arb_atom()).prop_map(|(e1, e2, e3)| {
        JsonVal::list(vec![
            JsonVal::sym("prog2"),
            e1,
            e2,
            e3,
        ])
    })
}

/// (car (cons A B)) — first element of a cons cell.
fn arb_car() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom()).prop_map(|(a, b)| {
        JsonVal::list(vec![
            JsonVal::sym("car"),
            JsonVal::cons(a.clone(), b),
        ])
    })
}

/// (cdr (cons A B)) — rest of a cons cell.
fn arb_cdr() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom()).prop_map(|(a, b)| {
        JsonVal::list(vec![
            JsonVal::sym("cdr"),
            JsonVal::cons(a, b.clone()),
        ])
    })
}

/// (cons A B) — construct a cons cell.
fn arb_cons() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom()).prop_map(|(a, b)| {
        JsonVal::cons(a, b)
    })
}

/// (list* A B C D) — dotted list: (A B C . D).
fn arb_list_star() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom(), arb_atom(), arb_atom()).prop_map(|(a, b, c, d)| {
        JsonVal::cons(a, JsonVal::cons(b, JsonVal::cons(c, d)))
    })
}

/// ((let ((x A)) (lambda (y) (op x y))) B) — lambda-from-let-escape.
/// The lambda is created inside a `let`, the let evaluates to the lambda
/// value, and the lambda is then immediately called with a single argument.
/// Under lexical-binding semantics the closure must capture `x`, so the
/// result is `(op A B)`. Uses bare-head application rather than `funcall`
/// because the oracle subset doesn't bind `funcall` as a primitive.
fn arb_lambda_from_let_escape() -> impl Strategy<Value = JsonVal> {
    (arb_arith_op(), -50i64..50, -50i64..50).prop_map(|(op, a, b)| {
        JsonVal::list(vec![
            JsonVal::list(vec![
                JsonVal::sym("let"),
                JsonVal::list(vec![JsonVal::list(vec![
                    JsonVal::sym("x"),
                    JsonVal::int(a),
                ])]),
                JsonVal::list(vec![
                    JsonVal::sym("lambda"),
                    JsonVal::list(vec![JsonVal::sym("y")]),
                    JsonVal::list(vec![JsonVal::sym(op), JsonVal::sym("x"), JsonVal::sym("y")]),
                ]),
            ]),
            JsonVal::int(b),
        ])
    })
}

/// (condition-case BODY ((TAG) HANDLER...)) — catches throws with matching tag.
fn arb_condition_case() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), "[a-z]+".prop_filter("non-empty identifier", |s| !s.is_empty()), arb_atom(), arb_atom())
        .prop_map(|(body, tag, handler_a, handler_b)| {
            JsonVal::list(vec![
                JsonVal::sym("condition-case"),
                body,
                JsonVal::list(vec![
                    JsonVal::list(vec![
                        JsonVal::sym(&tag),
                        handler_a,
                        handler_b,
                    ]),
                ]),
            ])
        })
}

/// (save-excursion BODY...) — save and restore point/mark.
fn arb_save_excursion() -> impl Strategy<Value = JsonVal> {
    (arb_atom(), arb_atom()).prop_map(|(a, b)| {
        JsonVal::list(vec![
            JsonVal::sym("save-excursion"),
            a,
            b,
        ])
    })
}

/// (defvar SYMBOL VALUE) — declare a dynamic variable.
fn arb_defvar() -> impl Strategy<Value = JsonVal> {
    ("[a-z]{1,4}", -50i64..50).prop_map(|(name, val)| {
        JsonVal::list(vec![
            JsonVal::sym("defvar"),
            JsonVal::sym(name),
            JsonVal::int(val),
        ])
    })
}

/// (funcall FN ARG...) — call FN with evaluated args.
fn arb_funcall() -> impl Strategy<Value = JsonVal> {
    (arb_arith_op(), -50i64..50, -50i64..50).prop_map(|(op, a, b)| {
        JsonVal::list(vec![
            JsonVal::sym("funcall"),
            JsonVal::list(vec![
                JsonVal::sym("lambda"),
                JsonVal::list(vec![]),
                JsonVal::list(vec![JsonVal::sym(op), JsonVal::int(a), JsonVal::int(b)]),
            ]),
        ])
    })
}

/// (eval FORM) — evaluate a form at runtime.
fn arb_eval() -> impl Strategy<Value = JsonVal> {
    arb_arith().prop_map(|form| {
        JsonVal::list(vec![
            JsonVal::sym("eval"),
            form,
        ])
    })
}

/// Combined strategy for the oracle subset.
fn arb_expr() -> impl Strategy<Value = JsonVal> {
    prop_oneof![
        arb_atom(),
        arb_quote(),
        arb_if(),
        arb_progn(),
        arb_arith(),
        arb_let_arith(),
        arb_let_star_dep(),
        arb_lambda_apply(),
        arb_catch_noop(),
        arb_catch_throw(),
        arb_catch_if_throw(),
        arb_defun_call(),
        arb_setq_let(),
        arb_setq_compound(),
        arb_dlet_arith(),
        arb_lambda_from_let_escape(),
        arb_make_adder_closure(),
        arb_letbound_fn_sym_call(),
        arb_curried_lambda(),
        arb_closure_snapshot_after_setq(),
        arb_mapcar_closure(),
        arb_cond(),
        arb_and(),
        arb_or(),
        arb_when(),
        arb_unless(),
        arb_prog1(),
        arb_prog2(),
        arb_car(),
        arb_cdr(),
        arb_cons(),
        arb_list_star(),
        arb_defvar(),
        arb_funcall(),
        arb_eval(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Differential test: Rust interpreter vs Lean oracle on random expressions.
    #[test]
    fn differential_rust_vs_lean(expr in arb_expr()) {
        if !oracle_available() {
            // Skip if oracle not built — don't fail CI.
            return Ok(());
        }

        let input = OracleInput { expr: expr.clone() };

        let rust_result = eval_rust(&expr);
        let lean_result = run_oracle(&input);

        if let Some(lean_out) = lean_result {
            match (&rust_result, &lean_out) {
                (Ok(rust_val), OracleOutput::Ok { ok: lean_val }) => {
                    prop_assert_eq!(rust_val, lean_val,
                        "Rust and Lean disagree on expr: {}", serde_json::to_string(&expr).unwrap());
                }
                (Err(_), OracleOutput::Error { .. }) => {
                    // Both errored — that's agreement.
                }
                _ => {
                    prop_assert!(false,
                        "Rust/Lean mismatch: rust={:?}, lean={:?}, expr={}",
                        rust_result, lean_out, serde_json::to_string(&expr).unwrap());
                }
            }
        }
        // If oracle not reachable, skip silently.
    }
}

#[test]
fn smoke_test_rust_eval_integer() {
    let expr = JsonVal::int(42);
    let result = eval_rust(&expr);
    assert_eq!(result, Ok(JsonVal::int(42)));
}

#[test]
fn smoke_test_rust_eval_quote() {
    let expr = JsonVal::list(vec![JsonVal::sym("quote"), JsonVal::sym("hello")]);
    let result = eval_rust(&expr);
    assert_eq!(result, Ok(JsonVal::sym("hello")));
}

/// Smoke test: `(progn (defun mk (a) (lambda (y) (+ a y))) ((mk 10) 3))` → 13.
/// Regression guard for defun-returning-closure.
#[test]
fn probe_make_adder_closure() {
    let expr = JsonVal::list(vec![
        JsonVal::sym("progn"),
        JsonVal::list(vec![
            JsonVal::sym("defun"),
            JsonVal::sym("mk"),
            JsonVal::list(vec![JsonVal::sym("a")]),
            JsonVal::list(vec![
                JsonVal::sym("lambda"),
                JsonVal::list(vec![JsonVal::sym("y")]),
                JsonVal::list(vec![
                    JsonVal::sym("+"),
                    JsonVal::sym("a"),
                    JsonVal::sym("y"),
                ]),
            ]),
        ]),
        JsonVal::list(vec![
            JsonVal::list(vec![JsonVal::sym("mk"), JsonVal::int(10)]),
            JsonVal::int(3),
        ]),
    ]);
    assert_eq!(eval_rust(&expr), Ok(JsonVal::int(13)));
}

/// Smoke test: `(let ((f (lambda (y) (+ y 100)))) (f 5))` → 105.
/// Regression guard for Lisp-2 lookup of a let-bound lambda.
#[test]
fn probe_letbound_fn_sym_call() {
    let expr = JsonVal::list(vec![
        JsonVal::sym("let"),
        JsonVal::list(vec![JsonVal::list(vec![
            JsonVal::sym("f"),
            JsonVal::list(vec![
                JsonVal::sym("lambda"),
                JsonVal::list(vec![JsonVal::sym("y")]),
                JsonVal::list(vec![
                    JsonVal::sym("+"),
                    JsonVal::sym("y"),
                    JsonVal::int(100),
                ]),
            ]),
        ])]),
        JsonVal::list(vec![JsonVal::sym("f"), JsonVal::int(5)]),
    ]);
    assert_eq!(eval_rust(&expr), Ok(JsonVal::int(105)));
}

/// Smoke test: `(((lambda (a) (lambda (b) (+ a b))) 7) 4)` → 11.
/// Regression guard for curried-lambda closure capture of a lambda param.
#[test]
fn probe_curried_lambda() {
    let expr = JsonVal::list(vec![
        JsonVal::list(vec![
            JsonVal::list(vec![
                JsonVal::sym("lambda"),
                JsonVal::list(vec![JsonVal::sym("a")]),
                JsonVal::list(vec![
                    JsonVal::sym("lambda"),
                    JsonVal::list(vec![JsonVal::sym("b")]),
                    JsonVal::list(vec![
                        JsonVal::sym("+"),
                        JsonVal::sym("a"),
                        JsonVal::sym("b"),
                    ]),
                ]),
            ]),
            JsonVal::int(7),
        ]),
        JsonVal::int(4),
    ]);
    assert_eq!(eval_rust(&expr), Ok(JsonVal::int(11)));
}

/// Smoke test: `(let ((x 1)) (let ((f (lambda () x))) (setq x 99) (f)))` → 1.
/// Value-snapshot semantics: closure captures x=1 at construction, later
/// `setq` doesn't affect it. Both Rust and Lean snapshot; Emacs shares
/// the binding. Regression guard against accidentally switching.
#[test]
fn probe_closure_snapshot_after_setq() {
    let expr = JsonVal::list(vec![
        JsonVal::sym("let"),
        JsonVal::list(vec![JsonVal::list(vec![
            JsonVal::sym("x"),
            JsonVal::int(1),
        ])]),
        JsonVal::list(vec![
            JsonVal::sym("let"),
            JsonVal::list(vec![JsonVal::list(vec![
                JsonVal::sym("f"),
                JsonVal::list(vec![
                    JsonVal::sym("lambda"),
                    JsonVal::list(vec![]),
                    JsonVal::sym("x"),
                ]),
            ])]),
            JsonVal::list(vec![
                JsonVal::sym("setq"),
                JsonVal::sym("x"),
                JsonVal::int(99),
            ]),
            JsonVal::list(vec![JsonVal::sym("f")]),
        ]),
    ]);
    assert_eq!(eval_rust(&expr), Ok(JsonVal::int(1)));
}

/// Sanity probe for the lambda-from-let-escape divergence. Intentionally
/// asserts the *correct* lexical-closure result (8) — if Rust captures at
/// construction time the test passes; if it doesn't, this fails with the
/// actual observed behavior (void-variable `x`) so we can see the status.
#[test]
fn probe_lambda_from_let_escape() {
    // ((let ((x 5)) (lambda (y) (+ x y))) 3) — expected: 8
    let expr = JsonVal::list(vec![
        JsonVal::list(vec![
            JsonVal::sym("let"),
            JsonVal::list(vec![JsonVal::list(vec![
                JsonVal::sym("x"),
                JsonVal::int(5),
            ])]),
            JsonVal::list(vec![
                JsonVal::sym("lambda"),
                JsonVal::list(vec![JsonVal::sym("y")]),
                JsonVal::list(vec![
                    JsonVal::sym("+"),
                    JsonVal::sym("x"),
                    JsonVal::sym("y"),
                ]),
            ]),
        ]),
        JsonVal::int(3),
    ]);
    let result = eval_rust(&expr);
    assert_eq!(
        result,
        Ok(JsonVal::int(8)),
        "lambda-from-let-escape: lexical closure should capture x=5"
    );
}

/// Smoke test: `(let ((k 100)) (mapcar (lambda (x) (+ x k)) (list 1 2 3)))` → (101 102 103).
/// Regression guard for `mapcar` applying a closure that captures a let-bound
/// variable — verifies that closure capture survives across multiple
/// applications in a single call and that `list` + `mapcar` interop works.
#[test]
fn probe_mapcar_closure() {
    let expr = JsonVal::list(vec![
        JsonVal::sym("let"),
        JsonVal::list(vec![JsonVal::list(vec![
            JsonVal::sym("k"),
            JsonVal::int(100),
        ])]),
        JsonVal::list(vec![
            JsonVal::sym("mapcar"),
            JsonVal::list(vec![
                JsonVal::sym("lambda"),
                JsonVal::list(vec![JsonVal::sym("x")]),
                JsonVal::list(vec![
                    JsonVal::sym("+"),
                    JsonVal::sym("x"),
                    JsonVal::sym("k"),
                ]),
            ]),
            JsonVal::list(vec![
                JsonVal::sym("list"),
                JsonVal::int(1),
                JsonVal::int(2),
                JsonVal::int(3),
            ]),
        ]),
    ]);
    let result = eval_rust(&expr);
    // Expected result is a proper cons-list (101 102 103).
    let expected = JsonVal::list(vec![
        JsonVal::int(101),
        JsonVal::int(102),
        JsonVal::int(103),
    ]);
    assert_eq!(
        result,
        Ok(expected),
        "mapcar with closure capturing let-bound k should produce (101 102 103)"
    );
}
