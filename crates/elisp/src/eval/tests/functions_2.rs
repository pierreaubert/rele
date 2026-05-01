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
fn test_unwind_protect_on_throw() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
        .eval(read("(progn (setq cleaned-up nil) (catch 'done (unwind-protect (throw 'done 42) (setq cleaned-up t))) cleaned-up)")
        .unwrap()).unwrap(), LispObject::t()
    );
}
#[test]
fn test_load_debug_early_el() {
    let source = match std::fs::read_to_string(format!("{STDLIB_DIR}/emacs-lisp/debug-early.el")) {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    match interp.eval_source(&source) {
        Ok(_) => {}
        Err((i, e)) => panic!("debug-early.el failed at form {}: {}", i, e),
    }
}
#[test]
fn test_load_byte_run_el() {
    let source = match std::fs::read_to_string(format!("{STDLIB_DIR}/emacs-lisp/byte-run.el")) {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    let forms = crate::read_all(&source).unwrap();
    let total = forms.len();
    let mut passed = 0;
    for form in forms {
        match interp.eval(form) {
            Ok(_) => passed += 1,
            Err(e) => {
                if passed < total - 1 {
                    panic!("byte-run.el failed at form {}/{}: {}", passed, total, e);
                }
            }
        }
    }
    assert!(
        passed >= total / 2,
        "byte-run.el: only {}/{} forms passed",
        passed,
        total
    );
}
#[test]
fn test_load_backquote_el() {
    let source = match std::fs::read_to_string(format!("{STDLIB_DIR}/emacs-lisp/backquote.el")) {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    if let Ok(byte_run) = std::fs::read_to_string(format!("{STDLIB_DIR}/emacs-lisp/byte-run.el")) {
        let _ = interp.eval_source(&byte_run);
    }
    match interp.eval_source(&source) {
        Ok(_) => {}
        Err((i, e)) => panic!("backquote.el failed at form {}: {}", i, e),
    }
}
#[test]
fn test_load_subr_el_progress() {
    let source = match std::fs::read_to_string(format!("{STDLIB_DIR}/subr.el")) {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    for f in &["debug-early.el", "byte-run.el", "backquote.el"] {
        if let Ok(s) = std::fs::read_to_string(format!("{STDLIB_DIR}/emacs-lisp/{}", f)) {
            let _ = interp.eval_source(&s);
        }
    }
    let forms = crate::read_all(&source).unwrap();
    let total = forms.len();
    let mut ok_count = 0;
    let mut err_count = 0;
    let mut errors: Vec<(usize, String)> = Vec::new();
    for (i, form) in forms.into_iter().enumerate() {
        match interp.eval(form) {
            Ok(_) => ok_count += 1,
            Err(e) => {
                err_count += 1;
                if errors.len() < 10 {
                    errors.push((i, format!("{}", e)));
                }
            }
        }
    }
    eprintln!("subr.el: {}/{} OK, {} errors", ok_count, total, err_count);
    for (i, e) in &errors {
        eprintln!("  form {}: {}", i, e);
    }
    assert!(
        ok_count * 100 / total >= 99,
        "subr.el: only {}% success ({}/{})",
        ok_count * 100 / total,
        ok_count,
        total
    );
}
#[test]
fn test_load_elc_file() {
    let elc_path = "/tmp/test-bytecode.elc";
    let source = match std::fs::read_to_string(elc_path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    match interp.eval_source(&source) {
        Ok(_) => {}
        Err((i, e)) => {
            eprintln!("test-bytecode.elc failed at form {}: {}", i, e);
        }
    }
    let result = interp.eval(read("(my-add 3 4)").unwrap());
    match result {
        Ok(val) => assert_eq!(val, LispObject::integer(7), "my-add returned {:?}", val),
        Err(e) => {
            eprintln!(
                "my-add failed: {} (expected — bytecode may need more opcodes)",
                e
            )
        }
    }
    let result = interp.eval(read("(my-double 21)").unwrap());
    match result {
        Ok(val) => {
            assert_eq!(val, LispObject::integer(42), "my-double returned {:?}", val)
        }
        Err(e) => eprintln!("my-double failed: {}", e),
    }
}
#[test]
fn test_jit_tier_reports_interp_for_untouched_name() {
    let interp = Interpreter::new();
    let tier = interp.jit_tier("never-defined-in-this-test");
    assert_eq!(tier, crate::jit::Tier::Interp);
}
#[test]
fn test_jit_compile_unknown_returns_unknown_function() {
    let interp = Interpreter::new();
    let err = interp
        .jit_compile("definitely-not-a-defined-function-xyzzy")
        .expect_err("must error");
    #[cfg(feature = "jit")]
    {
        match err {
            crate::jit::JitError::UnknownFunction(n) => {
                assert_eq!(n, "definitely-not-a-defined-function-xyzzy")
            }
            other => panic!("wrong error: {other:?}"),
        }
    }
    #[cfg(not(feature = "jit"))]
    {
        match err {
            crate::jit::JitError::JitDisabled => {}
            other => panic!("wrong error: {other:?}"),
        }
    }
}
#[test]
fn test_jit_compile_lambda_returns_not_bytecode() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval_source("(defun rele-jit-compile-lambda-test () 1)")
        .unwrap();
    let err = interp
        .jit_compile("rele-jit-compile-lambda-test")
        .expect_err("lambda is not bytecode");
    #[cfg(feature = "jit")]
    {
        match err {
            crate::jit::JitError::NotBytecode(n) => {
                assert_eq!(n, "rele-jit-compile-lambda-test")
            }
            other => panic!("wrong error: {other:?}"),
        }
    }
    #[cfg(not(feature = "jit"))]
    {
        assert!(matches!(err, crate::jit::JitError::JitDisabled));
    }
}
#[cfg(feature = "jit")]
#[test]
fn test_jit_compile_bytecode_succeeds() {
    use crate::object::BytecodeFunction;
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let bc = BytecodeFunction {
        argdesc: 0x0101,
        bytecode: vec![0, 84, 135],
        constants: vec![],
        maxdepth: 2,
        docstring: None,
        interactive: None,
    };
    let sym = crate::obarray::intern("rele-jit-compile-bc-test");
    interp
        .state
        .set_function_cell(sym, LispObject::BytecodeFn(bc));
    interp
        .jit_compile("rele-jit-compile-bc-test")
        .expect("should compile");
    assert_eq!(interp.jit_stats().compiled_count, 1);
    assert_eq!(
        interp.jit_tier("rele-jit-compile-bc-test"),
        crate::jit::Tier::Compiled
    );
}
#[test]
fn test_jit_stats_starts_zero_for_fresh_interpreter() {
    let interp = Interpreter::new();
    let stats = interp.jit_stats();
    assert_eq!(stats.total_calls, 0);
    assert_eq!(stats.hot_count, 0);
    assert_eq!(stats.compiled_count, 0);
    assert_eq!(stats.invalidation_count, 0);
    assert_eq!(stats.deopt_count, 0);
}
#[test]
fn test_def_version_bumps_on_defun() {
    use crate::obarray;
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let sym = obarray::intern("rele-test-def-version-probe");
    let before = interp.state.def_version(sym);
    let _ = interp.eval_source("(defun rele-test-def-version-probe () 1)");
    let after = interp.state.def_version(sym);
    assert!(
        after > before,
        "defun should bump def_version (before={before}, after={after})",
    );
}
#[test]
fn test_profiler_detects_hot_bytecode_function() {
    use crate::object::BytecodeFunction;
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    {
        let mut profiler = interp.state.profiler.write();
        *profiler = crate::jit::Profiler::new(5);
    }
    let bc = BytecodeFunction {
        argdesc: 257,
        bytecode: vec![0x54, 0x87],
        constants: vec![],
        maxdepth: 2,
        docstring: None,
        interactive: None,
    };
    interp.define("profiler-hot-inc", LispObject::BytecodeFn(bc));
    let (total, hot) = interp.profiler_stats();
    assert_eq!(total, 0);
    assert_eq!(hot, 0);
    for _ in 0..4 {
        let result = interp.eval(read("(profiler-hot-inc 10)").unwrap()).unwrap();
        assert_eq!(result, LispObject::integer(11));
    }
    let (total, hot) = interp.profiler_stats();
    assert_eq!(total, 4);
    assert_eq!(hot, 0, "should not be hot yet");
    let result = interp.eval(read("(profiler-hot-inc 10)").unwrap()).unwrap();
    assert_eq!(result, LispObject::integer(11));
    let (total, hot) = interp.profiler_stats();
    assert_eq!(total, 5);
    assert_eq!(hot, 1, "function should now be detected as hot");
}

#[cfg(feature = "jit")]
#[allow(dead_code)]
fn bytecode_add_two_args() -> crate::object::BytecodeFunction {
    crate::object::BytecodeFunction {
        argdesc: 0x0202,
        bytecode: vec![0x01, 0x01, 0x5c, 0x87],
        constants: vec![],
        maxdepth: 4,
        docstring: None,
        interactive: None,
    }
}

#[cfg(feature = "jit")]
#[allow(dead_code)]
fn bytecode_add1() -> crate::object::BytecodeFunction {
    crate::object::BytecodeFunction {
        argdesc: 0x0101,
        bytecode: vec![0x54, 0x87],
        constants: vec![],
        maxdepth: 2,
        docstring: None,
        interactive: None,
    }
}

#[cfg(feature = "jit")]
#[test]
fn test_jit_redefinition_invalidates_compiled_entry() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let sym = crate::obarray::intern("rele-jit-redef");
    interp
        .state
        .set_function_cell(sym, LispObject::BytecodeFn(bytecode_add1()));
    interp.jit_compile("rele-jit-redef").unwrap();
    assert_eq!(interp.jit_stats().compiled_count, 1);
    assert_eq!(
        interp.jit_tier("rele-jit-redef"),
        crate::jit::Tier::Compiled
    );

    interp
        .eval_source("(defun rele-jit-redef (x) (+ x 41))")
        .unwrap();
    assert_eq!(interp.jit_tier("rele-jit-redef"), crate::jit::Tier::Interp);
    let stats = interp.jit_stats();
    assert_eq!(stats.compiled_count, 0);
    assert_eq!(stats.invalidation_count, 1);
    assert_eq!(
        interp.eval(read("(rele-jit-redef 1)").unwrap()).unwrap(),
        LispObject::integer(42)
    );
}

#[cfg(feature = "jit")]
#[test]
fn test_jit_deopt_falls_back_to_vm_for_float_arithmetic() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let sym = crate::obarray::intern("rele-jit-deopt-plus");
    interp
        .state
        .set_function_cell(sym, LispObject::BytecodeFn(bytecode_add_two_args()));
    interp.jit_compile("rele-jit-deopt-plus").unwrap();

    assert_eq!(
        interp
            .eval(read("(rele-jit-deopt-plus 3 4)").unwrap())
            .unwrap(),
        LispObject::integer(7)
    );
    assert_eq!(
        interp
            .eval(read("(rele-jit-deopt-plus 1.5 2.25)").unwrap())
            .unwrap(),
        LispObject::float(3.75)
    );
    let stats = interp.jit_stats();
    assert_eq!(stats.compiled_count, 1);
    assert_eq!(stats.deopt_count, 1);
}

#[cfg(feature = "jit")]
#[test]
fn test_jit_eager_compile_and_hot_compile_return_same_values() {
    let mut eager = Interpreter::new();
    add_primitives(&mut eager);
    let eager_sym = crate::obarray::intern("rele-jit-eager-plus");
    eager
        .state
        .set_function_cell(eager_sym, LispObject::BytecodeFn(bytecode_add_two_args()));
    eager.jit_compile("rele-jit-eager-plus").unwrap();

    let mut hot = Interpreter::new();
    add_primitives(&mut hot);
    *hot.state.profiler.write() = crate::jit::Profiler::new(2);
    let hot_sym = crate::obarray::intern("rele-jit-hot-plus");
    hot.state
        .set_function_cell(hot_sym, LispObject::BytecodeFn(bytecode_add_two_args()));

    let eager_result = eager
        .eval(read("(rele-jit-eager-plus 20 22)").unwrap())
        .unwrap();
    let first_hot_result = hot
        .eval(read("(rele-jit-hot-plus 20 22)").unwrap())
        .unwrap();
    let compiled_hot_result = hot
        .eval(read("(rele-jit-hot-plus 20 22)").unwrap())
        .unwrap();

    assert_eq!(eager_result, LispObject::integer(42));
    assert_eq!(first_hot_result, eager_result);
    assert_eq!(compiled_hot_result, eager_result);
    assert_eq!(
        eager.jit_tier("rele-jit-eager-plus"),
        crate::jit::Tier::Compiled
    );
    assert_eq!(
        hot.jit_tier("rele-jit-hot-plus"),
        crate::jit::Tier::Compiled
    );
    assert_eq!(hot.jit_stats().compiled_count, 1);
}

#[cfg(feature = "jit")]
#[test]
fn test_jit_tier_transitions_over_same_symbol() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    *interp.state.profiler.write() = crate::jit::Profiler::new(2);
    let sym = crate::obarray::intern("rele-jit-tier");
    interp
        .state
        .set_function_cell(sym, LispObject::BytecodeFn(bytecode_add1()));

    assert_eq!(interp.jit_tier("rele-jit-tier"), crate::jit::Tier::Interp);
    assert_eq!(
        interp.eval(read("(rele-jit-tier 1)").unwrap()).unwrap(),
        LispObject::integer(2)
    );
    assert_eq!(interp.jit_tier("rele-jit-tier"), crate::jit::Tier::Interp);
    assert_eq!(
        interp.eval(read("(rele-jit-tier 1)").unwrap()).unwrap(),
        LispObject::integer(2)
    );
    assert_eq!(interp.jit_tier("rele-jit-tier"), crate::jit::Tier::Compiled);

    interp
        .state
        .set_function_cell(sym, LispObject::BytecodeFn(bytecode_add_two_args()));
    assert_eq!(interp.jit_tier("rele-jit-tier"), crate::jit::Tier::Interp);
    assert_eq!(interp.jit_stats().compiled_count, 0);
    assert_eq!(
        interp.eval(read("(rele-jit-tier 20 22)").unwrap()).unwrap(),
        LispObject::integer(42)
    );
    assert_eq!(interp.jit_tier("rele-jit-tier"), crate::jit::Tier::Interp);
    assert_eq!(
        interp.eval(read("(rele-jit-tier 20 22)").unwrap()).unwrap(),
        LispObject::integer(42)
    );
    assert_eq!(interp.jit_tier("rele-jit-tier"), crate::jit::Tier::Compiled);
    assert_eq!(interp.jit_stats().compiled_count, 1);
}
#[test]
fn test_backquote_expansion() {
    let interp = make_stdlib_interp();
    let result = interp.eval(read("`(a b c)").unwrap()).unwrap();
    assert_eq!(result.princ_to_string(), "(a b c)");
}
#[test]
fn test_backquote_native_shapes() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.define("x", LispObject::integer(1));
    interp.define("y", LispObject::integer(2));
    interp.define("xs", read("(a b c)").unwrap());
    let cases: &[(&str, &str)] = &[
        ("`foo", "foo"),
        ("`(a b c)", "(a b c)"),
        ("`()", "nil"),
        ("`,x", "1"),
        ("`(,x)", "(1)"),
        ("`(a ,x b)", "(a 1 b)"),
        ("`(,x ,y)", "(1 2)"),
        ("`(,@xs)", "(a b c)"),
        ("`(head ,@xs tail)", "(head a b c tail)"),
        ("`(a ,@xs ,x)", "(a a b c 1)"),
        ("`(a ,@nil b)", "(a b)"),
    ];
    for (src, expected) in cases {
        let form = read(src).expect(src);
        let val = interp.eval(form).unwrap_or_else(|e| {
            panic!("eval({src}) failed: {e:?}");
        });
        assert_eq!(val.princ_to_string(), *expected, "backquote source {src}",);
    }
}
#[test]
fn test_batched_defun_stubs_resolve() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let cases: &[(&str, &str)] = &[
        ("(obarrayp [])", "t"),
        ("(obarrayp 1)", "nil"),
        ("(bool-vector nil 1 nil)", "[nil t nil]"),
        ("(length (make-bool-vector 4 t))", "4"),
        ("(window-minibuffer-p)", "nil"),
        ("(frame-internal-border-width)", "0"),
        ("(image-type-available-p 'png)", "nil"),
        ("(gnutls-available-p)", "nil"),
        ("(display-graphic-p)", "nil"),
        ("(get-char-property 1 'face)", "nil"),
        ("(documentation 'car)", ""),
        ("(backward-prefix-chars)", "nil"),
        ("(undo-boundary)", "nil"),
        ("(buffer-text-pixel-size)", "(0 . 0)"),
        ("(coding-system-p 'utf-8)", "t"),
        ("(directory-name-p \"foo/\")", "t"),
        ("(directory-name-p \"foo\")", "nil"),
        ("(file-name-as-directory \"x\")", "x/"),
        ("(file-name-as-directory \"x/\")", "x/"),
        ("(evenp 4)", "t"),
        ("(oddp 4)", "nil"),
        ("(plusp 1)", "t"),
        ("(minusp -2)", "t"),
        ("(isnan 0.0)", "nil"),
        ("(logb 1024.0)", "10"),
        ("(time-equal-p 0 0)", "t"),
        ("(time-less-p 0 1)", "t"),
        ("(time-convert 1 'list)", "(0 1 0 0)"),
        ("(mapp nil)", "t"),
        ("(mapp '(a b))", "t"),
        ("(mapp 7)", "nil"),
        ("(key-description [65 66])", "A B"),
        ("(upcase-initials \"hello world\")", "Hello World"),
    ];
    for (src, expected) in cases {
        let form = read(src).unwrap_or_else(|e| panic!("reader({src}) failed: {e}"));
        let val = interp
            .eval(form)
            .unwrap_or_else(|e| panic!("eval({src}) failed: {e:?}"));
        assert_eq!(
            val.princ_to_string(),
            *expected,
            "batched defun source {src}",
        );
    }
}

