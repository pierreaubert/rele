#![allow(clippy::disallowed_methods)]
//! Phase 5 semantic parity gates.
//!
//! These tests are intentionally broader than single-regression fixtures:
//! they cover the interpreter and VM seams that must stay boring while we
//! replace bootstrap shortcuts with real Emacs semantics.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;

use rele_elisp::eval::bootstrap::{ensure_stdlib_files, load_full_bootstrap, make_stdlib_interp};
use rele_elisp::{Interpreter, LispObject, add_primitives};

fn make_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
}

fn eval(interp: &Interpreter, src: &str) -> LispObject {
    interp
        .eval_source(src)
        .unwrap_or_else(|err| panic!("eval error in {src:?}: {err:?}"))
}

fn list(items: impl IntoIterator<Item = LispObject>) -> LispObject {
    let mut out = LispObject::nil();
    let mut values: Vec<_> = items.into_iter().collect();
    while let Some(item) = values.pop() {
        out = LispObject::cons(item, out);
    }
    out
}

fn sym(name: &str) -> LispObject {
    LispObject::symbol(name)
}

fn repo_tmp_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under repo/crates/elisp")
        .join("tmp")
        .join(name)
}

fn elisp_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[test]
fn optional_and_rest_args_normalize_like_emacs_lambda_lists() {
    let interp = make_interp();
    let result = eval(
        &interp,
        r#"
(list
  (funcall (lambda (a &optional b c &rest rest) (list a b c rest))
           'a)
  (funcall (lambda (a &optional b c &rest rest) (list a b c rest))
           'a 'b 'c 'd 'e))
"#,
    );

    assert_eq!(
        result,
        list([
            list([
                sym("a"),
                LispObject::nil(),
                LispObject::nil(),
                LispObject::nil(),
            ]),
            list([sym("a"), sym("b"), sym("c"), list([sym("d"), sym("e")])]),
        ])
    );
}

#[test]
fn unwind_protect_runs_cleanup_when_throw_exits_body() {
    let interp = make_interp();
    let result = eval(
        &interp,
        r#"
(progn
  (setq phase5-cleanup-ran nil)
  (list
    (catch 'phase5-done
      (unwind-protect
          (throw 'phase5-done 42)
        (setq phase5-cleanup-ran t)))
    phase5-cleanup-ran))
"#,
    );

    assert_eq!(result, list([LispObject::integer(42), LispObject::t()]));
}

#[test]
fn byte_code_literal_executes_through_vm_and_matches_interpreter_constant() {
    let interp = make_interp();
    let interpreted = eval(&interp, "42");
    let vm = eval(&interp, r#"(byte-code "\xc0\x87" [42] 1)"#);

    assert_eq!(vm, interpreted);
}

#[test]
fn bytecode_call_path_observes_function_redefinition() {
    let interp = make_interp();
    let result = eval(
        &interp,
        r#"
(progn
  (defun phase5-redef-target () 1)
  (let ((before (byte-code "\300 \207" [phase5-redef-target] 2)))
    (defun phase5-redef-target () 2)
    (list before
          (phase5-redef-target)
          (byte-code "\300 \207" [phase5-redef-target] 2))))
"#,
    );

    assert_eq!(
        result,
        list([
            LispObject::integer(1),
            LispObject::integer(2),
            LispObject::integer(2),
        ])
    );
}

#[test]
fn lexical_closures_and_special_variables_keep_distinct_binding_rules() {
    let interp = make_interp();
    let result = eval(
        &interp,
        r#"
(progn
  (defvar phase5-special 1)
  (list
    (let ((phase5-lexical 10))
      (let ((capture (function (lambda () phase5-lexical))))
        (let ((phase5-lexical 20))
          (funcall capture))))
    (let ((phase5-special 10))
      (let ((capture (function (lambda () phase5-special))))
        (let ((phase5-special 20))
          (funcall capture))))))
"#,
    );

    assert_eq!(
        result,
        list([LispObject::integer(10), LispObject::integer(20)])
    );
}

#[test]
fn features_and_match_data_are_interpreter_local() {
    let a = make_interp();
    let b = make_interp();

    eval(&a, "(provide 'phase5-isolated-feature)");
    assert_eq!(
        eval(&a, "(featurep 'phase5-isolated-feature)"),
        LispObject::t()
    );
    assert_eq!(
        eval(&b, "(featurep 'phase5-isolated-feature)"),
        LispObject::nil()
    );

    eval(&a, r#"(string-match "f\\(oo\\)" "xxfoobar")"#);
    assert_eq!(eval(&a, "(match-string 1)"), LispObject::string("oo"));
    assert_eq!(eval(&b, "(match-beginning 0)"), LispObject::nil());
}

#[test]
fn eieio_class_registry_is_interpreter_local() {
    let a = make_interp();
    let b = make_interp();

    let result = eval(
        &a,
        r#"
(progn
  (defclass phase5-eieio-local ()
    ((slot :initarg :slot)))
  (list
    (find-class 'phase5-eieio-local)
    (slot-value (make-instance 'phase5-eieio-local :slot 7) 'slot)))
"#,
    );

    assert_eq!(
        result,
        list([sym("phase5-eieio-local"), LispObject::integer(7)])
    );
    assert_eq!(
        eval(&b, "(find-class 'phase5-eieio-local)"),
        LispObject::nil()
    );
}

#[test]
fn current_buffer_registry_is_parallel_worker_local() {
    let barrier = Arc::new(Barrier::new(2));
    let a_barrier = barrier.clone();
    let a = thread::spawn(move || {
        let interp = make_interp();
        eval(
            &interp,
            r#"(progn
                 (set-buffer "phase5-thread-buffer")
                 (insert "from-a"))"#,
        );
        a_barrier.wait();
        assert_eq!(
            eval(&interp, "(buffer-string)"),
            LispObject::string("from-a")
        );
    });

    let b_barrier = barrier;
    let b = thread::spawn(move || {
        let interp = make_interp();
        b_barrier.wait();
        assert_eq!(eval(&interp, "(buffer-string)"), LispObject::string(""));
    });

    a.join().expect("thread A panicked");
    b.join().expect("thread B panicked");
}

#[test]
fn keymap_fidelity_covers_canonical_keys_remaps_and_where_is() {
    let interp = make_interp();
    let result = eval(
        &interp,
        r#"
(progn
  (global-set-key (kbd "control-x   control-s") 'phase5-save)
  (define-key (current-global-map) [remap phase5-old] 'phase5-new)
  (let ((menu-map (make-sparse-keymap)))
    (define-key menu-map (kbd "return") '(menu-item "Open" phase5-open))
    (list
      (key-binding (key-parse "C-x C-s"))
      (where-is-internal 'phase5-save)
      (command-remapping 'phase5-old)
      (car (lookup-key menu-map (kbd "RET"))))))
"#,
    );

    assert_eq!(
        result,
        list([
            sym("phase5-save"),
            list([LispObject::string("C-x C-s")]),
            sym("phase5-new"),
            sym("menu-item"),
        ])
    );
}

#[test]
fn runtime_metadata_replaces_pre_jit_noops() {
    let interp = make_interp();
    let result = eval(
        &interp,
        r#"
(progn
  (define-coding-system 'phase5-coding :mnemonic "P")
  (coding-system-put 'phase5-coding 'phase5-prop 99)
  (define-coding-system-alias 'phase5-alias 'phase5-coding)
  (set-coding-system-priority 'phase5-coding 'utf-8)
  (define-translation-table 'phase5-trans '((1 . 2)))
  (custom-declare-variable 'phase5-custom 7 "doc" :type 'integer)
  (custom-add-version 'phase5-custom "1.0")
  (advice-add 'phase5-advised :around #'ignore)
  (let ((advised-before (advice-member-p #'ignore 'phase5-advised)))
    (advice-remove 'phase5-advised #'ignore)
    (list
      (coding-system-p 'phase5-coding)
      (coding-system-base 'phase5-alias)
      (coding-system-get 'phase5-coding 'phase5-prop)
      (car (coding-system-priority-list))
      (translation-table-id 'phase5-trans)
      (and (get 'phase5-custom 'custom-metadata) t)
      (get 'phase5-custom 'custom-version)
      advised-before
      (advice-member-p #'ignore 'phase5-advised))))
"#,
    );

    assert_eq!(
        result,
        list([
            LispObject::t(),
            sym("phase5-coding"),
            LispObject::integer(99),
            sym("phase5-coding"),
            sym("phase5-trans"),
            LispObject::t(),
            LispObject::string("1.0"),
            LispObject::t(),
            LispObject::nil(),
        ])
    );
}

#[test]
fn bootstrap_shortcut_shapes_are_pinned_before_replacement() {
    let interp = make_interp();
    eval(
        &interp,
        "(let ((map '((1 . 2))))
           (setq phase5-translation-shortcut-ran t)
           (define-translation-table 'phase5-demo map)
           (mapc (lambda (_) (error \"translation shortcut widened\")) map))",
    );
    assert_eq!(
        eval(&interp, "(boundp 'phase5-translation-shortcut-ran)"),
        LispObject::nil()
    );
    assert_eq!(
        eval(&interp, "(translation-table-id 'phase5-demo)"),
        sym("phase5-demo")
    );

    eval(
        &interp,
        "(let ((standard '((phase5 . ignored)))
               (quoter 'identity))
           (setq phase5-cus-start-shortcut-ran t)
           (pcase-dolist (`(,sym . ,value) standard)
             (error \"cus-start shortcut widened\")))",
    );
    assert_eq!(
        eval(&interp, "(boundp 'phase5-cus-start-shortcut-ran)"),
        LispObject::nil()
    );
}

#[test]
fn load_require_and_provide_round_trip_through_real_file() {
    let interp = make_stdlib_interp();
    let dir = repo_tmp_dir(&format!("elisp-phase5-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create phase5 tmp dir");

    let feature = "phase5-require-feature";
    fs::write(
        dir.join(format!("{feature}.el")),
        "(setq phase5-required-value 42)\n(provide 'phase5-require-feature)\n",
    )
    .expect("write phase5 feature file");

    let dir_str = elisp_string(&dir.to_string_lossy());
    let result = eval(
        &interp,
        &format!(
            r#"
(progn
  (setq load-path (list "{dir_str}"))
  (eval-after-load "{feature}"
    '(setq phase5-after-load-value (+ phase5-required-value 1)))
  (require '{feature})
  (list (featurep '{feature}) phase5-required-value phase5-after-load-value after-load-alist))
"#
        ),
    );

    let _ = fs::remove_dir_all(&dir);
    assert_eq!(
        result,
        list([
            LispObject::t(),
            LispObject::integer(42),
            LispObject::integer(43),
            LispObject::nil(),
        ])
    );
}

#[test]
fn bootstrapped_stdlib_macro_and_function_remain_executable() {
    let handle = thread::Builder::new()
        .name("phase5-stdlib-parity".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            if !ensure_stdlib_files() {
                eprintln!("skipping phase5 stdlib parity test: Emacs stdlib files unavailable");
                return;
            }

            let interp = make_stdlib_interp();
            eval(&interp, "(defmacro phase5-plus-one (x) (list '+ x 1))");
            load_full_bootstrap(&interp);
            let result = eval(
                &interp,
                r#"
(list
  (let ((expanded (macroexpand '(phase5-plus-one 41))))
    (and (consp expanded) (eq (car expanded) '+)))
  (seq-empty-p nil))
"#,
            );

            assert_eq!(result, list([LispObject::t(), LispObject::t()]));
        })
        .expect("spawn phase5 stdlib parity test");

    handle.join().expect("phase5 stdlib parity test panicked");
}
