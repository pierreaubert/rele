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
