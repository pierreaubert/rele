//! R9 regression: `cl-defmacro` destructuring parameter lists must bind
//! inner symbols so the macro body can unquote them.
//!
//! Background: 61 `files-tests-file-name-non-special-*` tests in Emacs
//! ERT failed with "void variable: name" because
//! `files-tests--with-temp-non-special` is a `cl-defmacro` whose first
//! formal parameter is a destructured list:
//!
//! ```elisp
//! (cl-defmacro files-tests--with-temp-non-special
//!     ((name non-special-name &optional dir-flag) &rest body)
//!   ...
//!   `(let* (...
//!           (,name (make-temp-file "files-tests" ,dir-flag))
//!           (,non-special-name (file-name-quote ,name)))
//!      ...))
//! ```
//!
//! The macro expander previously treated the inner list `(name ...)` as
//! an unsupported shape and consumed the positional slot with a SKIP
//! sentinel, never binding `name`/`non-special-name`. The backquote
//! body's `,name` / `,non-special-name` therefore raised
//! `void variable: name` at expansion time.
//!
//! The fix implements recursive destructuring of sub-lambda-lists in
//! `expand_macro` (crates/elisp/src/eval/special_forms.rs).

use rele_elisp::{Interpreter, LispObject, add_primitives, primitives_modules};

fn make_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    primitives_modules::register(&mut interp);
    interp
}

#[test]
fn test_cl_defmacro_destructures_list_param() {
    // Minimal reduction of `files-tests--with-temp-non-special`:
    // the first formal is a destructuring pattern that binds two
    // symbols the backquoted body unquotes.
    let interp = make_interp();
    let res = interp.eval_source(
        r#"
(defmacro my-with ((name other) &rest body)
  `(let ((,name 1) (,other 2)) ,@body))
(my-with (a b) (+ a b))
"#,
    );
    assert_eq!(
        res.map_err(|e| format!("{:?}", e)),
        Ok(LispObject::integer(3)),
        "destructuring should bind `name` and `other` to the call-site list elements"
    );
}

#[test]
fn test_cl_defmacro_destructures_with_optional() {
    // Mirror the real macro shape: `((name other &optional flag) &rest body)`.
    let interp = make_interp();
    let res = interp.eval_source(
        r#"
(defmacro my-with ((name other &optional flag) &rest body)
  `(let ((,name 1) (,other 2) (the-flag ,flag)) ,@body))
(my-with (a b) (list a b the-flag))
"#,
    );
    match res {
        Ok(LispObject::Cons(_)) => {}
        other => panic!(
            "expected a proper list result, got {:?}",
            other.map_err(|e| format!("{:?}", e))
        ),
    }
}

#[test]
fn test_cl_defmacro_destructuring_body_has_no_void_name() {
    // The whole point of the bug report: the macro body references
    // `,name` inside a backquote, which must NOT raise
    // `void variable: name`.
    let interp = make_interp();

    // Define a tiny `file-name-quote` stand-in so the generated `let*`
    // body compiles.
    interp.eval_source("(defun my-quote (x) x)").unwrap();

    let res = interp.eval_source(
        r#"
(defmacro files-tests--with-temp-non-special-lite
    ((name non-special-name &optional dir-flag) &rest body)
  `(let* ((,name (list 'temp ',dir-flag))
          (,non-special-name (my-quote ,name)))
     ,@body))
(files-tests--with-temp-non-special-lite (tmpfile nospecial)
  (list tmpfile nospecial))
"#,
    );

    let err_detail = match &res {
        Err(e) => format!("{:?}", e),
        Ok(_) => String::new(),
    };
    assert!(
        res.is_ok(),
        "expected macro to expand and evaluate without \"void variable: name\" \
         (got {})",
        err_detail
    );
    assert!(
        !err_detail.contains("VoidVariable(\"name\")"),
        "regression: macro destructuring still leaves `name` unbound: {}",
        err_detail
    );
}
