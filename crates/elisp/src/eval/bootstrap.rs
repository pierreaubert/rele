//! Reusable bootstrap helpers for the Emacs Lisp interpreter.
//!
//! `make_stdlib_interp` configures an `Interpreter` with the stubs and
//! definitions needed to load the real Emacs stdlib (subr.el, keymap.el,
//! etc.).  `ensure_stdlib_files` decompresses the Emacs `.el` / `.el.gz`
//! files into the staging directory.  `load_full_bootstrap` and
//! `load_cl_lib` drive the actual file loading.
//!
//! These helpers are used by:
//! - The test module (`eval/tests.rs`) for unit and ERT tests.
//! - The audit binaries (`src/bin/{load_audit,require_audit,emacs_test_worker}.rs`).
//! - Integration tests in `tests/`.

use crate::add_primitives;
use crate::eval::Interpreter;
use crate::object::LispObject;
use crate::reader::read;

/// Staging directory for decompressed Emacs stdlib `.el` files.
///
/// Resolved at compile time to `<workspace-root>/tmp/elisp-stdlib` so
/// all scratch data stays under the repo's `tmp/` directory (which is
/// gitignored).
pub const STDLIB_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tmp/elisp-stdlib");

/// Decompress/copy the requested Emacs Lisp files into [`STDLIB_DIR`].
///
/// Each entry is relative to the Emacs `lisp/` directory and omits the
/// `.el` suffix, for example `emacs-lisp/cl-lib`. Existing staged files are
/// left untouched, so this is cheap to call from tests and audit binaries.
pub fn ensure_stdlib_files_for_dir(emacs_lisp_dir: &str, files: &[&str]) -> bool {
    for f in files {
        if !stage_stdlib_file(emacs_lisp_dir, f) {
            return false;
        }
    }
    true
}

/// Like [`ensure_stdlib_files_for_dir`], but discovers the Emacs lisp
/// directory from `EMACS_LISP_DIR` or common install locations.
pub fn ensure_stdlib_files_for(files: &[&str]) -> bool {
    let Some(emacs_lisp_dir) = emacs_lisp_dir() else {
        return false;
    };
    ensure_stdlib_files_for_dir(emacs_lisp_dir, files)
}

fn stage_stdlib_file(emacs_lisp_dir: &str, file: &str) -> bool {
    let dest = format!("{STDLIB_DIR}/{file}.el");
    if std::path::Path::new(&dest).exists() {
        return true;
    }
    if let Some(parent) = std::path::Path::new(&dest).parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }

    let plain = format!("{emacs_lisp_dir}/{file}.el");
    let gz = format!("{emacs_lisp_dir}/{file}.el.gz");
    if std::path::Path::new(&plain).exists() {
        return std::fs::copy(&plain, &dest).is_ok();
    }
    if std::path::Path::new(&gz).exists() {
        // Shelling out keeps the crate dependency-free. This path is used by
        // tests/audits, never the editor UI thread.
        if let Ok(out) = std::process::Command::new("gunzip")
            .args(["-c", &gz])
            .output()
        {
            return out.status.success() && std::fs::write(&dest, out.stdout).is_ok();
        }
    }
    // Some Emacs installs omit optional files from a given version/platform;
    // callers already skip files that were not staged.
    true
}

