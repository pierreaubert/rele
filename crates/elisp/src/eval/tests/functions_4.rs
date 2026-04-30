#![allow(clippy::manual_checked_ops)]
#![allow(clippy::disallowed_methods)]
//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

pub use super::bootstrap::{
    BOOTSTRAP_FILES, ErtRunStats, ErtTestResult, STDLIB_DIR, emacs_lisp_dir, emacs_source_root,
    ensure_stdlib_files, load_cl_lib, load_file_progress, load_full_bootstrap, load_prerequisites,
    make_stdlib_interp, probe_emacs_file, read_emacs_source, run_rele_ert_tests,
    run_rele_ert_tests_detailed, run_rele_ert_tests_detailed_with_timeout,
};
#[allow(unused_imports)]
use super::*;

#[test]
fn cl_mapcar_pairs_multiple_lists() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.define(
        "buffer-local-toplevel-value",
        LispObject::primitive("buffer-local-toplevel-value"),
    );
    let r = interp
        .eval(read("(cl-mapcar '+ '(1 2 3) '(10 20 30))").unwrap())
        .unwrap();
    let items: Vec<_> = {
        let mut out = Vec::new();
        let mut c = r;
        while let Some((car, cdr)) = c.destructure_cons() {
            out.push(car);
            c = cdr;
        }
        out
    };
    assert_eq!(items[0], LispObject::integer(11));
    assert_eq!(items[2], LispObject::integer(33));
}
#[test]
fn cl_position_member_count_remove() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(cl-position 3 '(1 2 3 4))").unwrap())
            .unwrap(),
        LispObject::integer(2)
    );
    assert_eq!(
        interp
            .eval(read("(cl-count 2 '(1 2 2 3 2))").unwrap())
            .unwrap(),
        LispObject::integer(3)
    );
    let r = interp
        .eval(read("(cl-remove 2 '(1 2 3 2 4))").unwrap())
        .unwrap();
    let items: Vec<_> = {
        let mut out = Vec::new();
        let mut c = r;
        while let Some((car, cdr)) = c.destructure_cons() {
            out.push(car);
            c = cdr;
        }
        out
    };
    assert_eq!(
        items,
        vec![
            LispObject::integer(1),
            LispObject::integer(3),
            LispObject::integer(4)
        ]
    );
    let filtered = interp
        .eval(
            read(
                "(cl-remove-if-not
                   (lambda (x) (eql (length x) 1))
                   '(\"\" \"0\" \"aa\"))",
            )
            .unwrap(),
        )
        .unwrap();
    let filtered_items: Vec<_> = {
        let mut out = Vec::new();
        let mut c = filtered;
        while let Some((car, cdr)) = c.destructure_cons() {
            out.push(car);
            c = cdr;
        }
        out
    };
    assert_eq!(filtered_items, vec![LispObject::string("0")]);
}

#[test]
fn defvar_inside_let_initializes_default_not_dynamic_binding() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(
                "(progn
                   (defvar rele-defvar-let-x)
                   (list
                    (let ((rele-defvar-let-x 1))
                      (defvar rele-defvar-let-x 2)
                      rele-defvar-let-x)
                    rele-defvar-let-x))",
            )
            .unwrap(),
        )
        .unwrap();
    let mut items = Vec::new();
    let mut current = result;
    while let Some((car, cdr)) = current.destructure_cons() {
        items.push(car);
        current = cdr;
    }
    assert_eq!(items, vec![LispObject::integer(1), LispObject::integer(2)]);
}

#[test]
fn default_toplevel_value_signals_void_variable_when_unbound() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let err = interp
        .eval(read("(default-toplevel-value 'rele-unbound-default)").unwrap())
        .unwrap_err();
    assert!(matches!(err, ElispError::VoidVariable(name) if name == "rele-unbound-default"));
}

#[test]
fn make_symbol_is_not_interned() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(eq (make-symbol "rele-uninterned") 'rele-uninterned)"#).unwrap())
            .unwrap(),
        LispObject::nil(),
    );
}

