//! R17: Reader-invariant regression tests for the two symbol shapes flagged
//! in R15's void-variable classification (PR #26):
//!
//!   - `-with-timeout-value-` (real name in `timer.el`; 9 hits)
//!   - `ispell-tests--constants/correct-list` (5 hits)
//!
//! Classification summary
//! ----------------------
//! When this stream landed, the reader already tokenised both shapes
//! correctly:
//!
//!   * `-with-timeout-value-`: handled by R12's `'+' | '-'` arm, which
//!     falls through to `read_symbol` when the next char is a symbol
//!     constituent. The trailing `-` is also preserved because `-` is in
//!     `is_symbol_char`, so `read_symbol`'s inner loop keeps consuming.
//!   * `ispell-tests--constants/correct-list`: `/` is already listed in
//!     `is_symbol_char` — `read_symbol` treats it as a constituent the
//!     same as `-` or `_`, so the whole name round-trips.
//!
//! The two "hits" therefore do NOT come from reader-level splitting of
//! these tokens. They come from the evaluator / macro expander further
//! down: `with-timeout` expands to a `let` whose lexical binding needs
//! to survive the macro boundary, and the ispell fixture is a top-level
//! defvar whose init form currently errors (hence the `boundp`-style
//! fix in R15 for similar cases).
//!
//! This file nevertheless exists because these two names are tiny
//! tripwires — it takes a one-character change to `is_symbol_char` or
//! the `'+' | '-'` arm in `reader::Reader::read` to break them, and
//! the R12 corpus doesn't cover the trailing-hyphen or interior-slash
//! shapes explicitly. Locking them in as regression tests is cheap
//! insurance.
//!
//! Preservation
//! ------------
//! We also reassert the full R12 corpus so that nobody accidentally
//! narrows the sign-character fall-through:
//!   - `(read "+abba")` and `(read "-init")` stay single symbols;
//!   - standalone `+` / `-` remain the one-char operator symbols;
//!   - numeric literals `+42`, `-.5` are still numbers, not symbols.

use rele_elisp::{read, read_all, LispObject};

/// Walk a proper list into a flat `Vec<LispObject>`.
fn list_to_vec(obj: &LispObject) -> Vec<LispObject> {
    let mut out = Vec::new();
    let mut cur = obj.clone();
    while let Some((car, cdr)) = cur.destructure_cons() {
        out.push(car);
        cur = cdr;
    }
    out
}

// ---------- Bug A: `-with-timeout-value-` ------------------------------------

#[test]
fn reader_double_hyphen_wrap_single_symbol() {
    // The real name in subr-x.el is `-with-timeout-value-`. The leading
    // AND trailing `-` must survive unharmed.
    let obj = read("-with-timeout-value-").expect("read");
    assert!(obj.is_symbol(), "expected symbol, got {:?}", obj);
    assert_eq!(obj.as_symbol().as_deref(), Some("-with-timeout-value-"));
}

#[test]
fn reader_double_hyphen_wrap_inside_let_binding() {
    // End-to-end shape: a let binding and a reference to the same symbol
    // must parse into exactly (let BINDINGS BODY), where BINDINGS holds
    // 1 entry whose name is the exact `-with-timeout-value-` symbol.
    let obj = read("(let ((-with-timeout-value- 1)) -with-timeout-value-)").expect("read");
    let list = list_to_vec(&obj);
    assert_eq!(list.len(), 3, "expected (let BINDINGS BODY), got {:?}", obj);
    assert_eq!(list[0].as_symbol().as_deref(), Some("let"));

    // BINDINGS should be a 1-element list; the single binding is
    // (-with-timeout-value- 1).
    let bindings = list_to_vec(&list[1]);
    assert_eq!(bindings.len(), 1, "expected 1 binding, got {:?}", list[1]);
    let binding = list_to_vec(&bindings[0]);
    assert_eq!(binding.len(), 2, "expected (sym val), got {:?}", bindings[0]);
    assert_eq!(
        binding[0].as_symbol().as_deref(),
        Some("-with-timeout-value-")
    );

    // BODY: the lone reference to the same symbol.
    assert_eq!(list[2].as_symbol().as_deref(), Some("-with-timeout-value-"));
}

#[test]
fn reader_double_hyphen_trailing_only() {
    // `foo-` (name that just ends in a hyphen) must also round-trip.
    let obj = read("foo-").expect("read");
    assert_eq!(obj.as_symbol().as_deref(), Some("foo-"));
}

// ---------- Bug B: `/` is a symbol constituent -------------------------------

#[test]
fn reader_slash_inside_symbol_is_part_of_name() {
    let obj = read("foo/bar").expect("read");
    assert!(obj.is_symbol(), "expected symbol, got {:?}", obj);
    assert_eq!(obj.as_symbol().as_deref(), Some("foo/bar"));
}

