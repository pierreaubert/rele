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
fn test_bootstrap_loading_chain() {
    if !ensure_stdlib_files() {
        return;
    }
    let interp = make_stdlib_interp();
    let files = vec![
        ("debug-early", "debug-early"),
        ("byte-run", "byte-run"),
        ("backquote", "backquote"),
        ("subr", "subr"),
    ];
    for (file_name, label) in &files {
        let path = format!("{STDLIB_DIR}/{}.el", file_name);
        if let Ok(source) = std::fs::read_to_string(&path) {
            match interp.eval_source(&source) {
                Ok(_) => eprintln!("  [bootstrap] {} OK", label),
                Err((i, e)) => eprintln!("  [bootstrap] {} form {}: {}", label, i, e),
            }
        } else {
            eprintln!("  [bootstrap] {} not found at {}", label, path);
        }
    }
}
#[test]
fn test_full_bootstrap_chain() {
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(run_full_bootstrap_chain)
        .expect("failed to spawn bootstrap thread");
    handle.join().expect("bootstrap thread panicked");
}
#[allow(dead_code)]
fn run_full_bootstrap_chain() {
    if !ensure_stdlib_files() {
        return;
    }
    let interp = make_stdlib_interp();
    let file_count = BOOTSTRAP_FILES.len() - 1;
    let files: &[&str] = &BOOTSTRAP_FILES[..file_count];
    let mut loaded = 0;
    let mut partial = 0;
    let mut failed = 0;
    let mut skipped = 0;
    for f in files {
        let path = format!("{STDLIB_DIR}/{}.el", f);
        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => match std::fs::read(&path) {
                Ok(bytes) => bytes.iter().map(|&b| char::from(b)).collect(),
                Err(_) => {
                    eprintln!("  [{}] SKIPPED (not found at {})", f, path);
                    skipped += 1;
                    continue;
                }
            },
        };
        let forms = match crate::read_all(&source) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  [{}] PARSE ERROR: {}", f, e);
                failed += 1;
                continue;
            }
        };
        let total = forms.len();
        let mut ok = 0;
        let mut first_err: Option<String> = None;
        interp.set_eval_ops_limit(super::bootstrap::bootstrap_eval_ops_limit(f));
        for form in forms {
            interp.reset_eval_ops();
            match interp.eval(form) {
                Ok(_) => ok += 1,
                Err(e) => {
                    if first_err.is_none() {
                        first_err = Some(format!("form {}: {}", ok, e));
                    }
                }
            }
        }
        interp.set_eval_ops_limit(0);
        if ok == total {
            eprintln!("  [{}] OK ({}/{})", f, ok, total);
            loaded += 1;
        } else {
            let pct = if total > 0 { ok * 100 / total } else { 0 };
            eprintln!("  [{}] PARTIAL ({}/{} = {}%)", f, ok, total, pct);
            if let Some(err) = &first_err {
                eprintln!("    first error: {}", err);
            }
            partial += 1;
        }
    }
    eprintln!(
        "\nBootstrap summary: {} OK, {} partial, {} failed, {} skipped out of {} files",
        loaded,
        partial,
        failed,
        skipped,
        files.len()
    );
    assert!(
        loaded >= 25,
        "Expected at least 25 files to load fully, got {}",
        loaded
    );
}
#[test]
fn test_current_buffer_no_editor() {
    let interp = Interpreter::new();
    let result = interp
        .eval(read("(buffer-name (current-buffer))").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::string("*scratch*"));
}
#[test]
fn test_buffer_name() {
    let interp = Interpreter::new();
    let result = interp.eval(read("(buffer-name)").unwrap()).unwrap();
    assert_eq!(result, LispObject::string("*scratch*"));
}
#[test]
fn test_get_buffer() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read("(progn (get-buffer-create \"foo\") (buffer-name (get-buffer \"foo\")))").unwrap(),
        )
        .unwrap();
    assert_eq!(result, LispObject::string("foo"));
}
#[test]
fn test_get_buffer_create() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(buffer-name (get-buffer-create \"test\"))").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::string("test"));
}
#[test]
fn test_find_buffer_by_local_value() {
    crate::buffer::reset();
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(
                "(progn
                   (get-buffer-create \"find-buffer-probe\")
                   (set-buffer \"find-buffer-probe\")
                   (setq-local rele-find-buffer-key 42)
                   (buffer-name (find-buffer 'rele-find-buffer-key 42)))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(result, LispObject::string("find-buffer-probe"));
}
#[test]
fn test_kill_current_buffer_keeps_fallback_buffer() {
    crate::buffer::reset();
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(
                "(progn
                   (get-buffer-create \"kill-probe\")
                   (set-buffer \"kill-probe\")
                   (kill-buffer)
                   (buffer-name (current-buffer)))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(result, LispObject::string("*scratch*"));
}
#[test]
fn test_buffer_list() {
    let interp = Interpreter::new();
    let result = interp
        .eval(read("(mapcar #'buffer-name (buffer-list))").unwrap())
        .unwrap();
    assert_eq!(
        result,
        LispObject::cons(LispObject::string("*scratch*"), LispObject::nil())
    );
}
#[test]
fn test_version_to_list_heap_migration() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read(r#"(version-to-list "1.2.3")"#).unwrap())
        .unwrap();
    assert_eq!(
        result,
        LispObject::cons(
            LispObject::integer(1),
            LispObject::cons(
                LispObject::integer(2),
                LispObject::cons(LispObject::integer(3), LispObject::nil()),
            ),
        )
    );
}
#[test]
fn test_version_to_list_empty_parts() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read(r#"(version-to-list "10.0.x")"#).unwrap())
        .unwrap();
    assert_eq!(
        result,
        LispObject::cons(
            LispObject::integer(10),
            LispObject::cons(
                LispObject::integer(0),
                LispObject::cons(LispObject::integer(0), LispObject::nil()),
            ),
        )
    );
}
#[test]
fn test_nreverse_heap_migration() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(nreverse (list 1 2 3 4))").unwrap())
        .unwrap();
    let expected = [4i64, 3, 2, 1]
        .into_iter()
        .rev()
        .fold(LispObject::nil(), |acc, n| {
            LispObject::cons(LispObject::integer(n), acc)
        });
    assert_eq!(result, expected);
}
#[test]
fn test_nreverse_empty_list() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval(read("(nreverse nil)").unwrap()).unwrap();
    assert_eq!(result, LispObject::nil());
}
#[test]
fn test_split_string_heap_migration() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read(r#"(split-string "a.b.c" "\\.")"#).unwrap())
        .unwrap();
    assert_eq!(
        result,
        LispObject::cons(
            LispObject::string("a"),
            LispObject::cons(
                LispObject::string("b"),
                LispObject::cons(LispObject::string("c"), LispObject::nil()),
            ),
        )
    );
}
#[test]
fn test_read_from_string_heap_migration() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read(r#"(read-from-string "42")"#).unwrap())
        .unwrap();
    assert_eq!(
        result,
        LispObject::cons(LispObject::integer(42), LispObject::integer(2))
    );
}
#[test]
fn test_hashtable_identity_preserved_under_heap_scope() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(
                r#"(let ((h (make-hash-table :test 'equal)))
                  (puthash "a" 1 h)
                  (puthash "b" 2 h)
                  (+ (gethash "a" h) (gethash "b" h)))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(result, LispObject::integer(3));
}
#[test]
fn test_vector_decode_preserves_content() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(let ((v (make-vector 3 7))) (length v))").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::integer(3));
}

#[test]
fn test_bool_vector_operations_and_coerce() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(
                "(let* ((a (bool-vector nil t nil t))
                        (b (bool-vector t nil nil t))
                        (u (bool-vector-union a b))
                        (n (bool-vector-not a)))
                   (list (length u)
                         (cl-coerce u 'list)
                         (cl-coerce n 'list)
                         (bool-vector-count-consecutive u t 0)))",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(result, read("(4 (t t nil t) (t nil t nil) 2)").unwrap());
}

#[test]
fn test_cons_setcar_after_heap_round_trip() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(let ((x (cons 'a 'b))) (setcar x 'z) (car x))").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::symbol("z"));
}
#[test]
fn test_cons_setcdr_after_heap_round_trip() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(let ((x (cons 'a 'b))) (setcdr x 'z) (cdr x))").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::symbol("z"));
}
#[test]
fn test_hashtable_puthash_persists_across_rebindings() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(
                r#"(progn
                  (setq h1 (make-hash-table :test 'equal))
                  (puthash "key" 99 h1)
                  (setq h2 h1)
                  (gethash "key" h2))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(result, LispObject::integer(99));
}
#[test]
fn test_symbol_function_macro_form_heap_migration() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(defmacro my-mac (x) (list 'quote x))").unwrap())
        .unwrap();
    let result = interp
        .eval(read("(symbol-function 'my-mac)").unwrap())
        .unwrap();
    let first = result.car().unwrap();
    assert_eq!(first, LispObject::symbol("macro"));
    let rest = result.rest().unwrap();
    let second = rest.car().unwrap();
    assert_eq!(second, LispObject::symbol("lambda"));
}
#[test]
fn test_sort_ascending_heap_migration() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(sort (list 3 1 4 1 5 9 2 6) '<)").unwrap())
        .unwrap();
    let expected = [1i64, 1, 2, 3, 4, 5, 6, 9]
        .into_iter()
        .rev()
        .fold(LispObject::nil(), |acc, n| {
            LispObject::cons(LispObject::integer(n), acc)
        });
    assert_eq!(result, expected);
}
#[test]
fn test_buffer_live_p() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(buffer-live-p (current-buffer))").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::t());
}
#[test]
fn test_set_buffer_noop() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval(read("(set-buffer \"foo\")").unwrap()).unwrap();
    assert_eq!(result, LispObject::nil());
}
#[test]
fn test_with_current_buffer() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(with-current-buffer \"buf\" (+ 1 2))").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::integer(3));
}
#[test]
fn test_generate_new_buffer_name() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(generate-new-buffer-name \"test\")").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::string("test"));
}
#[test]
fn test_file_name_quote() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(file-name-quote \"/tmp/foo\")").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::string("/:/tmp/foo"));
}
#[test]
fn test_file_name_unquote() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(file-name-unquote \"/:/tmp/foo\")").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::string("/tmp/foo"));
}
#[test]
fn test_file_name_unquote_no_prefix() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(file-name-unquote \"/tmp/foo\")").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::string("/tmp/foo"));
}