#[test]
fn uninterned_lambda_params_bind_by_identity() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(
                r#"(let* ((param (make-symbol "funs"))
                          (fn (list 'lambda (list param) param)))
                     (funcall fn 42))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(result, LispObject::integer(42));
}

#[test]
fn buffer_local_dynamic_binding_preserves_toplevel_slot() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.define(
        "buffer-local-toplevel-value",
        LispObject::primitive("buffer-local-toplevel-value"),
    );
    let result = interp
        .eval(
            read(
                "(progn
                   (defvar-local rele-buffer-local-dyn 'foo)
                   (setq-local rele-buffer-local-dyn 'bar)
                   (let ((rele-buffer-local-dyn 'baz))
                     (setq-local rele-buffer-local-dyn 'quux)
                     (list rele-buffer-local-dyn
                           (buffer-local-toplevel-value
                            'rele-buffer-local-dyn))))",
            )
            .unwrap(),
        )
        .unwrap();
    let mut items = Vec::new();
    let mut current = result;
    while let Some((car, cdr)) = current.destructure_cons() {
        items.push(car);
        current = cdr;
    }
    assert_eq!(
        items,
        vec![LispObject::symbol("quux"), LispObject::symbol("bar")]
    );
}
#[test]
fn cl_accessors_first_through_tenth() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(cl-first '(a b c))").unwrap()).unwrap(),
        LispObject::symbol("a")
    );
    assert_eq!(
        interp.eval(read("(cl-second '(a b c))").unwrap()).unwrap(),
        LispObject::symbol("b")
    );
    assert_eq!(
        interp.eval(read("(cl-third '(a b c))").unwrap()).unwrap(),
        LispObject::symbol("c")
    );
    assert_eq!(
        interp.eval(read("(cl-fourth '(a b c))").unwrap()).unwrap(),
        LispObject::nil()
    );
}
#[test]
fn cl_gcd_lcm_isqrt() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(cl-gcd 12 18)").unwrap()).unwrap(),
        LispObject::integer(6)
    );
    assert_eq!(
        interp.eval(read("(cl-lcm 4 6)").unwrap()).unwrap(),
        LispObject::integer(12)
    );
    assert_eq!(
        interp.eval(read("(cl-isqrt 100)").unwrap()).unwrap(),
        LispObject::integer(10)
    );
    assert_eq!(
        interp.eval(read("(cl-isqrt 99)").unwrap()).unwrap(),
        LispObject::integer(9)
    );
}
#[test]
fn cl_union_intersection_difference() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-intersection '(1 2 3) '(2 3 4))").unwrap())
        .unwrap();
    let items: Vec<_> = {
        let mut out = Vec::new();
        let mut c = r;
        while let Some((car, cdr)) = c.destructure_cons() {
            out.push(car);
            c = cdr;
        }
        out
    };
    assert_eq!(items.len(), 2);
    assert!(items.contains(&LispObject::integer(2)));
    assert!(items.contains(&LispObject::integer(3)));
    let r = interp
        .eval(read("(cl-subsetp '(1 2) '(1 2 3))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::t());
    let r = interp
        .eval(read("(cl-subsetp '(1 9) '(1 2 3))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::nil());
}
#[test]
fn cl_caddr_and_friends() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(cl-caddr '(a b c d))").unwrap()).unwrap(),
        LispObject::symbol("c")
    );
    assert_eq!(
        interp
            .eval(read("(cl-cadddr '(a b c d))").unwrap())
            .unwrap(),
        LispObject::symbol("d")
    );
    assert_eq!(
        interp.eval(read("(cl-cdddr '(a b c d))").unwrap()).unwrap(),
        read("(d)").unwrap()
    );
    assert_eq!(
        interp
            .eval(read("(cl-caadr '((a b) (c d)))").unwrap())
            .unwrap(),
        LispObject::symbol("c")
    );
}
#[test]
fn cl_predicates_evenp_plusp() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(cl-evenp 4)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(cl-oddp 4)").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp.eval(read("(cl-plusp 3)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(cl-plusp -3)").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp.eval(read("(cl-minusp -0.5)").unwrap()).unwrap(),
        LispObject::t()
    );
}
#[test]
fn cl_list_utilities() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(cl-list* 1 2 3 '(4 5))").unwrap())
            .unwrap(),
        read("(1 2 3 4 5)").unwrap()
    );
    assert_eq!(
        interp
            .eval(read("(cl-revappend '(1 2 3) '(4 5))").unwrap())
            .unwrap(),
        read("(3 2 1 4 5)").unwrap()
    );
    assert_eq!(
        interp.eval(read("(cl-endp nil)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(cl-endp '(1))").unwrap()).unwrap(),
        LispObject::nil()
    );
}
#[test]
fn cl_incf_decf_mutate() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(setq x 5)").unwrap()).unwrap();
    interp.eval(read("(cl-incf x)").unwrap()).unwrap();
    assert_eq!(
        interp.eval(read("x").unwrap()).unwrap(),
        LispObject::integer(6)
    );
    interp.eval(read("(cl-decf x 2)").unwrap()).unwrap();
    assert_eq!(
        interp.eval(read("x").unwrap()).unwrap(),
        LispObject::integer(4)
    );
}
#[test]
fn cl_assoc_with_test() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-assoc 2 '((1 . a) (2 . b) (3 . c)))").unwrap())
        .unwrap();
    assert_eq!(r, read("(2 . b)").unwrap());
    let r = interp
        .eval(read("(cl-rassoc 'b '((1 . a) (2 . b) (3 . c)))").unwrap())
        .unwrap();
    assert_eq!(r, read("(2 . b)").unwrap());
}
#[test]
fn cl_substitute_replaces() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(cl-substitute 99 2 '(1 2 3 2 4))").unwrap())
            .unwrap(),
        read("(1 99 3 99 4)").unwrap()
    );
    assert_eq!(
        interp
            .eval(read("(cl-subst 'x 'a '(a (a b) (c a)))").unwrap())
            .unwrap(),
        read("(x (x b) (c x))").unwrap()
    );
}
#[test]
fn cl_tree_equal_recursive() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(cl-tree-equal '(1 (2 3) 4) '(1 (2 3) 4))").unwrap())
            .unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp
            .eval(read("(cl-tree-equal '(1 (2 3) 4) '(1 (2 9) 4))").unwrap())
            .unwrap(),
        LispObject::nil()
    );
}
#[test]
fn cl_merge_sorted() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(cl-merge 'list '(1 3 5) '(2 4 6) '<)").unwrap())
            .unwrap(),
        read("(1 2 3 4 5 6)").unwrap()
    );
}
#[test]
fn cl_search_finds_subseq() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(cl-search '(2 3) '(1 2 3 4))").unwrap())
            .unwrap(),
        LispObject::integer(1)
    );
    assert_eq!(
        interp
            .eval(read("(cl-search '(9) '(1 2 3))").unwrap())
            .unwrap(),
        LispObject::nil()
    );
}
#[test]
fn funcall_nil_signals_void_function() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let err = interp.eval(read("(funcall nil 1 2)").unwrap()).unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("VoidFunction"),
        "expected VoidFunction, got {msg}"
    );
}
#[test]
fn function_form_captures_lexical_env() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(
            read(
                "(let ((y 10))
                   (funcall (function (lambda (x) (+ x y))) 5))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(r, LispObject::integer(15));
}

