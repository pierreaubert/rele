/// R5 void-variable fixtures round 2 — tests for legitimate Emacs globals
///
/// This test module verifies that all void-variable stubs added in R5
/// are properly initialized by the shared runtime bootstrap helper.
use rele_elisp::eval::bootstrap::make_stdlib_interp;
use rele_elisp::{LispObject, read};

#[test]
fn test_r5_eshell_last_output_end() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'eshell-last-output-end)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(
        result,
        LispObject::nil(),
        "eshell-last-output-end should be nil"
    );
}

#[test]
fn test_r5_shortdoc_groups() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'shortdoc--groups)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::nil(), "shortdoc--groups should be nil");
}

#[test]
fn test_r5_erc_modules() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'erc-modules)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::nil(), "erc-modules should be nil");
}

#[test]
fn test_r5_erc_autojoin_delay() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'erc-autojoin-delay)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(
        result,
        LispObject::nil(),
        "erc-autojoin-delay should be nil"
    );
}

#[test]
fn test_r5_erc_reuse_buffers() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'erc-reuse-buffers)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::nil(), "erc-reuse-buffers should be nil");
}

#[test]
fn test_r5_macroexp_dynvars() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'macroexp--dynvars)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::nil(), "macroexp--dynvars should be nil");
}

#[test]
fn test_r5_executing_kbd_macro() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'executing-kbd-macro)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(
        result,
        LispObject::nil(),
        "executing-kbd-macro should be nil"
    );
}

#[test]
fn test_r5_require_public_key() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'require-public-key)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(
        result,
        LispObject::nil(),
        "require-public-key should be nil"
    );
}

#[test]
fn test_r5_delete_by_moving_to_trash() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'delete-by-moving-to-trash)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(
        result,
        LispObject::nil(),
        "delete-by-moving-to-trash should be nil"
    );
}

#[test]
fn test_r5_syntax_propertize_done() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'syntax-propertize--done)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(
        result,
        LispObject::nil(),
        "syntax-propertize--done should be nil"
    );
}

#[test]
fn test_r5_parse_sexp_lookup_properties() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'parse-sexp-lookup-properties)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(
        result,
        LispObject::nil(),
        "parse-sexp-lookup-properties should be nil"
    );
}

#[test]
fn test_r5_minibuffer_auto_raise() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'minibuffer-auto-raise)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(
        result,
        LispObject::nil(),
        "minibuffer-auto-raise should be nil"
    );
}

#[test]
fn test_r5_so_long_file_local_mode_function() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'so-long-file-local-mode-function)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(
        result,
        LispObject::nil(),
        "so-long-file-local-mode-function should be nil"
    );
}

#[test]
fn test_r5_window_system() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'window-system)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::nil(), "window-system should be nil");
}

#[test]
fn test_r5_mh_sys_path() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'mh-sys-path)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::nil(), "mh-sys-path should be nil");
}

#[test]
fn test_r5_mh_cmd_note() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'mh-cmd-note)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::integer(0), "mh-cmd-note should be 0");
}

#[test]
fn test_r5_tramp_methods() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'tramp-methods)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::nil(), "tramp-methods should be nil");
}

#[test]
fn test_r5_eshell_ls_use_in_dired() {
    let interp = make_stdlib_interp();
    let expr = read("(symbol-value 'eshell-ls-use-in-dired)").unwrap();
    let result = interp.eval(expr).unwrap();
    assert_eq!(
        result,
        LispObject::nil(),
        "eshell-ls-use-in-dired should be nil"
    );
}