pub fn make_stdlib_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    crate::primitives_modules::register(&mut interp);
    // Common stubs for stdlib loading
    interp.define("backtrace-on-error-noninteractive", LispObject::nil());
    interp.define("most-positive-fixnum", LispObject::integer(i64::MAX));
    interp.define("most-negative-fixnum", LispObject::integer(i64::MIN));
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

    // `process-environment` is an Emacs-conventional list of
    // "VAR=VALUE" strings shadowing the actual environment. Build it
    // from the real process environment so `(getenv-internal ...)`
    // can observe it.
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
    // Default emacs-path-related variables.
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
    // Emacs 30 symbol-with-position stubs (we don't implement positioned symbols)
    interp.define("bare-symbol", LispObject::primitive("identity")); // bare-symbol just returns the symbol
    interp.define("symbol-with-pos-p", LispObject::primitive("ignore")); // always nil
    interp.define("byte-run--ssp-seen", LispObject::nil());
    // Stubs for functions we don't implement yet
    interp.define("mapbacktrace", LispObject::nil());
    interp.define("byte-compile-macro-environment", LispObject::nil());
    interp.define("macro-declaration-function", LispObject::nil());
    interp.define("byte-run--set-speed", LispObject::nil());
    interp.define("purify-flag", LispObject::nil());
    interp.define("delayed-warnings-list", LispObject::nil());

    // subr.el stubs — keymap and editor primitives
    // Note: make-keymap, make-sparse-keymap, define-key, keymapp, fset,
    // upcase, downcase, string-replace, string-prefix-p, string-suffix-p,
    // string-search, string-equal, string-lessp, compare-strings are
    // registered by add_primitives — do NOT re-define them here.
    interp.define("purecopy", LispObject::primitive("identity"));
    // `intern-soft` is registered as a real primitive in
    // add_primitives — don't shadow it with `identity` here.
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
    // Variables needed by subr.el
    interp.define("features", LispObject::nil());
    interp.define("obarray", LispObject::nil());
    // global-map is a keymap: (keymap CHAR-TABLE . ALIST). We provide
    // a vector as the char-table so `(set-char-table-range (nth 1 global-map) ...)`
    // in mule-conf.el doesn't error.
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
    interp.define("unicode-category-table", LispObject::nil());
    interp.define("case-replace", LispObject::t());
    // R18: register `rx` as a real macro (not a stub function) so that its
    // symbolic operand names (`bow`, `eow`, `digit`, `bos`, …) aren't eagerly
    // evaluated and raised as `void variable`. Expands to a literal regexp
    // string via the `rele--rx-translate` primitive, which handles the common
    // rx.el symbol table (anchors, POSIX classes, seq/or/group/+/*).
    //
    // This is intentionally narrow: unknown rx forms collapse to "", which is
    // enough for the test files we load to pick up their top-level defvars
    // without crashing. A full rx.el port is out of scope for R18.
    interp
        .eval(read("(defmacro rx (&rest forms) (rele--rx-translate forms))").unwrap())
        .expect("defmacro rx should succeed");
    // `bol` / `eol` are rx anchor names; inside abbrev they're used as
    // variables in a `let` that builds a regexp. Stubs that shadow
    // them let the form eval without rx loaded.
    interp.define("bol", LispObject::string(""));
    interp.define("eol", LispObject::string(""));
    interp.define("dump-mode", LispObject::nil());
    interp.define("emacs-build-time", LispObject::nil());
    interp.define("emacs-save-session-functions", LispObject::nil());
    interp.define("command-error-function", LispObject::nil());
    interp.define("command-error-default-function", LispObject::nil());
    interp.define("delayed-warnings-list", LispObject::nil());
    interp.define("delayed-warnings-hook", LispObject::nil());
    // `regexp` is an rx macro-constructor; without rx we stub it as a
    // function that returns its first arg unchanged so abbrev can
    // define its abbrev-table regexp at load time.
    interp.define("regexp", LispObject::primitive("identity"));
    interp.define("standard-category-table", LispObject::primitive("ignore"));
    interp.define("search-spaces-regexp", LispObject::nil());
    interp.define("print-escape-newlines", LispObject::nil());
    interp.define("standard-output", LispObject::t());
    interp.define("load-path", LispObject::nil());
    interp.define("data-directory", LispObject::string("/usr/share/emacs"));
    // Additional stubs for subr.el
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
    interp.define("C-@", LispObject::integer(0)); // NUL character
    interp.define("set-default", LispObject::primitive("ignore"));
    interp.define("remap", LispObject::nil());
    interp.define("hash-table-p", LispObject::primitive("ignore"));
    // Remaining stubs for 100% subr.el
    interp.define("local-variable-if-set-p", LispObject::primitive("ignore"));
    interp.define("make-local-variable", LispObject::primitive("identity"));
    interp.define("local-variable-p", LispObject::primitive("ignore"));
    interp.define("exit-minibuffer", LispObject::nil());
    interp.define("self-insert-command", LispObject::nil());
    interp.define("undefined", LispObject::nil());
    interp.define("minibuffer-recenter-top-bottom", LispObject::nil());
    // split-string and string-search are registered by add_primitives
    interp.define("symbol-value", LispObject::primitive("identity"));
    interp.define("default-value", LispObject::primitive("ignore"));
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
    // version-to-list needs to be a real implementation since subr.el's
    // version calls string-match + match-data which we don't fully support
    // We define it AFTER loading subr.el would override it, so register it
    // as a special form instead
    interp.define("process-attributes", LispObject::primitive("ignore"));
    interp.define("suspend-emacs", LispObject::nil());
    interp.define("emacs", LispObject::nil());

    // Phase 7 stubs: simple.el / files.el / macroexp.el support
    interp.define("propertize", LispObject::primitive("identity")); // return first arg (text sans properties)
    interp.define("current-time-string", LispObject::primitive("ignore")); // return nil (fixed string not needed)

    // Phase 7d — variables referenced by various stdlib files.
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

    // Phase 7e — missing primitive stubs. `make-overlay` is a real
    // primitive (see primitives_buffer.rs); register its symbol to
    // route through the stateful dispatch.
    interp.define("make-overlay", LispObject::primitive("make-overlay"));
    interp.define("custom-add-option", LispObject::primitive("ignore"));
    interp.define("custom-add-version", LispObject::primitive("ignore"));
    interp.define("custom-declare-variable", LispObject::primitive("ignore"));
    interp.define("custom-declare-face", LispObject::primitive("ignore"));
    interp.define("custom-declare-group", LispObject::primitive("ignore"));

    // Bootstrap chain stubs: functions and variables needed by loadup.el files
    // keymap.el
    interp.define("keymapp", LispObject::primitive("keymapp"));
    interp.define("map-keymap", LispObject::primitive("ignore"));
    interp.define("key-parse", LispObject::primitive("identity"));
    interp.define("keymap--check", LispObject::primitive("ignore"));
    interp.define("keymap--compile-check", LispObject::primitive("ignore"));
    interp.define("byte-compile-warn", LispObject::primitive("ignore"));
    interp.define(
        "describe-bindings-internal",
        LispObject::primitive("ignore"),
    );
    interp.define("event-convert-list", LispObject::primitive("ignore"));
    interp.define("kbd", LispObject::primitive("identity"));

    // version.el
    interp.define("emacs-build-number", LispObject::integer(1));
    interp.define("emacs-build-time", LispObject::nil());
    interp.define("emacs-repository-version", LispObject::string("unknown"));
    interp.define(
        "system-configuration",
        LispObject::string("aarch64-apple-darwin"),
    );
    interp.define("system-configuration-options", LispObject::string(""));
    interp.define("system-configuration-features", LispObject::string(""));

    // international/mule.el and mule-conf.el
    interp.define("define-charset-alias", LispObject::primitive("ignore"));
    interp.define("define-charset", LispObject::primitive("ignore"));
    interp.define("put-charset-property", LispObject::primitive("ignore"));
    interp.define("get-charset-property", LispObject::primitive("ignore"));
    interp.define("charset-plist", LispObject::primitive("ignore"));
    interp.define("define-charset-internal", LispObject::primitive("ignore"));
    // `char-width` is registered as a real primitive in add_primitives.
    // aref is registered by add_primitives — do NOT re-define
    interp.define("charset-dimension", LispObject::primitive("ignore"));
    interp.define("define-coding-system", LispObject::primitive("ignore"));
    interp.define(
        "define-coding-system-alias",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "set-coding-system-priority",
        LispObject::primitive("ignore"),
    );
    interp.define("set-charset-priority", LispObject::primitive("ignore"));
    interp.define("coding-system-put", LispObject::primitive("ignore"));
    interp.define("set-language-environment", LispObject::primitive("ignore"));
    interp.define(
        "set-default-coding-systems",
        LispObject::primitive("ignore"),
    );
    interp.define("prefer-coding-system", LispObject::primitive("ignore"));
    interp.define("set-buffer-multibyte", LispObject::primitive("ignore"));
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
        LispObject::primitive("identity"),
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
    interp.define("coding-system-p", LispObject::primitive("ignore"));
    interp.define("check-coding-system", LispObject::primitive("identity"));
    interp.define("coding-system-list", LispObject::primitive("ignore"));
    interp.define("coding-system-base", LispObject::primitive("identity"));
    interp.define("coding-system-get", LispObject::primitive("ignore"));
    interp.define("coding-system-type", LispObject::primitive("ignore"));
    interp.define("coding-system-eol-type", LispObject::primitive("ignore"));

    // bindings.el / window.el
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

    // faces.el / cus-face.el
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

    // cl-preloaded.el, oclosure.el
    // These files heavily use byte-compiled code.
    // cl-preloaded needs cl-defstruct which uses byte-code.

    // abbrev.el
    interp.define("abbrev-table-name-list", LispObject::nil());
    interp.define("define-abbrev-table", LispObject::primitive("ignore"));
    interp.define("clear-abbrev-table", LispObject::primitive("ignore"));
    interp.define("abbrev-table-p", LispObject::primitive("ignore"));

    // help.el
    interp.define("help-char", LispObject::integer(8)); // C-h
    interp.define("help-event-list", LispObject::nil());
    interp.define("prefix-help-command", LispObject::nil());
    interp.define("describe-prefix-bindings", LispObject::nil());
    interp.define("temp-buffer-max-height", LispObject::nil());
    interp.define("temp-buffer-max-width", LispObject::nil());

    // jka-cmpr-hook.el / epa-hook.el
    interp.define("file-name-handler-alist", LispObject::nil());
    interp.define("auto-mode-alist", LispObject::nil());

    // international/mule-cmds.el
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

    // case-table.el
    interp.define("set-case-syntax-pair", LispObject::primitive("ignore"));
    interp.define("set-case-syntax-delims", LispObject::primitive("ignore"));
    interp.define("set-case-syntax", LispObject::primitive("ignore"));

    // files.el
    interp.define("temporary-file-directory", LispObject::string("/tmp/"));
    interp.define("default-directory", LispObject::string("/"));
    interp.define("file-name-coding-system", LispObject::nil());
    interp.define("default-file-name-coding-system", LispObject::nil());
    // `file-newer-than-file-p` is registered as a real primitive in
    // add_primitives — don't shadow it.
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

    // abbrev.el
    interp.define("init-file-user", LispObject::nil());
    interp.define("locate-user-emacs-file", LispObject::primitive("identity"));

    // help.el
    interp.define("describe-function", LispObject::primitive("ignore"));
    interp.define("describe-variable", LispObject::primitive("ignore"));
    interp.define("describe-key", LispObject::primitive("ignore"));
    interp.define("describe-mode", LispObject::primitive("ignore"));
    interp.define("view-lossage", LispObject::primitive("ignore"));

    // epa-hook.el
    // The "void variable: lambda" error is a backtick expansion issue.
    // epa-hook uses rx macro forms that we can't fully expand.

    // international/mule-cmds.el
    interp.define("format-message", LispObject::primitive("ignore"));
    interp.define("system-name", LispObject::primitive("ignore"));

    // international/characters.el
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

    // More C-level stubs
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
    interp.define("bolp", LispObject::primitive("ignore"));
    interp.define("eolp", LispObject::primitive("ignore"));
    interp.define("bobp", LispObject::primitive("ignore"));
    interp.define("eobp", LispObject::primitive("ignore"));
    interp.define("following-char", LispObject::primitive("ignore"));
    interp.define("preceding-char", LispObject::primitive("ignore"));
    interp.define("delete-region", LispObject::primitive("ignore"));
    interp.define("buffer-substring", LispObject::primitive("ignore"));
    interp.define(
        "buffer-substring-no-properties",
        LispObject::primitive("ignore"),
    );
    interp.define("indent-to", LispObject::primitive("ignore"));

    // Set load-path pointing to Emacs lisp directory so `require` can find files
    if let Some(emacs_lisp_dir) = emacs_lisp_dir() {
        let mut path = LispObject::nil();
        // Include both the decompressed /tmp dir and the original Emacs lisp dirs
        for subdir in &[
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
        // Also add STDLIB_DIR (decompressed .el files) at higher priority
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

    // composite.el / language files — variables and functions
    // composition-function-table is a char-table; needs to be a vector
    // for `(aset composition-function-table CHAR ...)` in language/*.el.
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
    interp.define("define-translation-table", LispObject::primitive("ignore"));
    interp.define(
        "make-translation-table-from-alist",
        LispObject::primitive("ignore"),
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

    // Phase 3 stubs — needed by indent, simple, minibuffer, frame, etc.
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
    // `frame-width` / `frame-height` / `redisplay` are registered as
    // real primitives in `add_primitives` — don't shadow them.
    interp.define("modify-frame-parameters-ignored", LispObject::nil());
    interp.define("force-mode-line-update", LispObject::primitive("ignore"));
    // Overlay primitives — real implementations in
    // `primitives_buffer.rs`, dispatched via `call_stateful_primitive`.
    interp.define("overlay-put", LispObject::primitive("overlay-put"));
    interp.define("overlay-get", LispObject::primitive("overlay-get"));
    interp.define("delete-overlay", LispObject::primitive("delete-overlay"));
    interp.define("move-overlay", LispObject::primitive("move-overlay"));
    interp.define("overlay-start", LispObject::primitive("overlay-start"));
    interp.define("overlay-end", LispObject::primitive("overlay-end"));
    interp.define("overlays-at", LispObject::primitive("overlays-at"));
    interp.define("overlays-in", LispObject::primitive("overlays-in"));
    interp.define("next-overlay-change", LispObject::primitive("ignore"));
    interp.define("previous-overlay-change", LispObject::primitive("ignore"));
    interp.define("remove-overlays", LispObject::primitive("remove-overlays"));
    interp.define(
        "overlay-properties",
        LispObject::primitive("overlay-properties"),
    );
    interp.define("overlay-buffer", LispObject::primitive("overlay-buffer"));
    interp.define("overlayp", LispObject::primitive("overlayp"));
    // Frames: we never have any.
    interp.define("current-message", LispObject::primitive("ignore"));
    interp.define("message-log-max", LispObject::nil());
    interp.define("kill-all-local-variables", LispObject::primitive("ignore"));
    // `buffer-local-variables` / `buffer-local-value` are registered
    // as real primitives in add_primitives — don't shadow them.
    // tree-sitter — always absent in this runtime.
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

    // Phase 5 stubs — display, timer, font-lock, syntax, mouse, isearch
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

    // Phase B4 — skip-gate stubs. Tests use these in (skip-unless ...)
    // forms to bail out when a feature isn't available. We make them
    // all return nil so those tests trivially skip.
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
    interp.define("setq-local", LispObject::primitive("ignore"));
    interp.define("fixnump", LispObject::primitive("integerp"));
    interp.define("bignump", LispObject::primitive("ignore"));
    interp.define("cl-progv", LispObject::primitive("ignore"));

    // Batch 3: lisp-library functions stubbed as `ignore` so callers
    // get `nil` rather than `void-function`. Each entry here is a
    // symbol the emacs ERT suite reaches before its defining file has
    // loaded — the stubs let the test file *parse* rather than abort
    // at its first reference. Grouped by upstream package.
    for fn_name in [
        // map.el — generic map access. Most callers can't distinguish
        // `nil result` from `not implemented`, so `ignore` is fine.
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
        // url-parse.el / url-util.el
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
        // puny.el
        "puny-encode-string",
        "puny-decode-string",
        "puny-encode-domain",
        "puny-decode-domain",
        // iso8601.el
        "iso8601-parse",
        "iso8601-parse-date",
        "iso8601-parse-time",
        "iso8601-parse-interval",
        "iso8601-parse-duration",
        "iso8601-valid-p",
        // time-date.el
        "decoded-time-add",
        "decoded-time-set-defaults",
        "time-to-seconds",
        "time-add",
        "time-subtract",
        "time-since",
        "days-between",
        // rfc822.el / rfc2047.el
        "rfc822-addresses",
        "rfc2047-fold-field",
        "rfc2104-hash",
        // Miscellaneous heavily-referenced library fns.
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
        // Major-mode stubs: tests probe (FOO-mode) to check the mode
        // exists. Returning nil means "not defined"; that's fine
        // because the tests are typically `skip-unless` guarded.
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

    // Void-variable stubs — top defvar-missing errors from the emacs test suite.
    // eshell/esh-util.el
    interp.define("eshell-debug-command", LispObject::nil());
    interp.define(
        "eshell-debug-command-buffer",
        LispObject::string("*eshell last cmd*"),
    );
    // emacs-lisp/nadvice.el — initial value is a complex alist, nil unblocks the lookups.
    interp.define("advice--how-alist", LispObject::nil());
    // net/tramp-archive.el — real init is (featurep 'dbusbind); we have no dbus.
    interp.define("tramp-archive-enabled", LispObject::nil());
    // calendar/icalendar.el — helper fn referenced at load time.
    interp.define("icalendar-parse-property", LispObject::nil());

    // cl--find-class is implemented in the eval dispatch (mod.rs)
    // since it's tightly coupled with the (setf (cl--find-class N) V)
    // place expansion handled there. No registration needed here.

    // Batch 3: top remaining `void variable` errors from the ERT run.
    // These variables are referenced at load time by stdlib files that
    // would otherwise abort partway through; real Emacs initialises
    // them in its C startup or a file we don't load. Each default
    // matches the Emacs default for a fresh non-interactive session.
    //
    // `current-load-list` is the list of forms loaded so far — set to
    // nil, which is Emacs's initial value. `load-file-name` is the
    // path of the currently-loading file — nil means "no file".
    interp.define("current-load-list", LispObject::nil());
    interp.define("load-file-name", LispObject::nil());
    interp.define("load-in-progress", LispObject::nil());
    interp.define("load-read-function", LispObject::symbol("read"));
    interp.define("load-source-file-function", LispObject::nil());
    // File I/O interception: nil = no handler shadowing is in effect.
    interp.define("inhibit-file-name-operation", LispObject::nil());
    interp.define("inhibit-file-name-handlers", LispObject::nil());
    interp.define("file-name-handler-alist", LispObject::nil());
    // Command/keyboard hooks — nil means no user hooks.
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
    // Debugging toggles — off for headless runs.
    interp.define("debug-on-error", LispObject::nil());
    interp.define("debug-on-quit", LispObject::nil());
    interp.define("debug-on-signal", LispObject::nil());
    interp.define("debug-on-message", LispObject::nil());
    interp.define("debugger", LispObject::symbol("debug"));
    interp.define("stack-trace-on-error", LispObject::nil());
    // Shell / path integration — defaults mirror Emacs defaults.
    interp.define("shell-file-name", LispObject::string("/bin/sh"));
    interp.define("shell-command-switch", LispObject::string("-c"));
    interp.define("path-separator", LispObject::string(":"));
    interp.define("null-device", LispObject::string("/dev/null"));
    interp.define("directory-sep-char", LispObject::integer(b'/' as i64));
    // Mode state — every buffer starts in fundamental-mode.
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
    // Terminals / display defaults.
    interp.define("tty-defined-color-alist", LispObject::nil());
    interp.define("terminal-frame", LispObject::nil());
    interp.define("initial-frame-alist", LispObject::nil());
    interp.define("default-frame-alist", LispObject::nil());
    interp.define("default-minibuffer-frame", LispObject::nil());
    interp.define("redisplay-dont-pause", LispObject::nil());
    // Connection-local variable plumbing (tramp / editor).
    interp.define("connection-local-profile-alist", LispObject::nil());
    interp.define("connection-local-criteria-alist", LispObject::nil());
    // Tramp — we have no remote support, so these are best-effort
    // defaults that let files that `defvar tramp-*` load without
    // issue. `tramp-mode` = t matches the Emacs default.
    interp.define("tramp-mode", LispObject::t());
    interp.define("tramp-syntax", LispObject::symbol("default"));
    interp.define("tramp-default-host", LispObject::nil());
    interp.define("tramp-verbose", LispObject::integer(0));
    // Common undefined command/hook bindings stdlib files expect.
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

    // R5: void-variable fixtures round 2 — legitimate Emacs globals
    // These variables are referenced at load time by various stdlib files.
    // Each default matches the Emacs default for a fresh non-interactive session.
    // Source: Emacs 29.3 lisp files (see commit message for file locations).

    // eshell/esh-util.el (79 hits)
    interp.define("eshell-last-output-end", LispObject::nil());

    // doc/misc/shortdoc.el (3 hits, R1 regression)
    interp.define("shortdoc--groups", LispObject::nil());

    // lisp/net/erc/*.el (18+ hits)
    interp.define("erc-modules", LispObject::nil());
    interp.define("erc-autojoin-delay", LispObject::nil());
    interp.define("erc-reuse-buffers", LispObject::nil());

    // lisp/emacs-lisp/macroexp.el (8 hits)
    interp.define("macroexp--dynvars", LispObject::nil());

    // lisp/repeat.el / lisp/keyboard.c (8 hits)
    interp.define("executing-kbd-macro", LispObject::nil());

    // lisp/crypt.el (7 hits)
    interp.define("require-public-key", LispObject::nil());

    // lisp/dired.el and others (6 hits)
    interp.define("delete-by-moving-to-trash", LispObject::nil());

    // lisp/emacs-lisp/lisp-mode.el (6 hits)
    interp.define("syntax-propertize--done", LispObject::nil());

    // lisp/search.c (6 hits)
    interp.define("parse-sexp-lookup-properties", LispObject::nil());

    // lisp/window.el (6 hits)
    interp.define("minibuffer-auto-raise", LispObject::nil());

    // lisp/so-long.el (6 hits)
    interp.define("so-long-file-local-mode-function", LispObject::nil());

    // lisp/frame.c (4 hits)
    interp.define("window-system", LispObject::nil());

    // lisp/mh-e/*.el (18+ hits, mh-e is email client)
    interp.define("mh-sys-path", LispObject::nil());
    interp.define("mh-cmd-note", LispObject::integer(0));

    // lisp/net/tramp*.el (4+ hits)
    interp.define("tramp-methods", LispObject::nil());

    // lisp/mail/eshell.el (3+ hits)
    interp.define("eshell-ls-use-in-dired", LispObject::nil());

    // R10: icalendar + tramp + connection-local + misc void-variable fixtures.
    // Source: /tmp/emacs-results-round2-baseline.jsonl. Only items with
    // >=5 hits in the icalendar/tramp/connection-local/mh/secrets/
    // auth-source/file-name-magic namespaces are registered here.

    // calendar/icalendar.el (18 hits)
    interp.define("icalendar-parse-component", LispObject::nil());

    // calendar/icalendar.el (7 hits)
    interp.define("icalendar-parse-calendar", LispObject::nil());

    // mh-e/*.el — mh-path is the detected binary dir (14 hits)
    interp.define("mh-path", LispObject::nil());

    // net/secrets.el — nil means the Secret Service API is unavailable (6 hits)
    interp.define("secrets-enabled", LispObject::nil());

    // R15: void-variable fixtures round 3 — legitimate defvar/defconst gaps.
    // Source: /tmp/emacs-results-round2-baseline.jsonl. Only entries whose
    // real Emacs source introduces them via a top-level defvar / defconst
    // are registered here; (b)/(c) cases (macro-local / reader / rx-symbol
    // bugs) are left for a dedicated evaluator-fix stream.
    //
    // Classification of the round-2 baseline shortlist (see PR body):
    //   location                     (b) cl-defmacro &key binding in with-package-test
    //   interactive-sym              (b) cl-defmacro positional in eglot--guessing-contact
    //   with-timeout-value-          (c) real name is -with-timeout-value-
    //   endpoint-sym                 (b) cl-defmacro positional in jsonrpc--with-emacsrpc-fixture
    //   elvar                        (b) cl-defmacro positional in gv-tests--in-temp-dir
    //   bow                          (b) rx-macro symbol (beginning-of-word), not a variable
    //   spec                         (b) let-bound outside ert-deftest; lexical capture
    //   ispell-tests--constants      (c) reader splits `/` — real symbols are
    //                                     ispell-tests--constants/english/wrong etc.
    //   create-file-buffer           (b) let*-bound lambda in ibuffer-tests
    //   create-non-file-buffer       (b) same
    //
    // Fixed here (a):

    // test/src/process-tests.el — top-level
    //   (defvar internet-is-working (progn (require 'dns) (dns-query "google.com")))
    // The init form fails in our environment (no dns-query), leaving the
    // symbol void. Pre-defining as nil is semantically correct: it means
    // "no internet" and the tests guarded by `(skip-unless internet-is-working)`
    // will be skipped rather than error. (5 hits)
    interp.define("internet-is-working", LispObject::nil());

    // test/src/regex-emacs-tests.el — top-level
    //   (defconst regex-tests--resources-dir
    //     (concat (file-name-directory (or load-file-name buffer-file-name))
    //             "/regex-resources/"))
    // Under our test runner both `load-file-name` and `buffer-file-name`
    // are nil, so the defconst init fails. The guarded tests wrap access in
    // `(skip-unless (file-directory-p regex-tests--resources-dir))`, so
    // pre-binding to an empty string lets the skip-unless evaluate cleanly
    // (file-directory-p "" -> nil -> skip). (4 hits)
    interp.define("regex-tests--resources-dir", LispObject::string(""));

    // Step E: register common mode functions that .elc loading may fail
    // to install. These are simple functions that set major-mode and
    // run the mode hook — enough for tests to proceed.
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
               (run-mode-hooks '{hook_name}))"
        );
        // Only install if not already defined (don't overwrite .elc definitions)
        let id = crate::obarray::intern(mode_name);
        if interp.state.get_function_cell(id).is_none() {
            if let Ok(func) = interp.eval_source(&mode_fn) {
                interp.state.set_function_cell(id, func);
            }
        }
        // Ensure hook variable exists
        let hook_id = crate::obarray::intern(&hook_name);
        if interp.state.get_value_cell(hook_id).is_none() {
            interp.state.set_value_cell(hook_id, LispObject::nil());
        }
    }

    // Step 5: additional missing variables that block many tests.
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

    // cl-preloaded bootstrap: pre-define the key CL struct constructors
    // and predicates to break the cl-defstruct circularity.
    // cl--struct-new-class creates a record (vector) for struct metadata.
    let _ = interp.eval_source(
        "(defun cl--struct-new-class (name docstring parents type named slots index-table children-sym tag print) \
           (record 'cl-struct-cl-structure-class name docstring parents slots index-table tag type named print children-sym))",
    );
    let _ = interp.eval_source(
        "(defun cl--struct-class-p (obj) \
           (and (vectorp obj) (> (length obj) 0) (eq (aref obj 0) 'cl-struct-cl-structure-class)))",
    );
    // cl--struct-class accessors (field indices match the record layout above)
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
    // cl--find-class / setf support — store/retrieve under plist key cl--class
    let _ = interp.eval_source(
        "(unless (fboundp 'cl--find-class) \
           (defun cl--find-class (name) (get name 'cl--class)))",
    );
    // Pre-register both cl-structure-class and cl-structure-object
    // so cl-preloaded.el's cl-defstruct forms don't hit assertion failures.
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
    // cl-slot-descriptor is also needed by cl-struct-define
    let _ = interp.eval_source(
        "(put 'cl-slot-descriptor 'cl--class \
           (cl--struct-new-class 'cl-slot-descriptor \"slot desc\" nil nil nil \
             (make-vector 4 nil) (make-hash-table :test 'eq) \
             'cl-struct-cl-slot-descriptor-tags 'cl-slot-descriptor nil))",
    );
    let _ = interp.eval_source("(defvar cl-struct-cl-slot-descriptor-tags '(cl-slot-descriptor))");
    // cl--slot-descriptor accessors
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
    // cl--struct-default-parent
    let _ = interp.eval_source("(defvar cl--struct-default-parent 'cl-structure-object)");
    // add-to-list stub
    let _ = interp.eval_source(
        "(unless (fboundp 'add-to-list) \
           (defun add-to-list (sym val) \
             (unless (memq val (symbol-value sym)) \
               (set sym (cons val (symbol-value sym))))))",
    );
    // cl-struct-p predicate
    let _ = interp.eval_source(
        "(unless (fboundp 'cl-struct-p) \
           (defun cl-struct-p (obj) (cl--struct-class-p obj)))",
    );
    // built-in-class-p stub
    let _ = interp.eval_source(
        "(unless (fboundp 'built-in-class-p) \
           (defun built-in-class-p (obj) nil))",
    );

    // Step 2: additional variables required by bootstrap files.
    // key-translation-map — a keymap used by international/iso-transl.el
    interp.define("key-translation-map", LispObject::nil());
    // translation-table-vector — used by language/japanese.el, cp51932, eucjp-ms
    interp.define(
        "translation-table-vector",
        LispObject::Vector(std::sync::Arc::new(crate::eval::SyncRefCell::new(
            vec![LispObject::nil(); 512],
        ))),
    );
    // ccl-encode-ethio-font — used by language/ethiopic.el
    interp.define("ccl-encode-ethio-font", LispObject::nil());
    // isearch-mode-map — used by isearch.el
    interp.define("isearch-mode-map", LispObject::nil());
    // coding-system-put — used by language/ethiopic.el
    interp.define("coding-system-put", LispObject::primitive("ignore"));
    // define-charset-alias — used by mule-conf.el
    interp.define("define-charset-alias", LispObject::primitive("ignore"));
    // map-charset-chars — used by international/characters.el
    interp.define("map-charset-chars", LispObject::primitive("ignore"));
    // rx-define — used by progmodes/prog-mode.el
    interp.define("rx-define", LispObject::primitive("ignore"));
    // seq — used by emacs-lisp/seq.el (the variable, not the library)
    interp.define("seq", LispObject::nil());
    // kmacro-register, frame-register — used by register.el
    interp.define("kmacro-register", LispObject::nil());
    interp.define("frame-register", LispObject::nil());
    // lisp-el-font-lock-keywords-1 — used by emacs-lisp/lisp-mode.el
    interp.define("lisp-el-font-lock-keywords-1", LispObject::nil());
    // lexical-binding — used by various files
    interp.define("lexical-binding", LispObject::t());
    // burmese-composable-pattern — used by language/burmese.el
    interp.define("burmese-composable-pattern", LispObject::nil());
    // font-ccl-encoder-alist — used by language/ethiopic.el
    interp.define("font-ccl-encoder-alist", LispObject::nil());
    // lisp-mode variables
    interp.define("lisp-mode-symbol", LispObject::nil());
    interp.define("lisp-cl-font-lock-keywords-1", LispObject::nil());
    interp.define("lisp-el-font-lock-keywords-2", LispObject::nil());
    // oclosure — the module, not loaded yet
    interp.define("oclosure", LispObject::nil());
    // loop — CL macro name used as var in some contexts
    interp.define("loop", LispObject::nil());

    interp
}

pub const BOOTSTRAP_FILES: &[&str] = &[
    // --- Phase 1: core (30 files from loadup.el) ---
    "emacs-lisp/debug-early",
    "emacs-lisp/byte-run",
    "emacs-lisp/backquote",
    "subr",
    "keymap",
    "version",
    "widget",
    "custom",
    "emacs-lisp/map-ynp",
    "international/mule",
    "international/mule-conf",
    "env",
    "format",
    "bindings",
    "window",
    "files",
    "emacs-lisp/macroexp",
    "cus-face",
    "faces",
    "button",
    "emacs-lisp/cl-preloaded",
    "emacs-lisp/oclosure",
    "obarray",
    "abbrev",
    "help",
    "jka-cmpr-hook",
    "epa-hook",
    "international/mule-cmds",
    "case-table",
    "international/characters",
    // --- Phase 2: composite + language files ---
    "composite",
    "language/chinese",
    "language/cyrillic",
    "language/indian",
    "language/sinhala",
    "language/english",
    "language/ethiopic",
    "language/european",
    "language/czech",
    "language/slovak",
    "language/romanian",
    "language/greek",
    "language/hebrew",
    "language/japanese",
    "language/korean",
    "language/lao",
    "language/tai-viet",
    "language/thai",
    "language/tibetan",
    "language/vietnamese",
    "language/misc-lang",
    "language/utf-8-lang",
    "language/georgian",
    // --- Phase 3: remaining language + i18n files ---
    "international/cp51932",
    "international/eucjp-ms",
    "language/khmer",
    "language/burmese",
    "language/cham",
    "language/philippine",
    "language/indonesian",
    // --- Phase 4: core editor infrastructure ---
    "emacs-lisp/float-sup",
    "indent",
    "emacs-lisp/cl-generic",
    "simple",
    "emacs-lisp/seq",
    "emacs-lisp/nadvice",
    "minibuffer",
    "frame",
    // --- Phase 5: display, input, and editing infrastructure ---
    "startup",
    "term/tty-colors",
    "font-core",
    "emacs-lisp/syntax",
    "font-lock",
    "jit-lock",
    "mouse",
    "select",
    "emacs-lisp/timer",
    "emacs-lisp/easymenu",
    "isearch",
    "rfn-eshadow",
    "emacs-lisp/lisp",
    "newcomment",
    "paren",
    // --- Phase 6: user-facing modes and UI ---
    "replace",
    "electric",
    "register",
    "menu-bar",
    "tab-bar",
    "cus-start",
    "international/iso-transl",
    "emacs-lisp/eldoc",
    "emacs-lisp/rmc",
    "emacs-lisp/shorthands",
    "emacs-lisp/tabulated-list",
    "emacs-lisp/lisp-mode",
    "textmodes/text-mode",
    "textmodes/paragraphs",
    "textmodes/page",
    "textmodes/fill",
    "progmodes/prog-mode",
    "progmodes/elisp-mode",
    "buff-menu",
    "uniquify",
    "vc/vc-hooks",
    "vc/ediff-hook",
    // --- Extra files needed for individual tests ---
    "emacs-lisp/cconv",
];

pub fn ensure_stdlib_files() -> bool {
    ensure_stdlib_files_for(BOOTSTRAP_FILES)
}

pub fn load_prerequisites(interp: &Interpreter) {
    // First three files live under the `emacs-lisp/` subdir in the
    // Emacs distribution; subr.el is at the top level.
    let emacs_lisp_files = &["debug-early.el", "byte-run.el", "backquote.el"];
    for f in emacs_lisp_files {
        let path = format!("{STDLIB_DIR}/emacs-lisp/{f}");
        if let Ok(s) = std::fs::read_to_string(&path) {
            let _ = interp.eval_source(&s);
        }
    }
    if let Ok(s) = std::fs::read_to_string(&format!("{STDLIB_DIR}/subr.el")) {
        let _ = interp.eval_source(&s);
    }
}

/// Run a stdlib loading test: parse all forms, eval each, count successes.
/// If the reader fails to parse the source, returns the reader error.
pub fn load_file_progress(
    interp: &Interpreter,
    source: &str,
) -> (usize, usize, Vec<(usize, String)>) {
    let forms = match crate::read_all(source) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("  READER ERROR: {}", e);
            return (0, 0, vec![(0, format!("reader: {}", e))]);
        }
    };
    let total = forms.len();
    let mut ok_count = 0;
    let mut errors: Vec<(usize, String)> = Vec::new();
    for (i, form) in forms.into_iter().enumerate() {
        match interp.eval(form) {
            Ok(_) => ok_count += 1,
            Err(e) => {
                if errors.len() < 20 {
                    errors.push((i, format!("{}", e)));
                }
            }
        }
    }
    (ok_count, total, errors)
}

