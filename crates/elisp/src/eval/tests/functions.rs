//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

#[allow(unused_imports)]
use super::*;

#[test]
fn test_eval_quote_reader() {
    let interp = Interpreter::new();
    assert_eq!(
        interp.eval(read("'foo").unwrap()).unwrap(),
        LispObject::symbol("foo")
    );
    assert_eq!(
        interp.eval(read("'(1 2 3)").unwrap()).unwrap(),
        LispObject::cons(
            LispObject::integer(1),
            LispObject::cons(
                LispObject::integer(2),
                LispObject::cons(LispObject::integer(3), LispObject::nil())
            )
        )
    );
}
#[test]
fn test_car_quote() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(car '(1 2 3))").unwrap()).unwrap(),
        LispObject::integer(1)
    );
}
#[test]
fn test_primitives() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(+ 1 2 3)").unwrap()).unwrap(),
        LispObject::integer(6)
    );
    assert_eq!(
        interp.eval(read("(- 10 3)").unwrap()).unwrap(),
        LispObject::integer(7)
    );
    assert_eq!(
        interp.eval(read("(* 2 3 4)").unwrap()).unwrap(),
        LispObject::integer(24)
    );
    assert_eq!(
        interp.eval(read("(/ 10 2)").unwrap()).unwrap(),
        LispObject::integer(5)
    );
    assert_eq!(
        interp.eval(read("(< 1 2)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(> 2 1)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(= 3 3)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(car '(1 2 3))").unwrap()).unwrap(),
        LispObject::integer(1)
    );
    assert_eq!(
        interp.eval(read("(cdr '(1 2 3))").unwrap()).unwrap(),
        read("(2 3)").unwrap()
    );
    assert_eq!(
        interp.eval(read("(cons 1 '(2 3))").unwrap()).unwrap(),
        read("(1 2 3)").unwrap()
    );
    assert_eq!(
        interp.eval(read("(length '(1 2 3))").unwrap()).unwrap(),
        LispObject::integer(3)
    );
    assert_eq!(
        interp.eval(read("(list 1 2 3)").unwrap()).unwrap(),
        read("(1 2 3)").unwrap()
    );
    assert_eq!(
        interp.eval(read("(not t)").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp.eval(read("(null nil)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(numberp 42)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(symbolp 'foo)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(listp '(1 2))").unwrap()).unwrap(),
        LispObject::t()
    );
}
#[test]
fn test_eval_quote() {
    let interp = Interpreter::new();
    let expr = LispObject::cons(
        LispObject::symbol("quote"),
        LispObject::cons(LispObject::symbol("foo"), LispObject::nil()),
    );
    assert_eq!(interp.eval(expr).unwrap(), LispObject::symbol("foo"));
}
#[test]
fn test_eval_if() {
    let interp = Interpreter::new();
    let expr = LispObject::cons(
        LispObject::symbol("if"),
        LispObject::cons(
            LispObject::t(),
            LispObject::cons(
                LispObject::integer(1),
                LispObject::cons(LispObject::integer(2), LispObject::nil()),
            ),
        ),
    );
    assert_eq!(interp.eval(expr).unwrap(), LispObject::integer(1));
    let expr = LispObject::cons(
        LispObject::symbol("if"),
        LispObject::cons(
            LispObject::nil(),
            LispObject::cons(
                LispObject::integer(1),
                LispObject::cons(LispObject::integer(2), LispObject::nil()),
            ),
        ),
    );
    assert_eq!(interp.eval(expr).unwrap(), LispObject::integer(2));
}
#[test]
fn test_eval_setq() {
    let interp = Interpreter::new();
    let expr = LispObject::cons(
        LispObject::symbol("setq"),
        LispObject::cons(
            LispObject::symbol("x"),
            LispObject::cons(LispObject::integer(42), LispObject::nil()),
        ),
    );
    assert_eq!(interp.eval(expr).unwrap(), LispObject::integer(42));
    assert_eq!(
        interp.eval(LispObject::symbol("x")).unwrap(),
        LispObject::integer(42)
    );
}
#[test]
fn test_eval_let() {
    let interp = Interpreter::new();
    let x = LispObject::symbol("x");
    let ten = LispObject::integer(10);
    let nil = LispObject::nil();
    let binding = LispObject::cons(x.clone(), LispObject::cons(ten.clone(), nil.clone()));
    let bindings = LispObject::cons(binding, nil.clone());
    let body = LispObject::cons(x.clone(), nil);
    let expr = LispObject::cons(LispObject::symbol("let"), LispObject::cons(bindings, body));
    assert_eq!(interp.eval(expr).unwrap(), ten);
}
#[test]
fn test_cond() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(cond ((> 3 2) 'greater) ((< 3 2) 'less))").unwrap())
            .unwrap(),
        LispObject::symbol("greater")
    );
    assert_eq!(
        interp
            .eval(read("(cond ((< 3 2) 'greater) (t 'less))").unwrap())
            .unwrap(),
        LispObject::symbol("less")
    );
    assert_eq!(
        interp
            .eval(read("(cond (nil 'never) (t 'default))").unwrap())
            .unwrap(),
        LispObject::symbol("default")
    );
}
#[test]
fn test_function() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(function (lambda (x) (+ x 1)))").unwrap())
        .unwrap();
    assert!(matches!(result, LispObject::Cons(_)));
}
#[test]
fn test_apply() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(apply '+ '(1 2 3))").unwrap()).unwrap(),
        LispObject::integer(6)
    );
    assert_eq!(
        interp
            .eval(read("(apply 'list '(1 2 3))").unwrap())
            .unwrap(),
        read("(1 2 3)").unwrap()
    );
}
#[test]
fn test_funcall() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(funcall '+ 1 2 3)").unwrap()).unwrap(),
        LispObject::integer(6)
    );
    assert_eq!(
        interp.eval(read("(funcall 'list 1 2 3)").unwrap()).unwrap(),
        read("(1 2 3)").unwrap()
    );
}
#[test]
fn test_string_primitives() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(string= \"hello\" \"hello\")").unwrap())
            .unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp
            .eval(read("(string= \"hello\" \"world\")").unwrap())
            .unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp
            .eval(read("(string< \"apple\" \"banana\")").unwrap())
            .unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp
            .eval(read("(string< \"banana\" \"apple\")").unwrap())
            .unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp
            .eval(read("(concat \"hello\" \" \" \"world\")").unwrap())
            .unwrap(),
        LispObject::string("hello world")
    );
    assert_eq!(
        interp
            .eval(read("(substring \"hello world\" 0 5)").unwrap())
            .unwrap(),
        LispObject::string("hello")
    );
}
#[test]
fn test_prog1() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(prog1 (+ 1 2) (+ 3 4))").unwrap())
            .unwrap(),
        LispObject::integer(3)
    );
}
#[test]
fn test_prog2() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(prog2 (+ 1 2) (+ 3 4))").unwrap())
            .unwrap(),
        LispObject::integer(7)
    );
}
#[test]
fn test_and() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(and t t t)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(and t nil t)").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp.eval(read("(and)").unwrap()).unwrap(),
        LispObject::t()
    );
}
#[test]
fn test_or() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(or nil nil t)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(or nil nil)").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp.eval(read("(or)").unwrap()).unwrap(),
        LispObject::nil()
    );
}
#[test]
fn test_when() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(when t 1 2 3)").unwrap()).unwrap(),
        LispObject::integer(3)
    );
    assert_eq!(
        interp.eval(read("(when nil 1 2 3)").unwrap()).unwrap(),
        LispObject::nil()
    );
}
#[test]
fn test_unless() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(unless nil 1 2 3)").unwrap()).unwrap(),
        LispObject::integer(3)
    );
    assert_eq!(
        interp.eval(read("(unless t 1 2 3)").unwrap()).unwrap(),
        LispObject::nil()
    );
}
#[test]
fn test_eq_atoms() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(eq 'foo 'foo)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(eq 42 42)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(eq nil nil)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(eq t t)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(eq 'foo 'bar)").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp.eval(read("(eq '(1 2) '(1 2))").unwrap()).unwrap(),
        LispObject::nil()
    );
}
#[test]
fn test_div_integer_semantics() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(/ 7 2)").unwrap()).unwrap(),
        LispObject::integer(3)
    );
    assert_eq!(
        interp.eval(read("(/ 10 3)").unwrap()).unwrap(),
        LispObject::integer(3)
    );
    assert_eq!(
        interp.eval(read("(/ 2)").unwrap()).unwrap(),
        LispObject::integer(0)
    );
}
#[test]
fn test_div_by_zero() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert!(interp.eval(read("(/ 1 0)").unwrap()).is_err());
    assert!(interp.eval(read("(/ 0)").unwrap()).is_err());
}
#[test]
fn test_cons_arg_validation() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert!(interp.eval(read("(cons 1 2)").unwrap()).is_ok());
    assert!(interp.eval(read("(cons)").unwrap()).is_err());
}
#[test]
fn test_prog1_eval_order() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(progn (setq x 1) (prog1 x (setq x 2)))").unwrap())
            .unwrap(),
        LispObject::integer(1)
    );
    assert_eq!(
        interp.eval(read("x").unwrap()).unwrap(),
        LispObject::integer(2)
    );
}
#[test]
fn test_macros_per_interpreter() {
    let mut interp1 = Interpreter::new();
    add_primitives(&mut interp1);
    let mut interp2 = Interpreter::new();
    add_primitives(&mut interp2);
    interp1
        .eval(read("(defmacro my-inc (x) (list '+ x 1))").unwrap())
        .unwrap();
    assert_eq!(
        interp1.eval(read("(my-inc 5)").unwrap()).unwrap(),
        LispObject::integer(6)
    );
    assert!(interp2.eval(read("(my-inc 5)").unwrap()).is_err());
}
#[test]
fn test_symbol_cells_per_interpreter() {
    let mut interp1 = Interpreter::new();
    add_primitives(&mut interp1);
    let mut interp2 = Interpreter::new();
    add_primitives(&mut interp2);
    interp1
        .eval(read("(defun isolation-probe () 42)").unwrap())
        .unwrap();
    assert_eq!(
        interp1.eval(read("(isolation-probe)").unwrap()).unwrap(),
        LispObject::integer(42),
    );
    assert!(
        interp2.eval(read("(isolation-probe)").unwrap()).is_err(),
        "function cell leaked between interpreters",
    );
    interp1
        .eval(read("(setq isolation-var 99)").unwrap())
        .unwrap();
    assert_eq!(
        interp1.eval(read("isolation-var").unwrap()).unwrap(),
        LispObject::integer(99),
    );
    assert!(
        interp2.eval(read("isolation-var").unwrap()).is_err()
            || interp2
                .eval(read("isolation-var").unwrap())
                .unwrap()
                .is_nil(),
        "value cell leaked between interpreters",
    );
}