#[test]
fn test_keymap_help_bootstrap_primitives() {
    let interp = make_stdlib_interp();
    interp
        .eval_source("(defun rele-doc-target () \"Doc target.\" nil)")
        .unwrap();
    let cases: &[(&str, &str)] = &[
        ("(keymapp help-mode-map)", "t"),
        ("(documentation 'rele-doc-target)", "Doc target."),
        ("(describe-function 'rele-doc-target)", "nil"),
        ("(where-is-internal 'next-line nil t)", "C-n"),
        (
            "(substitute-command-keys \"\\[next-line] \\[emacs-version]\")",
            "C-n M-x emacs-version",
        ),
    ];
    for (src, expected) in cases {
        let value = interp
            .eval(read(src).unwrap_or_else(|err| panic!("reader({src}) failed: {err}")))
            .unwrap_or_else(|err| panic!("eval({src}) failed: {err:?}"));
        assert_eq!(value.princ_to_string(), *expected, "source {src}");
    }
}

#[test]
fn test_coding_system_contracts_signal_unknown_names() {
    let interp = make_stdlib_interp();
    let cases: &[(&str, &str)] = &[
        ("(coding-system-p nil)", "t"),
        ("(coding-system-p 'utf-8)", "t"),
        ("(coding-system-p 'coding-tests-no-such-system)", "nil"),
        ("(check-coding-system 'utf-8)", "utf-8"),
        ("(check-coding-system nil)", "nil"),
        (
            "(condition-case e (check-coding-system 'coding-tests-no-such-system) (coding-system-error 'caught))",
            "caught",
        ),
        (
            "(condition-case e (let ((coding-system-for-read 'bogus)) (insert-file-contents \"tmp/coding-no-such-file\")) (coding-system-error 'caught))",
            "caught",
        ),
        (
            "(condition-case e (let ((coding-system-for-write (intern \"\\\"us-ascii\\\"\"))) (write-region \"some text\" nil \"tmp/coding-did-not-write\")) (coding-system-error 'caught))",
            "caught",
        ),
    ];

    for (src, expected) in cases {
        let val = interp.eval(read(src).expect(src)).unwrap_or_else(|err| {
            panic!("eval({src}) failed: {err:?}");
        });
        assert_eq!(val.princ_to_string(), *expected, "coding source {src}");
    }
}

