#![allow(clippy::manual_checked_ops)]
#![allow(clippy::disallowed_methods)]
//! Audit the bootstrap loading chain.
//!
//! Loads the bootstrap files in order, reporting per-file and per-form
//! success rates.
//!
//! Usage:
//!   load_audit [--emacs-lisp-dir DIR]
//!
//! Uses the shared runtime bootstrap helpers that tests, audits, and future
//! editor integration all exercise.

fn main() {
    let emacs_dir = std::env::args()
        .skip_while(|a| a != "--emacs-lisp-dir")
        .nth(1)
        .or_else(|| rele_elisp::eval::bootstrap::emacs_lisp_dir().map(ToString::to_string));

    let Some(emacs_dir) = emacs_dir else {
        eprintln!("Cannot find Emacs lisp dir. Set --emacs-lisp-dir or EMACS_LISP_DIR env.");
        std::process::exit(2);
    };

    eprintln!("Using Emacs lisp dir: {emacs_dir}");

    let stdlib_dir = rele_elisp::eval::bootstrap::STDLIB_DIR;
    let bootstrap_files = rele_elisp::eval::bootstrap::BOOTSTRAP_FILES;

    // Ensure bootstrap files + commonly-required libraries exist
    let extra_libs = [
        "emacs-lisp/cl-lib",
        "emacs-lisp/cl-macs",
        "emacs-lisp/cl-extra",
        "emacs-lisp/cl-seq",
        "emacs-lisp/cl-print",
        "emacs-lisp/subr-x",
        "emacs-lisp/pcase",
        "emacs-lisp/rx",
        "emacs-lisp/help-macro",
        "emacs-lisp/icons",
        "textmodes/text-mode",
        "emacs-lisp/rect",
        "international/cp51932",
        "international/eucjp-ms",
        "international/charscript",
    ];
    let all_files: Vec<&str> = bootstrap_files
        .iter()
        .copied()
        .chain(extra_libs.iter().copied())
        .collect();
    if !rele_elisp::eval::bootstrap::ensure_stdlib_files_for_dir(&emacs_dir, &all_files) {
        eprintln!("Failed to stage one or more Emacs Lisp files into {stdlib_dir}");
        std::process::exit(2);
    }

    // Create interpreter through the shared runtime bootstrap path.
    let interp = rele_elisp::eval::bootstrap::make_stdlib_interp();

    let mut total_ok: usize = 0;
    let mut total_forms: usize = 0;
    let mut file_ok: usize = 0;
    let mut file_fail: usize = 0;

    for f in bootstrap_files {
        let path = format!("{stdlib_dir}/{f}.el");
        let source = match read_file(&path) {
            Some(s) => s,
            None => {
                println!("SKIP {f} (not found)");
                file_fail += 1;
                continue;
            }
        };

        let forms = match rele_elisp::read_all(&source) {
            Ok(f) => f,
            Err(e) => {
                println!("ERR  {f}: reader error: {e}");
                file_fail += 1;
                continue;
            }
        };

        let form_count = forms.len();
        let mut ok_count: usize = 0;
        let mut first_errors: Vec<String> = Vec::new();

        // Heavy files (japanese, cp51932, mouse) need a high budget.
        interp.set_eval_ops_limit(50_000_000);
        let mut since_gc: usize = 0;

        for form in forms {
            interp.reset_eval_ops();
            match interp.eval(form) {
                Ok(_) => ok_count += 1,
                Err(e) => {
                    if first_errors.len() < 3 {
                        first_errors.push(format!("{e}"));
                    }
                }
            }
            since_gc += 1;
            if since_gc >= 200 {
                interp.gc();
                since_gc = 0;
            }
        }
        interp.gc();
        interp.set_eval_ops_limit(0);

        let pct = if form_count > 0 {
            ok_count * 100 / form_count
        } else {
            100
        };

        if ok_count == form_count {
            println!("OK   {f} ({ok_count}/{form_count} = {pct}%)");
            file_ok += 1;
        } else {
            println!("PART {f} ({ok_count}/{form_count} = {pct}%)");
            for e in &first_errors {
                println!("      {}", &e[..e.len().min(100)]);
            }
            file_fail += 1;
        }

        total_ok += ok_count;
        total_forms += form_count;
    }

    eprintln!();
    eprintln!("=== Bootstrap Load Audit ===");
    eprintln!("  Files OK:   {file_ok}");
    eprintln!("  Files PART: {file_fail}");
    eprintln!(
        "  Forms OK:   {total_ok}/{total_forms} ({:.1}%)",
        if total_forms > 0 {
            total_ok as f64 / total_forms as f64 * 100.0
        } else {
            0.0
        }
    );
}

fn read_file(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().or_else(|| {
        std::fs::read(path)
            .ok()
            .map(|bytes| bytes.iter().map(|&b| char::from(b)).collect())
    })
}