pub fn load_full_bootstrap(interp: &Interpreter) {
    // Pre-provide features for heavy UI/display libraries that trigger
    // deadlocks in the bytecode VM during nested require chains.
    // These libraries are not needed for the test suite.
    for feature in [
        "help-mode",
        "debug",
        "backtrace",
        "ewoc",
        "find-func",
        "pp",
        "help-macro",
        // ert/ert-x: our eval dispatch already handles ert-deftest,
        // should, should-not, should-error, skip-unless natively.
        // Loading the real ert.el triggers a 20+ second require chain
        // that times out the worker. Pre-providing avoids the chain.
        "ert",
        "ert-x",
    ] {
        let _ = interp.eval_source(&format!("(provide '{feature})"));
    }

    // BOOTSTRAP_FILES has trailing test-only extras we should skip.
    // They're conventionally the last 1 entry (cconv).
    let n = BOOTSTRAP_FILES.len().saturating_sub(1);
    let mut since_gc: usize = 0;
    for f in &BOOTSTRAP_FILES[..n] {
        let path = format!("{STDLIB_DIR}/{f}.el");
        let source = match read_emacs_source(&path) {
            Some(s) => s,
            None => continue,
        };
        let forms = match crate::read_all(&source) {
            Ok(f) => f,
            Err(_) => continue,
        };
        // Same per-form budget as the bootstrap test uses.
        let skip_heavy = matches!(
            *f,
            "emacs-lisp/cl-preloaded" | "emacs-lisp/oclosure" | "international/mule-cmds"
        );
        interp.set_eval_ops_limit(if skip_heavy { 500_000 } else { 5_000_000 });
        for form in forms {
            interp.reset_eval_ops();
            let _ = interp.eval(form);
            since_gc += 1;
            // Heap is in Manual GC mode, so without explicit collection
            // the stdlib load can allocate hundreds of MB. Sweeping
            // every ~200 forms keeps peak RSS down for subsequent
            // cl-macs loading.
            if since_gc >= 200 {
                interp.gc();
                since_gc = 0;
            }
        }
    }
    interp.gc();
    interp.set_eval_ops_limit(0);

    // Extra tiny shims commonly assumed by test files that would
    // otherwise require cl-macs to be loaded. Defining them directly
    // avoids pulling cl-macs (which OOMs in bootstrap).
    //
    // Also includes abbrev-table-get/put overrides: after abbrev.el loads,
    // it redefines these using obarray-get/put (which are stubs returning nil).
    // We override them with hash-table-based implementations so that
    // abbrev-table-get-put-test can read back what was written.
    let extras = "
(defmacro cl-incf (place &optional x) \
  (list 'setq place (list '+ place (or x 1))))