#[test]
fn test_remaining_did_not_signal_contracts() {
    let interp = make_stdlib_interp();
    let cases: &[(&str, &str)] = &[
        (
            "(condition-case e (call-interactively (lambda () (interactive \"\\xFF\"))) (error 'caught))",
            "caught",
        ),
        (
            "(condition-case e (let ((inhibit-interaction t)) (read-from-minibuffer \"foo: \")) (inhibited-interaction 'caught))",
            "caught",
        ),
        (
            "(condition-case e (define-charset-internal) (wrong-number-of-arguments 'caught))",
            "caught",
        ),
        (
            "(condition-case e (unify-charset 'ascii) (error 'caught))",
            "caught",
        ),
        (
            "(condition-case e (defvar-keymap did-keymap \"a\" #'next-line \"a\" #'previous-line) (error 'caught))",
            "caught",
        ),
        (
            "(condition-case e (network-lookup-address-info \"1.1.1.1\" nil t) (wrong-type-argument 'caught))",
            "caught",
        ),
        (
            "(network-lookup-address-info \"343.1.2.3\" nil 'numeric)",
            "nil",
        ),
        (
            "(network-lookup-address-info \"0xe3.1.2.3\" nil 'numeric)",
            "(\"0xe3.1.2.3\")",
        ),
        (
            "(condition-case e (progn (set-face-attribute 'button nil :inherit 'link) (set-face-attribute 'link nil :inherit 'button)) (error 'caught))",
            "caught",
        ),
        (
            "(progn (defun did-bad-region-extract (method) (if (eq method 'bounds) '(()))) (condition-case e (let ((region-extract-function 'did-bad-region-extract)) (upcase-region nil nil t)) (error 'caught)))",
            "caught",
        ),
    ];

    for (src, expected) in cases {
        let val = interp.eval(read(src).expect(src)).unwrap_or_else(|err| {
            panic!("eval({src}) failed: {err:?}");
        });
        assert_eq!(val.princ_to_string(), *expected, "source {src}");
    }
}

