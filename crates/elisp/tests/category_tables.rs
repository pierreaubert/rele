use rele_elisp::eval::bootstrap::make_stdlib_interp;
use rele_elisp::{Interpreter, LispObject, add_primitives, read};

fn make_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
}

fn eval(interp: &Interpreter, src: &str) -> LispObject {
    interp.eval(read(src).expect("read ok")).expect("eval ok")
}

#[test]
fn category_table_records_categories_and_docstrings() {
    let interp = make_interp();
    let result = eval(
        &interp,
        r#"(let ((tbl (make-category-table)))
             (define-category ?x "example" tbl)
             (modify-category-entry ?A ?x tbl)
             (and (category-table-p tbl)
                  (equal (category-docstring ?x tbl) "example")
                  (equal (category-set-mnemonics (char-category-set ?A tbl)) "x")))"#,
    );
    assert_eq!(result, LispObject::t());
}

#[test]
fn syntax_tables_answer_ascii_classes_and_overrides() {
    let interp = make_interp();
    let result = eval(
        &interp,
        r#"(let ((tbl (make-syntax-table)))
             (modify-syntax-entry ?_ "w" tbl)
             (and (= (char-syntax ?a) ?w)
                  (= (char-syntax 32) 32)
                  (= (char-syntax ?_ tbl) ?w)
                  (syntax-table-p tbl)))"#,
    );
    assert_eq!(result, LispObject::t());
}

#[test]
fn charset_predicates_and_char_lookup_are_not_ignore_aliases() {
    let interp = make_interp();
    let result = eval(
        &interp,
        r#"(and (charsetp 'ascii)
                (charsetp 'unicode)
                (not (charsetp 'rele-missing-charset))
                (eq (char-charset ?A) 'ascii)
                (eq (char-charset 955) 'unicode)
                (equal (find-charset-string (string ?A 955)) '(ascii unicode)))"#,
    );
    assert_eq!(result, LispObject::t());
}

#[test]
fn stdlib_bootstrap_keeps_category_primitives_registered() {
    let interp = make_stdlib_interp();
    let result = eval(
        &interp,
        r#"(and (charsetp 'ascii)
                (category-table-p (category-table))
                (= (char-syntax ?a) ?w))"#,
    );
    assert_eq!(result, LispObject::t());
}
