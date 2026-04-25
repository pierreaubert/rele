//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

#[allow(unused_imports)]
use super::*;

#[test]
fn test_puthash_mutates_in_place() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
        .eval(read("(let ((h (make-hash-table :test 'equal))) (puthash \"key\" 42 h) (gethash \"key\" h))")
        .unwrap()).unwrap(), LispObject::integer(42)
    );
}
#[test]
fn test_apply_variadic() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(apply '+ '(1 2 3))").unwrap()).unwrap(),
        LispObject::integer(6)
    );
    assert_eq!(
        interp
            .eval(read("(apply '+ 10 20 '(1 2 3))").unwrap())
            .unwrap(),
        LispObject::integer(36)
    );
    assert_eq!(
        interp.eval(read("(apply '+ 100 '(1 2))").unwrap()).unwrap(),
        LispObject::integer(103)
    );
    assert_eq!(
        interp.eval(read("(apply '+ 5 '())").unwrap()).unwrap(),
        LispObject::integer(5)
    );
    assert_eq!(
        interp
            .eval(read("(apply 'list 1 2 '(3 4))").unwrap())
            .unwrap(),
        read("(1 2 3 4)").unwrap()
    );
}
#[test]
fn test_split_string_with_separator() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(split-string "hello world")"#).unwrap())
            .unwrap(),
        read(r#"("hello" "world")"#).unwrap()
    );
    assert_eq!(
        interp
            .eval(read(r#"(split-string "a,b,c" ",")"#).unwrap())
            .unwrap(),
        read(r#"("a" "b" "c")"#).unwrap()
    );
    assert_eq!(
        interp
            .eval(read(r#"(split-string ",a,,b," "," t)"#).unwrap())
            .unwrap(),
        read(r#"("a" "b")"#).unwrap()
    );
    assert_eq!(
        interp
            .eval(read(r#"(split-string "a::b" ":")"#).unwrap())
            .unwrap(),
        read(r#"("a" "" "b")"#).unwrap()
    );
    assert_eq!(
        interp
            .eval(read(r#"(split-string "  foo  bar  " nil)"#).unwrap())
            .unwrap(),
        read(r#"("foo" "bar")"#).unwrap()
    );
}
#[test]
fn test_read_from_string_position() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read(r#"(read-from-string "42 rest")"#).unwrap())
        .unwrap();
    assert_eq!(result.first().unwrap(), LispObject::integer(42));
    let end_pos = result.rest().unwrap().as_integer().unwrap();
    assert_eq!(end_pos, 2);
    let result = interp
        .eval(read(r#"(read-from-string "  hello world" 2)"#).unwrap())
        .unwrap();
    assert_eq!(result.first().unwrap(), LispObject::symbol("hello"));
    let end_pos = result.rest().unwrap().as_integer().unwrap();
    assert_eq!(end_pos, 7);
}
#[test]
fn test_intern_soft_returns_nil_for_unknown() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(intern-soft "nonexistent-symbol")"#).unwrap())
            .unwrap(),
        LispObject::nil()
    );
    interp.define("my-var", LispObject::integer(42));
    assert_eq!(
        interp
            .eval(read(r#"(intern-soft "my-var")"#).unwrap())
            .unwrap(),
        LispObject::symbol("my-var")
    );
}
#[test]
fn test_fset_sets_function_cell() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(defun my-original (x) (+ x 10))").unwrap())
        .unwrap();
    interp
        .eval(read("(fset 'my-alias (symbol-function 'my-original))").unwrap())
        .unwrap();
    let result = interp.eval(read("(my-alias 5)").unwrap()).unwrap();
    assert_eq!(result, LispObject::integer(15));
}
/// Regression: S1. `cl-destructuring-bind` used to fall through to
/// the generic funcall path, which evaluated *every* arg including
/// the VAR-LIST `(tag start end)` — producing "void function: tag"
/// across 92 tests in buffer-tests.el. Now handled as a source-level
/// special form that binds positionally like a lambda param list.
#[test]
fn cl_destructuring_bind_flat() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(
            read(
                "(cl-destructuring-bind (a b c) (list 1 2 3) \
                   (+ a b c))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(r, LispObject::integer(6));
    let r = interp
        .eval(
            read(
                "(cl-destructuring-bind (tag start end) (list 'yellow 1 10) \
                   (list tag start end))",
            )
            .unwrap(),
        )
        .unwrap();
    assert!(matches!(r, LispObject::Cons(_)));
    let r = interp
        .eval(
            read(
                "(cl-destructuring-bind (a &optional b c) (list 1) \
                   (list a b c))",
            )
            .unwrap(),
        )
        .unwrap();
    assert!(matches!(r, LispObject::Cons(_)));
    let r = interp
        .eval(
            read(
                "(cl-destructuring-bind (first &rest others) (list 1 2 3 4) \
                   (length others))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(r, LispObject::integer(3));
}
/// Regression: S2. `ert-with-temp-directory NAME BODY` is a macro
/// that binds NAME (unevaluated) to a fresh tempdir path. Previously
/// it fell through to the funcall path, which evaluated NAME as a
/// variable reference — yielding "void variable: NAME" across 278+
/// tests (171 eshell-directory-name + 107 eshell-debug-command-buffer).
/// Now handled as a source-level special form.
#[test]
fn ert_with_temp_directory_binds_name_unevaluated() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(
            read(
                "(ert-with-temp-directory my-dir-name \
                   (stringp my-dir-name))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(r, LispObject::t());
    let r = interp
        .eval(
            read(
                "(ert-with-temp-directory d \
                   (file-directory-p d))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(r, LispObject::t());
}
#[test]
fn ert_with_temp_file_binds_name_unevaluated() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(
            read(
                "(ert-with-temp-file eshell-debug-command-buffer \
                   (stringp eshell-debug-command-buffer))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(r, LispObject::t());
    let r = interp
        .eval(
            read(
                "(ert-with-temp-file f \
                   (file-exists-p f))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(r, LispObject::t());
}
#[test]
fn cl_block_return_from() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-block foo (cl-return-from foo 42) 99)").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::integer(42));
    let r = interp.eval(read("(cl-block foo 1 2 3)").unwrap()).unwrap();
    assert_eq!(r, LispObject::integer(3));
    let r = interp
        .eval(read("(cl-block outer (cl-block inner (cl-return-from outer 7)))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::integer(7));
}
#[test]
fn cl_case_matches_values_and_default() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-case 2 (1 'a) (2 'b) (t 'z))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::symbol("b"));
    let r = interp
        .eval(read("(cl-case 3 ((1 2) 'ab) ((3 4) 'cd) (t 'z))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::symbol("cd"));
    let r = interp
        .eval(read("(cl-case 99 (1 'a) (t 'z))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::symbol("z"));
}
#[test]
fn cl_flet_introduces_local_function() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(
            read(
                "(cl-flet ((double (x) (* x 2))) \
                   (double 21))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(r, LispObject::integer(42));
}
#[test]
fn cl_labels_supports_mutual_recursion() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(
            read(
                "(cl-labels ((evenp (n) (if (= n 0) t (oddp (- n 1)))) \
                             (oddp (n) (if (= n 0) nil (evenp (- n 1))))) \
                   (evenp 10))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(r, LispObject::t());
}
#[test]
fn cl_letf_restores_on_exit() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defvar x 1)").unwrap()).unwrap();
    let r = interp.eval(read("(cl-letf ((x 99)) x)").unwrap()).unwrap();
    assert_eq!(r, LispObject::integer(99));
    let r = interp.eval(read("x").unwrap()).unwrap();
    assert_eq!(r, LispObject::integer(1));
}
#[test]
fn cl_the_returns_form_unchanged() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-the integer (+ 1 2))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::integer(3));
}
#[test]
fn cl_assert_signals_on_nil() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let err = interp.eval(read("(cl-assert nil)").unwrap()).unwrap_err();
    match err {
        crate::error::ElispError::Signal(sig) => {
            assert_eq!(
                sig.symbol.as_symbol().as_deref(),
                Some("cl-assertion-failed")
            );
        }
        _ => panic!("expected Signal, got {err:?}"),
    }
    let r = interp.eval(read("(cl-assert t)").unwrap()).unwrap();
    assert_eq!(r, LispObject::nil());
}
#[test]
fn cl_check_type_signals_on_wrong_type() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let err = interp
        .eval(read("(cl-check-type \"hi\" integer)").unwrap())
        .unwrap_err();
    match err {
        crate::error::ElispError::Signal(sig) => {
            assert_eq!(
                sig.symbol.as_symbol().as_deref(),
                Some("wrong-type-argument")
            );
        }
        _ => panic!("expected Signal"),
    }
    let r = interp
        .eval(read("(cl-check-type 7 integer)").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::nil());
}
#[test]
fn cl_do_counts_down() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(
            read(
                "(cl-do ((i 0 (+ i 1)) (acc 0 (+ acc i))) \
                   ((= i 5) acc))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(r, LispObject::integer(10));
}
#[test]
fn cl_loop_for_in_collect() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-loop for x in '(1 2 3 4) collect (* x 10))").unwrap())
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
    assert_eq!(items.len(), 4);
    assert_eq!(items[0], LispObject::integer(10));
    assert_eq!(items[3], LispObject::integer(40));
}
#[test]
fn cl_loop_for_from_to_sum() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-loop for i from 1 to 10 sum i)").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::integer(55));
}
#[test]
fn cl_loop_for_from_below() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-loop for i from 0 below 4 collect i)").unwrap())
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
    assert_eq!(items.len(), 4);
    assert_eq!(items[0], LispObject::integer(0));
    assert_eq!(items[3], LispObject::integer(3));
}
#[test]
fn cl_loop_count_and_maximize() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(
            read(
                "(cl-loop for x in '(3 1 4 1 5 9 2 6) \
                   count (> x 3) into big \
                   maximize x into top \
                   finally return (list big top))",
            )
            .unwrap(),
        )
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
    assert_eq!(items[0], LispObject::integer(4));
    assert_eq!(items[1], LispObject::integer(9));
}
#[test]
fn cl_loop_while_breaks_early() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(
            read(
                "(cl-loop for i from 1 to 100 \
                   while (< i 5) \
                   collect i)",
            )
            .unwrap(),
        )
        .unwrap();
    let mut count = 0;
    let mut c = r;
    while let Some((_, cdr)) = c.destructure_cons() {
        count += 1;
        c = cdr;
    }
    assert_eq!(count, 4);
}
#[test]
fn cl_loop_always_returns_t_or_nil() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-loop for x in '(2 4 6) always (= 0 (mod x 2)))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::t());
    let r = interp
        .eval(read("(cl-loop for x in '(2 3 6) always (= 0 (mod x 2)))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::nil());
}
#[test]
fn cl_loop_thereis_returns_value() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-loop for x in '(1 2 3) thereis (and (> x 1) x))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::integer(2));
}
#[test]
fn cl_loop_repeat() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-loop repeat 5 sum 2)").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::integer(10));
}
#[test]
fn cl_find_and_if() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-find 3 '(1 2 3 4))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::integer(3));
    let r = interp
        .eval(read("(cl-find-if (lambda (x) (> x 2)) '(1 2 3 4))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::integer(3));
}
#[test]
fn cl_some_and_every() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-some (lambda (x) (> x 5)) '(1 2 3))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::nil());
    let r = interp
        .eval(read("(cl-some (lambda (x) (> x 2)) '(1 2 3))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::t());
    let r = interp
        .eval(read("(cl-every 'integerp '(1 2 3))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::t());
    let r = interp
        .eval(read("(cl-every 'integerp '(1 2 \"x\"))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::nil());
}
#[test]
fn cl_reduce_accumulates() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let r = interp
        .eval(read("(cl-reduce '+ '(1 2 3 4))").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::integer(10));
    let r = interp
        .eval(read("(cl-reduce '+ '(1 2 3) :initial-value 100)").unwrap())
        .unwrap();
    assert_eq!(r, LispObject::integer(106));
}
