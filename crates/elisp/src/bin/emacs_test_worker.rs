//! Worker subprocess for the Emacs ERT test harness.
//!
//! One instance bootstraps the stdlib once (~2 s), then processes
//! many test files. The parent harness (`test_emacs_all_files_run`)
//! maintains a pool of these and dispatches file paths via stdin,
//! collecting JSONL results on stdout. If a worker crashes (stack
//! overflow, SIGABRT, etc.) only the parent's per-worker manager
//! thread sees it; the parent respawns and moves on.
//!
//! Protocol (line-oriented, stdin/stdout):
//!
//! parent → worker, one per request:
//!   <absolute-path-to-el-file>\n
//!
//! worker → parent per file, in order:
//!   {"file":...,"test":...,"result":"pass|fail|error|skip|panic","ms":N,"detail":"..."}\n
//!   ...
//!   __SUMMARY__ PASS FAIL ERROR SKIP PANIC LOADED TOTAL\n
//!   __DONE__\n
//!
//! Worker exits cleanly on stdin EOF. All diagnostic noise
//! (stdlib-load errors, GC chatter) goes to stderr so it can't
//! corrupt the stdout protocol.

use std::io::{BufRead, Write};

use rele_elisp::eval::tests::{
    ensure_stdlib_files, load_cl_lib, load_full_bootstrap, make_stdlib_interp,
    probe_emacs_file, run_rele_ert_tests_detailed,
};

fn main() {
    // Make sure the stdlib `.el` files exist under /tmp/elisp-stdlib/
    // (ensure_stdlib_files gunzips them from the Emacs installation on
    // first call — cheap if they already exist).
    if !ensure_stdlib_files() {
        eprintln!("emacs_test_worker: Emacs source not found; exiting");
        std::process::exit(2);
    }

    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);
    // cl-macs.el loading still OOMs even with periodic GC and per-
    // subprocess isolation — macro expansion in our interpreter
    // allocates ~20 GB RSS before finishing. Tests that need
    // `cl-loop` / `cl-flet` / `cl-destructuring-bind` rely on the
    // tiny Rust-side shims in `load_full_bootstrap` (`cl-incf`,
    // `cl-decf`, `gv-ref`) plus whatever native handlers the eval
    // dispatch provides. Implementing a full native cl-macs subset
    // is tracked as plan-B future work.
    let _ = load_cl_lib; // keep the symbol live / referenced

    // Signal readiness to the parent.
    eprintln!("__READY__");

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let path = line.trim();
        if path.is_empty() {
            continue;
        }

        let load = probe_emacs_file(&interp, path);
        let (stats, results) = run_rele_ert_tests_detailed(&interp);

        for r in &results {
            if writeln!(out, "{}", r.to_jsonl(path)).is_err() {
                return; // Parent closed stdout — exit cleanly.
            }
        }
        let (loaded, total) = load.unwrap_or((0, 0));
        let _ = writeln!(
            out,
            "__SUMMARY__ {} {} {} {} {} {} {}",
            stats.passed,
            stats.failed,
            stats.errored,
            stats.skipped,
            stats.panicked,
            loaded,
            total,
        );
        let _ = writeln!(out, "__DONE__");
        if out.flush().is_err() {
            return;
        }
    }
}