#[test]
fn test_compare_strings_emacs_argument_order() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(compare-strings \"foo\" 0 3 \"foo\" 0 3 nil)").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::t());
    let result = interp
        .eval(read("(compare-strings \"a\" 0 nil \"b\" 0 nil nil)").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::integer(-1));
}
#[test]
fn test_autoload_records_mapping() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(autoload 'my-func \"my-file\")").unwrap())
        .unwrap();
    let autoloads = interp.state.autoloads.read();
    assert_eq!(autoloads.get("my-func"), Some(&"my-file".to_string()));
}
#[test]
fn test_autoload_triggers_load() {
    use std::io::Write;
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let dir = std::env::temp_dir().join("elisp-autoload-test");
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("autoloaded.el");
    let mut f = std::fs::File::create(&file_path).unwrap();
    writeln!(f, "(defun autoloaded-fn (x) (+ x 10))").unwrap();
    let expr = format!(
        "(autoload 'autoloaded-fn \"{}\")",
        file_path.to_string_lossy()
    );
    interp.eval(read(&expr).unwrap()).unwrap();
    let result = interp.eval(read("(autoloaded-fn 5)").unwrap()).unwrap();
    assert_eq!(result, LispObject::integer(15));
    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn test_quote_dotted_form() {
    let interp = Interpreter::new();
    let expr = LispObject::cons(LispObject::symbol("quote"), LispObject::symbol("foo"));
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::symbol("foo"));
}
#[test]
fn test_plist_put_get_roundtrip() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(put 'plist-test-sym-a 'bar 42)").unwrap())
        .unwrap();
    assert_eq!(
        interp
            .eval(read("(get 'plist-test-sym-a 'bar)").unwrap())
            .unwrap(),
        LispObject::integer(42)
    );
    assert_eq!(
        interp
            .eval(read("(get 'plist-test-sym-a 'missing)").unwrap())
            .unwrap(),
        LispObject::nil()
    );
}
#[test]
fn test_plist_put_replaces_in_place() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(put 'plist-test-sym-b 'k 1)").unwrap())
        .unwrap();
    interp
        .eval(read("(put 'plist-test-sym-b 'k 2)").unwrap())
        .unwrap();
    assert_eq!(
        interp
            .eval(read("(get 'plist-test-sym-b 'k)").unwrap())
            .unwrap(),
        LispObject::integer(2)
    );
}
#[test]
fn test_function_put_get_and_alias_lookup() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval_source(
            "(progn
               (defun function-prop-target () nil)
               (defalias 'function-prop-alias #'function-prop-target)
               (function-put 'function-prop-target 'prop 'value)
               (list
                 (function-get 'function-prop-target 'prop)
                 (function-get 'function-prop-alias 'prop)))",
        )
        .map(|value| {
            assert_eq!(
                value,
                LispObject::cons(
                    LispObject::symbol("value"),
                    LispObject::cons(LispObject::symbol("value"), LispObject::nil()),
                )
            );
        })
        .unwrap();
}
#[test]
fn test_function_get_non_symbol_returns_nil() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval_source("(function-get '(lambda (x) x) 'gv-expander)")
            .unwrap(),
        LispObject::nil()
    );
}
#[test]
fn test_bootstrap_metadata_primitives() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval_source(
            "(progn
               (def-edebug-elem-spec 'bootstrap-edebug '(sexp))
               (defvar-1 'bootstrap-defvar 10 \"doc\")
               (list
                 (get 'bootstrap-edebug 'edebug-elem-spec)
                 bootstrap-defvar
                 (get 'bootstrap-defvar 'variable-documentation)))",
        )
        .map(|value| {
            assert_eq!(
                value,
                LispObject::cons(
                    LispObject::cons(LispObject::symbol("sexp"), LispObject::nil()),
                    LispObject::cons(
                        LispObject::integer(10),
                        LispObject::cons(LispObject::string("doc"), LispObject::nil()),
                    ),
                )
            );
        })
        .unwrap();
}
#[test]
fn test_feature_primitives_work_through_function_cells() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval_source(
            "(progn
               (funcall 'provide 'function-cell-feature)
               (list
                 (funcall 'featurep 'function-cell-feature)
                 (funcall 'require 'function-cell-feature)))",
        )
        .map(|value| {
            assert_eq!(
                value,
                LispObject::cons(
                    LispObject::t(),
                    LispObject::cons(
                        LispObject::symbol("function-cell-feature"),
                        LispObject::nil(),
                    ),
                )
            );
        })
        .unwrap();
}
#[test]
fn test_cl_generic_define_method_installs_primary_function() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval_source(
            "(progn
               (defalias 'bootstrap-generic
                 (cl-generic-define 'bootstrap-generic '(x) nil))
               (cl-generic-define-method
                 'bootstrap-generic nil '(x) nil
                 (lambda (x) (list 'primary x)))
               (bootstrap-generic 7))",
        )
        .map(|value| {
            assert_eq!(
                value,
                LispObject::cons(
                    LispObject::symbol("primary"),
                    LispObject::cons(LispObject::integer(7), LispObject::nil()),
                )
            );
        })
        .unwrap();
}

#[test]
fn test_position_symbol_round_trips_through_bare_symbol_helpers() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let value = interp
        .eval_source(
            "(let ((s (position-symbol 'bootstrap-pos-symbol 7)))
               (list
                 (symbol-with-pos-p s)
                 (symbol-with-pos-pos s)
                 (bare-symbol s)
                 (remove-pos-from-symbol s)))",
        )
        .unwrap();
    assert_eq!(
        value,
        read("(t 7 bootstrap-pos-symbol bootstrap-pos-symbol)").unwrap()
    );
}