#[test]
fn reader_ispell_constants_correct_list() {
    // The specific symbol that hit the bug: `ispell-tests--constants/correct-list`.
    let obj = read("ispell-tests--constants/correct-list").expect("read");
    assert!(obj.is_symbol(), "expected symbol, got {:?}", obj);
    assert_eq!(
        obj.as_symbol().as_deref(),
        Some("ispell-tests--constants/correct-list")
    );
}

#[test]
fn reader_multiple_slashes_single_symbol() {
    // `a/b/c` has two slashes; still a single symbol. (Standalone `/` is
    // the Emacs division operator only when it stands alone as a token.)
    let forms = read_all("a/b/c").expect("read_all");
    assert_eq!(forms.len(), 1, "expected 1 form, got {:?}", forms);
    assert_eq!(forms[0].as_symbol().as_deref(), Some("a/b/c"));
}

#[test]
fn reader_standalone_slash_still_operator_symbol() {
    // Bare `/` (no adjacent symbol chars) is still the division-operator
    // symbol — same shape as bare `+` / `-`.
    let obj = read("/").expect("read");
    assert!(obj.is_symbol());
    assert_eq!(obj.as_symbol().as_deref(), Some("/"));
}

#[test]
fn reader_slash_inside_list_of_bindings() {
    // End-to-end: (setq foo/bar 1) must parse as 3 elements, with the
    // middle element a single symbol `foo/bar`.
    let obj = read("(setq foo/bar 1)").expect("read");
    let list = list_to_vec(&obj);
    assert_eq!(list.len(), 3, "expected (setq SYM VAL), got {:?}", obj);
    assert_eq!(list[0].as_symbol().as_deref(), Some("setq"));
    assert_eq!(list[1].as_symbol().as_deref(), Some("foo/bar"));
}

// ---------- Preservation: R12 must still hold --------------------------------

#[test]
fn r12_preserved_plus_alpha_single_symbol() {
    let obj = read("+abba").expect("read");
    assert_eq!(obj.as_symbol().as_deref(), Some("+abba"));
}

#[test]
fn r12_preserved_minus_alpha_single_symbol() {
    let obj = read("-init").expect("read");
    assert_eq!(obj.as_symbol().as_deref(), Some("-init"));
}

#[test]
fn r12_preserved_standalone_plus_minus_operator_symbols() {
    assert_eq!(read("+").unwrap().as_symbol().as_deref(), Some("+"));
    assert_eq!(read("-").unwrap().as_symbol().as_deref(), Some("-"));
}

#[test]
fn r12_preserved_signed_integer() {
    let obj = read("+42").expect("read");
    assert!(obj.is_integer(), "expected integer, got {:?}", obj);
    assert_eq!(obj.as_integer(), Some(42));
}

#[test]
fn r12_preserved_signed_float() {
    let obj = read("-.5").expect("read");
    assert!(obj.is_float(), "expected float, got {:?}", obj);
    let f = obj.as_float().expect("as_float");
    assert!((f - -0.5).abs() < 1e-12, "expected -0.5, got {}", f);
}

// ---------- End-to-end integration (read_all over real defvar shapes) --------

#[test]
fn reader_defvar_double_hyphen_symbol_toplevel() {
    // Shape mirrors subr-x.el's actual `(defvar -with-timeout-value- …)`.
    // read_all must emit a single toplevel form whose 2nd element is the
    // full symbol — not split into `-` + `with-timeout-value-`.
    let src = "(defvar -with-timeout-value- nil \"docstring\")";
    let forms = read_all(src).expect("read_all");
    assert_eq!(forms.len(), 1, "expected 1 toplevel form, got {:?}", forms);
    let list = list_to_vec(&forms[0]);
    assert_eq!(list.len(), 4, "expected 4-element defvar, got {:?}", list);
    assert_eq!(list[0].as_symbol().as_deref(), Some("defvar"));
    assert_eq!(list[1].as_symbol().as_deref(), Some("-with-timeout-value-"));
}

#[test]
fn reader_standalone_minus_after_symbol() {
    // `(a -)` must parse as (a -), not (a-) or (a). This keeps `-` as the
    // operator when surrounded by whitespace/delimiters.
    let obj = read("(a -)").expect("read");
    let list = list_to_vec(&obj);
    assert_eq!(list.len(), 2, "got {:?}", obj);
    assert_eq!(list[0].as_symbol().as_deref(), Some("a"));
    assert_eq!(list[1].as_symbol().as_deref(), Some("-"));
}

#[test]
fn reader_defvar_slash_symbol_toplevel() {
    // Shape mirrors ispell-tests' `(defvar ispell-tests--constants/correct-list …)`.
    let src = "(defvar ispell-tests--constants/correct-list '(a b c))";
    let forms = read_all(src).expect("read_all");
    assert_eq!(forms.len(), 1, "expected 1 toplevel form, got {:?}", forms);
    let list = list_to_vec(&forms[0]);
    assert_eq!(list.len(), 3, "expected 3-element defvar, got {:?}", list);
    assert_eq!(list[0].as_symbol().as_deref(), Some("defvar"));
    assert_eq!(
        list[1].as_symbol().as_deref(),
        Some("ispell-tests--constants/correct-list")
    );
}
