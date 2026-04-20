//! R13: regression for hierarchy-tests.el errors.
//!
//! Two related buckets of errors in the baseline, both rooted in
//! `cl-defstruct` struct-tagging:
//!
//! 1. **`signal wrong-type-argument: (hierarchy [hierarchy nil ...])`** — 53
//!    errors. `hierarchy.elc`'s byte-compiled predicate expands to
//!    `(memq (type-of cl-x) cl-struct-hierarchy-tags)`. Records are created
//!    with `(record 'hierarchy ...)` (tag = bare symbol `hierarchy`). Our
//!    `type-of` returned `vector` for every `LispObject::Vector`, so the
//!    predicate always failed and every accessor signalled with
//!    `(hierarchy VECTOR)` as the data — matching the baseline detail
//!    exactly.
//!
//! 2. **Custom `:constructor` support** — `hierarchy.el` uses
//!    `(cl-defstruct (hierarchy (:constructor hierarchy--make) (:conc-name hierarchy--)) ...)`.
//!    Our source-level `cl-defstruct` ignored `:constructor` and `:conc-name`,
//!    so `hierarchy--make` / `hierarchy--roots` / `hierarchy--parents` were
//!    all "void function" when the test file used the `.el` source.
//!
//! Fixes (see `crates/elisp/src/eval/mod.rs` and
//! `crates/elisp/src/primitives.rs`):
//! - `prim_type_of`: if the vector's first slot is a symbol that has been
//!   registered as a class, return that symbol.
//! - `eval_cl_defstruct`: register the struct name as a class, use the
//!   bare `NAME` (not `cl-struct-NAME`) as the tag, parse `:constructor` /
//!   `:conc-name` options, and set `cl-struct-NAME-tags` to `(NAME)` so
//!   byte-compiled predicates work.
//! - `setf (cl--find-class NAME)`: also register the class — this is the
//!   path byte-compiled `cl-struct-define` takes.
//! - `cl-check-type` now handles `(satisfies PRED)` (calls PRED via
//!   `eval`), which `cl-struct-define` itself uses to validate names.

use rele_elisp::add_primitives;
use rele_elisp::eval::Interpreter;
use rele_elisp::LispObject;

fn make_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
}

fn eval(interp: &Interpreter, src: &str) -> LispObject {
    interp
        .eval_source(src)
        .unwrap_or_else(|(_i, e)| panic!("eval error in {:?}: {:?}", src, e))
}

/// Plain `(cl-defstruct foo a b)` still works: predicate, constructor,
/// accessors.
#[test]
fn cl_defstruct_basic_predicate_and_accessor() {
    let interp = make_interp();
    eval(&interp, "(cl-defstruct foo a b)");
    let t = eval(&interp, "(foo-p (make-foo 1 2))");
    assert!(!t.is_nil(), "foo-p must recognise its own instance");
    let a = eval(&interp, "(foo-a (make-foo 10 20))");
    assert_eq!(a.as_integer(), Some(10));
    let b = eval(&interp, "(foo-b (make-foo 10 20))");
    assert_eq!(b.as_integer(), Some(20));
}

/// `type-of` on a record must return the tag symbol (the struct name),
/// not `vector`. Baseline behaviour before R13: `type-of` returned
/// `vector`, which broke byte-compiled `.elc` predicates that do
/// `(memq (type-of cl-x) cl-struct-NAME-tags)`.
#[test]
fn type_of_record_returns_tag_symbol() {
    let interp = make_interp();
    eval(&interp, "(cl-defstruct mything a)");
    let t = eval(&interp, "(type-of (make-mything 1))");
    assert_eq!(
        t.as_symbol().as_deref(),
        Some("mything"),
        "type-of on a struct instance must return the struct's tag symbol"
    );
}

/// `type-of` on a plain vector (not a registered struct) must stay
/// `vector`. We don't want to regress other code that checks
/// `(eq (type-of X) 'vector)`.
#[test]
fn type_of_plain_vector_is_vector() {
    let interp = make_interp();
    let t = eval(&interp, "(type-of [1 2 3])");
    assert_eq!(t.as_symbol().as_deref(), Some("vector"));
    // A vector whose first slot is an unregistered symbol stays `vector`.
    let t2 = eval(&interp, "(type-of [not-a-struct-tag 1 2])");
    assert_eq!(t2.as_symbol().as_deref(), Some("vector"));
}

