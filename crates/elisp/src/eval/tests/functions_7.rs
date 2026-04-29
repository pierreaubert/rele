#![allow(clippy::manual_checked_ops)]
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
fn test_cl_generic_generalizers_bootstrap_head_and_eql() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval_source(
            "(progn
               (cl-defmethod cl-generic-generalizers
                 ((specializer (head eql)))
                 'source-method)
               (list
                 (funcall 'cl-generic-generalizers '(head eql))
                 (funcall 'cl-generic-generalizers '(eql quote))))",
        )
        .map(|value| {
            assert_eq!(
                value,
                LispObject::cons(
                    LispObject::cons(
                        LispObject::symbol("cl--generic-head-generalizer"),
                        LispObject::nil(),
                    ),
                    LispObject::cons(
                        LispObject::cons(
                            LispObject::symbol("cl--generic-eql-generalizer"),
                            LispObject::nil(),
                        ),
                        LispObject::nil(),
                    ),
                )
            );
        })
        .unwrap();
}
#[test]
fn test_with_memoization_short_circuits_gv_expansion() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval_source(
            "(progn
               (setq cached-value 'already)
               (list
                 (with-memoization cached-value (error \"should not run\"))
                 (with-memoization missing-cache 'computed)))",
        )
        .map(|value| {
            assert_eq!(
                value,
                LispObject::cons(
                    LispObject::symbol("already"),
                    LispObject::cons(LispObject::symbol("computed"), LispObject::nil()),
                )
            );
        })
        .unwrap();
}

#[test]
fn test_remaining_pre_jit_semantics_batch() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    let cases = [
        (
            "(let ((a (bool-vector t))
                   (target (bool-vector t)))
               (bool-vector-union a a target))",
            "nil",
        ),
        (
            "(progn
               (defvar watched-var 0)
               (defvar watched-log nil)
               (add-variable-watcher
                'watched-var
                (lambda (sym new op where)
                  (setq watched-log
                        (cons (list sym new op (stringp where)) watched-log))))
               (setq watched-var 1)
               (setq-local watched-var 2)
               (reverse watched-log))",
            "((watched-var 1 set nil) (watched-var 2 set nil))",
        ),
        (
            "(condition-case err
                 (fset nil #'ignore)
               (setting-constant 'setting-constant))",
            "setting-constant",
        ),
        (
            "(condition-case err
                 (setq :immutable 1)
               (setting-constant 'setting-constant))",
            "setting-constant",
        ),
        (
            "(progn
               (defclass prejit-parent () ())
               (defclass prejit-child (prejit-parent) ())
               (cl-type-of (make-instance 'prejit-child)))",
            "prejit-child",
        ),
        (
            "(condition-case err
                 (aref \"\u{e9}x\" 2)
               (error 'range-error))",
            "range-error",
        ),
        ("(aset \"abc\" 1 ?x)", "120"),
        (
            "(condition-case err
                 (unibyte-string 65 'x)
               (wrong-type-argument 'wrong-type-argument))",
            "wrong-type-argument",
        ),
        (
            "(condition-case err
                 (local-variable-p 1)
               (wrong-type-argument 'wrong-type-argument))",
            "wrong-type-argument",
        ),
    ];

    for (source, expected) in cases {
        let value = interp
            .eval_source(source)
            .unwrap_or_else(|err| panic!("{source} failed: {err:?}"));
        assert_eq!(value.princ_to_string(), expected, "{source}");
    }
}
#[test]
fn test_sparse_char_table_aref_aset_and_ranges() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval_source(
            "(let ((tbl (make-char-table 'rele-test 'missing)))
               (aset tbl #x1f642 'smile)
               (set-char-table-range tbl '(#x20000 . #x20002) 'wide)
               (set-char-table-extra-slot tbl 2 'extra)
               (list
                 (char-table-p tbl)
                 (arrayp tbl)
                 (aref tbl #x1f642)
                 (char-table-range tbl #x20001)
                 (aref tbl #x10ffff)
                 (char-table-extra-slot tbl 2)))",
        )
        .map(|value| {
            assert_eq!(
                value,
                LispObject::cons(
                    LispObject::t(),
                    LispObject::cons(
                        LispObject::t(),
                        LispObject::cons(
                            LispObject::symbol("smile"),
                            LispObject::cons(
                                LispObject::symbol("wide"),
                                LispObject::cons(
                                    LispObject::symbol("missing"),
                                    LispObject::cons(
                                        LispObject::symbol("extra"),
                                        LispObject::nil(),
                                    ),
                                ),
                            ),
                        ),
                    ),
                )
            );
        })
        .unwrap();
}

#[test]
fn test_generated_translation_table_let_is_short_circuited() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval_source(
            "(let ((map '((1 . 2) (3 . 4))))
               (setq marker 'should-not-run)
               (define-translation-table 'demo map)
               (mapc (lambda (x) (error \"should not run\")) map))",
        )
        .unwrap();
    assert_eq!(
        interp.eval_source("(boundp 'marker)").unwrap(),
        LispObject::nil()
    );
}

#[test]
fn test_defun_writes_function_cell() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(defun phase1b-fn-a (x) (* x 2))").unwrap())
        .unwrap();
    assert_eq!(
        interp
            .eval(read("(fboundp 'phase1b-fn-a)").unwrap())
            .unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(phase1b-fn-a 21)").unwrap()).unwrap(),
        LispObject::integer(42)
    );
}
#[test]
fn test_defvar_writes_value_cell_and_mirrored_in_env() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(defvar phase1b-var-a 123)").unwrap())
        .unwrap();
    assert_eq!(
        interp.eval(read("phase1b-var-a").unwrap()).unwrap(),
        LispObject::integer(123)
    );
    assert_eq!(
        interp
            .eval(read("(boundp 'phase1b-var-a)").unwrap())
            .unwrap(),
        LispObject::t()
    );
}
#[test]
fn test_fset_writes_function_cell() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(defun phase1b-fset-target (y) (+ y 1))").unwrap())
        .unwrap();
    interp
        .eval(read("(fset 'phase1b-fset-alias (symbol-function 'phase1b-fset-target))").unwrap())
        .unwrap();
    assert_eq!(
        interp
            .eval(read("(phase1b-fset-alias 10)").unwrap())
            .unwrap(),
        LispObject::integer(11)
    );
}
#[test]
fn test_hashkey_symbol_with_eq_test() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(setq h (make-hash-table :test 'eq))").unwrap())
        .unwrap();
    interp
        .eval(read("(puthash 'phase1b-ht-key 99 h)").unwrap())
        .unwrap();
    assert_eq!(
        interp
            .eval(read("(gethash 'phase1b-ht-key h)").unwrap())
            .unwrap(),
        LispObject::integer(99)
    );
}
#[test]
fn test_symbol_plist_returns_full_list() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(put 'plist-test-sym-c 'a 1)").unwrap())
        .unwrap();
    interp
        .eval(read("(put 'plist-test-sym-c 'b 2)").unwrap())
        .unwrap();
    let result = interp
        .eval(read("(symbol-plist 'plist-test-sym-c)").unwrap())
        .unwrap();
    let expected = LispObject::cons(
        LispObject::symbol("a"),
        LispObject::cons(
            LispObject::integer(1),
            LispObject::cons(
                LispObject::symbol("b"),
                LispObject::cons(LispObject::integer(2), LispObject::nil()),
            ),
        ),
    );
    assert_eq!(result, expected);
}
/// Rough performance signal for the Value-native fast path. Evaluates
/// `(+ 1 2 3)` many times and prints the rate. Failure indicates a
/// massive regression; we don't assert on absolute numbers.
#[test]
fn test_value_native_fastpath_perf() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let expr = read("(+ 1 2 3)").unwrap();
    crate::primitives_value::reset_hit_counters();
    let iterations = 50_000;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = interp.eval(expr.clone()).unwrap();
    }
    let elapsed = start.elapsed();
    let per_call_ns = elapsed.as_nanos() as f64 / iterations as f64;
    let calls_per_sec = iterations as f64 / elapsed.as_secs_f64();
    let fast = crate::primitives_value::fast_hits();
    let slow = crate::primitives_value::slow_hits();
    eprintln!(
        "fast path: {iterations} × (+ 1 2 3) in {elapsed:?} \
         = {per_call_ns:.0} ns/call, {calls_per_sec:.0} calls/s \
         (fast_hits={fast}, slow_hits={slow})"
    );
    assert!(elapsed.as_secs() < 10, "Value-native fast path too slow");
}
/// Load the full stdlib bootstrap chain (BOOTSTRAP_FILES) into an
/// existing interpreter. `make_stdlib_interp` only registers Rust
/// stubs — the actual Lisp stdlib (backquote.el, subr.el, etc.) is
/// loaded by the bootstrap test itself. The full-suite harness needs
/// the same Lisp infrastructure or every macro using ` will fail.
/// Load Emacs cl-* files (cl-macs, cl-extra, cl-seq, cl-print) into an
/// existing interpreter. These provide cl-loop, cl-flet, cl-labels,
/// cl-destructuring-bind, cl-letf*, cl-typep, cl-do — used by ~30% of
/// the test files. Loading them in `make_stdlib_interp` directly causes
/// memory pressure across all unit tests, so we load them only in the
/// full-suite harness where it pays off.
///
/// Each call ensures the files exist on disk first.
/// Probe: how much of each CL .el loads against our bootstrapped interp?
/// Runs in a dedicated thread with a 64 MB stack so deeply nested
/// macro expansions (cl-macs.el is ~3500 lines) don't overflow.
#[test]
fn test_cl_files_load_progress() {
    if !ensure_stdlib_files() {
        return;
    }
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let interp = make_stdlib_interp();
            let _ = probe_emacs_file(&interp, &format!("{STDLIB_DIR}/emacs-lisp/cl-preloaded.el"));
            for f in [
                "emacs-lisp/cl-macs",
                "emacs-lisp/cl-extra",
                "emacs-lisp/cl-seq",
                "emacs-lisp/cl-print",
            ] {
                let path = format!("{STDLIB_DIR}/{f}.el");
                match probe_emacs_file(&interp, &path) {
                    Some((ok, total)) => {
                        let pct = if total > 0 { ok * 100 / total } else { 0 };
                        eprintln!("  {f}: {ok}/{total} ({pct}%)");
                    }
                    None => eprintln!("  {f}: not loadable"),
                }
            }
        })
        .expect("spawn");
    handle.join().expect("join");
}
/// This test does a full stdlib + cl-lib bootstrap, which is heavy
/// (~5s, ~200MB). Ignored from `cargo test` by default; run with
/// `cargo test -- --ignored test_backquote_in_macro_after_stdlib_bootstrap`.
#[test]
#[ignore]
fn test_backquote_in_macro_after_stdlib_bootstrap() {
    if !ensure_stdlib_files() {
        return;
    }
    let handle = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let interp = make_stdlib_interp();
            load_full_bootstrap(&interp);
            let r = interp.eval(read("(defmacro rele-bq-test (x) `(list ,x))").unwrap());
            assert!(r.is_ok(), "defmacro failed: {r:?}");
            let result = interp.eval(read("(rele-bq-test 42)").unwrap());
            match result {
                Ok(v) => {
                    let expected = LispObject::cons(LispObject::integer(42), LispObject::nil());
                    assert_eq!(v, expected, "backquote expansion produced wrong value");
                }
                Err(e) => panic!("backquote macro invocation failed: {e}"),
            }
        })
        .expect("spawn");
    handle.join().expect("join");
}
#[test]
fn test_with_temp_buffer_basic() {
    crate::buffer::reset();
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read(r#"(with-temp-buffer (insert "hello") (buffer-string))"#).unwrap())
        .unwrap();
    assert_eq!(result, LispObject::string("hello"));
    let scratch_text = interp.eval(read("(buffer-string)").unwrap()).unwrap();
    assert_eq!(scratch_text, LispObject::string(""));
}
#[test]
fn test_with_temp_buffer_point_min_max() {
    crate::buffer::reset();
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(r#"(with-temp-buffer (insert "abc") (list (point-min) (point-max) (point)))"#)
                .unwrap(),
        )
        .unwrap();
    let expected = LispObject::cons(
        LispObject::integer(1),
        LispObject::cons(
            LispObject::integer(4),
            LispObject::cons(LispObject::integer(4), LispObject::nil()),
        ),
    );
    assert_eq!(result, expected);
}
#[test]
fn test_with_temp_buffer_delete_region() {
    crate::buffer::reset();
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(
                r#"(with-temp-buffer
                     (insert "0123456789")
                     (delete-region 3 6)
                     (buffer-string))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(result, LispObject::string("0156789"));
}
/// Microbenchmark verifying the Value-native fast path works for hot
/// primitives. Doesn't enforce a time budget — just prints the rate.
#[test]
fn test_value_native_fastpath_smoke() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let expr = read("(+ 1 2 3 4 5)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::integer(15));
    let expr = read("(- 7)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::integer(-7));
    assert_eq!(
        interp.eval(read("(= 1 1 1)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(< 1 2 3)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(< 1 2 2)").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp.eval(read("(1+ 41)").unwrap()).unwrap(),
        LispObject::integer(42)
    );
    assert_eq!(
        interp.eval(read("(1- 43)").unwrap()).unwrap(),
        LispObject::integer(42)
    );
    assert_eq!(
        interp.eval(read("(null nil)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(not 42)").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp.eval(read("(eq 1 1)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(integerp 42)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(zerop 0)").unwrap()).unwrap(),
        LispObject::t()
    );
}
/// Root of the Emacs source tree, if available. Used for ERT compatibility
/// probes. Returns `None` if the tree isn't present (e.g. on CI).
///
/// Discovery order:
/// 1. `EMACS_SRC_ROOT` environment variable.
/// 2. Common paths on macOS / Linux: `/Volumes/SSD2TB/src/emacs`,
///    `/usr/src/emacs`, `/usr/local/src/emacs`, `$HOME/src/emacs`,
///    `$HOME/emacs`.
///
/// Cached after the first call via `OnceLock`, so subsequent probes
/// are free.
/// Directory containing Emacs's precompiled `.el` / `.el.gz` stdlib
/// files. Used to decompress a bootstrap set into `STDLIB_DIR`.
/// Returns `None` when no installation is present.
///
/// Discovery order:
/// 1. `EMACS_LISP_DIR` environment variable.
/// 2. Homebrew on macOS (`/opt/homebrew/share/emacs/*/lisp`,
///    pinned `/opt/homebrew/share/emacs/30.2/lisp` first for speed).
/// 3. Intel-Mac Homebrew (`/usr/local/share/emacs/*/lisp`).
/// 4. System Linux (`/usr/share/emacs/*/lisp`).
/// 5. Compiled-from-source Linux (`/usr/local/share/emacs/*/lisp`).
///
/// Cached after the first call via `OnceLock`.
/// Emit a human-readable diagnostic describing what the ERT harness
/// can and can't do on this host. Prints to stderr and always
/// succeeds — the goal is visibility for CI, not assertion. Read
/// the output with `cargo test -- --nocapture test_framework_status`.
#[test]
fn test_framework_status() {
    eprintln!("── rele-elisp test framework status ──");
    match emacs_lisp_dir() {
        Some(p) => eprintln!("  emacs_lisp_dir:    {p}"),
        None => {
            eprintln!("  emacs_lisp_dir:    NOT FOUND (set EMACS_LISP_DIR or install Emacs)")
        }
    }
    match emacs_source_root() {
        Some(p) => eprintln!("  emacs_source_root: {p}"),
        None => {
            eprintln!("  emacs_source_root: NOT FOUND (set EMACS_SRC_ROOT for ERT file probing)")
        }
    }
    let stdlib_ready = ensure_stdlib_files();
    eprintln!(
        "  stdlib bootstrap:  {}",
        if stdlib_ready {
            "READY (STDLIB_DIR populated)"
        } else {
            "SKIPPED (no emacs_lisp_dir)"
        }
    );
}
#[test]
fn test_emacs_lisp_dir_honours_env_var() {
    if let Some(path) = emacs_lisp_dir() {
        assert!(
            std::path::Path::new(path).is_dir(),
            "emacs_lisp_dir returned {path} which isn't a directory"
        )
    }
}
#[test]
fn test_emacs_source_root_is_directory_when_present() {
    if let Some(path) = emacs_source_root() {
        assert!(
            std::path::Path::new(path).is_dir(),
            "emacs_source_root returned {path} which isn't a directory"
        )
    }
}
/// Read an Emacs source file as UTF-8 with Latin-1 fallback.
/// Load a bootstrap-initialized interpreter and try to evaluate every
/// top-level form in `file_path`. Returns `(ok_count, total)` on success,
/// or `None` if the file cannot be read or parsed.
#[test]
fn test_emacs_ert_can_be_loaded() {
    let Some(root) = emacs_source_root() else {
        return;
    };
    if !ensure_stdlib_files() {
        return;
    }
    let root = root.to_string();
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let interp = make_stdlib_interp();
            let ert_path = format!("{}/lisp/emacs-lisp/ert.el", root);
            match probe_emacs_file(&interp, &ert_path) {
                Some((ok, total)) => {
                    let pct = if total > 0 { ok * 100 / total } else { 0 };
                    eprintln!("ert.el: {ok}/{total} forms loaded ({pct}%)");
                    assert!(
                        pct >= 30,
                        "ert.el compatibility dropped below 30% (got {pct}%)",
                    );
                }
                None => eprintln!("ert.el not readable"),
            }
        })
        .expect("failed to spawn ERT compat thread");
    handle.join().expect("ERT compat thread panicked");
}
/// Try to actually RUN a simple ert-deftest: define it, then invoke it.
#[test]
fn test_emacs_ert_can_run_a_test() {
    if !ensure_stdlib_files() {
        return;
    }
    let handle = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let interp = make_stdlib_interp();
            interp
                .eval(read("(ert-deftest rele-smoke () (should (= (+ 1 2) 3)))").unwrap())
                .unwrap();
            interp
                .eval(read("(ert-deftest rele-fail () (should (= 1 2)))").unwrap())
                .unwrap();
            interp
                .eval(read("(ert-deftest rele-error () (should-error (error \"boom\")))").unwrap())
                .unwrap();
            let s = run_rele_ert_tests(&interp);
            eprintln!(
                "rele ERT smoke: {} pass / {} fail / {} error / {} skip / {} panic",
                s.passed, s.failed, s.errored, s.skipped, s.panicked,
            );
            assert_eq!(s.passed, 2, "expected 2 passes (rele-smoke + rele-error)");
            assert_eq!(s.failed, 1, "expected 1 fail (rele-fail)");
            assert_eq!(s.errored, 0);
            assert_eq!(s.panicked, 0);
        })
        .expect("spawn");
    handle.join().expect("join");
}
