//! R20: EIEIO byte-compiled class-definition support.
//!
//! Before R20, `(byte-code ...)` was stubbed to `nil`, which meant every
//! class defined inside a `.elc` top-level compiled form vanished. That
//! produced 22 ERT failures of the shape `invalid-slot-type:
//! (auth-source-backend)` because `(make-instance 'auth-source-backend
//! ...)` could not resolve the class. The fix wires the top-level
//! `byte-code` form through our VM and adds `eieio-defclass-internal`
//! plus `eieio-make-{class,child}-predicate` primitives (the lower-level
//! entry points the byte-compiler lowers `defclass` through).
//!
//! These tests exercise the fix at the interpreter level without pulling
//! in the full stdlib.

use rele_elisp::add_primitives;
use rele_elisp::eval::Interpreter;
use rele_elisp::LispObject;

fn make_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
}

fn eval(src: &str) -> LispObject {
    let interp = make_interp();
    interp
        .eval_source(src)
        .unwrap_or_else(|(_i, e)| panic!("eval error in {src:?}: {e:?}"))
}

/// Reduced `auth-source-backend` shape, defined via the lower-level
/// `eieio-defclass-internal` primitive (which is what `.elc` files
/// lower `defclass` to). After registering the class, `make-instance`
/// must succeed — that's the exact failure mode of the R20 baseline.
#[test]
fn eieio_defclass_internal_registers_class() {
    let result = eval(
        r#"
        (eieio-defclass-internal
          'auth-source-backend
          nil
          '((type :initarg :type :initform 'netrc :type symbol)
            (source :initarg :source :type string)
            (host :initarg :host :initform t :type t)
            (user :initarg :user :initform t :type t)
            (port :initarg :port :initform t :type t)
            (data :initarg :data :initform nil)
            (create-function :initarg :create-function :initform #'ignore :type function)
            (search-function :initarg :search-function :initform #'ignore :type function))
          nil)
        (let ((b (make-instance 'auth-source-backend :source "foo" :type 'netrc)))
          (slot-value b 'source))
        "#,
    );
    assert_eq!(
        result.as_string().map(|s| s.as_str()),
        Some("foo"),
        "make-instance must fill the :source slot"
    );
}

/// `(find-class 'CNAME)` returns a truthy value once the class is
/// registered through `eieio-defclass-internal`.
#[test]
fn eieio_defclass_internal_find_class() {
    let result = eval(
        r#"
        (eieio-defclass-internal 'r20-class nil '((x :initarg :x)) nil)
        (find-class 'r20-class)
        "#,
    );
    assert!(!result.is_nil(), "find-class must see the registered class");
}

/// `:initform` values like `'netrc` (a quoted symbol) are de-quoted
/// so the stored default is the bare symbol `netrc`, not the cons
/// `(quote netrc)`. Matches byte-compiled usage where all initforms
/// arrive as literal data.
#[test]
fn eieio_defclass_internal_dequotes_initforms() {
    let result = eval(
        r#"
        (eieio-defclass-internal
          'r20-dequote nil
          '((kind :initarg :kind :initform 'netrc)) nil)
        (let ((b (make-instance 'r20-dequote)))
          (slot-value b 'kind))
        "#,
    );
    assert_eq!(
        result.as_symbol().as_deref(),
        Some("netrc"),
        "`'netrc` initform must unwrap to the bare `netrc` symbol"
    );
}

/// `eieio-make-class-predicate` returns a unary closure that routes
/// through `object-of-class-p` — so `(CNAME-P obj)` behaves correctly
/// after the usual `(defalias 'CNAME-p (eieio-make-class-predicate ...))`.
#[test]
fn eieio_make_class_predicate_recognises_instances() {
    let interp = make_interp();
    let ok = interp
        .eval_source(
            r#"
        (eieio-defclass-internal 'r20-pred nil '((x :initarg :x)) nil)
        (defalias 'r20-pred-p (eieio-make-class-predicate 'r20-pred))
        (let ((b (make-instance 'r20-pred :x 1)))
          (list (r20-pred-p b) (r20-pred-p 42)))
        "#,
        )
        .unwrap();
    let first = ok.first().unwrap_or(LispObject::nil());
    let second = ok.nth(1).unwrap_or(LispObject::nil());
    assert_eq!(first, LispObject::t(), "instance must match predicate");
    assert_eq!(second, LispObject::nil(), "non-instance must not match");
}

/// Top-level `(byte-code ...)` forms must execute, not silently
/// return nil. This is what `.elc` files at the top of
/// `auth-source.elc` contain — the one-form compiled body that runs
/// all the class-registration and defalias bookkeeping.
///
/// We exercise the wiring with a tiny hand-rolled bytecode: push
/// constant 0 (= 42) then return. Opcodes: `constant 0` = `0xc0`,
/// `return` = `0x87`.
#[test]
fn byte_code_top_level_form_executes() {
    let interp = make_interp();
    let result = interp
        .eval_source(r#"(byte-code "\xc0\x87" [42] 1)"#)
        .unwrap();
    assert_eq!(
        result.as_integer(),
        Some(42),
        "top-level byte-code form must produce its return value"
    );
}