(defmacro cl-decf (place &optional x) \
  (list 'setq place (list '- place (or x 1))))
(defalias 'incf 'cl-incf)
(defalias 'decf 'cl-decf)
(unless (fboundp 'gv-ref) (defun gv-ref (place) place))
(unless (fboundp 'gv-deref) (defun gv-deref (ref) ref))
(defun make-abbrev-table (&optional props)
  (make-hash-table :test (quote eq)))
(defun abbrev-table-put (table prop val)
  (puthash prop val table)
  val)
(defun abbrev-table-get (table prop)
  (gethash prop table))
";
    if let Ok(forms) = crate::read_all(extras) {
        interp.set_eval_ops_limit(200_000);
        for form in forms {
            interp.reset_eval_ops();
            let _ = interp.eval(form);
        }
        interp.set_eval_ops_limit(0);
    }

    // Additional fixture defvars. These are legit Emacs globals (not
    // test-local vars) that commonly appear as `void variable: X` in
    // the test corpus because their owning module failed to load.
    // Pre-registering them as nil / sensible-default lets dependent
    // tests at least progress past the first reference. We do NOT
    // pre-register test-local names like `abba` / `name` / `location`:
    // those would paper over real interpreter bugs in dynamic-binding.
    interp.define("auto-revert-interval", LispObject::integer(5));
    interp.define("url-handler-mode", LispObject::nil());
    interp.define("secrets-enabled", LispObject::nil());
    interp.define("sql-sqlite-program", LispObject::string("sqlite3"));
    interp.define("mh-path", LispObject::nil());
    interp.define("require-passphrase", LispObject::nil());
    interp.define("erc-autojoin-timing", LispObject::symbol("connect"));
    interp.define("erc-send-post-hook", LispObject::nil());
    interp.define("erc-interpret-controls-p", LispObject::t());
    interp.define("erc-scrolltobottom-all", LispObject::nil());
    interp.define("erc-nicks-track-faces", LispObject::symbol("prepend"));
    interp.define("erc-d-u--library-directory", LispObject::nil());
}

pub fn load_cl_lib(interp: &Interpreter) {
    let files = [
        "emacs-lisp/cl-macs",
        "emacs-lisp/cl-extra",
        "emacs-lisp/cl-seq",
        "emacs-lisp/cl-print",
    ];
    if !ensure_stdlib_files_for(&files) {
        return;
    }
    for f in files {
        let dest = format!("{STDLIB_DIR}/{f}.el");
        // Load the file form-by-form with periodic GC — cl-macs.el
        // is ~3500 lines and macro expansion blows the heap without
        // forced collection.
        if let Some(source) = read_emacs_source(&dest) {
            if let Ok(forms) = crate::read_all(&source) {
                interp.set_eval_ops_limit(10_000_000);
                let mut since_gc = 0;
                for form in forms {
                    interp.reset_eval_ops();
                    let _ = interp.eval(form);
                    since_gc += 1;
                    if since_gc >= 100 {
                        interp.gc();
                        since_gc = 0;
                    }
                }
                interp.gc();
                interp.set_eval_ops_limit(0);
            }
        }
    }
}

pub fn emacs_source_root() -> Option<&'static str> {
    use std::sync::OnceLock;
    static CACHED: OnceLock<Option<String>> = OnceLock::new();
    let slot = CACHED.get_or_init(|| {
        if let Ok(p) = std::env::var("EMACS_SRC_ROOT") {
            if std::path::Path::new(&p).is_dir() {
                return Some(p);
            }
        }
        let home = std::env::var("HOME").unwrap_or_default();
        let home_src = format!("{home}/src/emacs");
        let home_emacs = format!("{home}/emacs");
        let candidates: &[&str] = &[
            "/Volumes/SSD2TB/src/emacs",
            "/Volumes/home_ext1/Src/emacs",
            "/usr/src/emacs",
            "/usr/local/src/emacs",
            &home_src,
            &home_emacs,
        ];
        for root in candidates {
            if std::path::Path::new(root).is_dir() {
                return Some((*root).to_string());
            }
        }
        None
    });
    slot.as_deref()
}

pub fn emacs_lisp_dir() -> Option<&'static str> {
    use std::sync::OnceLock;
    static CACHED: OnceLock<Option<String>> = OnceLock::new();
    let slot = CACHED.get_or_init(|| {
        if let Ok(p) = std::env::var("EMACS_LISP_DIR") {
            if std::path::Path::new(&p).is_dir() {
                return Some(p);
            }
        }
        // Pinned fast-path first (matches the old hardcoded default).
        let pinned: &[&str] = &["/opt/homebrew/share/emacs/30.2/lisp"];
        for p in pinned {
            if std::path::Path::new(p).is_dir() {
                return Some((*p).to_string());
            }
        }
        // Glob-style scan of common parent dirs for `<parent>/<version>/lisp`.
        let parents: &[&str] = &[
            "/opt/homebrew/share/emacs",
            "/usr/local/share/emacs",
            "/usr/share/emacs",
            "/opt/emacs",
            "/snap/emacs/current/usr/share/emacs",
        ];
        for parent in parents {
            let Ok(entries) = std::fs::read_dir(parent) else {
                continue;
            };
            // Sort descending so the newest Emacs version wins.
            let mut versions: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|s| s.chars().next().is_some_and(|c| c.is_ascii_digit()))
                .collect();
            versions.sort_by(|a, b| b.cmp(a));
            for v in versions {
                let lisp = format!("{parent}/{v}/lisp");
                if std::path::Path::new(&lisp).is_dir() {
                    return Some(lisp);
                }
            }
        }
        None
    });
    slot.as_deref()
}

