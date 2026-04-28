//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use crate::add_primitives;
use crate::eval::Interpreter;
use crate::object::LispObject;
use crate::read;

use super::functions::STDLIB_DIR;
use super::{emacs_lisp_dir, emacs_source_root};

pub fn make_stdlib_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    crate::primitives_modules::register(&mut interp);
    interp.define("backtrace-on-error-noninteractive", LispObject::nil());
    interp.define(
        "most-positive-fixnum",
        LispObject::integer((1_i64 << 47) - 1),
    );
    interp.define("most-negative-fixnum", LispObject::integer(-(1_i64 << 47)));
    interp.define("emacs-version", LispObject::string("30.2"));
    interp.define("emacs-major-version", LispObject::integer(30));
    interp.define("emacs-minor-version", LispObject::integer(2));
    interp.define("system-type", LispObject::symbol("darwin"));
    interp.define("noninteractive", LispObject::t());
    interp.define(
        "load-suffixes",
        LispObject::cons(
            LispObject::string(".elc"),
            LispObject::cons(LispObject::string(".el"), LispObject::nil()),
        ),
    );
    interp.define(
        "load-file-rep-suffixes",
        LispObject::cons(LispObject::string(""), LispObject::nil()),
    );
    let mut env_list = LispObject::nil();
    for (k, v) in std::env::vars() {
        env_list = LispObject::cons(LispObject::string(&format!("{k}={v}")), env_list);
    }
    interp.define("process-environment", env_list);
    interp.define("exec-path", LispObject::nil());
    interp.define(
        "exec-suffixes",
        LispObject::cons(LispObject::string(""), LispObject::nil()),
    );
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    interp.define(
        "user-emacs-directory",
        LispObject::string(&format!("{home}/.emacs.d/")),
    );
    interp.define("user-full-name", LispObject::string(""));
    interp.define("user-login-name", LispObject::string(""));
    interp.define("user-real-login-name", LispObject::string(""));
    interp.define("user-mail-address", LispObject::nil());
    interp.define("temporary-file-directory", LispObject::string("/tmp/"));
    interp.define("small-temporary-file-directory", LispObject::nil());
    interp.define("invocation-directory", LispObject::string("/"));
    interp.define("invocation-name", LispObject::string("emacs"));
    interp.define("bare-symbol", LispObject::primitive("identity"));
    interp.define("symbol-with-pos-p", LispObject::primitive("ignore"));
    interp.define("byte-run--ssp-seen", LispObject::nil());
    interp.define("mapbacktrace", LispObject::nil());
    interp.define("byte-compile-macro-environment", LispObject::nil());
    interp.define("macro-declaration-function", LispObject::nil());
    interp.define("byte-run--set-speed", LispObject::nil());
    interp.define("purify-flag", LispObject::nil());
    interp.define("delayed-warnings-list", LispObject::nil());
    interp.define("purecopy", LispObject::primitive("identity"));
    interp.define("make-byte-code", LispObject::primitive("ignore"));
    interp.define("set-standard-case-table", LispObject::primitive("ignore"));
    interp.define("downcase-region", LispObject::primitive("ignore"));
    interp.define("upcase-region", LispObject::primitive("ignore"));
    interp.define("capitalize-region", LispObject::primitive("ignore"));
    interp.define(
        "replace-regexp-in-string",
        LispObject::primitive("identity"),
    );
    interp.define("string-collate-lessp", LispObject::primitive("ignore"));
    interp.define("mapconcat", LispObject::primitive("ignore"));
    interp.define("process-attributes", LispObject::primitive("ignore"));
    interp.define("set-process-sentinel", LispObject::primitive("ignore"));
    interp.define("event-modifiers", LispObject::primitive("ignore"));
    interp.define("event-basic-type", LispObject::primitive("ignore"));
    interp.define("read-event", LispObject::primitive("ignore"));
    interp.define("listify-key-sequence", LispObject::primitive("ignore"));
    interp.define("features", LispObject::nil());
    interp.define("obarray", LispObject::nil());
    {
        let char_table = LispObject::Vector(std::sync::Arc::new(crate::eval::SyncRefCell::new(
            vec![LispObject::nil(); 0x10000],
        )));
        let global_map = LispObject::cons(
            LispObject::symbol("keymap"),
            LispObject::cons(char_table, LispObject::nil()),
        );
        interp.define("global-map", global_map);
    }
    interp.define("ctl-x-map", LispObject::nil());
    interp.define("ctl-x-4-map", LispObject::nil());
    interp.define("ctl-x-5-map", LispObject::nil());
    interp.define("esc-map", LispObject::nil());
    interp.define("help-map", LispObject::nil());
    interp.define("mode-specific-map", LispObject::nil());
    interp.define("special-event-map", LispObject::nil());
    interp.define("minor-mode-map-alist", LispObject::nil());
    interp.define("emulation-mode-map-alists", LispObject::nil());
    interp.define("auto-fill-chars", LispObject::nil());
    interp.define("char-script-table", LispObject::nil());
    interp.define("char-width-table", LispObject::nil());
    interp.define("printable-chars", LispObject::nil());
    interp.define("word-combining-categories", LispObject::nil());
    interp.define("word-separating-categories", LispObject::nil());
    interp.define("ambiguous-width-chars", LispObject::nil());
    interp.define("translation-table-for-input", LispObject::nil());
    interp.define("use-default-ascent", LispObject::nil());
    interp.define("ignored-local-variables", LispObject::nil());
    interp.define("find-word-boundary-function-table", LispObject::nil());
    interp.define(
        "latin-extra-code-table",
        LispObject::Vector(std::sync::Arc::new(crate::eval::SyncRefCell::new(
            vec![LispObject::nil(); 256],
        ))),
    );
    interp.define("buffer-invisibility-spec", LispObject::nil());
    // `menu-bar-separator` is `(defconst menu-bar-separator '("--"))`
    // in `subr.el`. Define it up front so files that reference it as
    // a *value* (`dired.el` uses it inside menu definitions) don't hit
    // void-variable when subr.el's `defconst` hasn't been replayed.
    interp.define(
        "menu-bar-separator",
        LispObject::cons(LispObject::string("--"), LispObject::nil()),
    );
    // Defcustoms from uniquify.el / files.el / dired.el that are
    // referenced before their defining file loads. Define them as
    // nil so callers that just check (if VAR ...) get the expected
    // false value.
    for name in [
        "uniquify-trailing-separator-flag",
        "uniquify-buffer-name-style",
        "uniquify-after-kill-buffer-p",
        "uniquify-min-dir-content",
        "uniquify-strip-common-suffix",
        "uniquify-list-buffers-directory-modes",
        "find-file-visit-truename",
        "find-file-existing-other-name",
        "remote-file-name-inhibit-cache",
        "default-directory",
        "buffer-display-table",
    ] {
        interp.define(name, LispObject::nil());
    }
    interp.define("unicode-category-table", LispObject::nil());
    interp.define("case-replace", LispObject::t());
    interp
        .eval(read("(defmacro rx (&rest forms) (rele--rx-translate forms))").unwrap())
        .expect("defmacro rx should succeed");
    interp.define("bol", LispObject::string(""));
    interp.define("eol", LispObject::string(""));
    interp.define("dump-mode", LispObject::nil());
    interp.define("emacs-build-time", LispObject::nil());
    interp.define("emacs-save-session-functions", LispObject::nil());
    interp.define("command-error-function", LispObject::nil());
    interp.define("command-error-default-function", LispObject::nil());
    interp.define("delayed-warnings-list", LispObject::nil());
    interp.define("delayed-warnings-hook", LispObject::nil());
    interp.define("regexp", LispObject::primitive("identity"));
    interp.define("standard-category-table", LispObject::primitive("ignore"));
    interp.define("search-spaces-regexp", LispObject::nil());
    interp.define("print-escape-newlines", LispObject::nil());
    interp.define("standard-output", LispObject::t());
    interp.define("load-path", LispObject::nil());
    interp.define("data-directory", LispObject::string("/usr/share/emacs"));
    interp.define("autoload", LispObject::primitive("ignore"));
    interp.define("default-boundp", LispObject::primitive("ignore"));
    interp.define("minibuffer-local-map", LispObject::nil());
    interp.define("minibuffer-local-ns-map", LispObject::nil());
    interp.define("minibuffer-local-completion-map", LispObject::nil());
    interp.define("minibuffer-local-must-match-map", LispObject::nil());
    interp.define(
        "minibuffer-local-filename-completion-map",
        LispObject::nil(),
    );
    interp.define("C-@", LispObject::integer(0));
    interp.define("meta-prefix-char", LispObject::integer(27));
    interp.define("set-default", LispObject::primitive("set-default"));
    interp.define("remap", LispObject::nil());
    interp.define("hash-table-p", LispObject::primitive("ignore"));
    interp.define(
        "local-variable-if-set-p",
        LispObject::primitive("local-variable-if-set-p"),
    );
    interp.define(
        "make-local-variable",
        LispObject::primitive("make-local-variable"),
    );
    interp.define(
        "local-variable-p",
        LispObject::primitive("local-variable-p"),
    );
    interp.define(
        "variable-binding-locus",
        LispObject::primitive("variable-binding-locus"),
    );
    interp.define(
        "default-toplevel-value",
        LispObject::primitive("default-toplevel-value"),
    );
    interp.define(
        "set-default-toplevel-value",
        LispObject::primitive("set-default-toplevel-value"),
    );
    interp.define(
        "buffer-local-toplevel-value",
        LispObject::primitive("buffer-local-toplevel-value"),
    );
    interp.define(
        "set-buffer-local-toplevel-value",
        LispObject::primitive("set-buffer-local-toplevel-value"),
    );
    interp.define("exit-minibuffer", LispObject::nil());
    interp.define("self-insert-command", LispObject::nil());
    interp.define("undefined", LispObject::nil());
    interp.define("minibuffer-recenter-top-bottom", LispObject::nil());
    interp.define("symbol-value", LispObject::primitive("symbol-value"));
    interp.define("default-value", LispObject::primitive("default-value"));
    interp.define("recenter-top-bottom", LispObject::nil());
    interp.define("keymap-set-after", LispObject::primitive("ignore"));
    interp.define("key-valid-p", LispObject::primitive("ignore"));
    interp.define("text-quoting-style", LispObject::symbol("grave"));
    interp.define("scroll-up-command", LispObject::nil());
    interp.define("scroll-down-command", LispObject::nil());
    interp.define("beginning-of-buffer", LispObject::nil());
    interp.define("end-of-buffer", LispObject::nil());
    interp.define("scroll-other-window", LispObject::nil());
    interp.define("scroll-other-window-down", LispObject::nil());
    interp.define("isearch-forward", LispObject::nil());
    interp.define("isearch-backward", LispObject::nil());
    interp.define("emacs-pid", LispObject::primitive("ignore"));
    interp.define("process-attributes", LispObject::primitive("ignore"));
    interp.define("suspend-emacs", LispObject::nil());
    interp.define("emacs", LispObject::nil());
    interp.define("propertize", LispObject::primitive("identity"));
    interp.define("current-time-string", LispObject::primitive("ignore"));
    interp.define(
        "function-key-map",
        LispObject::Vector(std::sync::Arc::new(crate::eval::SyncRefCell::new(
            Vec::new(),
        ))),
    );
    interp.define(
        "exec-path",
        LispObject::cons(
            LispObject::string("/usr/bin"),
            LispObject::cons(LispObject::string("/bin"), LispObject::nil()),
        ),
    );
    interp.define("pre-redisplay-function", LispObject::nil());
    interp.define("pre-redisplay-functions", LispObject::nil());
    interp.define("window-size-change-functions", LispObject::nil());
    interp.define("window-configuration-change-hook", LispObject::nil());
    interp.define("buffer-list-update-hook", LispObject::nil());
    interp.define("make-overlay", LispObject::primitive("make-overlay"));
    interp.define(
        "custom-add-option",
        LispObject::primitive("custom-add-option"),
    );
    interp.define(
        "custom-add-version",
        LispObject::primitive("custom-add-version"),
    );
    interp.define(
        "custom-declare-variable",
        LispObject::primitive("custom-declare-variable"),
    );
    interp.define(
        "custom-declare-face",
        LispObject::primitive("custom-declare-face"),
    );
    interp.define(
        "custom-declare-group",
        LispObject::primitive("custom-declare-group"),
    );
    interp.define("keymapp", LispObject::primitive("keymapp"));
    interp.define("map-keymap", LispObject::primitive("ignore"));
    interp.define("key-parse", LispObject::primitive("key-parse"));
    interp.define("keymap--check", LispObject::primitive("ignore"));
    interp.define("keymap--compile-check", LispObject::primitive("ignore"));
    interp.define("byte-compile-warn", LispObject::primitive("ignore"));
    interp.define(
        "describe-bindings-internal",
        LispObject::primitive("ignore"),
    );
    interp.define("event-convert-list", LispObject::primitive("ignore"));
    interp.define("kbd", LispObject::primitive("kbd"));
    interp.define("emacs-build-number", LispObject::integer(1));
    interp.define("emacs-build-time", LispObject::nil());
    interp.define("emacs-repository-version", LispObject::string("unknown"));
    interp.define(
        "system-configuration",
        LispObject::string("aarch64-apple-darwin"),
    );
    interp.define("system-configuration-options", LispObject::string(""));
    interp.define("system-configuration-features", LispObject::string(""));
    interp.define("define-charset-alias", LispObject::primitive("ignore"));
    interp.define("define-charset", LispObject::primitive("ignore"));
    interp.define("put-charset-property", LispObject::primitive("ignore"));
    interp.define("get-charset-property", LispObject::primitive("ignore"));
    interp.define("charset-plist", LispObject::primitive("ignore"));
    interp.define("define-charset-internal", LispObject::primitive("ignore"));
    interp.define("charset-dimension", LispObject::primitive("ignore"));
    interp.define(
        "define-coding-system",
        LispObject::primitive("define-coding-system"),
    );
    interp.define(
        "define-coding-system-alias",
        LispObject::primitive("define-coding-system-alias"),
    );
    interp.define(
        "set-coding-system-priority",
        LispObject::primitive("set-coding-system-priority"),
    );
    interp.define("set-charset-priority", LispObject::primitive("ignore"));
    interp.define(
        "coding-system-put",
        LispObject::primitive("coding-system-put"),
    );
    interp.define("set-language-environment", LispObject::primitive("ignore"));
    interp.define(
        "set-default-coding-systems",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "prefer-coding-system",
        LispObject::primitive("prefer-coding-system"),
    );
    interp.define(
        "set-buffer-multibyte",
        LispObject::primitive("set-buffer-multibyte"),
    );
    interp.define(
        "set-keyboard-coding-system-internal",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "set-terminal-coding-system-internal",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "set-selection-coding-system",
        LispObject::primitive("ignore"),
    );
    interp.define("charset-id-internal", LispObject::primitive("ignore"));
    interp.define("charsetp", LispObject::primitive("ignore"));
    interp.define("set-char-table-range", LispObject::primitive("ignore"));
    interp.define("map-charset-chars", LispObject::primitive("ignore"));
    interp.define(
        "unibyte-char-to-multibyte",
        LispObject::primitive("identity"),
    );
    interp.define(
        "multibyte-char-to-unibyte",
        LispObject::primitive("multibyte-char-to-unibyte"),
    );
    interp.define("string-to-multibyte", LispObject::primitive("identity"));
    interp.define("string-to-unibyte", LispObject::primitive("identity"));
    interp.define(
        "set-buffer-file-coding-system",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "find-operation-coding-system",
        LispObject::primitive("ignore"),
    );
    interp.define("coding-system-p", LispObject::primitive("coding-system-p"));
    interp.define(
        "check-coding-system",
        LispObject::primitive("check-coding-system"),
    );
    interp.define(
        "coding-system-list",
        LispObject::primitive("coding-system-list"),
    );
    interp.define(
        "coding-system-base",
        LispObject::primitive("coding-system-base"),
    );
    interp.define(
        "coding-system-get",
        LispObject::primitive("coding-system-get"),
    );
    interp.define("coding-system-type", LispObject::primitive("ignore"));
    interp.define("coding-system-eol-type", LispObject::primitive("ignore"));
    interp.define("minibuffer-prompt-properties", LispObject::nil());
    interp.define("mode-line-format", LispObject::nil());
    interp.define("header-line-format", LispObject::nil());
    interp.define("tab-line-format", LispObject::nil());
    interp.define("indicate-empty-lines", LispObject::nil());
    interp.define("indicate-buffer-boundaries", LispObject::nil());
    interp.define("fringe-indicator-alist", LispObject::nil());
    interp.define("fringe-cursor-alist", LispObject::nil());
    interp.define("scroll-up-aggressively", LispObject::nil());
    interp.define("scroll-down-aggressively", LispObject::nil());
    interp.define("fill-column", LispObject::integer(70));
    interp.define("left-margin", LispObject::integer(0));
    interp.define("tab-width", LispObject::integer(8));
    interp.define("ctl-arrow", LispObject::t());
    interp.define("truncate-lines", LispObject::nil());
    interp.define("word-wrap", LispObject::nil());
    interp.define("bidi-display-reordering", LispObject::t());
    interp.define("bidi-paragraph-direction", LispObject::nil());
    interp.define("selective-display", LispObject::nil());
    interp.define("selective-display-ellipses", LispObject::t());
    interp.define("inhibit-modification-hooks", LispObject::nil());
    interp.define("initial-window-system", LispObject::nil());
    interp.define("frame-initial-frame", LispObject::nil());
    interp.define("tool-bar-mode", LispObject::nil());
    interp.define("current-input-method", LispObject::nil());
    interp.define("input-method-function", LispObject::nil());
    interp.define("buffer-display-count", LispObject::integer(0));
    interp.define("buffer-display-time", LispObject::nil());
    interp.define("point-before-scroll", LispObject::nil());
    interp.define("window-point-insertion-type", LispObject::nil());
    interp.define("mark-active", LispObject::nil());
    interp.define("cursor-type", LispObject::t());
    interp.define("line-spacing", LispObject::nil());
    interp.define("cursor-in-non-selected-windows", LispObject::t());
    interp.define("transient-mark-mode", LispObject::t());
    interp.define("auto-fill-function", LispObject::nil());
    interp.define("scroll-bar-mode", LispObject::nil());
    interp.define("display-line-numbers", LispObject::nil());
    interp.define("display-fill-column-indicator", LispObject::nil());
    interp.define("display-fill-column-indicator-column", LispObject::nil());
    interp.define("display-fill-column-indicator-character", LispObject::nil());
    interp.define("global-abbrev-table", LispObject::nil());
    interp.define("local-abbrev-table", LispObject::nil());
    interp.define("abbrev-mode", LispObject::nil());
    interp.define("overwrite-mode", LispObject::nil());
    for name in [
        "left-margin",
        "line-spacing",
        "scroll-up-aggressively",
        "vertical-scroll-bar",
        "overwrite-mode",
    ] {
        let _ = interp.eval_source(&format!("(put '{name} 'variable-buffer-local t)"));
        interp
            .state
            .special_vars
            .write()
            .insert(crate::obarray::intern(name));
    }
    let _ = interp.eval_source("(put 'vertical-scroll-bar 'choice '(nil left right))");
    let _ = interp.eval_source(
        "(put 'overwrite-mode 'choice '(nil overwrite-mode-textual overwrite-mode-binary))",
    );
    interp.define("face-new-frame-defaults", LispObject::nil());
    interp.define("face-remapping-alist", LispObject::nil());
    interp.define("face-list", LispObject::primitive("ignore"));
    interp.define("face-attribute", LispObject::primitive("ignore"));
    interp.define("set-face-attribute", LispObject::primitive("ignore"));
    interp.define("internal-lisp-face-p", LispObject::primitive("ignore"));
    interp.define("internal-make-lisp-face", LispObject::primitive("ignore"));
    interp.define(
        "internal-set-lisp-face-attribute",
        LispObject::primitive("ignore"),
    );
    interp.define("face-spec-recalc", LispObject::primitive("ignore"));
    interp.define("display-graphic-p", LispObject::primitive("ignore"));
    interp.define("display-color-p", LispObject::primitive("ignore"));
    interp.define(
        "display-supports-face-attributes-p",
        LispObject::primitive("ignore"),
    );
    interp.define("color-defined-p", LispObject::primitive("ignore"));
    interp.define("color-values", LispObject::primitive("ignore"));
    interp.define("tty-color-define", LispObject::primitive("ignore"));
    interp.define("x-get-resource", LispObject::primitive("ignore"));
    interp.define("x-list-fonts", LispObject::primitive("ignore"));
    interp.define(
        "internal-face-x-get-resource",
        LispObject::primitive("ignore"),
    );
    interp.define("face-font-rescale-alist", LispObject::nil());
    interp.define("abbrev-table-name-list", LispObject::nil());
    interp.define("define-abbrev-table", LispObject::primitive("ignore"));
    interp.define("clear-abbrev-table", LispObject::primitive("ignore"));
    interp.define("abbrev-table-p", LispObject::primitive("ignore"));
    interp.define("help-char", LispObject::integer(8));
    interp.define("help-event-list", LispObject::nil());
    interp.define("prefix-help-command", LispObject::nil());
    interp.define("describe-prefix-bindings", LispObject::nil());
    interp.define("temp-buffer-max-height", LispObject::nil());
    interp.define("temp-buffer-max-width", LispObject::nil());
    interp.define("file-name-handler-alist", LispObject::nil());
    interp.define("auto-mode-alist", LispObject::nil());
    interp.define(
        "current-language-environment",
        LispObject::string("English"),
    );
    interp.define("locale-coding-system", LispObject::nil());
    interp.define("keyboard-coding-system", LispObject::nil());
    interp.define("terminal-coding-system", LispObject::nil());
    interp.define("selection-coding-system", LispObject::nil());
    interp.define("file-name-coding-system", LispObject::nil());
    interp.define("default-process-coding-system", LispObject::nil());
    interp.define("language-info-alist", LispObject::nil());
    interp.define("set-language-info-alist", LispObject::primitive("ignore"));
    interp.define("register-input-method", LispObject::primitive("ignore"));
    interp.define("set-case-syntax-pair", LispObject::primitive("ignore"));
    interp.define("set-case-syntax-delims", LispObject::primitive("ignore"));
    interp.define("set-case-syntax", LispObject::primitive("ignore"));
    interp.define("temporary-file-directory", LispObject::string("/tmp/"));
    interp.define("default-directory", LispObject::string("/"));
    interp.define("file-name-coding-system", LispObject::nil());
    interp.define("default-file-name-coding-system", LispObject::nil());
    interp.define(
        "verify-visited-file-modtime",
        LispObject::primitive("ignore"),
    );
    interp.define("set-visited-file-modtime", LispObject::primitive("ignore"));
    interp.define("set-file-modes", LispObject::primitive("ignore"));
    interp.define("set-file-times", LispObject::primitive("ignore"));
    interp.define("find-buffer-visiting", LispObject::primitive("ignore"));
    interp.define("backup-buffer", LispObject::primitive("ignore"));
    interp.define("ask-user-about-lock", LispObject::primitive("ignore"));
    interp.define("lock-buffer", LispObject::primitive("ignore"));
    interp.define("unlock-buffer", LispObject::primitive("ignore"));
    interp.define("file-locked-p", LispObject::primitive("ignore"));
    interp.define("yes-or-no-p", LispObject::primitive("ignore"));
    interp.define("y-or-n-p", LispObject::primitive("ignore"));
    interp.define("read-file-name", LispObject::primitive("ignore"));
    interp.define("read-directory-name", LispObject::primitive("ignore"));
    interp.define("read-string", LispObject::primitive("ignore"));
    interp.define("completing-read", LispObject::primitive("ignore"));
    interp.define("sit-for", LispObject::primitive("ignore"));
    interp.define("sleep-for", LispObject::primitive("ignore"));
    interp.define("recursive-edit", LispObject::primitive("ignore"));
    interp.define("top-level", LispObject::primitive("ignore"));
    interp.define("ding", LispObject::primitive("ignore"));
    interp.define("beep", LispObject::primitive("ignore"));
    interp.define("display-warning", LispObject::primitive("ignore"));
    interp.define("lwarn", LispObject::primitive("ignore"));
    interp.define("run-hooks", LispObject::primitive("ignore"));
    interp.define("run-hook-with-args", LispObject::primitive("ignore"));
    interp.define(
        "run-hook-with-args-until-success",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "run-hook-with-args-until-failure",
        LispObject::primitive("ignore"),
    );
    interp.define("add-hook", LispObject::primitive("ignore"));
    interp.define("remove-hook", LispObject::primitive("ignore"));
    interp.define("with-temp-buffer", LispObject::primitive("ignore"));
    interp.define("init-file-user", LispObject::nil());
    interp.define("locate-user-emacs-file", LispObject::primitive("identity"));
    interp.define("describe-function", LispObject::primitive("ignore"));
    interp.define("describe-variable", LispObject::primitive("ignore"));
    interp.define("describe-key", LispObject::primitive("ignore"));
    interp.define("describe-mode", LispObject::primitive("ignore"));
    interp.define("view-lossage", LispObject::primitive("ignore"));
    interp.define("format-message", LispObject::primitive("ignore"));
    interp.define("system-name", LispObject::primitive("ignore"));
    interp.define("define-category", LispObject::primitive("ignore"));
    interp.define("modify-category-entry", LispObject::primitive("ignore"));
    interp.define("category-docstring", LispObject::primitive("ignore"));
    interp.define("category-set-mnemonics", LispObject::primitive("ignore"));
    interp.define("char-category-set", LispObject::primitive("ignore"));
    interp.define("set-case-table", LispObject::primitive("ignore"));
    interp.define("modify-syntax-entry", LispObject::primitive("ignore"));
    interp.define("standard-syntax-table", LispObject::primitive("ignore"));
    interp.define("char-syntax", LispObject::primitive("ignore"));
    interp.define("string-to-syntax", LispObject::primitive("ignore"));
    interp.define("upper-bound", LispObject::primitive("ignore"));
    interp.define("locate-file-internal", LispObject::primitive("ignore"));
    interp.define("system-name", LispObject::primitive("ignore"));
    interp.define("setenv-internal", LispObject::primitive("ignore"));
    interp.define("set-buffer-major-mode", LispObject::primitive("ignore"));
    interp.define("text-properties-at", LispObject::primitive("ignore"));
    interp.define("get-text-property", LispObject::primitive("ignore"));
    interp.define("put-text-property", LispObject::primitive("ignore"));
    interp.define("remove-text-properties", LispObject::primitive("ignore"));
    interp.define("add-text-properties", LispObject::primitive("ignore"));
    interp.define(
        "next-single-property-change",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "previous-single-property-change",
        LispObject::primitive("ignore"),
    );
    interp.define("next-property-change", LispObject::primitive("ignore"));
    interp.define("text-property-any", LispObject::primitive("ignore"));
    interp.define("compare-buffer-substrings", LispObject::primitive("ignore"));
    interp.define("subst-char-in-region", LispObject::primitive("ignore"));
    interp.define("bolp", LispObject::primitive("bolp"));
    interp.define("eolp", LispObject::primitive("eolp"));
    interp.define("bobp", LispObject::primitive("bobp"));
    interp.define("eobp", LispObject::primitive("eobp"));
    interp.define("following-char", LispObject::primitive("following-char"));
    interp.define("preceding-char", LispObject::primitive("preceding-char"));
    interp.define("delete-region", LispObject::primitive("delete-region"));
    interp.define(
        "buffer-substring",
        LispObject::primitive("buffer-substring"),
    );
    interp.define(
        "buffer-substring-no-properties",
        LispObject::primitive("buffer-substring-no-properties"),
    );
    interp.define("indent-to", LispObject::primitive("ignore"));
    if let Some(emacs_lisp_dir) = emacs_lisp_dir() {
        let mut path = LispObject::nil();
        if let Some(source_root) = emacs_source_root() {
            for subdir in &["lisp/calendar", "lisp/emacs-lisp", "lisp"] {
                path =
                    LispObject::cons(LispObject::string(&format!("{source_root}/{subdir}")), path);
            }
        }
        for subdir in &[
            "calendar",
            "emacs-lisp",
            "international",
            "language",
            "progmodes",
            "textmodes",
            "vc",
            "",
        ] {
            let dir = if subdir.is_empty() {
                emacs_lisp_dir.to_string()
            } else {
                format!("{}/{}", emacs_lisp_dir, subdir)
            };
            path = LispObject::cons(LispObject::string(&dir), path);
        }
        for subdir in &["emacs-lisp", "international", ""] {
            let dir = if subdir.is_empty() {
                STDLIB_DIR.to_string()
            } else {
                format!("{STDLIB_DIR}/{subdir}")
            };
            path = LispObject::cons(LispObject::string(&dir), path);
        }
        interp.define("load-path", path);
    }
    interp.define(
        "composition-function-table",
        LispObject::Vector(std::sync::Arc::new(crate::eval::SyncRefCell::new(
            vec![LispObject::nil(); 0x10000],
        ))),
    );
    interp.define(
        "compose-chars-after-function",
        LispObject::primitive("ignore"),
    );
    interp.define("define-key-after", LispObject::primitive("ignore"));
    interp.define(
        "define-prefix-command",
        LispObject::primitive("make-sparse-keymap"),
    );
    interp.define("set-language-info", LispObject::primitive("ignore"));
    interp.define("register-input-method", LispObject::primitive("ignore"));
    interp.define(
        "define-translation-table",
        LispObject::primitive("define-translation-table"),
    );
    interp.define(
        "make-translation-table-from-alist",
        LispObject::primitive("make-translation-table-from-alist"),
    );
    interp.define("define-ccl-program", LispObject::primitive("ignore"));
    interp.define("set-nested-alist", LispObject::primitive("ignore"));
    interp.define("lookup-nested-alist", LispObject::primitive("ignore"));
    interp.define("use-cjk-char-width-table", LispObject::primitive("ignore"));
    interp.define("display-multi-font-p", LispObject::primitive("ignore"));
    interp.define("char-charset", LispObject::primitive("ignore"));
    interp.define("w32-add-charset-info", LispObject::primitive("ignore"));
    interp.define("regexp-opt", LispObject::primitive("ignore"));
    interp.define("compose-region-internal", LispObject::primitive("ignore"));
    interp.define("compose-string-internal", LispObject::primitive("ignore"));
    interp.define("find-composition-internal", LispObject::primitive("ignore"));
    interp.define("clear-composition-cache", LispObject::primitive("ignore"));
    interp.define("font-shape-gstring", LispObject::primitive("ignore"));
    interp.define("font-get", LispObject::primitive("ignore"));
    interp.define("font-at", LispObject::primitive("ignore"));
    interp.define("fontp", LispObject::primitive("ignore"));
    interp.define("font-get-glyphs", LispObject::primitive("ignore"));
    interp.define("font-variation-glyphs", LispObject::primitive("ignore"));
    interp.define("font-put", LispObject::primitive("ignore"));
    interp.define("font-object-p", LispObject::primitive("ignore"));
    interp.define("get-char-code-property", LispObject::primitive("ignore"));
    interp.define("ctext-non-standard-encodings", LispObject::nil());
    interp.define("nonascii-translation-table", LispObject::nil());
    interp.define("save-buffer", LispObject::primitive("ignore"));
    interp.define("match-data--translate", LispObject::primitive("ignore"));
    interp.define("make-obsolete", LispObject::primitive("ignore"));
    interp.define("make-obsolete-variable", LispObject::primitive("ignore"));
    interp.define("oclosure-define", LispObject::primitive("ignore"));
    interp.define("declare-function", LispObject::primitive("ignore"));
    interp.define("defvar-local", LispObject::primitive("ignore"));
    interp.define(
        "internal-make-var-non-special",
        LispObject::primitive("ignore"),
    );
    interp.define("minibufferp", LispObject::primitive("ignore"));
    interp.define("minibuffer-window", LispObject::primitive("ignore"));
    interp.define("active-minibuffer-window", LispObject::primitive("ignore"));
    interp.define("minibuffer-depth", LispObject::primitive("ignore"));
    interp.define("read-from-minibuffer", LispObject::primitive("ignore"));
    interp.define("try-completion", LispObject::primitive("ignore"));
    interp.define("all-completions", LispObject::primitive("ignore"));
    interp.define("test-completion", LispObject::primitive("ignore"));
    interp.define("internal-complete-buffer", LispObject::primitive("ignore"));
    interp.define("set-window-start", LispObject::primitive("ignore"));
    interp.define("window-dedicated-p", LispObject::primitive("ignore"));
    interp.define("pos-visible-in-window-p", LispObject::primitive("ignore"));
    interp.define("modify-frame-parameters-ignored", LispObject::nil());
    interp.define("force-mode-line-update", LispObject::primitive("ignore"));
    interp.define("overlay-put", LispObject::primitive("overlay-put"));
    interp.define("overlay-get", LispObject::primitive("overlay-get"));
    interp.define("delete-overlay", LispObject::primitive("delete-overlay"));
    interp.define("move-overlay", LispObject::primitive("move-overlay"));
    interp.define("overlay-start", LispObject::primitive("overlay-start"));
    interp.define("overlay-end", LispObject::primitive("overlay-end"));
    interp.define("overlays-at", LispObject::primitive("overlays-at"));
    interp.define("overlays-in", LispObject::primitive("overlays-in"));
    interp.define(
        "next-overlay-change",
        LispObject::primitive("next-overlay-change"),
    );
    interp.define(
        "previous-overlay-change",
        LispObject::primitive("previous-overlay-change"),
    );
    interp.define("remove-overlays", LispObject::primitive("remove-overlays"));
    interp.define(
        "overlay-properties",
        LispObject::primitive("overlay-properties"),
    );
    interp.define("overlay-buffer", LispObject::primitive("overlay-buffer"));
    interp.define("overlayp", LispObject::primitive("overlayp"));
    interp.define("current-message", LispObject::primitive("ignore"));
    interp.define("message-log-max", LispObject::nil());
    interp.define(
        "kill-all-local-variables",
        LispObject::primitive("kill-all-local-variables"),
    );
    interp.define(
        "treesit-language-available-p",
        LispObject::primitive("ignore"),
    );
    interp.define("treesit-ready-p", LispObject::primitive("ignore"));
    interp.define("treesit-parser-create", LispObject::primitive("ignore"));
    interp.define("indent-line-to", LispObject::primitive("ignore"));
    interp.define(
        "current-indentation",
        LispObject::primitive("current-indentation"),
    );
    interp.define("indent-to", LispObject::primitive("ignore"));
    interp.define("make-syntax-table", LispObject::primitive("ignore"));
    interp.define("syntax-table-p", LispObject::primitive("ignore"));
    interp.define("category-table-p", LispObject::primitive("ignore"));
    interp.define("syntax-ppss", LispObject::primitive("syntax-ppss"));
    interp.define("syntax-ppss-flush-cache", LispObject::primitive("ignore"));
    interp.define("syntax-propertize", LispObject::primitive("ignore"));
    interp.define("add-face-text-property", LispObject::primitive("ignore"));
    interp.define("define-fringe-bitmap", LispObject::primitive("ignore"));
    interp.define("set-face-background", LispObject::primitive("ignore"));
    interp.define("set-face-foreground", LispObject::primitive("ignore"));
    interp.define("face-name", LispObject::primitive("ignore"));
    interp.define("face-id", LispObject::primitive("ignore"));
    interp.define("face-documentation", LispObject::primitive("ignore"));
    interp.define("tty-type", LispObject::primitive("ignore"));
    interp.define("tty-color-alist", LispObject::primitive("ignore"));
    interp.define("tty-color-approximate", LispObject::primitive("ignore"));
    interp.define("tty-color-by-index", LispObject::primitive("ignore"));
    interp.define("tty-color-standard-values", LispObject::primitive("ignore"));
    interp.define("timer-create", LispObject::primitive("ignore"));
    interp.define("timer-set-time", LispObject::primitive("ignore"));
    interp.define("timer-set-function", LispObject::primitive("ignore"));
    interp.define("timer-activate", LispObject::primitive("ignore"));
    interp.define("cancel-timer", LispObject::primitive("ignore"));
    interp.define("timer-set-idle-time", LispObject::primitive("ignore"));
    interp.define("timer-activate-when-idle", LispObject::primitive("ignore"));
    interp.define("input-pending-p", LispObject::primitive("ignore"));
    interp.define("this-command-keys", LispObject::primitive("ignore"));
    interp.define("this-command-keys-vector", LispObject::primitive("ignore"));
    interp.define("this-single-command-keys", LispObject::primitive("ignore"));
    interp.define(
        "this-single-command-raw-keys",
        LispObject::primitive("ignore"),
    );
    interp.define("recent-keys", LispObject::primitive("ignore"));
    interp.define("set-input-mode", LispObject::primitive("ignore"));
    interp.define("current-input-mode", LispObject::primitive("ignore"));
    interp.define("x-display-list", LispObject::primitive("ignore"));
    interp.define("terminal-list", LispObject::primitive("ignore"));
    interp.define("set-terminal-parameter", LispObject::primitive("ignore"));
    interp.define("terminal-parameter", LispObject::primitive("ignore"));
    interp.define("terminal-live-p", LispObject::primitive("ignore"));
    interp.define("frame-terminal", LispObject::primitive("ignore"));
    interp.define("modify-frame-parameters", LispObject::primitive("ignore"));
    interp.define("x-parse-geometry", LispObject::primitive("ignore"));
    interp.define("make-frame-visible", LispObject::primitive("ignore"));
    interp.define("iconify-frame", LispObject::primitive("ignore"));
    interp.define("make-frame-invisible", LispObject::primitive("ignore"));
    interp.define("raise-frame", LispObject::primitive("ignore"));
    interp.define("lower-frame", LispObject::primitive("ignore"));
    interp.define("handle-switch-frame", LispObject::primitive("ignore"));
    interp.define("select-frame", LispObject::primitive("ignore"));
    interp.define("mouse-position", LispObject::primitive("ignore"));
    interp.define("set-mouse-position", LispObject::primitive("ignore"));
    interp.define("track-mouse", LispObject::primitive("ignore"));
    interp.define("mouse-pixel-position", LispObject::primitive("ignore"));
    interp.define("coordinates-in-window-p", LispObject::primitive("ignore"));
    interp.define("posn-at-point", LispObject::primitive("ignore"));
    interp.define("posn-at-x-y", LispObject::primitive("ignore"));
    interp.define("x-popup-menu", LispObject::primitive("ignore"));
    interp.define("x-popup-dialog", LispObject::primitive("ignore"));
    interp.define(
        "set-clipboard-coding-system",
        LispObject::primitive("ignore"),
    );
    interp.define("gui-set-selection", LispObject::primitive("ignore"));
    interp.define("gui-get-selection", LispObject::primitive("ignore"));
    interp.define("gui-selection-owner-p", LispObject::primitive("ignore"));
    interp.define("gui-selection-exists-p", LispObject::primitive("ignore"));
    interp.define("x-own-selection-internal", LispObject::primitive("ignore"));
    interp.define("x-get-selection-internal", LispObject::primitive("ignore"));
    interp.define(
        "x-disown-selection-internal",
        LispObject::primitive("ignore"),
    );
    interp.define("x-selection-owner-p", LispObject::primitive("ignore"));
    interp.define("x-selection-exists-p", LispObject::primitive("ignore"));
    interp.define("process-list", LispObject::primitive("ignore"));
    interp.define("display-images-p", LispObject::primitive("ignore"));
    interp.define("display-multi-frame-p", LispObject::primitive("ignore"));
    interp.define("display-popup-menus-p", LispObject::primitive("ignore"));
    interp.define("display-mouse-p", LispObject::primitive("ignore"));
    interp.define("pdumper-stats", LispObject::primitive("ignore"));
    interp.define("native-comp-available-p", LispObject::primitive("ignore"));
    interp.define("subr-native-elisp-p", LispObject::primitive("ignore"));
    interp.define("native-compile", LispObject::primitive("ignore"));
    interp.define("module-load", LispObject::primitive("ignore"));
    interp.define("dbus-ping", LispObject::primitive("ignore"));
    interp.define("sqlite-available-p", LispObject::primitive("ignore"));
    interp.define("json-available-p", LispObject::primitive("ignore"));
    interp.define("treesit-available-p", LispObject::primitive("ignore"));
    interp.define("daemonp", LispObject::primitive("ignore"));
    interp.define("subword-mode", LispObject::primitive("ignore"));
    interp.define("setq-local", LispObject::primitive("setq-local"));
    interp.define("fixnump", LispObject::primitive("fixnump"));
    interp.define("bignump", LispObject::primitive("bignump"));
    interp.define("cl-progv", LispObject::primitive("ignore"));
    for fn_name in [
        "map-elt",
        "map-put!",
        "map-insert",
        "map-delete",
        "map-contains-key",
        "map-empty-p",
        "map-length",
        "map-copy",
        "map-apply",
        "map-pairs",
        "map-into",
        "map-keys",
        "map-values",
        "map-filter",
        "map-merge",
        "map-merge-with",
        "map-every-p",
        "map-some",
        "url-generic-parse-url",
        "url-recreate-url",
        "url-expand-file-name",
        "url-hexify-string",
        "url-unhex-string",
        "url-parse-make-urlobj",
        "url-file-extension",
        "url-filename",
        "url-basepath",
        "url-domain",
        "puny-encode-string",
        "puny-decode-string",
        "puny-encode-domain",
        "puny-decode-domain",
        "iso8601-parse",
        "iso8601-parse-date",
        "iso8601-parse-time",
        "iso8601-parse-interval",
        "iso8601-parse-duration",
        "iso8601-valid-p",
        "decoded-time-add",
        "decoded-time-set-defaults",
        "time-to-seconds",
        "time-add",
        "time-subtract",
        "time-since",
        "days-between",
        "rfc822-addresses",
        "rfc2047-fold-field",
        "rfc2104-hash",
        "char-fold-to-regexp",
        "format-spec",
        "font-lock-ensure",
        "font-lock-flush",
        "browse-url",
        "grep-compute-defaults",
        "ert-with-test-buffer",
        "ert-run-tests-batch",
        "ert-run-tests-interactively",
        "should-parse",
        "mailcap-add-mailcap-entry",
        "mailcap-parse-mimetypes",
        "auth-source-forget-all-cached",
        "auth-source-search",
        "defadvice",
        "calc-eval",
        "math-simplify",
        "math-parse-date",
        "math-format-number",
        "eshell",
        "eshell-mode",
        "eshell-command",
        "eshell-command-result",
        "eshell-execute-file",
        "eshell-extended-glob",
        "eshell-eval-using-options",
        "eshell-stringify",
        "eshell-get-path",
        "eshell-glob-convert",
        "eshell-printable-size",
        "eshell-split-filename",
        "eshell-convertible-to-number-p",
        "erc-mode",
        "erc-d-t-with-cleanup",
        "erc-d-u--canned-load-dialog",
        "erc--target-from-string",
        "erc-parse-server-response",
        "erc-networks--id-create",
        "erc-networks--id-fixed-create",
        "erc-sasl--create-client",
        "erc-unique-channel-names",
        "semantic-mode",
        "semantic-gcc-fields",
        "javascript-mode",
        "js-mode",
        "nxml-mode",
        "html-mode",
        "tex-mode",
        "c-mode",
        "python-mode",
        "lua-mode",
        "sh-mode",
        "comint-mode",
        "eww-mode",
        "viper-mode",
        "rmail",
        "dired",
        "calendar",
        "vc-do-command",
        "vc-responsible-backend",
        "use-package",
        "timer--create",
    ] {
        interp.define(fn_name, LispObject::primitive("ignore"));
    }
    interp.define("eshell-debug-command", LispObject::nil());
    interp.define(
        "eshell-debug-command-buffer",
        LispObject::string("*eshell last cmd*"),
    );
    interp.define("advice--how-alist", LispObject::nil());
    interp.define("tramp-archive-enabled", LispObject::nil());
    interp.define("icalendar-parse-property", LispObject::nil());
    interp.define("current-load-list", LispObject::nil());
    interp.define("load-file-name", LispObject::nil());
    interp.define("load-in-progress", LispObject::nil());
    interp.define("load-read-function", LispObject::symbol("read"));
    interp.define("load-source-file-function", LispObject::nil());
    interp.define("inhibit-file-name-operation", LispObject::nil());
    interp.define("inhibit-file-name-handlers", LispObject::nil());
    interp.define("file-name-handler-alist", LispObject::nil());
    interp.define("pre-command-hook", LispObject::nil());
    interp.define("post-command-hook", LispObject::nil());
    interp.define("overriding-local-map", LispObject::nil());
    interp.define("overriding-terminal-local-map", LispObject::nil());
    interp.define("last-command", LispObject::nil());
    interp.define("this-command", LispObject::nil());
    interp.define("real-this-command", LispObject::nil());
    interp.define("last-input-event", LispObject::nil());
    interp.define("last-command-event", LispObject::nil());
    interp.define("last-nonmenu-event", LispObject::nil());
    interp.define("last-event-frame", LispObject::nil());
    interp.define("current-prefix-arg", LispObject::nil());
    interp.define("prefix-arg", LispObject::nil());
    interp.define("last-prefix-arg", LispObject::nil());
    interp.define("unread-command-events", LispObject::nil());
    interp.define("unread-post-input-method-events", LispObject::nil());
    interp.define("input-method-function", LispObject::nil());
    interp.define("deferred-action-list", LispObject::nil());
    interp.define("debug-on-error", LispObject::nil());
    interp.define("debug-on-quit", LispObject::nil());
    interp.define("debug-on-signal", LispObject::nil());
    interp.define("debug-on-message", LispObject::nil());
    interp.define("debugger", LispObject::symbol("debug"));
    interp.define("stack-trace-on-error", LispObject::nil());
    interp.define("shell-file-name", LispObject::string("/bin/sh"));
    interp.define("shell-command-switch", LispObject::string("-c"));
    interp.define("path-separator", LispObject::string(":"));
    interp.define("null-device", LispObject::string("/dev/null"));
    interp.define("directory-sep-char", LispObject::integer(b'/' as i64));
    interp.define("major-mode", LispObject::symbol("fundamental-mode"));
    interp.define("mode-name", LispObject::string("Fundamental"));
    interp.define("buffer-read-only", LispObject::nil());
    interp.define("buffer-file-name", LispObject::nil());
    interp.define("buffer-file-coding-system", LispObject::nil());
    interp.define("buffer-file-truename", LispObject::nil());
    interp.define("default-directory", LispObject::string("/tmp/"));
    interp.define("inhibit-read-only", LispObject::nil());
    interp.define("inhibit-modification-hooks", LispObject::nil());
    interp.define("inhibit-point-motion-hooks", LispObject::nil());
    interp.define("inhibit-field-text-motion", LispObject::nil());
    interp.define("inhibit-changing-match-data", LispObject::nil());
    interp.define("inhibit-quit", LispObject::nil());
    interp.define("inhibit-debugger", LispObject::nil());
    interp.define("inhibit-redisplay", LispObject::nil());
    interp.define("inhibit-message", LispObject::nil());
    interp.define("inhibit-startup-screen", LispObject::t());
    interp.define("noninteractive-frame", LispObject::nil());
    interp.define("tty-defined-color-alist", LispObject::nil());
    interp.define("terminal-frame", LispObject::nil());
    interp.define("initial-frame-alist", LispObject::nil());
    interp.define("default-frame-alist", LispObject::nil());
    interp.define("default-minibuffer-frame", LispObject::nil());
    interp.define("redisplay-dont-pause", LispObject::nil());
    interp.define("connection-local-profile-alist", LispObject::nil());
    interp.define("connection-local-criteria-alist", LispObject::nil());
    interp.define("tramp-mode", LispObject::t());
    interp.define("tramp-syntax", LispObject::symbol("default"));
    interp.define("tramp-default-host", LispObject::nil());
    interp.define("tramp-verbose", LispObject::integer(0));
    interp.define("kill-emacs-hook", LispObject::nil());
    interp.define("after-init-hook", LispObject::nil());
    interp.define("emacs-startup-hook", LispObject::nil());
    interp.define("window-configuration-change-hook", LispObject::nil());
    interp.define("buffer-list-update-hook", LispObject::nil());
    interp.define("find-file-hook", LispObject::nil());
    interp.define("after-save-hook", LispObject::nil());
    interp.define("before-save-hook", LispObject::nil());
    interp.define("write-file-functions", LispObject::nil());
    interp.define("after-change-functions", LispObject::nil());
    interp.define("before-change-functions", LispObject::nil());
    interp.define("activate-mark-hook", LispObject::nil());
    interp.define("deactivate-mark-hook", LispObject::nil());
    interp.define("quit-flag", LispObject::nil());
    interp.define("gc-cons-threshold", LispObject::integer(800_000));
    interp.define("gc-cons-percentage", LispObject::float(0.1));
    interp.define("max-specpdl-size", LispObject::integer(2000));
    interp.define("max-lisp-eval-depth", LispObject::integer(1600));
    interp.define("print-level", LispObject::nil());
    interp.define("print-length", LispObject::nil());
    interp.define("print-circle", LispObject::nil());
    interp.define("print-gensym", LispObject::nil());
    interp.define("eshell-last-output-end", LispObject::nil());
    interp.define("shortdoc--groups", LispObject::nil());
    interp.define("erc-modules", LispObject::nil());
    interp.define("erc-autojoin-delay", LispObject::nil());
    interp.define("erc-reuse-buffers", LispObject::nil());
    interp.define("macroexp--dynvars", LispObject::nil());
    interp.define("executing-kbd-macro", LispObject::nil());
    interp.define("require-public-key", LispObject::nil());
    interp.define("delete-by-moving-to-trash", LispObject::nil());
    interp.define("syntax-propertize--done", LispObject::nil());
    interp.define("parse-sexp-lookup-properties", LispObject::nil());
    interp.define("minibuffer-auto-raise", LispObject::nil());
    interp.define("so-long-file-local-mode-function", LispObject::nil());
    interp.define("window-system", LispObject::nil());
    interp.define("mh-sys-path", LispObject::nil());
    interp.define("mh-cmd-note", LispObject::integer(0));
    interp.define("tramp-methods", LispObject::nil());
    interp.define("eshell-ls-use-in-dired", LispObject::nil());
    interp.define("icalendar-parse-component", LispObject::nil());
    interp.define("icalendar-parse-calendar", LispObject::nil());
    interp.define("mh-path", LispObject::nil());
    interp.define("secrets-enabled", LispObject::nil());
    interp.define("internet-is-working", LispObject::nil());
    interp.define("regex-tests--resources-dir", LispObject::string(""));
    for mode_name in [
        "ruby-mode",
        "cperl-mode",
        "f90-mode",
        "sgml-mode",
        "shell-script-mode",
        "asm-mode",
        "bat-mode",
        "shell-mode",
        "mhtml-mode",
        "conf-colon-mode",
        "diff-mode",
        "c-mode",
        "c++-mode",
        "java-mode",
        "objc-mode",
        "idl-mode",
        "pike-mode",
        "awk-mode",
        "autoconf-mode",
        "scheme-mode",
        "pascal-mode",
        "sh-mode",
        "perl-mode",
        "makefile-mode",
        "tcl-mode",
        "ada-mode",
        "sql-mode",
        "latex-mode",
        "octave-mode",
        "fortran-mode",
        "nxml-mode",
        "css-mode",
        "js-mode",
        "typescript-mode",
        "python-mode",
        "elixir-ts-mode",
    ] {
        let hook_name = format!("{mode_name}-hook");
        let mode_fn = format!(
            "(lambda () \
               (setq major-mode '{mode_name}) \
               (setq mode-name \"{mode_name}\") \
               (editor--set-current-buffer-major-mode '{mode_name}) \
               (run-mode-hooks '{hook_name}))"
        );
        let id = crate::obarray::intern(mode_name);
        if interp.state.get_function_cell(id).is_none() {
            if let Ok(func) = interp.eval_source(&mode_fn) {
                interp.state.set_function_cell(id, func);
            }
        }
        let hook_id = crate::obarray::intern(&hook_name);
        if interp.state.get_value_cell(hook_id).is_none() {
            interp.state.set_value_cell(hook_id, LispObject::nil());
        }
    }
    interp.define("after-init-time", LispObject::t());
    interp.define("load-history", LispObject::nil());
    interp.define("buffer-undo-list", LispObject::nil());
    interp.define("open-paren-in-column-0-is-defun-start", LispObject::t());
    interp.define(
        "grep-find-template",
        LispObject::string("find <D> <X> -type f <F> -exec grep <C> -nH --null -e <R> \\{\\} +"),
    );
    interp.define("internal--daemon-sockname", LispObject::nil());
    interp.define("show-all", LispObject::nil());
    interp.define("filter-binaries", LispObject::nil());
    interp.define("find-function-space-re", LispObject::string(""));
    interp.define("ert-results-mode-map", LispObject::nil());
    interp.define("auto-save-timeout", LispObject::integer(30));
    interp.define("commandp", LispObject::primitive("ignore"));
    let _ = interp
        .eval_source(
            "(defun cl--struct-new-class (name docstring parents type named slots index-table children-sym tag print) \
           (record 'cl-struct-cl-structure-class name docstring parents slots index-table tag type named print children-sym))",
        );
    let _ = interp.eval_source(
        "(defun cl--struct-class-p (obj) \
           (and (vectorp obj) (> (length obj) 0) (eq (aref obj 0) 'cl-struct-cl-structure-class)))",
    );
    for (i, name) in [
        (1, "cl--struct-class-name"),
        (2, "cl--struct-class-docstring"),
        (3, "cl--struct-class-parents"),
        (4, "cl--struct-class-slots"),
        (5, "cl--struct-class-index-table"),
        (6, "cl--struct-class-tag"),
        (7, "cl--struct-class-type"),
        (8, "cl--struct-class-named"),
        (9, "cl--struct-class-print"),
        (10, "cl--struct-class-children-sym"),
    ] {
        let _ = interp.eval_source(&format!(
            "(defun {name} (obj) (if (vectorp obj) (aref obj {i}) nil))"
        ));
    }
    let _ = interp.eval_source(
        "(unless (fboundp 'cl--find-class) \
           (defun cl--find-class (name) (get name 'cl--class)))",
    );
    let _ = interp.eval_source(
        "(put 'cl-structure-class 'cl--class \
           (cl--struct-new-class 'cl-structure-class \"CL struct metaclass\" nil nil nil \
             (make-vector 10 nil) (make-hash-table :test 'eq) \
             'cl-struct-cl-structure-class-tags 'cl-structure-class nil))",
    );
    let _ = interp.eval_source("(defvar cl-struct-cl-structure-class-tags '(cl-structure-class))");
    let _ = interp.eval_source(
        "(put 'cl-structure-object 'cl--class \
           (cl--struct-new-class 'cl-structure-object \"root\" nil nil nil \
             [] (make-hash-table :test 'eq) \
             'cl-struct-cl-structure-object-tags 'cl-structure-object nil))",
    );
    let _ =
        interp.eval_source("(defvar cl-struct-cl-structure-object-tags '(cl-structure-object))");
    let _ = interp.eval_source(
        "(put 'cl-slot-descriptor 'cl--class \
           (cl--struct-new-class 'cl-slot-descriptor \"slot desc\" nil nil nil \
             (make-vector 4 nil) (make-hash-table :test 'eq) \
             'cl-struct-cl-slot-descriptor-tags 'cl-slot-descriptor nil))",
    );
    let _ = interp.eval_source("(defvar cl-struct-cl-slot-descriptor-tags '(cl-slot-descriptor))");
    for (i, name) in [
        (1, "cl--slot-descriptor-name"),
        (2, "cl--slot-descriptor-initform"),
        (3, "cl--slot-descriptor-type"),
        (4, "cl--slot-descriptor-props"),
    ] {
        let _ = interp.eval_source(&format!(
            "(unless (fboundp '{name}) (defun {name} (obj) (if (vectorp obj) (aref obj {i}) nil)))"
        ));
    }
    let _ = interp.eval_source("(defvar cl--struct-default-parent 'cl-structure-object)");
    let _ = interp.eval_source(
        "(unless (fboundp 'add-to-list) \
           (defun add-to-list (sym val) \
             (unless (memq val (symbol-value sym)) \
               (set sym (cons val (symbol-value sym))))))",
    );
    let _ = interp.eval_source(
        "(unless (fboundp 'cl-struct-p) \
           (defun cl-struct-p (obj) (cl--struct-class-p obj)))",
    );
    let _ = interp.eval_source(
        "(unless (fboundp 'built-in-class-p) \
           (defun built-in-class-p (obj) nil))",
    );
    interp.define("key-translation-map", LispObject::nil());
    interp.define(
        "translation-table-vector",
        LispObject::Vector(std::sync::Arc::new(crate::eval::SyncRefCell::new(
            vec![LispObject::nil(); 512],
        ))),
    );
    interp.define("ccl-encode-ethio-font", LispObject::nil());
    interp.define("isearch-mode-map", LispObject::nil());
    interp.define(
        "coding-system-put",
        LispObject::primitive("coding-system-put"),
    );
    interp.define("define-charset-alias", LispObject::primitive("ignore"));
    interp.define("map-charset-chars", LispObject::primitive("ignore"));
    interp.define("rx-define", LispObject::primitive("ignore"));
    interp.define("seq", LispObject::nil());
    interp.define("kmacro-register", LispObject::nil());
    interp.define("frame-register", LispObject::nil());
    interp.define("lisp-el-font-lock-keywords-1", LispObject::nil());
    interp.define("lexical-binding", LispObject::t());
    interp.define("burmese-composable-pattern", LispObject::nil());
    interp.define("font-ccl-encoder-alist", LispObject::nil());
    interp.define("lisp-mode-symbol", LispObject::nil());
    interp.define("lisp-cl-font-lock-keywords-1", LispObject::nil());
    interp.define("lisp-el-font-lock-keywords-2", LispObject::nil());
    interp.define("oclosure", LispObject::nil());
    interp.define("loop", LispObject::nil());
    // Reset after-load-alist so user code observes an empty alist. The
    // bootstrap stub-evaluator (primitives_modules::register) executes many
    // `with-eval-after-load` forms; their entries are an implementation detail
    // and should not be visible to user code that inspects after-load-alist.
    {
        let sym = crate::obarray::intern("after-load-alist");
        interp
            .state
            .global_env
            .write()
            .set_id(sym, LispObject::nil());
        interp.state.set_value_cell(sym, LispObject::nil());
    }
    interp
}