#[test]
fn lexical_closure_setq_persists_across_calls() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(
            read(
                "(let ((x 0))
                   (let ((f (function (lambda () (setq x (+ x 1)) x))))
                     (list (funcall f) (funcall f) x)))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        r,
        LispObject::cons(
            LispObject::integer(1),
            LispObject::cons(
                LispObject::integer(2),
                LispObject::cons(LispObject::integer(2), LispObject::nil()),
            ),
        )
    );
}

#[test]
fn cl_typep_recognizes_common_types() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(cl-typep 3 'integer)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(cl-typep 3 'string)").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp
            .eval(read("(cl-typep '(1 2) 'list)").unwrap())
            .unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(cl-typep nil 'list)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(cl-typep 1 'number)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp
            .eval(read("(cl-typep 1.5 'number)").unwrap())
            .unwrap(),
        LispObject::t()
    );
}
/// `cl-multiple-value-bind` — wraps a single value in a list and
/// applies the same positional-bind semantics.
#[test]
fn cl_multiple_value_bind_single_value() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(
            read(
                "(cl-multiple-value-bind (a) (+ 40 2) \
                   a)",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(r, LispObject::integer(42));
}
#[test]
fn test_cl_defstruct_basic() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(cl-defstruct point x y z)").unwrap())
        .unwrap();
    let p = interp.eval(read("(make-point 1 2 3)").unwrap()).unwrap();
    assert!(matches!(p, LispObject::Vector(_)));
    assert_eq!(
        interp
            .eval(read("(point-p (make-point 1 2 3))").unwrap())
            .unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(point-p '(1 2 3))").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp
            .eval(read("(point-x (make-point 10 20 30))").unwrap())
            .unwrap(),
        LispObject::integer(10)
    );
    assert_eq!(
        interp
            .eval(read("(point-z (make-point 10 20 30))").unwrap())
            .unwrap(),
        LispObject::integer(30)
    );
}
#[test]
fn test_cl_defstruct_with_options() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(
            read(
                r#"(cl-defstruct (my-struct (:copier nil))
                         "A struct with a docstring."
                         field-a field-b)"#,
            )
            .unwrap(),
        )
        .unwrap();
    let s = interp
        .eval(read("(my-struct-field-b (make-my-struct 100 200))").unwrap())
        .unwrap();
    assert_eq!(s, LispObject::integer(200));
}
#[test]
fn test_cl_defstruct_type_vector_setf_accessor() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(
                r#"(progn
                     (cl-defstruct (timer
                                    (:constructor timer--create ())
                                    (:type vector)
                                    (:conc-name timer--))
                       (triggered t)
                       high-seconds low-seconds usecs)
                     (let ((timer (timer--create)))
                       (setf (timer--high-seconds timer) 12)
                       (list (length timer)
                             (timer--triggered timer)
                             (timer--high-seconds timer))))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(result.princ_to_string(), "(4 t 12)",);
}
#[test]
fn test_push_pop_generated_symbol_place() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(
                r#"(progn
                     (defmacro rele-with-generated-list ()
                       (let ((items (make-symbol "items"))
                             (first (make-symbol "first"))
                             (second (make-symbol "second")))
                         `(let ((,items nil)
                                (,first 'a)
                                (,second 'b))
                            (push ,first ,items)
                            (push ,second ,items)
                            (list (length ,items)
                                  (pop ,items)
                                  (length ,items)))))
                     (rele-with-generated-list))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(result.princ_to_string(), "(2 b 1)");
}
#[test]
fn test_defun_gv_setter_declaration() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(
                r#"(progn
                     (defun rele-test-time (obj)
                       (declare (gv-setter rele-test-set-time))
                       (aref obj 0))
                     (defun rele-test-set-time (obj value)
                       (aset obj 0 value)
                       value)
                     (let ((obj (vector nil)))
                       (setf (rele-test-time obj) 42)
                       (rele-test-time obj)))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(result, LispObject::integer(42));
}
#[test]
fn test_defalias_creates_function_alias() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defun my-fn () 42)").unwrap()).unwrap();
    interp
        .eval(read("(defalias 'my-alias-fn 'my-fn)").unwrap())
        .unwrap();
    let result = interp.eval(read("(my-alias-fn)").unwrap()).unwrap();
    assert_eq!(result, LispObject::integer(42));
}
/// Ensure all required stdlib files are decompressed to STDLIB_DIR/.
/// Returns false if Emacs lisp directory is not available (skip tests).
/// All files in loadup.el order that ensure_stdlib_files will decompress.
/// Load the standard prerequisite files into an interpreter.
#[test]
fn test_load_macroexp_el() {
    if !ensure_stdlib_files() {
        return;
    }
    let source = match std::fs::read_to_string(format!("{STDLIB_DIR}/emacs-lisp/macroexp.el")) {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    load_prerequisites(&interp);
    let (ok, total, errors) = load_file_progress(&interp, &source);
    let pct = if total > 0 { ok * 100 / total } else { 0 };
    eprintln!("macroexp.el: {}/{} OK ({}%)", ok, total, pct);
    for (i, e) in &errors {
        eprintln!("  form {}: {}", i, e);
    }
}
#[test]
fn test_load_cconv_el() {
    if !ensure_stdlib_files() {
        return;
    }
    let source = match std::fs::read_to_string(format!("{STDLIB_DIR}/emacs-lisp/cconv.el")) {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    load_prerequisites(&interp);
    if let Ok(s) = std::fs::read_to_string(format!("{STDLIB_DIR}/emacs-lisp/macroexp.el")) {
        let _ = interp.eval_source(&s);
    }
    let (ok, total, errors) = load_file_progress(&interp, &source);
    let pct = if total > 0 { ok * 100 / total } else { 0 };
    eprintln!("cconv.el: {}/{} OK ({}%)", ok, total, pct);
    for (i, e) in &errors {
        eprintln!("  form {}: {}", i, e);
    }
}
#[test]
fn test_load_simple_el() {
    if !ensure_stdlib_files() {
        return;
    }
    let source = match std::fs::read_to_string(format!("{STDLIB_DIR}/simple.el")) {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    load_prerequisites(&interp);
    let (ok, total, errors) = load_file_progress(&interp, &source);
    let pct = if total > 0 { ok * 100 / total } else { 0 };
    eprintln!("simple.el: {}/{} OK ({}%)", ok, total, pct);
    for (i, e) in &errors {
        eprintln!("  form {}: {}", i, e);
    }
}
#[test]
fn test_load_files_el() {
    if !ensure_stdlib_files() {
        return;
    }
    let source = match std::fs::read_to_string(format!("{STDLIB_DIR}/files.el")) {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    load_prerequisites(&interp);
    let (ok, total, errors) = load_file_progress(&interp, &source);
    let pct = if total > 0 { ok * 100 / total } else { 0 };
    eprintln!("files.el: {}/{} OK ({}%)", ok, total, pct);
    for (i, e) in &errors {
        eprintln!("  form {}: {}", i, e);
    }
}
#[test]
fn test_dynamic_binding_basic() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defvar x 10)").unwrap()).unwrap();
    interp.eval(read("(defun get-x () x)").unwrap()).unwrap();
    assert_eq!(
        interp.eval(read("(get-x)").unwrap()).unwrap(),
        LispObject::integer(10)
    );
    assert_eq!(
        interp
            .eval(read("(let ((x 20)) (get-x))").unwrap())
            .unwrap(),
        LispObject::integer(20)
    );
    assert_eq!(
        interp.eval(read("(get-x)").unwrap()).unwrap(),
        LispObject::integer(10)
    );
}
#[test]
fn test_dynamic_binding_nested() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defvar x 1)").unwrap()).unwrap();
    interp.eval(read("(defun get-x () x)").unwrap()).unwrap();
    assert_eq!(
        interp
            .eval(read("(let ((x 10)) (let ((x 100)) (get-x)))").unwrap())
            .unwrap(),
        LispObject::integer(100)
    );
    assert_eq!(
        interp.eval(read("(get-x)").unwrap()).unwrap(),
        LispObject::integer(1)
    );
}
#[test]
fn test_dynamic_binding_setq() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defvar x 5)").unwrap()).unwrap();
    interp.eval(read("(defun get-x () x)").unwrap()).unwrap();
    assert_eq!(
        interp
            .eval(read("(let ((x 10)) (setq x 99) (get-x))").unwrap())
            .unwrap(),
        LispObject::integer(99)
    );
    assert_eq!(
        interp.eval(read("(get-x)").unwrap()).unwrap(),
        LispObject::integer(5)
    );
}
#[test]
fn test_dynamic_binding_let_star() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defvar x 1)").unwrap()).unwrap();
    interp.eval(read("(defun get-x () x)").unwrap()).unwrap();
    assert_eq!(
        interp
            .eval(read("(let* ((x 42)) (get-x))").unwrap())
            .unwrap(),
        LispObject::integer(42)
    );
    assert_eq!(
        interp.eval(read("(get-x)").unwrap()).unwrap(),
        LispObject::integer(1)
    );
}
#[test]
fn test_dynamic_binding_lambda_params() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defvar x 0)").unwrap()).unwrap();
    interp.eval(read("(defun get-x () x)").unwrap()).unwrap();
    interp
        .eval(read("(defun set-x-via-param (x) (get-x))").unwrap())
        .unwrap();
    assert_eq!(
        interp.eval(read("(set-x-via-param 77)").unwrap()).unwrap(),
        LispObject::integer(77)
    );
    assert_eq!(
        interp.eval(read("(get-x)").unwrap()).unwrap(),
        LispObject::integer(0)
    );
}
#[test]
fn test_dynamic_binding_defconst() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(defconst my-const 42)").unwrap())
        .unwrap();
    interp
        .eval(read("(defun get-my-const () my-const)").unwrap())
        .unwrap();
    assert_eq!(
        interp
            .eval(read("(let ((my-const 99)) (get-my-const))").unwrap())
            .unwrap(),
        LispObject::integer(99)
    );
    assert_eq!(
        interp.eval(read("(get-my-const)").unwrap()).unwrap(),
        LispObject::integer(42)
    );
}
