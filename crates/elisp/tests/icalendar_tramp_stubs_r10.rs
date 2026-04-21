//! R10: icalendar + tramp + connection-local stub fixtures.
//!
//! Smoke tests that verify every stub added by round 10 is registered in
//! `make_stdlib_interp()` and can be loaded/evaluated without error.
//! These are not behavioural tests — they only assert that the callable
//! no longer signals `void-function` / `void-variable`.
//!
//! Baseline source: /tmp/emacs-results-round2-baseline.jsonl
//! Only items with >=5 hits in the target namespaces are covered.

use rele_elisp::eval::tests::make_stdlib_interp;
use rele_elisp::{LispObject, read};

/// Helper that reads + evaluates the given source using a stdlib
/// interpreter and asserts it does not error.
fn eval_ok(src: &str) -> LispObject {
    let interp = make_stdlib_interp();
    let expr = read(src).unwrap_or_else(|e| panic!("read failed for {src:?}: {e:?}"));
    interp
        .eval(expr)
        .unwrap_or_else(|e| panic!("eval failed for {src:?}: {e:?}"))
}

// ---------------------------------------------------------------------------
// Function stubs added in crates/elisp/src/primitives_modules.rs (R10 block)
// ---------------------------------------------------------------------------

#[test]
fn r10_ical_make_date_time_callable() {
    // (ical:make-date-time) — 20 hits. Stub returns nil.
    let v = eval_ok("(ical:make-date-time)");
    assert_eq!(v, LispObject::nil());
}

#[test]
fn r10_ical_make_date_time_accepts_args() {
    // Real signature takes several args; stub uses &rest — make sure
    // passing args doesn't error.
    let v = eval_ok("(ical:make-date-time 2026 4 20 12 0)");
    assert_eq!(v, LispObject::nil());
}

#[test]
fn r10_icalendar_unfolded_buffer_from_file_callable() {
    // (icalendar-unfolded-buffer-from-file FILE) — 16 hits. Stub returns nil.
    let v = eval_ok("(icalendar-unfolded-buffer-from-file \"/tmp/missing.ics\")");
    assert_eq!(v, LispObject::nil());
}

// ---------------------------------------------------------------------------
// Variable fixtures added in make_stdlib_interp() R10 block
// ---------------------------------------------------------------------------

#[test]
fn r10_icalendar_parse_component_defined() {
    // icalendar-parse-component — 18 hits. Registered as nil.
    let v = eval_ok("(symbol-value 'icalendar-parse-component)");
    assert_eq!(v, LispObject::nil());
}

#[test]
fn r10_icalendar_parse_calendar_defined() {
    // icalendar-parse-calendar — 7 hits. Registered as nil.
    let v = eval_ok("(symbol-value 'icalendar-parse-calendar)");
    assert_eq!(v, LispObject::nil());
}

#[test]
fn r10_mh_path_defined() {
    // mh-path — 14 hits. Registered as nil.
    let v = eval_ok("(symbol-value 'mh-path)");
    assert_eq!(v, LispObject::nil());
}

#[test]
fn r10_secrets_enabled_defined() {
    // secrets-enabled — 6 hits. Registered as nil.
    let v = eval_ok("(symbol-value 'secrets-enabled)");
    assert_eq!(v, LispObject::nil());
}

// ---------------------------------------------------------------------------
// Pre-existing (previously stubbed) namespace items also re-exercised so
// the R10 namespace coverage is visible in one place.
// ---------------------------------------------------------------------------

#[test]
fn r10_preexisting_auth_source_stubs_still_callable() {
    // auth-source-forget-all-cached (14 hits), auth-source-search (8 hits) —
    // registered in make_stdlib_interp as #'ignore in earlier rounds.
    let _ = eval_ok("(auth-source-forget-all-cached)");
    let _ = eval_ok("(auth-source-search :host \"example.com\")");
}

#[test]
fn r10_preexisting_icalendar_export_region_still_callable() {
    // icalendar-export-region (10 hits) — stubbed in R3.
    let v = eval_ok("(icalendar-export-region 1 1 \"/tmp/none.ics\")");
    assert_eq!(v, LispObject::nil());
}

#[test]
fn r10_preexisting_connection_local_profile_alist_defined() {
    // connection-local-profile-alist (7 hits) — pre-existing fixture.
    let v = eval_ok("(symbol-value 'connection-local-profile-alist)");
    assert_eq!(v, LispObject::nil());
}

#[test]
fn r10_preexisting_tramp_syntax_defined() {
    // tramp-syntax (12 hits) — pre-existing fixture (symbol 'default').
    let v = eval_ok("(symbol-value 'tramp-syntax)");
    assert_eq!(v, LispObject::symbol("default"));
}

#[test]
fn r10_preexisting_tramp_mode_defined() {
    // tramp-mode (8 hits) — pre-existing fixture (t).
    let v = eval_ok("(symbol-value 'tramp-mode)");
    assert_eq!(v, LispObject::t());
}

#[test]
fn r10_preexisting_inhibit_file_name_operation_defined() {
    // inhibit-file-name-operation (16 hits) — pre-existing fixture (nil).
    let v = eval_ok("(symbol-value 'inhibit-file-name-operation)");
    assert_eq!(v, LispObject::nil());
}
