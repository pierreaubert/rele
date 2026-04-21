//! R18 — `rx` macro symbol coverage.
//!
//! In round 2 the baseline logged 6 `void variable: bow`, 2 `void variable:
//! bos`, and 2 `void variable: digit` errors, all of which came from real
//! Emacs test files using `(rx bow …)` / `(rx bos …)` / `(rx digit …)`.
//! Root cause: our `make_stdlib_interp()` stubbed `rx` as a *function*
//! (`prim_ignore`) rather than a macro, so the argument list was evaluated
//! before the stub body ran, and the bare symbols raised `void-variable`.
//!
//! R18 registers `rx` as a real `defmacro` that expands (at macroexpansion
//! time, not call time) to a literal regexp string via a small
//! `rele--rx-translate` primitive. The primitive handles the symbols
//! enumerated in this test; everything else collapses to the empty string,
//! which is sufficient for the test files we load (they use `rx` inside
//! top-level `defvar` initialisers whose exact regex does not matter for
//! `boundp` / `skip-unless` guard evaluation).
//!
//! This file asserts the Emacs-regexp strings for every symbol we claim to
//! translate, so a future refactor of `rx_symbol_to_regexp` can't silently
//! break the R18 fix.

use rele_elisp::{Interpreter, LispObject, add_primitives, read};

/// Build a fresh interpreter with `rx` registered as in
/// `make_stdlib_interp`. Mirrors the production call so the tests exercise
/// the same macro definition path.
fn make_rx_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(defmacro rx (&rest forms) (rele--rx-translate forms))").unwrap())
        .expect("defmacro rx should succeed");
    interp
}

/// Evaluate `(rx …)` form text and return the resulting string, or panic
/// with the error if evaluation failed.
fn rx(interp: &Interpreter, form: &str) -> String {
    let obj = interp
        .eval(read(form).unwrap())
        .unwrap_or_else(|e| panic!("eval of `{form}` failed: {e:?}"));
    match obj {
        LispObject::String(s) => s,
        other => panic!("expected string from `{form}`, got {other:?}"),
    }
}

// -------------------------------------------------------------------------
// The target bug: `bow` / `eow` leak as free variables (6 + 0 hits).
// -------------------------------------------------------------------------

#[test]
fn rx_bow_expands_to_word_start_anchor() {
    let interp = make_rx_interp();
    assert_eq!(rx(&interp, "(rx bow)"), "\\<");
}

#[test]
fn rx_eow_expands_to_word_end_anchor() {
    let interp = make_rx_interp();
    assert_eq!(rx(&interp, "(rx eow)"), "\\>");
}

#[test]
fn rx_word_start_is_alias_for_bow() {
    let interp = make_rx_interp();
    assert_eq!(rx(&interp, "(rx word-start)"), "\\<");
}

#[test]
fn rx_word_end_is_alias_for_eow() {
    let interp = make_rx_interp();
    assert_eq!(rx(&interp, "(rx word-end)"), "\\>");
}

#[test]
fn rx_seq_bow_alpha_plus_eow_round_trip() {
    let interp = make_rx_interp();
    // (rx (seq bow (one-or-more alpha) eow)) is the canonical "match a
    // whole word" idiom — a plausible shape in real test code.
    assert_eq!(
        rx(&interp, "(rx (seq bow (one-or-more alpha) eow))"),
        "\\<[[:alpha:]]+\\>"
    );
}

// -------------------------------------------------------------------------
// Buffer / string anchors (accounts for the 2 `bos` hits in round 2).
// -------------------------------------------------------------------------

#[test]
fn rx_bos_is_buffer_start_anchor() {
    let interp = make_rx_interp();
    assert_eq!(rx(&interp, "(rx bos)"), "\\`");
}

#[test]
fn rx_eos_is_buffer_end_anchor() {
    let interp = make_rx_interp();
    assert_eq!(rx(&interp, "(rx eos)"), "\\'");
}

#[test]
fn rx_bol_eol_are_line_anchors() {
    let interp = make_rx_interp();
    assert_eq!(rx(&interp, "(rx bol)"), "^");
    assert_eq!(rx(&interp, "(rx eol)"), "$");
}

// -------------------------------------------------------------------------
// POSIX character classes (accounts for the 2 `digit` hits + commonly used
// siblings). Smoke-tested so a future translator rewrite can't regress.
// -------------------------------------------------------------------------

