//! R7: ERC test-support stubs — integration tests.
//!
//! Verifies that the ERC test-support stubs added by stream R7 to
//! `primitives_modules::register` are actually visible after
//! registration: each stub function is `fboundp`, each stub variable
//! is `boundp`, and each one can be invoked / read without signalling
//! a `void-function` / `void-variable` error.
//!
//! These stubs do NOT implement real ERC behaviour. Their purpose is
//! to flip the ERT baseline from `error` (void function / variable)
//! to `fail` (real assertion mismatch) so downstream streams can work
//! on the actual ERC semantics.
//!
//! Source for the prioritisation: `/tmp/emacs-results-round2-baseline.jsonl`.

use rele_elisp::{ElispError, Interpreter, LispObject, add_primitives, primitives_modules, read};

fn make_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    primitives_modules::register(&mut interp);
    interp
}

fn eval_str(interp: &Interpreter, s: &str) -> Result<LispObject, ElispError> {
    let form = read(s).unwrap();
    interp.eval(form)
}

// ---- macro from R3, re-tested here for coverage -----------------------------

/// `erc-d-t-with-cleanup` must expand its body and run cleanup tail.
/// R3 registered it as a macro that wraps its body in `(progn ...)`.
/// This keeps the ~75-hit test files reachable past the macro call.
#[test]
fn test_r7_erc_d_t_with_cleanup_executes_body() {
    let interp = make_interp();
    // The macro pattern in ERC source is:
    //   (erc-d-t-with-cleanup (BINDINGS) CLEANUP &rest BODY)
    // Our stub just wraps everything in progn, so every form runs.
    let result = eval_str(
        &interp,
        "(erc-d-t-with-cleanup () nil (+ 1 2) (+ 3 4) (+ 10 20))",
    )
    .expect("macro expansion failed");
    assert_eq!(result, LispObject::integer(30));
}

// ---- void-function stubs ----------------------------------------------------

/// Assert that SYMBOL is `fboundp` in INTERP.
fn assert_fboundp(interp: &Interpreter, name: &str) {
    let expr = format!("(fboundp '{name})");
    let result = eval_str(interp, &expr)
        .unwrap_or_else(|e| panic!("fboundp check for {name} errored: {e:?}"));
    assert_eq!(
        result,
        LispObject::t(),
        "{name} should be fboundp after primitives_modules::register",
    );
}

/// Assert that calling SYMBOL with no args returns successfully (any value).
fn assert_callable_zero_args(interp: &Interpreter, name: &str) {
    let expr = format!("({name})");
    eval_str(interp, &expr).unwrap_or_else(|e| panic!("calling ({name}) errored: {e:?}"));
}

#[test]
fn test_r7_erc_mode_stub_fboundp() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-mode");
    assert_callable_zero_args(&interp, "erc-mode");
}

#[test]
fn test_r7_erc_d_u_canned_load_dialog_stub() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-d-u--canned-load-dialog");
    assert_callable_zero_args(&interp, "erc-d-u--canned-load-dialog");
}

#[test]
fn test_r7_erc_nicks_reduce_stub() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-nicks--reduce");
    assert_callable_zero_args(&interp, "erc-nicks--reduce");
}

#[test]
fn test_r7_erc_target_from_string_is_identity() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc--target-from-string");
    // Identity over its first arg.
    let result = eval_str(&interp, "(erc--target-from-string \"#chan\")").unwrap();
    assert_eq!(result, LispObject::string("#chan"));
}

#[test]
fn test_r7_erc_unique_channel_names_stub() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-unique-channel-names");
    assert_callable_zero_args(&interp, "erc-unique-channel-names");
}

#[test]
fn test_r7_erc_sasl_create_client_stub() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-sasl--create-client");
    assert_callable_zero_args(&interp, "erc-sasl--create-client");
}

#[test]
fn test_r7_erc_parse_server_response_stub() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-parse-server-response");
    assert_callable_zero_args(&interp, "erc-parse-server-response");
}

#[test]
fn test_r7_erc_networks_id_fixed_create_stub() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-networks--id-fixed-create");
    assert_callable_zero_args(&interp, "erc-networks--id-fixed-create");
}

#[test]
fn test_r7_erc_keep_place_mode_stub() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-keep-place-mode");
    assert_callable_zero_args(&interp, "erc-keep-place-mode");
}

// ---- erc-d-i family ---------------------------------------------------------

#[test]
fn test_r7_erc_d_i_validate_tags_stub() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-d-i--validate-tags");
    assert_callable_zero_args(&interp, "erc-d-i--validate-tags");
}

#[test]
fn test_r7_erc_d_i_parse_message_stub() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-d-i--parse-message");
    assert_callable_zero_args(&interp, "erc-d-i--parse-message");
}

#[test]
fn test_r7_erc_d_i_unescape_tag_value_is_identity() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-d-i--unescape-tag-value");
    let result = eval_str(&interp, "(erc-d-i--unescape-tag-value \"abc\\\\def\")").unwrap();
    assert_eq!(result, LispObject::string("abc\\def"));
}

#[test]
fn test_r7_erc_d_i_escape_tag_value_is_identity() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-d-i--escape-tag-value");
    let result = eval_str(&interp, "(erc-d-i--escape-tag-value \"xyz\")").unwrap();
    assert_eq!(result, LispObject::string("xyz"));
}

// ---- defvar stub (dumb-server-var, 20 hits) ---------------------------------

#[test]
fn test_r7_dumb_server_var_boundp() {
    let interp = make_interp();
    let boundp = eval_str(&interp, "(boundp 'dumb-server-var)").unwrap();
    assert_eq!(
        boundp,
        LispObject::t(),
        "dumb-server-var should be boundp after primitives_modules::register"
    );
    let v = eval_str(&interp, "(symbol-value 'dumb-server-var)").unwrap();
    assert_eq!(
        v,
        LispObject::nil(),
        "dumb-server-var default should be nil"
    );
}

// ---- existing R3 stubs we rely on (sanity: don't regress) -------------------

#[test]
fn test_r7_existing_erc_networks_id_create_still_there() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-networks--id-create");
    assert_callable_zero_args(&interp, "erc-networks--id-create");
}

#[test]
fn test_r7_existing_make_erc_networks_id_qualifying_still_there() {
    let interp = make_interp();
    assert_fboundp(&interp, "make-erc-networks--id-qualifying");
    assert_callable_zero_args(&interp, "make-erc-networks--id-qualifying");
}

#[test]
fn test_r7_existing_erc_d_u_normalize_canned_name_still_there() {
    let interp = make_interp();
    assert_fboundp(&interp, "erc-d-u--normalize-canned-name");
    assert_callable_zero_args(&interp, "erc-d-u--normalize-canned-name");
}
