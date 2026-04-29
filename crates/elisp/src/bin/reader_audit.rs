#![allow(clippy::disallowed_methods)]
//! Audit the reader against a tree of `.el` files.
//!
//! Usage:
//!   reader_audit /path/to/emacs/lisp [/path/to/emacs/test]
//!
//! Walks every `.el` file under each root, calls `read_all()`, and prints
//! one line per file: `OK <path>` or `ERR <path>: <error>`.
//! Exits with a summary and a non-zero exit code when any file fails.

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: reader_audit DIR [DIR ...]");
        std::process::exit(2);
    }

    let mut ok: usize = 0;
    let mut fail: usize = 0;
    let mut errors: Vec<(String, String)> = Vec::new();

    for root in &args {
        let p = Path::new(root);
        if p.is_dir() {
            walk_dir(p, &mut ok, &mut fail, &mut errors);
        } else if p.extension().and_then(|e| e.to_str()) == Some("el") {
            audit_file(p, &mut ok, &mut fail, &mut errors);
        } else {
            eprintln!("SKIP {root}: not a directory or .el file");
        }
    }

    eprintln!();
    eprintln!("=== Reader Audit Summary ===");
    eprintln!("  OK:   {ok}");
    eprintln!("  FAIL: {fail}");
    let total = ok + fail;
    if total > 0 {
        let pct = ok as f64 / total as f64 * 100.0;
        eprintln!("  Rate: {pct:.1}%");
    }

    if !errors.is_empty() {
        eprintln!();
        eprintln!("=== Error classification ===");
        let mut by_kind: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (_, e) in &errors {
            // Classify by first line / first 60 chars
            let key = e
                .lines()
                .next()
                .unwrap_or(e)
                .chars()
                .take(60)
                .collect::<String>();
            *by_kind.entry(key).or_default() += 1;
        }
        let mut sorted: Vec<_> = by_kind.into_iter().collect();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.1));
        for (kind, count) in sorted.iter().take(30) {
            eprintln!("  [{count:4}] {kind}");
        }
    }

    if fail > 0 {
        std::process::exit(1);
    }
}

fn walk_dir(dir: &Path, ok: &mut usize, fail: &mut usize, errors: &mut Vec<(String, String)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("SKIP {}: {e}", dir.display());
            return;
        }
    };

    let mut paths: Vec<std::path::PathBuf> =
        entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();

    for path in paths {
        if path.is_dir() {
            walk_dir(&path, ok, fail, errors);
        } else if path.extension().and_then(|e| e.to_str()) == Some("el") {
            audit_file(&path, ok, fail, errors);
        }
    }
}

fn audit_file(path: &Path, ok: &mut usize, fail: &mut usize, errors: &mut Vec<(String, String)>) {
    // Try UTF-8 first; fall back to Latin-1 mapping (byte → char)
    // for files with raw non-UTF-8 bytes (Ethiopic, Tibetan, etc.).
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => match std::fs::read(path) {
            Ok(bytes) => bytes.iter().map(|&b| char::from(b)).collect(),
            Err(e) => {
                let p = path.display().to_string();
                println!("ERR {p}: cannot read: {e}");
                *fail += 1;
                errors.push((p, format!("cannot read: {e}")));
                return;
            }
        },
    };

    let p = path.display().to_string();
    match rele_elisp::read_all(&source) {
        Ok(forms) => {
            println!("OK  {p} ({} forms)", forms.len());
            *ok += 1;
        }
        Err(e) => {
            let msg = format!("{e}");
            println!("ERR {p}: {msg}");
            *fail += 1;
            errors.push((p, msg));
        }
    }
}
