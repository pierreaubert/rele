//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

#[allow(unused_imports)]
use super::*;

#[test]
fn test_emacs_small_test_files_load() {
    let Some(root) = emacs_source_root() else {
        return;
    };
    if !ensure_stdlib_files() {
        return;
    }
    let files = [
        "test/lisp/emacs-lisp/cl-lib-tests.el",
        "test/lisp/emacs-lisp/cl-extra-tests.el",
        "test/src/data-tests.el",
        "test/src/fns-tests.el",
    ];
    let root = root.to_string();
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let interp = make_stdlib_interp();
            let _ = probe_emacs_file(&interp, &format!("{}/lisp/emacs-lisp/ert.el", root));
            for f in files {
                let path = format!("{root}/{f}");
                match probe_emacs_file(&interp, &path) {
                    Some((ok, total)) => {
                        let pct = if total > 0 { ok * 100 / total } else { 0 };
                        eprintln!("  {f}: {ok}/{total} ({pct}%)");
                    }
                    None => eprintln!("  {f}: (not readable)"),
                }
            }
        })
        .expect("failed to spawn ERT test-file probe thread");
    handle.join().expect("ERT test-file probe thread panicked");
}
/// Probe how much of `lisp/dired.el` (the real Emacs file) we can
/// load. Reports a `forms-ok / total` ratio + first 10 form errors
/// so we can see exactly where the loader bails. Skips silently if
/// the Emacs source tree isn't present (CI machines without it).
#[test]
fn test_dired_load_progress() {
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
            // dired's body uses dnd, dired-loaddefs, autorevert, desktop;
            // running `(dired ...)` further needs files / fileio / simple
            // for `find-file-visit-truename`, `default-directory`, etc.
            // Load subr.el first, then immediately install our
            // dolist-based workarounds for `internal--build-binding(s)`
            // before any other library that uses if-let* / when-let*
            // can call them.
            let _ = probe_emacs_file(&interp, &format!("{root}/lisp/subr.el"));
            let workaround = r#"
(defun internal--build-binding (binding prev-var)
  (let ((b (cond
            ((symbolp binding) (list binding binding))
            ((null (cdr binding)) (list (make-symbol "s") (car binding)))
            ((eq '_ (car binding)) (list (make-symbol "s") (cadr binding)))
            (t binding))))
    (when (> (length b) 2)
      (signal 'error (cons "`let' bindings can have only one value-form" b)))
    (list (car b) (list 'and prev-var (cadr b)))))

(defun internal--build-bindings (bindings)
  (let ((prev-var t) (out nil))
    (dolist (binding bindings)
      (let ((entry (internal--build-binding binding prev-var)))
        (push entry out)
        (setq prev-var (car entry))))
    (nreverse out)))
"#;
            for form in crate::read_all(workaround).unwrap() {
                interp.eval(form).expect("workaround eval");
            }
            for prereq in [
                "lisp/emacs-lisp/seq.el",
                "lisp/emacs-lisp/easymenu.el",
                "lisp/menu-bar.el",
                "lisp/files.el",
                "lisp/dnd.el",
                "lisp/autorevert.el",
                "lisp/dired-loaddefs.el",
            ] {
                let path = format!("{root}/{prereq}");
                if let Some((ok, total)) = probe_emacs_file(&interp, &path) {
                    eprintln!("  prereq {prereq}: {ok}/{total}");
                }
            }
            let path = format!("{root}/lisp/dired.el");
            if let Ok(src) = std::fs::read_to_string(&path) {
                let (ok, total, errs) = load_file_progress(&interp, &src);
                let pct = if total > 0 { ok * 100 / total } else { 0 };
                eprintln!("  lisp/dired.el: {ok}/{total} ({pct}%)");
                for (i, e) in errs.iter().take(10) {
                    eprintln!("    form #{i}: {e}");
                }
            } else {
                eprintln!("  lisp/dired.el: (not readable)");
            }
            // Now actually try invoking dired on a real directory and see
            // what it errors out with — the next set of work to make
            // `M-x dired` real.
            let tmp = std::env::temp_dir();
            let tmp_str = tmp.display().to_string();
            // Probe the dired call chain: each form is wrapped in
            // `condition-case` so a failure surfaces as data, not a
            // panic. Reports show how far execution gets so we can
            // watch the gap close as Phases 1-2 add buffer + file
            // primitives.
            for (label, src) in [
                ("expand-file-name", format!("(expand-file-name \"{tmp_str}\")")),
                ("abbreviate-file-name", format!("(abbreviate-file-name \"{tmp_str}\")")),
                ("dired-noselect", format!("(prin1-to-string (condition-case e (dired-noselect \"{tmp_str}\") (error e)))")),
                ("dired", format!("(prin1-to-string (condition-case e (dired \"{tmp_str}\") (error e)))")),
            ] {
                match crate::read_all(&src) {
                    Ok(mut forms) => {
                        if let Some(form) = forms.pop() {
                            interp.reset_eval_ops();
                            interp.set_eval_ops_limit(5_000_000);
                            interp.set_deadline(std::time::Instant::now()
                                + std::time::Duration::from_secs(5));
                            match interp.eval(form) {
                                Ok(v) => eprintln!("  {label} => {v:?}"),
                                Err(e) => eprintln!("  {label} ERROR: {e}"),
                            }
                            interp.set_eval_ops_limit(0);
                            interp.clear_deadline();
                        }
                    }
                    Err(e) => eprintln!("  {label} parse error: {e}"),
                }
            }
        })
        .expect("failed to spawn dired probe thread");
    handle.join().expect("dired probe thread panicked");
}

mod module_stubs {
    #[allow(unused_imports)]
    use crate::{Interpreter, LispObject, add_primitives, primitives_modules, read};
    /// Helper to create an interpreter with module stubs
    #[allow(dead_code)]
    fn make_interp() -> Interpreter {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        primitives_modules::register(&mut interp);
        interp
    }
    #[test]
    fn test_eshell_stub() {
        let interp = make_interp();
        let expr = read("(eshell)").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(result, LispObject::nil(), "eshell should return nil");
    }
    #[test]
    fn test_erc_mode_stub() {
        let interp = make_interp();
        let expr = read("(erc-mode)").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(result, LispObject::nil(), "erc-mode should return nil");
    }
    #[test]
    fn test_eshell_command_result_stub() {
        let interp = make_interp();
        let expr = read("(eshell-command-result)").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(
            result,
            LispObject::string(""),
            "eshell-command-result should return empty string"
        );
    }
    #[test]
    fn test_eshell_extended_glob_identity() {
        let interp = make_interp();
        let expr = read("(eshell-extended-glob \"test-value\")").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(
            result,
            LispObject::string("test-value"),
            "eshell-extended-glob should return its argument unchanged"
        );
    }
    #[test]
    fn test_eshell_stringify() {
        let interp = make_interp();
        let expr = read("(eshell-stringify 42)").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(
            result,
            LispObject::string("42"),
            "eshell-stringify should format its argument as a string"
        );
    }
    #[test]
    fn test_erc_d_t_with_cleanup_macro() {
        let interp = make_interp();
        let expr = read("(erc-d-t-with-cleanup (+ 1 2))").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(
            result,
            LispObject::integer(3),
            "erc-d-t-with-cleanup should expand body and return its result"
        );
    }
    #[test]
    fn test_semantic_mode_stub() {
        let interp = make_interp();
        let expr = read("(semantic-mode)").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(result, LispObject::nil(), "semantic-mode should return nil");
    }
    #[test]
    fn test_ispell_stub() {
        let interp = make_interp();
        let expr = read("(ispell-tests--some-backend-available-p)").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(
            result,
            LispObject::nil(),
            "ispell-tests--some-backend-available-p should return nil"
        );
    }
    #[test]
    fn test_url_generic_parse_url_stub() {
        let interp = make_interp();
        let expr = read("(url-generic-parse-url)").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(
            result,
            LispObject::nil(),
            "url-generic-parse-url should return nil"
        );
    }
    #[test]
    fn test_tramp_stub() {
        let interp = make_interp();
        let expr = read("(tramp-mode)").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(result, LispObject::nil(), "tramp-mode should return nil");
    }
    #[test]
    fn test_gnus_stub() {
        let interp = make_interp();
        let expr = read("(gnus-mode)").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(result, LispObject::nil(), "gnus-mode should return nil");
    }
    #[test]
    fn test_rcirc_stub() {
        let interp = make_interp();
        let expr = read("(rcirc-mode)").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(result, LispObject::nil(), "rcirc-mode should return nil");
    }
    #[test]
    fn test_message_mode_stub() {
        let interp = make_interp();
        let expr = read("(message-mode)").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(result, LispObject::nil(), "message-mode should return nil");
    }
    #[test]
    fn test_w3m_mode_stub() {
        let interp = make_interp();
        let expr = read("(w3m-mode)").unwrap();
        let result = interp.eval(expr).unwrap();
        assert_eq!(result, LispObject::nil(), "w3m-mode should return nil");
    }
}