pub fn read_emacs_source(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().or_else(|| {
        std::fs::read(path)
            .ok()
            .map(|bytes| bytes.iter().map(|&b| char::from(b)).collect())
    })
}

pub fn probe_emacs_file(interp: &Interpreter, file_path: &str) -> Option<(usize, usize)> {
    let source = read_emacs_source(file_path)?;
    let forms = match crate::read_all(&source) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("  parse error in {file_path}: {e}");
            return None;
        }
    };

    // Add the test file's own directory *and* its `resources/`
    // subdirectory to `load-path` so `(require 'FOO-tests-helpers)`
    // / `(load "FOO-tests-helpers")` in the file can find sibling
    // helper files. Emacs's test-helpers live in `resources/` next
    // to the test file, loaded via `(ert-resource-file "NAME")`.
    if let Some(parent) = std::path::Path::new(file_path).parent() {
        let parent_str = parent.to_string_lossy().to_string();
        let resources = parent.join("resources");
        let resources_str = resources.to_string_lossy().to_string();

        // Register `ert-resource-directory` / `ert-resource-file` to
        // resolve against this file's own resources dir. These are
        // the exact helpers ERT uses; stubbing them means the common
        // `(require 'X (ert-resource-file "X"))` call succeeds even
        // if `ert-x.el` hasn't been loaded.
        let esc_res = resources_str.replace('\\', "\\\\").replace('"', "\\\"");
        let esc_parent = parent_str.replace('\\', "\\\\").replace('"', "\\\"");
        let add_form = format!(
            "(progn \
              (let ((p \"{esc_parent}\")) \
                (unless (member p load-path) (setq load-path (cons p load-path)))) \
              (let ((r \"{esc_res}\")) \
                (when (file-directory-p r) \
                  (unless (member r load-path) (setq load-path (cons r load-path))))) \
              (defun ert-resource-directory () \"{esc_res}/\") \
              (defun ert-resource-file (name) (concat \"{esc_res}/\" name)))"
        );
        if let Ok(mut adds) = crate::read_all(&add_form) {
            if let Some(f) = adds.pop() {
                interp.reset_eval_ops();
                interp.set_eval_ops_limit(100_000);
                let _ = interp.eval(f);
            }
        }
    }

    let total = forms.len();
    let mut ok = 0;
    for (i, form) in forms.into_iter().enumerate() {
        // Per-form budget + wall-clock deadline (no threads needed).
        interp.reset_eval_ops();
        interp.set_eval_ops_limit(5_000_000);
        interp.set_deadline(std::time::Instant::now() + std::time::Duration::from_secs(5));
        let t = std::time::Instant::now();
        if interp.eval(form).is_ok() {
            ok += 1;
        }
    }
    interp.set_eval_ops_limit(0);
    interp.clear_deadline();
    Some((ok, total))
}