#[test]
fn test_current_keymaps_per_interpreter() {
    let mut interp1 = Interpreter::new();
    add_primitives(&mut interp1);
    let mut interp2 = Interpreter::new();
    add_primitives(&mut interp2);

    interp1
        .eval(read("(global-set-key \"x\" 'cmd-one)").unwrap())
        .unwrap();
    assert_eq!(
        interp1
            .eval(read("(lookup-key (current-global-map) \"x\")").unwrap())
            .unwrap(),
        LispObject::symbol("cmd-one"),
    );
    assert_eq!(
        interp2
            .eval(read("(lookup-key (current-global-map) \"x\")").unwrap())
            .unwrap(),
        LispObject::nil(),
        "current-global-map leaked between interpreters",
    );

    interp2
        .eval(read("(global-set-key \"x\" 'cmd-two)").unwrap())
        .unwrap();
    interp1
        .eval(read("(local-set-key \"x\" 'cmd-local)").unwrap())
        .unwrap();

    assert_eq!(
        interp1.eval(read("(key-binding \"x\")").unwrap()).unwrap(),
        LispObject::symbol("cmd-local"),
    );
    assert_eq!(
        interp2.eval(read("(key-binding \"x\")").unwrap()).unwrap(),
        LispObject::symbol("cmd-two"),
    );
}

