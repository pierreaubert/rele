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
    json_val_to_lisp, lisp_to_json_val, JsonVal, OracleInput, OracleOutput,
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
    (arb_atom(), arb_atom()).prop_map(|(a, b)| {
        JsonVal::list(vec![JsonVal::sym("progn"), a, b])
    })
}

/// Combined strategy for the oracle subset.
/// NOTE: arb_arith excluded — Lean oracle doesn't implement +/-/* primitives yet.
/// Add it back once the oracle has builtin arithmetic.
fn arb_expr() -> impl Strategy<Value = JsonVal> {
    prop_oneof![arb_atom(), arb_quote(), arb_if(), arb_progn(),]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

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
    let expr = JsonVal::list(vec![
        JsonVal::sym("quote"),
        JsonVal::sym("hello"),
    ]);
    let result = eval_rust(&expr);
    assert_eq!(result, Ok(JsonVal::sym("hello")));
}