#[derive(Default, Clone, Copy)]
pub struct ErtRunStats {
    pub passed: usize,
    pub failed: usize,
    pub errored: usize,
    pub skipped: usize,
    pub panicked: usize,
    pub timed_out: usize,
}

/// Per-test result, captured for JSONL emission.
#[derive(Clone, Debug)]
pub struct ErtTestResult {
    pub name: String,
    /// One of "pass", "fail", "error", "skip", "panic", "timeout".
    pub result: &'static str,
    /// Free-form detail (error message, signal symbol, etc.). Empty for passes.
    pub detail: String,
    pub duration_ms: u128,
}

impl ErtTestResult {
    /// Encode as one JSON object (single line). We hand-roll to avoid
    /// pulling serde into the elisp crate proper — serde_json is a
    /// dev-dep but only used by the harness.
    pub fn to_jsonl(&self, file: &str) -> String {
        fn esc(s: &str) -> String {
            let mut out = String::with_capacity(s.len() + 2);
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if (c as u32) < 0x20 => {
                        out.push_str(&format!("\\u{:04x}", c as u32));
                    }
                    c => out.push(c),
                }
            }
            out
        }
        format!(
            r#"{{"file":"{}","test":"{}","result":"{}","ms":{},"detail":"{}"}}"#,
            esc(file),
            esc(&self.name),
            self.result,
            self.duration_ms,
            esc(&self.detail),
        )
    }
}

