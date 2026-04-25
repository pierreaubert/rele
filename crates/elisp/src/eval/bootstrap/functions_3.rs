//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use crate::eval::Interpreter;
use crate::object::LispObject;

use super::ensure_stdlib_files_for;
use super::functions::{STDLIB_DIR, ensure_stdlib_files_for_dir};
use super::functions_4::run_rele_ert_tests_detailed_inner;
use super::types::{ErtRunStats, ErtTestResult};

pub const BOOTSTRAP_FILES: &[&str] = &[
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
    "international/cp51932",
    "international/eucjp-ms",
    "language/khmer",
    "language/burmese",
    "language/cham",
    "language/philippine",
    "language/indonesian",
    "emacs-lisp/float-sup",
    "indent",
    "emacs-lisp/cl-generic",
    "simple",
    "emacs-lisp/seq",
    "emacs-lisp/nadvice",
    "minibuffer",
    "frame",
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
    "emacs-lisp/cconv",
];
pub fn bootstrap_eval_ops_limit(file: &str) -> u64 {
    match file {
        "emacs-lisp/cl-preloaded" | "emacs-lisp/oclosure" | "international/mule-cmds" => 500_000,
        _ => 5_000_000,
    }
}
pub fn ensure_stdlib_files() -> bool {
    ensure_stdlib_files_for(BOOTSTRAP_FILES)
}
pub fn load_prerequisites(interp: &Interpreter) {
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
    for feature in [
        "help-mode",
        "debug",
        "backtrace",
        "ewoc",
        "find-func",
        "pp",
        "help-macro",
        "ert",
        "ert-x",
    ] {
        let _ = interp.eval_source(&format!("(provide '{feature})"));
    }
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
        interp.set_eval_ops_limit(bootstrap_eval_ops_limit(f));
        for form in forms {
            interp.reset_eval_ops();
            let _ = interp.eval(form);
            since_gc += 1;
            if since_gc >= 200 {
                interp.gc();
                since_gc = 0;
            }
        }
    }
    interp.gc();
    interp.set_eval_ops_limit(0);
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
        let pinned: &[&str] = &["/opt/homebrew/share/emacs/30.2/lisp"];
        for p in pinned {
            if std::path::Path::new(p).is_dir() {
                return Some((*p).to_string());
            }
        }
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
    if let Some(parent) = std::path::Path::new(file_path).parent() {
        let parent_str = parent.to_string_lossy().to_string();
        let resources = parent.join("resources");
        let resources_str = resources.to_string_lossy().to_string();
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
