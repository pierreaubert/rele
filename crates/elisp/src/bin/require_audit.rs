//! Audit the require chain for secondary libraries.
//!
//! Bootstraps the stdlib, then attempts to load cl-lib, cl-macs,
//! cl-extra, cl-seq, cl-print, subr-x, pcase, ert — reporting
//! per-file form success rates.
//!
//! Usage:
//!   require_audit

use rele_elisp::eval::bootstrap::{
    emacs_lisp_dir, ensure_stdlib_files, ensure_stdlib_files_for_dir, load_full_bootstrap,
};

fn main() {
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(run)
        .expect("spawn");
    handle.join().expect("join");
}

fn run() {
    if !ensure_stdlib_files() {
        eprintln!("Cannot find Emacs lisp dir. Set EMACS_LISP_DIR env.");
        std::process::exit(2);
    }

    let Some(emacs_dir) = emacs_lisp_dir() else {
        eprintln!("Cannot find Emacs lisp dir.");
        std::process::exit(2);
    };

    // Ensure secondary lib files exist on disk.
    let extra_libs = [
        "emacs-lisp/cl-lib",
        "emacs-lisp/cl-macs",
        "emacs-lisp/cl-extra",
        "emacs-lisp/cl-seq",
        "emacs-lisp/cl-print",
        "emacs-lisp/subr-x",
        "emacs-lisp/pcase",
        "emacs-lisp/ert",
        "emacs-lisp/ert-x",
        "emacs-lisp/rx",
        "emacs-lisp/gv",
        "emacs-lisp/map",
    ];
    if !ensure_stdlib_files_for_dir(emacs_dir, &extra_libs) {
        eprintln!("Cannot stage secondary Emacs Lisp libraries.");
        std::process::exit(2);
    }

    // Create interpreter and run full bootstrap
    let interp = rele_elisp::eval::bootstrap::make_stdlib_interp();
    eprintln!("Running full bootstrap...");
    load_full_bootstrap(&interp);
    eprintln!("Bootstrap complete.");

    // Pre-provide features for heavy UI/display libraries that trigger
    // deadlocks in the bytecode VM during require chains. These are not
    // needed for the test suite.
    for feature in [
        "help-mode",
        "debug",
        "backtrace",
        "ewoc",
        "find-func",
        "pp",
        "help-macro",
    ] {
        let _ = interp.eval_source(&format!("(provide '{feature})"));
    }

    // Now audit each secondary library
    let libs_to_test = [
        "emacs-lisp/cl-lib",
        "emacs-lisp/cl-macs",
        "emacs-lisp/cl-extra",
        "emacs-lisp/cl-seq",
        "emacs-lisp/cl-print",
        "emacs-lisp/subr-x",
        "emacs-lisp/pcase",
        "emacs-lisp/gv",
        "emacs-lisp/ert",
    ];

    let mut total_ok: usize = 0;
    let mut total_forms: usize = 0;
    let mut file_ok: usize = 0;
    let mut file_fail: usize = 0;

    for lib in &libs_to_test {
        let path = format!("{}/{lib}.el", rele_elisp::eval::bootstrap::STDLIB_DIR);
        let source = match read_file(&path) {
            Some(s) => s,
            None => {
                println!("SKIP {lib} (not found)");
                file_fail += 1;
                continue;
            }
        };

        let forms = match rele_elisp::read_all(&source) {
            Ok(f) => f,
            Err(e) => {
                println!("ERR  {lib}: reader error: {e}");
                file_fail += 1;
                continue;
            }
        };

        let form_count = forms.len();
        let mut ok_count: usize = 0;
        let mut first_errors: Vec<String> = Vec::new();

        interp.set_eval_ops_limit(10_000_000);
        let mut since_gc: usize = 0;

        eprintln!("  loading {lib} ({form_count} forms)...");
        let file_start = std::time::Instant::now();
        for (i, form) in forms.into_iter().enumerate() {
            interp.reset_eval_ops();
            if i % 50 == 0 {
                eprintln!("    form {i}/{form_count}");
            }
            // Per-file wall-clock budget: 60s. Nested require chains
            // that trigger infinite loops must not hang forever.
            if file_start.elapsed().as_secs() >= 60 {
                eprintln!("    TIMEOUT at form {i}/{form_count}");
                first_errors.push("wall-clock timeout".to_string());
                break;
            }
            match interp.eval(form) {
                Ok(_) => ok_count += 1,
                Err(e) => {
                    if first_errors.len() < 5 {
                        first_errors.push(format!("{e}"));
                    }
                }
            }
            since_gc += 1;
            if since_gc >= 100 {
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
            println!("OK   {lib} ({ok_count}/{form_count} = {pct}%)");
            file_ok += 1;
        } else {
            println!("PART {lib} ({ok_count}/{form_count} = {pct}%)");
            for e in &first_errors {
                println!("      {}", &e[..e.len().min(120)]);
            }
            file_fail += 1;
        }

        total_ok += ok_count;
        total_forms += form_count;
    }

    println!();
    println!("=== Require Chain Audit ===");
    println!("  Files OK:   {file_ok}");
    println!("  Files PART: {file_fail}");
    println!(
        "  Forms OK:   {total_ok}/{total_forms} ({:.1}%)",
        if total_forms > 0 {
            total_ok as f64 / total_forms as f64 * 100.0
        } else {
            0.0
        }
    );

    // Also check if features were provided
    println!();
    println!("=== Feature check ===");
    for feature in [
        "cl-lib", "cl-macs", "cl-extra", "cl-seq", "subr-x", "pcase", "ert",
    ] {
        let check = format!("(featurep '{feature})");
        match interp.eval_source(&check) {
            Ok(v) => {
                let provided = !matches!(v, rele_elisp::LispObject::Nil);
                println!(
                    "  {feature}: {}",
                    if provided { "PROVIDED" } else { "missing" }
                );
            }
            Err(_) => println!("  {feature}: ERROR"),
        }
    }
}

fn read_file(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().or_else(|| {
        std::fs::read(path)
            .ok()
            .map(|bytes| bytes.iter().map(|&b| char::from(b)).collect())
    })
}