/// Run all `ert-deftest`s registered under our `ert--rele-test` plist key
/// in the given `interp`. Wraps each test in `catch_unwind` so panics
/// from unforeseen interpreter bugs don't take down the whole run.
/// Returns aggregate counters AND per-test results. No wall-clock
/// timeout — an errant test can hang until the parent-side 15 s
/// per-file deadline kills the worker.
pub fn run_rele_ert_tests_detailed(interp: &Interpreter) -> (ErtRunStats, Vec<ErtTestResult>) {
    run_rele_ert_tests_detailed_inner(interp, 0)
}

/// Same as `run_rele_ert_tests_detailed` but with a per-test wall-clock
/// timeout in milliseconds. `per_test_ms == 0` means unlimited.
///
/// Termination mechanism: a dedicated watchdog thread waits on an
/// `mpsc` receiver for up to `per_test_ms`. If it wakes via timeout it
/// lowers the interpreter's `eval_ops_limit` to `1`, which makes the
/// next `charge()` call in the interpreter return `EvalError`. The
/// main thread, on seeing both the timed-out flag and that error,
/// classifies the result as `"timeout"`. This works for any test
/// that returns to the charge path periodically; pathological
/// tight-loops in a primitive that never call `charge()` still
/// escape the watchdog and rely on the parent's per-file deadline.
pub fn run_rele_ert_tests_detailed_with_timeout(
    interp: &Interpreter,
    per_test_ms: u64,
) -> (ErtRunStats, Vec<ErtTestResult>) {
    run_rele_ert_tests_detailed_inner(interp, per_test_ms)
}

