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
fn test_dynamic_binding_mixed_special_and_lexical() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defvar x 10)").unwrap()).unwrap();
    interp
        .eval(read("(defun read-both () (+ x y))").unwrap())
        .unwrap();
    interp.eval(read("(setq y 1)").unwrap()).unwrap();
    assert_eq!(
        interp
            .eval(read("(let ((x 100) (y 200)) (read-both))").unwrap())
            .unwrap(),
        LispObject::integer(300)
    );
    assert_eq!(
        interp.eval(read("x").unwrap()).unwrap(),
        LispObject::integer(10)
    );
}
#[test]
fn test_garbage_collect_returns_stats() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(setq x '(1 2 3 4 5))").unwrap()).unwrap();
    let result = interp.eval(read("(garbage-collect)").unwrap()).unwrap();
    assert!(result.is_cons(), "garbage-collect should return a cons");
    let first = result.car().unwrap();
    assert!(first.is_cons());
    let key = first.car().unwrap();
    assert_eq!(key, LispObject::symbol("bytes-allocated"));
}
#[test]
fn test_garbage_collect_reports_cons_total() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let before = crate::object::global_cons_count();
    interp.eval(read("(setq x '(1 2 3))").unwrap()).unwrap();
    let after = crate::object::global_cons_count();
    assert!(
        after > before,
        "cons counter should increase after allocating a list: before={before}, after={after}"
    );
}
#[test]
fn test_gc_stress() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(let ((i 0)) (while (< i 1000) (cons i i) (setq i (1+ i))))").unwrap())
        .unwrap();
    let result = interp.eval(read("(garbage-collect)").unwrap()).unwrap();
    assert!(result.is_cons());
}
#[test]
fn test_gc_preserves_live_data() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(setq my-list '(a b c))").unwrap())
        .unwrap();
    interp.eval(read("(garbage-collect)").unwrap()).unwrap();
    let result = interp.eval(read("my-list").unwrap()).unwrap();
    assert_eq!(
        result,
        LispObject::cons(
            LispObject::symbol("a"),
            LispObject::cons(
                LispObject::symbol("b"),
                LispObject::cons(LispObject::symbol("c"), LispObject::nil())
            )
        )
    );
}
#[test]
fn test_eval_to_value_self_evaluating() {
    use crate::value::Value;
    let interp = Interpreter::new();
    assert_eq!(
        interp.eval_value(Value::fixnum(42)).unwrap(),
        Value::fixnum(42)
    );
    assert_eq!(interp.eval_value(Value::nil()).unwrap(), Value::nil());
    assert_eq!(interp.eval_value(Value::t()).unwrap(), Value::t());
    assert_eq!(
        interp.eval_value(Value::float(3.14)).unwrap(),
        Value::float(3.14)
    );
}
#[test]
fn test_eval_to_value_delegates_symbol() {
    use crate::value::Value;
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(setq my-var 99)").unwrap()).unwrap();
    let sym = LispObject::symbol("my-var");
    let val = Value::from_lisp_object(&sym);
    assert!(val.is_symbol());
    let result = interp.eval_value(val).unwrap();
    assert_eq!(result.as_fixnum(), Some(99));
}
#[test]
fn test_eval_to_value_via_source() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval_source_value("(+ 1 2)").unwrap();
    assert_eq!(result.as_fixnum(), Some(3));
}
#[test]
fn test_eval_source_value_basic() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval_source_value("(+ 10 20)").unwrap();
    assert_eq!(result.as_fixnum(), Some(30));
}
#[test]
fn test_eval_source_value_multiple_forms() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval_source_value("(+ 1 2) (+ 3 4)").unwrap();
    assert_eq!(result.as_fixnum(), Some(7));
}
#[test]
fn test_eval_source_value_nil() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval_source_value("nil").unwrap();
    assert!(result.is_nil());
}
#[test]
fn test_save_excursion_restores_point() {
    use crate::EditorCallbacks;
    struct FakeEditor {
        text: String,
        point: usize,
    }
    impl EditorCallbacks for FakeEditor {
        fn buffer_string(&self) -> String {
            self.text.clone()
        }
        fn buffer_size(&self) -> usize {
            self.text.len()
        }
        fn point(&self) -> usize {
            self.point
        }
        fn insert(&mut self, t: &str) {
            self.text.insert_str(self.point, t);
            self.point += t.len();
        }
        fn delete_char(&mut self, _n: i64) {}
        fn goto_char(&mut self, pos: usize) {
            self.point = pos;
        }
        fn forward_char(&mut self, n: i64) {
            self.point = (self.point as i64 + n) as usize;
        }
        fn find_file(&mut self, _path: &str) -> bool {
            false
        }
        fn save_buffer(&mut self) -> bool {
            false
        }
    }
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.set_editor(Box::new(FakeEditor {
        text: "hello world".to_string(),
        point: 5,
    }));
    let result = interp
        .eval_source("(save-excursion (goto-char 0) (point))")
        .unwrap();
    assert_eq!(result, LispObject::integer(0));
    let point_after = interp.eval_source("(point)").unwrap();
    assert_eq!(point_after, LispObject::integer(5));
}
#[test]
fn test_save_restriction_acts_as_progn() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval_source("(save-restriction (+ 1 2))").unwrap();
    assert_eq!(result, LispObject::integer(3));
}
#[test]
fn test_load_file_not_found_noerror() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval_source("(load \"/nonexistent/path/file\" t)")
        .unwrap();
    assert_eq!(result, LispObject::nil());
}
#[test]
fn test_load_file_not_found_error() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval_source("(load \"/nonexistent/path/file\")");
    assert!(result.is_err());
}
#[test]
fn test_load_real_file() {
    let dir = std::env::temp_dir();
    let path = dir.join("test_elisp_load.el");
    std::fs::write(&path, "(setq test-load-var 42)").unwrap();
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval_source(&format!("(load \"{}\")", path.display()))
        .unwrap();
    assert_eq!(result, LispObject::t());
    let var = interp.eval_source("test-load-var").unwrap();
    assert_eq!(var, LispObject::integer(42));
    std::fs::remove_file(&path).ok();
}
#[test]
fn test_detect_lexical_binding() {
    use crate::reader::detect_lexical_binding;
    assert!(detect_lexical_binding(
        ";;; -*- lexical-binding: t; -*-\n(defun foo () 1)"
    ));
    assert!(detect_lexical_binding(
        ";;; foo.el --- desc -*- lexical-binding: t -*-"
    ));
    assert!(!detect_lexical_binding(";;; no binding here\n(+ 1 2)"));
    assert!(!detect_lexical_binding(""));
    assert!(!detect_lexical_binding(
        "(setq x 1)\n;;; -*- lexical-binding: t -*-"
    ));
}
#[test]
fn test_file_exists_p() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(file-exists-p "/tmp")"#).unwrap())
            .unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp
            .eval(read(r#"(file-exists-p "/nonexistent-path-12345")"#).unwrap())
            .unwrap(),
        LispObject::nil()
    );
}
#[test]
fn test_expand_file_name() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(expand-file-name "/foo/bar")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/bar")
    );
    assert_eq!(
        interp
            .eval(read(r#"(expand-file-name "bar" "/foo")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/bar")
    );
    assert_eq!(
        interp
            .eval(read(r#"(expand-file-name "bar" "/foo/")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/bar")
    );
}
#[test]
fn test_file_name_directory() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(file-name-directory "/foo/bar.el")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/")
    );
}
#[test]
fn test_file_name_nondirectory() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(file-name-nondirectory "/foo/bar.el")"#).unwrap())
            .unwrap(),
        LispObject::string("bar.el")
    );
}
#[test]
fn test_file_directory_p() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(file-directory-p "/tmp")"#).unwrap())
            .unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp
            .eval(read(r#"(file-directory-p "/nonexistent-12345")"#).unwrap())
            .unwrap(),
        LispObject::nil()
    );
}
#[test]
fn test_directory_file_name() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(directory-file-name "/foo/bar/")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/bar")
    );
}
#[test]
fn test_file_name_as_directory() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(file-name-as-directory "/foo/bar")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/bar/")
    );
    assert_eq!(
        interp
            .eval(read(r#"(file-name-as-directory "/foo/")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/")
    );
}
/// Phase 7i regression: string-match must populate match data so that
/// subsequent match-beginning/match-end/match-string calls return the
/// positions of the match (Emacs semantics). key-parse in keymap.el
/// depends on this after a successful string-match against the key
/// string.
#[test]
fn test_match_data_after_string_match() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let _ = interp
        .eval(read(r#"(string-match "f\\(oo\\)b" "xxfoobar")"#).unwrap())
        .unwrap();
    assert_eq!(
        interp.eval(read("(match-beginning 0)").unwrap()).unwrap(),
        LispObject::integer(2)
    );
    assert_eq!(
        interp.eval(read("(match-end 0)").unwrap()).unwrap(),
        LispObject::integer(6)
    );
    assert_eq!(
        interp.eval(read("(match-beginning 1)").unwrap()).unwrap(),
        LispObject::integer(3)
    );
    assert_eq!(
        interp.eval(read("(match-end 1)").unwrap()).unwrap(),
        LispObject::integer(5)
    );
    assert_eq!(
        interp
            .eval(read("(match-string 1 \"xxfoobar\")").unwrap())
            .unwrap(),
        LispObject::string("oo")
    );
    let _ = interp
        .eval(read(r#"(string-match "z+" "abc")"#).unwrap())
        .unwrap();
    assert_eq!(
        interp.eval(read("(match-beginning 0)").unwrap()).unwrap(),
        LispObject::nil()
    );
}
/// Phase 7i regression: concat should accept lists of character codes
/// and nil (Emacs semantics). Used by help.el form 110:
///   (concat "[" (mapcar #'car alist) "]")
#[test]
fn test_concat_accepts_list_and_nil() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(concat "[" '(97 98 99) "]")"#).unwrap())
            .unwrap(),
        LispObject::string("[abc]")
    );
    assert_eq!(
        interp
            .eval(read(r#"(concat "[" nil "]")"#).unwrap())
            .unwrap(),
        LispObject::string("[]")
    );
}
/// Phase 7h — new bootstrap primitives all work standalone.
#[test]
fn test_phase7h_primitives() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(capitalize "hello world")"#).unwrap())
            .unwrap(),
        LispObject::string("Hello World")
    );
    assert_eq!(
        interp
            .eval(read("(safe-length '(a b c))").unwrap())
            .unwrap(),
        LispObject::integer(3)
    );
    assert_eq!(
        interp.eval(read("(safe-length nil)").unwrap()).unwrap(),
        LispObject::integer(0)
    );
    assert_eq!(
        interp.eval(read("(string 72 105)").unwrap()).unwrap(),
        LispObject::string("Hi")
    );
    assert_eq!(
        interp.eval(read("(characterp 65)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(characterp -1)").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp
            .eval(read(r#"(regexp-quote "a.b*c")"#).unwrap())
            .unwrap(),
        LispObject::string(r"a\.b\*c")
    );
    assert_eq!(
        interp.eval(read("(max-char)").unwrap()).unwrap(),
        LispObject::integer(0x3fffff)
    );
    assert_eq!(
        interp.eval(read(r#"(read "(1 2 3)")"#).unwrap()).unwrap(),
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
fn test_getenv() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval(read(r#"(getenv "HOME")"#).unwrap()).unwrap();
    assert!(matches!(result, LispObject::String(_)));
    assert_eq!(
        interp
            .eval(read(r#"(getenv "ELISP_NONEXISTENT_VAR_12345")"#).unwrap())
            .unwrap(),
        LispObject::nil()
    );
}
#[test]
fn test_backquote_unquote_function_call() {
    let interp = make_stdlib_interp();
    load_prerequisites(&interp);
    let result = interp.eval_source(r#"(list (purecopy "hello"))"#).unwrap();
    assert_eq!(
        result,
        LispObject::cons(LispObject::string("hello"), LispObject::nil())
    );
    let result = interp.eval_source(r#"`(,(purecopy "hello"))"#).unwrap();
    assert_eq!(
        result,
        LispObject::cons(LispObject::string("hello"), LispObject::nil())
    );
}
#[test]
fn test_format_el_form_2_isolated() {
    let interp = make_stdlib_interp();
    for f in &[
        "emacs-lisp/debug-early.el",
        "emacs-lisp/byte-run.el",
        "emacs-lisp/backquote.el",
        "subr.el",
    ] {
        let path = format!("{STDLIB_DIR}/{}", f);
        if let Ok(s) = std::fs::read_to_string(&path) {
            let _ = interp.eval_source(&s);
        }
    }
    let form = r#"(defvar format-alist
  `((text/enriched ,(purecopy "Extended MIME text/enriched format.")
                   ,(purecopy "Content-[Tt]ype:[ \t]*text/enriched"))))"#;
    let result = interp.eval_source(form);
    eprintln!("format.el form 2 isolated: {:?}", result);
}
#[test]
fn test_backquote_without_stdlib_backquote_el() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval_source(r#"`(,(car '(42 43)))"#);
    eprintln!("bare backquote result: {:?}", result);
}
/// Regression for Phase 7g: `&rest` and `&optional` in macro lambda
/// lists must bind the remaining args as a LIST (rest) and pad with
/// nil (optional). The previous `extract_param_names` stripped these
/// markers and bound every name positionally, which broke
/// `backquote-list*-macro` (which takes `(first &rest list)`) and
/// therefore broke backquote expansion for any shape that produced
/// `(backquote-list* ...)` — i.e. lists with leading or trailing
/// literals around an unquote.
#[test]
fn test_macro_rest_arg_binds_as_list() {
    if !ensure_stdlib_files() {
        return;
    }
    let interp = make_stdlib_interp();
    let bq = std::fs::read_to_string(&format!("{STDLIB_DIR}/emacs-lisp/backquote.el")).unwrap();
    let _ = interp.eval_source(&bq);
    let cases: &[&str] = &[
        r#"`(a ,(identity "x") b)"#,
        r#"`(,(identity "x") ,(identity "y") tail)"#,
        r#"`(a b ,(identity "x") c)"#,
    ];
    for src in cases {
        let result = interp.eval_source(src);
        assert!(
            result.is_ok(),
            "backquote shape {} should expand and evaluate: {:?}",
            src,
            result
        );
    }
    let direct = interp
        .eval_source(r#"(backquote-list* 'a "x" '(b))"#)
        .unwrap();
    assert_eq!(
        direct,
        LispObject::cons(
            LispObject::symbol("a"),
            LispObject::cons(
                LispObject::string("x"),
                LispObject::cons(LispObject::symbol("b"), LispObject::nil())
            )
        )
    );
}
#[test]
fn test_defvar_with_backquote_purecopy() {
    let interp = make_stdlib_interp();
    load_prerequisites(&interp);
    let result = interp.eval_source(
        r#"(defvar tbq-test
             `((a ,(purecopy "foo")) (b ,(purecopy "bar"))))"#,
    );
    assert!(
        result.is_ok(),
        "defvar+backquote+purecopy failed: {result:?}"
    );
}
#[test]
fn test_eval_when_compile() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(eval-when-compile 1 2 3)").unwrap())
            .unwrap(),
        LispObject::integer(3)
    );
    assert_eq!(
        interp
            .eval(read("(eval-and-compile (+ 1 2))").unwrap())
            .unwrap(),
        LispObject::integer(3)
    );
}
#[test]
fn test_system_name() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(system-name)").unwrap()).unwrap(),
        LispObject::string("localhost")
    );
}
#[test]
fn test_emacs_pid() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval(read("(emacs-pid)").unwrap()).unwrap();
    let pid = result
        .as_integer()
        .expect("emacs-pid should return integer");
    assert!(pid > 0);
}
