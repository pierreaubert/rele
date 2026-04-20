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
//!   {"file":...,"test":...,"result":"pass|fail|error|skip|panic|timeout","ms":N,"detail":"..."}\n
//!   ...
//!   __SUMMARY__ PASS FAIL ERROR SKIP PANIC TIMEOUT LOADED TOTAL\n
//!   __DONE__\n
//!
//! Worker exits cleanly on stdin EOF. All diagnostic noise
//! (stdlib-load errors, GC chatter) goes to stderr so it can't
//! corrupt the stdout protocol.
//!
//! CLI:
//!   emacs_test_worker                       # read paths from stdin (pool mode)
//!   emacs_test_worker --file-list PATH      # read paths from PATH, then exit
//!   emacs_test_worker --per-test-ms N       # override per-test wall-clock (default 8000)
//!
//! `--file-list` is the one-shot replay mode: a developer reproducing
//! a single file's results locally can point the worker at a file
//! that contains one absolute path per line and see the full JSONL
//! stream on stdout without spinning up the pool harness.

use std::io::{BufRead, Write};
use std::path::PathBuf;

use rele_elisp::eval::tests::{
    ensure_stdlib_files, load_cl_lib, load_full_bootstrap, make_stdlib_interp,
    probe_emacs_file, run_rele_ert_tests_detailed_with_timeout,
};

/// Default per-test wall-clock timeout, in milliseconds. A test that
/// spins past this budget has its interpreter's eval-ops limit forced
/// down by a watchdog, which yields an `EvalError` that the runner
/// reclassifies as a `timeout` result. The parent harness also has a
/// per-file deadline (currently 15 s) as a last-resort safety net.
const DEFAULT_PER_TEST_MS: u64 = 8_000;

enum Mode {
    /// Read paths from stdin (one per line); exit on EOF. Pool mode.
    Stdin,
    /// Read all paths from the given file, process them, then exit.
    FileList(PathBuf),
}

struct Config {
    mode: Mode,
    per_test_ms: u64,
}

fn parse_args() -> Result<Config, String> {
    let mut mode = Mode::Stdin;
    let mut per_test_ms = DEFAULT_PER_TEST_MS;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--file-list" => {
                let p = args
                    .next()
                    .ok_or_else(|| "--file-list requires a PATH argument".to_string())?;
                mode = Mode::FileList(PathBuf::from(p));
            }
            "--per-test-ms" => {
                let n = args
                    .next()
                    .ok_or_else(|| "--per-test-ms requires a number".to_string())?;
                per_test_ms = n
                    .parse()
                    .map_err(|_| format!("--per-test-ms: not a number: {n}"))?;
            }
            "-h" | "--help" => {
                return Err("help".to_string());
            }
            other => {
                return Err(format!("unknown argument: {other}"));
            }
        }
    }
    Ok(Config { mode, per_test_ms })
}

fn print_usage() {
    eprintln!(
        "Usage: emacs_test_worker [--file-list PATH] [--per-test-ms N]\n\
         \n\
         Modes:\n  \
           (no flags)              read paths from stdin (pool protocol)\n  \
           --file-list PATH        read newline-separated paths from PATH, then exit\n  \
           --per-test-ms N         per-test wall-clock timeout in ms (default {DEFAULT_PER_TEST_MS})"
    );
}

fn process_file<W: Write>(
    interp: &rele_elisp::Interpreter,
    path: &str,
    per_test_ms: u64,
    out: &mut W,
) -> std::io::Result<()> {
    let load = probe_emacs_file(interp, path);
    let (stats, results) = run_rele_ert_tests_detailed_with_timeout(interp, per_test_ms);
    for r in &results {
        writeln!(out, "{}", r.to_jsonl(path))?;
    }
    let (loaded, total) = load.unwrap_or((0, 0));
    writeln!(
        out,
        "__SUMMARY__ {} {} {} {} {} {} {} {}",
        stats.passed,
        stats.failed,
        stats.errored,
        stats.skipped,
        stats.panicked,
        stats.timed_out,
        loaded,
        total,
    )?;
    writeln!(out, "__DONE__")?;
    out.flush()
}

fn main() {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(msg) => {
            if msg != "help" {
                eprintln!("emacs_test_worker: {msg}");
            }
            print_usage();
            std::process::exit(if msg == "help" { 0 } else { 2 });
        }
    };

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

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match cfg.mode {
        Mode::Stdin => {
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                let Ok(line) = line else { break };
                let path = line.trim();
                if path.is_empty() {
                    continue;
                }
                if process_file(&interp, path, cfg.per_test_ms, &mut out).is_err() {
                    return; // Parent closed stdout — exit cleanly.
                }
            }
        }
        Mode::FileList(list_path) => {
            // This is a CLI worker with no UI thread and no async
            // runtime; blocking at startup is the desired behaviour.
            #[allow(
                clippy::disallowed_methods,
                reason = "CLI worker: no UI thread, blocking read is fine"
            )]
            let contents = match std::fs::read_to_string(&list_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "emacs_test_worker: cannot read --file-list {}: {e}",
                        list_path.display()
                    );
                    std::process::exit(2);
                }
            };
            for line in contents.lines() {
                let path = line.trim();
                if path.is_empty() || path.starts_with('#') {
                    continue;
                }
                if process_file(&interp, path, cfg.per_test_ms, &mut out).is_err() {
                    return;
                }
            }
        }
    }
}