#[test]
fn rx_character_classes_translate_to_posix_brackets() {
    let interp = make_rx_interp();
    assert_eq!(rx(&interp, "(rx digit)"), "[[:digit:]]");
    assert_eq!(rx(&interp, "(rx alpha)"), "[[:alpha:]]");
    assert_eq!(rx(&interp, "(rx alnum)"), "[[:alnum:]]");
    assert_eq!(rx(&interp, "(rx upper)"), "[[:upper:]]");
    assert_eq!(rx(&interp, "(rx lower)"), "[[:lower:]]");
    assert_eq!(rx(&interp, "(rx space)"), "[[:space:]]");
    assert_eq!(rx(&interp, "(rx blank)"), "[[:blank:]]");
    assert_eq!(rx(&interp, "(rx punct)"), "[[:punct:]]");
    assert_eq!(rx(&interp, "(rx cntrl)"), "[[:cntrl:]]");
}

#[test]
fn rx_word_and_symbol_boundaries() {
    let interp = make_rx_interp();
    assert_eq!(rx(&interp, "(rx word-boundary)"), "\\b");
    assert_eq!(rx(&interp, "(rx not-word-boundary)"), "\\B");
    assert_eq!(rx(&interp, "(rx symbol-start)"), "\\_<");
    assert_eq!(rx(&interp, "(rx symbol-end)"), "\\_>");
}

// -------------------------------------------------------------------------
// Compound forms — sanity check that the translator walks nested lists.
// -------------------------------------------------------------------------

#[test]
fn rx_or_joins_with_emacs_alternation() {
    let interp = make_rx_interp();
    assert_eq!(
        rx(&interp, "(rx (or digit alpha))"),
        "[[:digit:]]\\|[[:alpha:]]"
    );
}

#[test]
fn rx_zero_or_more_and_one_or_more_and_optional() {
    let interp = make_rx_interp();
    assert_eq!(rx(&interp, "(rx (zero-or-more alpha))"), "[[:alpha:]]*");
    assert_eq!(rx(&interp, "(rx (one-or-more digit))"), "[[:digit:]]+");
    assert_eq!(rx(&interp, "(rx (optional upper))"), "[[:upper:]]?");
}

#[test]
fn rx_group_wraps_with_backslash_parens() {
    let interp = make_rx_interp();
    assert_eq!(rx(&interp, "(rx (group alpha))"), "\\([[:alpha:]]\\)");
}

#[test]
fn rx_string_literal_is_regexp_quoted() {
    let interp = make_rx_interp();
    // Meta-chars in a literal string should be escaped.
    assert_eq!(rx(&interp, "(rx \"a.b\")"), "a\\.b");
}

#[test]
fn rx_any_and_nonl_translate_to_dot() {
    let interp = make_rx_interp();
    assert_eq!(rx(&interp, "(rx any)"), ".");
    assert_eq!(rx(&interp, "(rx nonl)"), ".");
}

// -------------------------------------------------------------------------
// The critical property: arguments must NOT be evaluated.
// Before R18 this form raised `void variable: bow`.
// -------------------------------------------------------------------------

#[test]
fn rx_does_not_evaluate_its_symbolic_arguments() {
    let interp = make_rx_interp();
    // If `rx` were a function, this would signal void-variable on `bow`.
    // As a macro it expands to a literal string at read/eval time.
    let result = interp.eval(read("(rx bow (one-or-more digit) eow)").unwrap());
    assert!(
        result.is_ok(),
        "rx must not eagerly evaluate its args; got {result:?}"
    );
    match result.unwrap() {
        LispObject::String(s) => assert_eq!(s, "\\<[[:digit:]]+\\>"),
        other => panic!("expected string, got {other:?}"),
    }
}

#[test]
fn rx_with_unknown_symbol_returns_empty_without_erroring() {
    let interp = make_rx_interp();
    // Scope cap: unknown rx forms must collapse to "", NOT error. Required
    // for test files that use rx forms beyond our narrow symbol table
    // (the surrounding defvar still succeeds).
    let result = interp
        .eval(read("(rx some-unknown-rx-symbol)").unwrap())
        .expect("unknown rx symbol must not raise");
    assert_eq!(result, LispObject::string(""));
}
