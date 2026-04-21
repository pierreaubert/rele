//! R12: Regression tests for the reader bug that caused
//! `minibuf-tests.el::all-completions-*` (and the `try-completion-*` and
//! `test-completion-*` cousins) to fail with `void variable: abba`.
//!
//! Root cause
//! ----------
//! The test fixtures in `minibuf-tests.el` define bindings like
//!   (let* ((+abba (funcall xform-collection '("abc" "abba" "def"))))
//!     (all-completions "a" +abba))
//! Emacs accepts `+abba` as a single symbol (the leading `+` is a symbol
//! constituent, just like `-` in `-init`). Our reader, however, treated
//! the leading `+`/`-` as a standalone sign symbol whenever the following
//! character wasn't a digit or `.`, and discarded the rest — so
//! `+abba` reader-tokenised as two forms: the symbol `+` and the symbol
//! `abba`. Inside the `let*` binding list, that desynchronised the form
//! structure, and subsequent references to `+abba` evaluated `abba` as a
//! free variable → `void variable: abba`.
//!
//! These tests lock in the fix: the reader must read symbols whose names
//! start with `+`/`-` followed by any symbol constituent as a single
//! symbol, and still return the bare `+` / `-` symbol when they stand
//! alone as operators.

use rele_elisp::{Interpreter, LispObject, add_primitives, read, read_all};

fn make_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
}

fn eval_str(interp: &Interpreter, s: &str) -> LispObject {
    let form = read(s).expect("reader error");
    interp.eval(form).expect("eval error")
}

// ---- Reader-level regressions -----------------------------------------------

#[test]
fn reader_plus_alpha_is_single_symbol() {
    let obj = read("+abba").expect("read");
    assert!(obj.is_symbol(), "expected symbol, got {:?}", obj);
    assert_eq!(obj.as_symbol().as_deref(), Some("+abba"));
}

#[test]
fn reader_minus_alpha_is_single_symbol() {
    let obj = read("-init").expect("read");
    assert!(obj.is_symbol());
    assert_eq!(obj.as_symbol().as_deref(), Some("-init"));
}

#[test]
fn reader_standalone_plus_minus_still_operator_symbol() {
    // `+` / `-` alone must still be read as the single-char operator symbol.
    let plus = read("+").expect("read");
    assert!(plus.is_symbol());
    assert_eq!(plus.as_symbol().as_deref(), Some("+"));

    let minus = read("-").expect("read");
    assert!(minus.is_symbol());
    assert_eq!(minus.as_symbol().as_deref(), Some("-"));
}

#[test]
fn reader_plus_digit_still_number() {
    // Signed numbers must not regress.
    let n = read("+42").expect("read");
    assert_eq!(n, LispObject::integer(42));
    let n = read("-10").expect("read");
    assert_eq!(n, LispObject::integer(-10));
    let f = read("+.5").expect("read");
    assert_eq!(f, LispObject::float(0.5));
    let f = read("-.5").expect("read");
    assert_eq!(f, LispObject::float(-0.5));
}

#[test]
fn reader_let_binding_with_plus_prefixed_symbol_is_one_form() {
    // The crash mode: `(+abba ...)` inside a let-binding list must be a
    // single cons `(+abba . rest)`, not two separate tokens.
    let forms = read_all("((+abba 1) (-beta 2))").expect("read_all");
    assert_eq!(forms.len(), 1, "expected 1 outer list form");
    // Outer list of length 2
    let outer = &forms[0];
    let mut count = 0;
    let mut cur = outer.clone();
    while let Some((car, cdr)) = cur.destructure_cons() {
        count += 1;
        // Each entry must itself be a 2-element list whose car is a symbol
        // named "+abba" or "-beta".
        let (inner_car, _) = car.destructure_cons().expect("inner cons");
        assert!(inner_car.is_symbol());
        let name = inner_car.as_symbol().unwrap();
        assert!(
            name == "+abba" || name == "-beta",
            "unexpected binding name: {name}"
        );
        cur = cdr;
    }
    assert_eq!(count, 2);
}

// ---- End-to-end: minibuf-tests style fixtures evaluate without error --------

#[test]
fn let_star_with_plus_abba_binding_evaluates() {
    let interp = make_interp();
    // This is the exact shape of the fixture used in minibuf-tests.el.
    let src = r#"(let* ((abcdef '("abc" "def"))
                        (+abba '("abc" "abba" "def")))
                   (length +abba))"#;
    let out = eval_str(&interp, src);
    assert_eq!(out, LispObject::integer(3));
}

#[test]
fn all_completions_style_list_of_strings() {
    // If user-space provides an all-completions implementation, symbol names
    // like +abba should survive reader → eval intact.
    let interp = make_interp();
    eval_str(
        &interp,
        r#"(defun r12-my-all-completions (prefix coll)
             (let (res)
               (dolist (s coll (nreverse res))
                 (when (and (stringp s)
                            (>= (length s) (length prefix))
                            (string= prefix (substring s 0 (length prefix))))
                   (push s res)))))"#,
    );
    let src = r#"(let* ((+abba '("abc" "abba" "def")))
                   (r12-my-all-completions "a" +abba))"#;
    let out = eval_str(&interp, src);
    // Expect ("abc" "abba")
    let mut items = Vec::new();
    let mut cur = out;
    while let Some((car, cdr)) = cur.destructure_cons() {
        items.push(car.as_string().unwrap().to_string());
        cur = cdr;
    }
    assert_eq!(items, vec!["abc".to_string(), "abba".to_string()]);
}

#[test]
fn symbol_list_fixture_evaluates() {
    // minibuf-tests--strings-to-symbol-list form: list of interned symbols.
    let interp = make_interp();
    let src = r#"(let* ((+abba (mapcar #'intern '("abc" "abba" "def"))))
                   (length +abba))"#;
    let out = eval_str(&interp, src);
    assert_eq!(out, LispObject::integer(3));
}

#[test]
fn symbol_alist_fixture_evaluates() {
    let interp = make_interp();
    // (intern str . N) pairs
    let src = r#"(let* ((+abba (let ((n 0))
                                 (mapcar (lambda (s)
                                           (cons (intern s) (setq n (1+ n))))
                                         '("abc" "abba" "def")))))
                   (length +abba))"#;
    let out = eval_str(&interp, src);
    assert_eq!(out, LispObject::integer(3));
}

#[test]
fn hashtable_with_symbol_keys_fixture_evaluates() {
    let interp = make_interp();
    let src = r#"(let* ((+abba (let ((ht (make-hash-table :test #'equal))
                                      (n 0))
                                  (dolist (s '("abc" "abba" "def") ht)
                                    (puthash (intern s) (setq n (1+ n)) ht)))))
                   (hash-table-count +abba))"#;
    let out = eval_str(&interp, src);
    assert_eq!(out, LispObject::integer(3));
}