#[test]
fn test_while() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
        .eval(read("(progn (setq x 0) (setq sum 0) (while (< x 5) (setq sum (+ sum x)) (setq x (+ x 1))) sum)")
        .unwrap()).unwrap(), LispObject::integer(10)
    );
}
#[test]
fn test_let_star() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(let* ((x 10) (y (+ x 5))) y)").unwrap())
            .unwrap(),
        LispObject::integer(15)
    );
}
#[test]
fn test_defvar() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defvar my-var 42)").unwrap()).unwrap();
    assert_eq!(
        interp.eval(read("my-var").unwrap()).unwrap(),
        LispObject::integer(42)
    );
    interp.eval(read("(defvar my-var 99)").unwrap()).unwrap();
    assert_eq!(
        interp.eval(read("my-var").unwrap()).unwrap(),
        LispObject::integer(42)
    );
}
#[test]
fn test_defconst() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(defconst my-const 42)").unwrap())
        .unwrap();
    assert_eq!(
        interp.eval(read("my-const").unwrap()).unwrap(),
        LispObject::integer(42)
    );
    interp
        .eval(read("(defconst my-const 99)").unwrap())
        .unwrap();
    assert_eq!(
        interp.eval(read("my-const").unwrap()).unwrap(),
        LispObject::integer(99)
    );
}
#[test]
fn test_defalias() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(defun my-add (a b) (+ a b))").unwrap())
        .unwrap();
    interp
        .eval(read("(defalias 'my-plus 'my-add)").unwrap())
        .unwrap();
}
#[test]
fn test_catch_throw_basic() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(catch 'done (throw 'done 42))").unwrap())
            .unwrap(),
        LispObject::integer(42)
    );
}
#[test]
fn test_catch_no_throw() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(catch 'done (+ 1 2))").unwrap()).unwrap(),
        LispObject::integer(3)
    );
}
#[test]
fn test_catch_throw_nested() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(catch 'outer (+ 10 (catch 'inner (throw 'inner 5))))").unwrap())
            .unwrap(),
        LispObject::integer(15)
    );
}
#[test]
fn test_catch_throw_propagates() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(catch 'outer (catch 'inner (throw 'outer 99)))").unwrap())
            .unwrap(),
        LispObject::integer(99)
    );
}
#[test]
fn test_throw_no_catch() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert!(interp.eval(read("(throw 'nothing 42)").unwrap()).is_err());
}
#[test]
fn test_condition_case_no_error() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(condition-case err (+ 1 2) (error 99))").unwrap())
            .unwrap(),
        LispObject::integer(3)
    );
}
#[test]
fn test_condition_case_catches_error() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(condition-case err (error \"boom\") (error 42))").unwrap())
            .unwrap(),
        LispObject::integer(42)
    );
}
#[test]
fn test_condition_case_binds_error() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(condition-case err (error \"boom\") (error (car err)))").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::symbol("error"));
}
#[test]
fn test_condition_case_specific_condition() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(condition-case nil (/ 1 0) (arith-error 42))").unwrap())
            .unwrap(),
        LispObject::integer(42)
    );
    assert_eq!(
        interp
            .eval(read("(condition-case nil undefined-var (void-variable 99))").unwrap())
            .unwrap(),
        LispObject::integer(99)
    );
}
#[test]
fn test_condition_case_no_match() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert!(
        interp
            .eval(read("(condition-case nil (/ 1 0) (void-variable 42))").unwrap())
            .is_err()
    );
}
#[test]
fn test_signal() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(condition-case nil (signal 'my-error '(data)) (my-error 42))").unwrap())
            .unwrap(),
        LispObject::integer(42)
    );
}
#[test]
fn test_unwind_protect_normal() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(progn (setq x 0) (unwind-protect (+ 1 2) (setq x 1)) x)").unwrap())
            .unwrap(),
        LispObject::integer(1)
    );
}
#[test]
fn test_unwind_protect_on_error() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
        .eval(read("(progn (setq cleaned-up nil) (condition-case nil (unwind-protect (error \"boom\") (setq cleaned-up t)) (error nil)) cleaned-up)")
        .unwrap()).unwrap(), LispObject::t()
    );
}