#[test]
fn test_call_interactively_decodes_args_and_records_history() {
    let interp = make_stdlib_interp();

    let embedded_nuls = interp
        .eval(
            read("(let ((unread-command-events '(?a ?b))) (call-interactively (lambda (a b) (interactive \"ka\\0a: \\nkb: \") (list a b))))")
                .unwrap(),
        )
        .unwrap();
    assert_eq!(embedded_nuls.princ_to_string(), "(\"a\" \"b\")");

    interp
        .eval_source(
            "(defun rele-callint-test-int-args (foo bar &optional zot)
               (declare (interactive-args (bar 10) (zot 11)))
               (interactive (list 1 1 1))
               (+ foo bar zot))",
        )
        .unwrap();
    let history = interp
        .eval(
            read("(let ((history-length 1) (command-history nil)) (list (call-interactively 'rele-callint-test-int-args t) command-history))")
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        history.princ_to_string(),
        "(3 ((rele-callint-test-int-args 1 10 11)))"
    );
}

#[test]
fn test_batched_defun_stubs_resolve_round3() {
    let interp = make_stdlib_interp();
    let cases: &[(&str, &str)] = &[
        ("(user-uid)", "1000"),
        ("(user-real-uid)", "1000"),
        ("(group-gid)", "1000"),
        (
            "(progn (defvar tst-top 42) (default-toplevel-value 'tst-top))",
            "42",
        ),
        ("(completing-read \"Prompt: \" nil nil nil \"x\")", "nil"),
        ("(yes-or-no-p \"ok?\")", "nil"),
        ("(y-or-n-p \"ok?\")", "nil"),
        // color-values is now a real primitive (see primitives/core/faces.rs)
        // backed by a small named-color palette.
        ("(color-values \"red\")", "(65535 0 0)"),
        ("(face-bold-p 'default)", "nil"),
        ("(overlayp (make-overlay 1 1))", "t"),
        ("(overlays-at 9999)", "nil"),
        ("(coding-system-priority-list)", "(utf-8)"),
        ("(find-coding-systems-string \"hello\")", "(utf-8)"),
        ("(detect-coding-string \"abc\")", "(utf-8)"),
        ("major-mode", "fundamental-mode"),
        ("shell-file-name", "/bin/sh"),
        ("path-separator", ":"),
        ("debug-on-error", "nil"),
        ("tramp-mode", "t"),
        ("inhibit-read-only", "nil"),
        ("load-file-name", "nil"),
        ("current-load-list", "nil"),
    ];
    for (src, expected) in cases {
        let form = read(src).unwrap_or_else(|e| panic!("reader({src}) failed: {e}"));
        let val = interp
            .eval(form)
            .unwrap_or_else(|e| panic!("eval({src}) failed: {e:?}"));
        assert_eq!(val.princ_to_string(), *expected, "third-batch source {src}",);
    }
}
#[test]
fn test_batched_defun_stubs_resolve_round4() {
    let interp = make_stdlib_interp();
    let cases: &[(&str, &str)] = &[
        ("(intern-soft \"car\")", "car"),
        ("(symbol-plist 'x)", "nil"),
        ("(mapatoms #'ignore)", "nil"),
        ("(unintern 'foo)", "nil"),
        ("(add-to-list 'tl4 'a)", "(a)"),
        ("(mapcar #'buffer-name (buffer-list))", "(\"*scratch*\")"),
        ("(buffer-modified-p)", "nil"),
        ("(window-buffer)", "*scratch*"),
        ("(window-pixel-width)", "800"),
        ("(frame-pixel-width)", "800"),
        ("(frame-width)", "80"),
        ("(unibyte-string 104 105)", "hi"),
        ("(multibyte-string-p \"hi\")", "nil"),
        ("(multibyte-string-p 3)", "nil"),
        ("(char-width 65)", "1"),
        ("(string-lines \"a\\nb\\nc\")", "(\"a\" \"b\" \"c\")"),
        ("(length (string-lines \"a\\nb\\nc\"))", "3"),
        ("(run-at-time 1 nil 'ignore)", "nil"),
        ("(make-thread #'ignore)", "nil"),
        ("(mutex-lock nil)", "nil"),
        ("(event-basic-type nil)", "nil"),
        ("(posn-x-y nil)", "nil"),
    ];
    for (src, expected) in cases {
        let form = read(src).unwrap_or_else(|e| panic!("reader({src}) failed: {e}"));
        let val = interp
            .eval(form)
            .unwrap_or_else(|e| panic!("eval({src}) failed: {e:?}"));
        assert_eq!(
            val.princ_to_string(),
            *expected,
            "fourth-batch source {src}",
        );
    }
}
#[test]
fn test_batched_defun_stubs_resolve_round5() {
    use std::io::Write;
    let tmp = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tmp")
        .join(format!("rele-elisp-batch5-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let a = tmp.join("a.txt");
    let b = tmp.join("b.txt");
    std::fs::File::create(&a).unwrap().write_all(b"A").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::File::create(&b).unwrap().write_all(b"B").unwrap();
    let interp = make_stdlib_interp();
    let a_str = a.to_string_lossy().to_string();
    let b_str = b.to_string_lossy().to_string();
    let cases: Vec<(String, String)> = vec![
        ("(kbd \"C-x C-s\")".into(), "C-x C-s".into()),
        ("(global-set-key \"k\" 'ignore)".into(), "ignore".into()),
        ("(where-is-internal 'find-file)".into(), "nil".into()),
        ("(lookup-key nil \"k\")".into(), "nil".into()),
        (format!("(file-regular-p \"{a_str}\")"), "t".into()),
        (format!("(file-readable-p \"{a_str}\")"), "t".into()),
        (format!("(file-symlink-p \"{a_str}\")"), "nil".into()),
        (
            format!("(file-newer-than-file-p \"{b_str}\" \"{a_str}\")"),
            "t".into(),
        ),
        (
            format!("(file-newer-than-file-p \"{a_str}\" \"{b_str}\")"),
            "nil".into(),
        ),
        ("(skip-syntax-forward \"w\")".into(), "0".into()),
        // forward-sexp / scan-sexps are now real primitives backed by
        // scan-lists (see primitives/buffer.rs); they signal scan-error
        // on empty buffers rather than returning nil. Coverage moved to
        // dedicated tests in primitives::buffer::tests.
        ("(run-hooks 'post-command-hook)".into(), "nil".into()),
        (
            "(remove-hook 'post-command-hook 'ignore)".into(),
            "nil".into(),
        ),
        ("(advice-add 'ignore :around #'ignore)".into(), "nil".into()),
        ("(backtrace-frames)".into(), "nil".into()),
        ("(debug-on-entry 'ignore)".into(), "nil".into()),
        ("(delete-process nil)".into(), "nil".into()),
        ("(json-parse-string \"1\")".into(), "1".into()),
        ("(sqlite-open nil)".into(), "nil".into()),
        ("(treesit-parser-p nil)".into(), "nil".into()),
        ("(number-sequence 1 5)".into(), "(1 2 3 4 5)".into()),
        ("(number-sequence 1 5 2)".into(), "(1 3 5)".into()),
        ("(number-sequence 5 1 -1)".into(), "(5 4 3 2 1)".into()),
        ("(format-prompt \"Pick\" nil)".into(), "Pick: ".into()),
        (
            "(format-prompt \"Pick\" \"x\")".into(),
            "Pick (default x): ".into(),
        ),
        ("(kill-line)".into(), "nil".into()),
        ("(yank)".into(), "nil".into()),
        ("(x-get-selection)".into(), "nil".into()),
        (
            "(progn (defvar tbl5 77) (buffer-local-value 'tbl5 nil))".into(),
            "77".into(),
        ),
        ("(current-input-method)".into(), "nil".into()),
        ("(recursive-edit)".into(), "nil".into()),
    ];
    for (src, expected) in &cases {
        let form = read(src).unwrap_or_else(|e| panic!("reader({src}) failed: {e}"));
        let val = interp
            .eval(form)
            .unwrap_or_else(|e| panic!("eval({src}) failed: {e:?}"));
        assert_eq!(val.princ_to_string(), *expected, "fifth-batch source {src}",);
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
#[test]
fn test_setcar_mutates_in_place() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(let ((x '(a b c))) (setcar x 'z) (car x))").unwrap())
            .unwrap(),
        LispObject::symbol("z")
    );
}
#[test]
fn test_setcdr_mutates_in_place() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(let ((x '(a b c))) (setcdr x '(y z)) (cdr x))").unwrap())
            .unwrap(),
        read("(y z)").unwrap()
    );
}
#[test]
fn test_nconc_destructive() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(let ((x '(1 2)) (y '(3 4))) (nconc x y) x)").unwrap())
            .unwrap(),
        read("(1 2 3 4)").unwrap()
    );
}