/// The exact hierarchy.el pattern: `(:constructor hierarchy--make)` /
/// `(:conc-name hierarchy--)`. These must install `hierarchy--make`
/// (in addition to `make-hierarchy`) and accessors `hierarchy--roots`
/// (instead of the default `hierarchy-roots`).
#[test]
fn cl_defstruct_constructor_and_conc_name_options() {
    let interp = make_interp();
    eval(
        &interp,
        "(cl-defstruct (hierarchy (:constructor hierarchy--make) \
                                   (:conc-name hierarchy--)) \
           (roots nil) \
           (parents nil))",
    );
    // Custom constructor is defined.
    let t = eval(&interp, "(hierarchy-p (hierarchy--make))");
    assert!(!t.is_nil(), "hierarchy-p must accept hierarchy--make's output");
    // Custom conc-name — the accessor is hierarchy--roots, not hierarchy-roots.
    let t2 = eval(&interp, "(hierarchy--roots (hierarchy--make))");
    assert!(t2.is_nil(), "hierarchy--roots on an empty struct is nil");
}

/// Byte-compiled `.elc` files build records with `(record 'NAME ...)`.
/// After `cl-defstruct` registers the class, such records must be
/// recognised by `NAME-p` (exactly the case that fails in the baseline
/// for hierarchy.el).
#[test]
fn record_constructed_instance_passes_predicate() {
    let interp = make_interp();
    eval(&interp, "(cl-defstruct hier x y)");
    let t = eval(&interp, "(hier-p (record 'hier 1 2))");
    assert!(
        !t.is_nil(),
        "(hier-p (record 'hier ...)) must be non-nil — the record() path \
         is how byte-compiled cl-defstruct constructs instances"
    );
    let v = eval(&interp, "(type-of (record 'hier 1 2))");
    assert_eq!(v.as_symbol().as_deref(), Some("hier"));
}

/// `setf (cl--find-class NAME) ...` — the path byte-compiled
/// `cl-struct-define` in cl-preloaded.el takes — must register NAME
/// so that `type-of` on records tagged NAME returns NAME.
#[test]
fn setf_cl_find_class_registers_record_tag() {
    let interp = make_interp();
    // cl-struct-define would normally pass a class record; our registry
    // just needs the name. The stored value is irrelevant — the side
    // effect of registering is what `type-of` consults.
    eval(&interp, "(setf (cl--find-class 'widget) 'dummy)");
    let t = eval(&interp, "(type-of (record 'widget 1))");
    assert_eq!(
        t.as_symbol().as_deref(),
        Some("widget"),
        "setf (cl--find-class NAME) must register NAME so type-of returns it"
    );
}

/// `cl-check-type` must call the predicate in `(satisfies PRED)`. This
/// was the blocker for loading `cl-struct-define` (which starts with
/// `(cl-check-type name (satisfies cl--struct-name-p))`).
#[test]
fn cl_check_type_satisfies_calls_predicate() {
    let interp = make_interp();
    eval(&interp, "(defun my-pred (x) (and x (symbolp x)))");
    // Success path: no signal, returns nil.
    let r = eval(&interp, "(progn (cl-check-type 'foo (satisfies my-pred)) 'ok)");
    assert_eq!(r.as_symbol().as_deref(), Some("ok"));
    // Failure path: wrong-type-argument signalled.
    let err = make_interp().eval_source(
        "(progn (defun my-pred (x) (and x (symbolp x))) \
                (cl-check-type 42 (satisfies my-pred)))",
    );
    assert!(err.is_err(), "cl-check-type should signal when predicate returns nil");
}

/// The tags list `cl-struct-NAME-tags` must be bound to `(NAME)` after
/// `cl-defstruct`. Byte-compiled predicates `memq` into it.
#[test]
fn cl_struct_tags_variable_is_bound() {
    let interp = make_interp();
    eval(&interp, "(cl-defstruct thing a)");
    let v = eval(&interp, "cl-struct-thing-tags");
    // v should be (thing)
    let first = v.first().unwrap_or(LispObject::nil());
    assert_eq!(first.as_symbol().as_deref(), Some("thing"));
}

/// End-to-end: the minimal hierarchy fixture behaves like Emacs. This
/// is the reduced repro from the baseline bucket of 53 errors.
#[test]
fn hierarchy_fixture_end_to_end() {
    let interp = make_interp();
    // Same as hierarchy.el (truncated to a minimum).
    eval(
        &interp,
        "(cl-defstruct (hierarchy (:constructor hierarchy--make) \
                                   (:conc-name hierarchy--)) \
           (roots nil) \
           (parents nil) \
           (children nil))",
    );
    eval(&interp, "(defun hierarchy-new () (hierarchy--make))");
    // The test helper does this roughly:
    //   (let ((h (hierarchy-new))) (hierarchy-p h))
    let t = eval(&interp, "(hierarchy-p (hierarchy-new))");
    assert!(
        !t.is_nil(),
        "This is the exact shape of the 53 baseline errors — after R13 \
         the predicate must return non-nil"
    );
    // Accessors work (no more `signal wrong-type-argument: (hierarchy ...)`).
    let r = eval(&interp, "(hierarchy--roots (hierarchy-new))");
    assert!(r.is_nil());
}