#[test]
fn test_symbol_plist_watchers_and_command_modes_are_stateful() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let value = interp
        .eval_source(
            "(progn
               (setplist 'bootstrap-plist-symbol '(a 1 b 2))
               (put 'bootstrap-command 'command-modes '(bootstrap-mode))
               (add-variable-watcher 'bootstrap-watch-symbol 'watch-one)
               (add-variable-watcher 'bootstrap-watch-symbol 'watch-one)
               (add-variable-watcher 'bootstrap-watch-symbol 'watch-two)
               (let ((before (get-variable-watchers 'bootstrap-watch-symbol)))
                 (remove-variable-watcher 'bootstrap-watch-symbol 'watch-one)
                 (list
                   (symbol-plist 'bootstrap-plist-symbol)
                   (command-modes 'bootstrap-command)
                   before
                   (get-variable-watchers 'bootstrap-watch-symbol))))",
        )
        .unwrap();
    assert_eq!(
        value,
        read("((a 1 b 2) (bootstrap-mode) (watch-one watch-two) (watch-two))").unwrap()
    );
}

#[test]
fn test_subr_introspection_handles_primitives_and_special_forms() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let value = interp
        .eval_source(
            "(list
               (subr-name (symbol-function 'car))
               (subr-arity (symbol-function 'car))
               (subr-arity (symbol-function 'if)))",
        )
        .unwrap();
    assert_eq!(value, read(r#"("car" (1 . 1) (2 . unevalled))"#).unwrap());
}

#[test]
fn test_numeric_comparisons_validate_like_emacs() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert!(interp.eval_source("(=)").is_err());
    assert!(interp.eval_source("(<)").is_err());
    assert_eq!(interp.eval_source("(< 1)").unwrap(), LispObject::t());
    assert_eq!(interp.eval_source("(> 1)").unwrap(), LispObject::t());
    assert_eq!(
        interp.eval_source("(< 9 8 'not-a-number)").unwrap(),
        LispObject::nil()
    );
    assert!(interp.eval_source("(< 8 9 'not-a-number)").is_err());
}

#[test]
fn test_interpreted_function_p_recognizes_lambda_objects() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval_source("(interpreted-function-p '(lambda (x) x))")
            .unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp
            .eval_source("(interpreted-function-p (symbol-function 'car))")
            .unwrap(),
        LispObject::nil()
    );
}

#[test]
fn test_special_variable_p_tracks_defvar_and_builtin_specials() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let value = interp
        .eval_source(
            "(progn
               (defvar bootstrap-special-var nil)
               (list
                 (special-variable-p 'bootstrap-special-var)
                 (special-variable-p 'standard-output)
                 (special-variable-p 'bootstrap-not-special)))",
        )
        .unwrap();
    assert_eq!(value, read("(t t nil)").unwrap());
}

#[test]
fn test_symbol_function_and_indirect_function_use_function_cells() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let value = interp
        .eval_source(
            "(progn
               (defun bootstrap-if-target () 'ok)
               (defalias 'bootstrap-if-alias 'bootstrap-if-target)
               (list
                 (fboundp 'bootstrap-if-target)
                 (eq (indirect-function 'bootstrap-if-alias)
                     (symbol-function 'bootstrap-if-target))))",
        )
        .unwrap();
    assert_eq!(value, read("(t t)").unwrap());
}
