//! Audit the bootstrap loading chain.
//!
//! Loads the bootstrap files in order (the same list as BOOTSTRAP_FILES
//! in eval/tests.rs), reporting per-file and per-form success rates.
//!
//! Usage:
//!   load_audit [--emacs-lisp-dir DIR]
//!
//! Falls back to the same probing logic as the test harness.

use rele_elisp::eval::Interpreter;
use rele_elisp::add_primitives;

fn main() {
    let emacs_dir = std::env::args()
        .skip_while(|a| a != "--emacs-lisp-dir")
        .nth(1)
        .or_else(find_emacs_lisp_dir);

    let Some(emacs_dir) = emacs_dir else {
        eprintln!("Cannot find Emacs lisp dir. Set --emacs-lisp-dir or EMACS_LISP_DIR env.");
        std::process::exit(2);
    };

    eprintln!("Using Emacs lisp dir: {emacs_dir}");

    // Copy/decompress files to /tmp/elisp-stdlib/ (same convention as the harness)
    let stdlib_dir = "/tmp/elisp-stdlib";
    let _ = std::fs::create_dir_all(stdlib_dir);

    let bootstrap_files = rele_elisp::eval::tests::BOOTSTRAP_FILES;

    // Ensure files exist
    for f in bootstrap_files {
        let dest = format!("{stdlib_dir}/{f}.el");
        if std::path::Path::new(&dest).exists() {
            continue;
        }
        if let Some(parent) = std::path::Path::new(&dest).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let plain = format!("{emacs_dir}/{f}.el");
        let gz = format!("{emacs_dir}/{f}.el.gz");
        if std::path::Path::new(&plain).exists() {
            let _ = std::fs::copy(&plain, &dest);
        } else if std::path::Path::new(&gz).exists() {
            if let Ok(out) = std::process::Command::new("gunzip")
                .args(["-c", &gz])
                .output()
            {
                if out.status.success() {
                    let _ = std::fs::write(&dest, out.stdout);
                }
            }
        }
    }

    // Create interpreter and load bootstrap files one by one
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    // Set up load-path
    interp.define("load-path", rele_elisp::LispObject::cons(
        rele_elisp::LispObject::string(stdlib_dir),
        rele_elisp::LispObject::nil(),
    ));

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

        interp.set_eval_ops_limit(5_000_000);
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
    eprintln!("  Forms OK:   {total_ok}/{total_forms} ({:.1}%)",
        if total_forms > 0 { total_ok as f64 / total_forms as f64 * 100.0 } else { 0.0 });
}

fn read_file(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .or_else(|| {
            std::fs::read(path).ok().map(|bytes| {
                bytes.iter().map(|&b| char::from(b)).collect()
            })
        })
}

fn find_emacs_lisp_dir() -> Option<String> {
    if let Ok(v) = std::env::var("EMACS_LISP_DIR") {
        return Some(v);
    }
    // Homebrew on macOS
    for pattern in [
        "/opt/homebrew/share/emacs/*/lisp",
        "/usr/local/share/emacs/*/lisp",
        "/usr/share/emacs/*/lisp",
    ] {
        if let Ok(mut entries) = glob_simple(pattern) {
            entries.sort();
            if let Some(last) = entries.pop() {
                return Some(last);
            }
        }
    }
    // Direct source tree
    let src = "/Volumes/home_ext1/Src/emacs/lisp";
    if std::path::Path::new(src).is_dir() {
        return Some(src.to_string());
    }
    None
}

fn glob_simple(pattern: &str) -> Result<Vec<String>, ()> {
    // Very simple glob: split on * and list directories
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() != 2 {
        return Err(());
    }
    let prefix = parts[0];
    let suffix = parts[1];
    let parent = std::path::Path::new(prefix);
    if !parent.is_dir() {
        return Err(());
    }
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let full = format!("{}{}", entry.path().display(), suffix);
            if std::path::Path::new(&full).is_dir() {
                results.push(full);
            }
        }
    }
    Ok(results)
}