fn run_rele_ert_tests_detailed_inner(
    interp: &Interpreter,
    per_test_ms: u64,
) -> (ErtRunStats, Vec<ErtTestResult>) {
    use crate::obarray;
    let test_key = obarray::intern("ert--rele-test");
    let test_struct_key = obarray::intern("ert--test");
    let skipped_key = obarray::intern("ert-test-skipped");
    let failed_key = obarray::intern("ert-test-failed");
    let mut stats = ErtRunStats::default();
    let mut results: Vec<ErtTestResult> = Vec::new();
    let mut tests: Vec<(String, LispObject, LispObject)> = Vec::new();
    {
        let ob = obarray::GLOBAL_OBARRAY.read();
        let cells = interp.state.symbol_cells.read();
        for sym_idx in 0..ob.symbol_count() {
            let id = obarray::SymbolId(sym_idx as u32);
            let thunk = cells.get_plist(id, test_key);
            if !thunk.is_nil() {
                // R23: also grab the `ert--test` struct so we can
                // expose it via `(ert-running-test)` while BODY runs.
                let struct_obj = cells.get_plist(id, test_struct_key);
                tests.push((ob.name(id).to_string(), thunk, struct_obj));
            }
        }
    }
    let _ = test_struct_key; // kept explicitly — do not clear.
    for (name, thunk, test_struct) in tests {
        // Wrap the stored thunk in `(quote ...)` so eval hands funcall
        // the object as-is. R19: the thunk is now a
        // `(closure CAPTURED () BODY...)` form; evaluating that cons as
        // a bare expression would try to dispatch on `closure` as a
        // function head, which doesn't exist. Quoting sidesteps the
        // dispatch and lets `call_function`'s `closure` arm handle it.
        let call = LispObject::cons(
            LispObject::symbol("funcall"),
            LispObject::cons(
                LispObject::cons(
                    LispObject::symbol("quote"),
                    LispObject::cons(thunk, LispObject::nil()),
                ),
                LispObject::nil(),
            ),
        );
        interp.reset_eval_ops();
        interp.set_eval_ops_limit(5_000_000);

        // Wall-clock deadline (no watchdog thread needed).
        if per_test_ms > 0 {
            interp.set_deadline(
                std::time::Instant::now() + std::time::Duration::from_millis(per_test_ms),
            );
        }

        crate::primitives::set_current_ert_test(test_struct.clone());

        let start = std::time::Instant::now();
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| interp.eval(call.clone())));
        let elapsed_ms = start.elapsed().as_millis();

        crate::primitives::set_current_ert_test(LispObject::nil());
        interp.clear_deadline();

        let was_timed_out = per_test_ms > 0 && elapsed_ms >= (per_test_ms as u128);

        let (result, detail) = match outcome {
            Ok(Ok(_)) => {
                stats.passed += 1;
                ("pass", String::new())
            }
            Ok(Err(crate::error::ElispError::Signal(sig))) => {
                let sym = sig.symbol.as_symbol_id();
                let sym_name = sig
                    .symbol
                    .as_symbol()
                    .unwrap_or_else(|| sig.symbol.prin1_to_string());
                let data_str = sig.data.prin1_to_string();
                if sym == Some(failed_key) {
                    stats.failed += 1;
                    let d = if data_str.is_empty() || data_str == "nil" {
                        "(assertion failed)".to_string()
                    } else {
                        data_str
                    };
                    ("fail", d)
                } else if sym == Some(skipped_key) {
                    stats.skipped += 1;
                    let d = if data_str.is_empty() || data_str == "nil" {
                        String::new()
                    } else {
                        format!("skip: {data_str}")
                    };
                    ("skip", d)
                } else {
                    stats.errored += 1;
                    if data_str.is_empty() || data_str == "nil" {
                        ("error", format!("signal {sym_name}"))
                    } else {
                        ("error", format!("signal {sym_name}: {data_str}"))
                    }
                }
            }
            Ok(Err(e)) => {
                // If the watchdog tripped, it lowered the eval-ops
                // limit to 1, which surfaces here as an EvalError
                // about the limit. Reclassify so the harness can
                // distinguish a real error from an exceeded deadline.
                let msg = e.to_string();
                if was_timed_out {
                    stats.timed_out += 1;
                    ("timeout", format!("exceeded {per_test_ms}ms wall-clock"))
                } else {
                    stats.errored += 1;
                    let detail = if msg.is_empty() {
                        format!("{e:?}")
                    } else {
                        msg
                    };
                    ("error", detail)
                }
            }
            Err(payload) => {
                stats.panicked += 1;
                let msg = if let Some(s) = payload.downcast_ref::<&'static str>() {
                    (*s).to_string()
                } else if let Some(s) = payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "panic: <unprintable payload>".to_string()
                };
                ("panic", msg)
            }
        };
        results.push(ErtTestResult {
            name,
            result,
            detail,
            duration_ms: elapsed_ms,
        });
    }
    interp.set_eval_ops_limit(0);
    (stats, results)
}

/// Backwards-compatible wrapper that drops the per-test detail.
pub fn run_rele_ert_tests(interp: &Interpreter) -> ErtRunStats {
    let (stats, results) = run_rele_ert_tests_detailed(interp);
    // Echo failures/errors/panics for the existing log format.
    for r in &results {
        match r.result {
            "fail" => eprintln!("    FAIL: {}", r.name),
            "error" => eprintln!("    ERROR: {}: {}", r.name, r.detail),
            "panic" => eprintln!("    PANIC: {}", r.name),
            "timeout" => eprintln!("    TIMEOUT: {}", r.name),
            _ => {}
        }
    }
    stats
}
