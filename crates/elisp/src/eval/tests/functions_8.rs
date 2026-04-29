#![allow(clippy::manual_checked_ops)]
//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

pub use super::bootstrap::{
    BOOTSTRAP_FILES, ErtRunStats, ErtTestResult, STDLIB_DIR, emacs_lisp_dir, emacs_source_root,
    ensure_stdlib_files, load_cl_lib, load_file_progress, load_full_bootstrap, load_prerequisites,
    make_stdlib_interp, probe_emacs_file, read_emacs_source, run_rele_ert_tests,
    run_rele_ert_tests_detailed, run_rele_ert_tests_detailed_with_timeout,
};
#[allow(unused_imports)]
use super::*;

use super::types::{FileOutcome, FileSummary, Worker};

/// Regression guard: the `detail` field must not be empty for any
/// non-pass result. Lots of downstream tooling (diff-emacs-results.sh,
/// error-bucket histograms) relies on a usable detail string.
#[test]
fn test_ert_run_detail_is_populated() {
    if !ensure_stdlib_files() {
        return;
    }
    let handle = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let interp = make_stdlib_interp();
            interp
                .eval(read("(ert-deftest rele-pass () (should (= (+ 1 2) 3)))").unwrap())
                .unwrap();
            interp
                .eval(read("(ert-deftest rele-fail () (should (= 1 2)))").unwrap())
                .unwrap();
            interp
                .eval(read("(ert-deftest rele-raw-err () (signal 'my-sig '(\"boom\")))").unwrap())
                .unwrap();
            let (_stats, results) = run_rele_ert_tests_detailed(&interp);
            for r in &results {
                if r.result != "pass" {
                    assert!(
                        !r.detail.is_empty(),
                        "{}: detail was empty for {} result",
                        r.name,
                        r.result,
                    );
                }
            }
            let fail = results
                .iter()
                .find(|r| r.name == "rele-fail")
                .expect("rele-fail missing");
            assert_eq!(fail.result, "fail");
            let raw = results
                .iter()
                .find(|r| r.name == "rele-raw-err")
                .expect("rele-raw-err missing");
            assert_eq!(raw.result, "error");
            assert!(
                raw.detail.contains("my-sig"),
                "raw-err detail should mention signal: {:?}",
                raw.detail,
            );
        })
        .expect("spawn");
    handle.join().expect("join");
}
/// The watchdog in `run_rele_ert_tests_detailed_with_timeout` must
/// trip hanging tests and label them `"timeout"` rather than
/// `"error"`. The test below registers a deftest that spins in a
/// pure-elisp loop — which charges eval ops on every iteration, so
/// the watchdog reliably catches it.
#[test]
fn test_ert_run_per_test_timeout() {
    if !ensure_stdlib_files() {
        return;
    }
    let handle = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let interp = make_stdlib_interp();
            interp
                .eval(
                    read(
                        "(ert-deftest rele-hang () \
                           (while t (ignore 1)))",
                    )
                    .unwrap(),
                )
                .unwrap();
            interp
                .eval(read("(ert-deftest rele-ok () (should (= 1 1)))").unwrap())
                .unwrap();
            let (_stats, results) = run_rele_ert_tests_detailed_with_timeout(&interp, 100);
            let hang = results
                .iter()
                .find(|r| r.name == "rele-hang")
                .expect("rele-hang missing");
            assert_eq!(hang.result, "timeout");
            assert!(
                hang.detail.contains("100ms"),
                "timeout detail should name the budget: {:?}",
                hang.detail,
            );
            let ok = results
                .iter()
                .find(|r| r.name == "rele-ok")
                .expect("rele-ok missing");
            assert_eq!(ok.result, "pass", "non-hang test should pass");
        })
        .expect("spawn");
    handle.join().expect("join");
}
/// Walk `<emacs>/test/**/*.el` and run every test in every file. Each file
/// gets a fresh interpreter. Per-test timeout is enforced by `eval-ops`.
///
/// This is the main full-suite test. To run:
///   cargo test -p rele-elisp --lib test_emacs_all_files_run -- --nocapture --ignored
/// Marked `#[ignore]` because a full run takes minutes; the test_emacs_test_files_run
/// shorter variant covers data-tests.el for routine CI.
#[test]
#[ignore]
fn test_emacs_all_files_run() {
    let Some(root) = emacs_source_root() else {
        return;
    };
    if !ensure_stdlib_files() {
        return;
    }
    let root = root.to_string();
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let test_root = format!("{root}/test");
            let env_files = std::env::var("EMACS_TEST_FILES").ok();
            let mut files: Vec<std::path::PathBuf> = if let Some(list) = env_files {
                list.split(':')
                    .filter(|s| !s.is_empty())
                    .map(|path| {
                        let p = std::path::PathBuf::from(path);
                        if p.is_absolute() {
                            p
                        } else {
                            std::path::PathBuf::from(&root).join(p)
                        }
                    })
                    .collect()
            } else {
                walkdir::WalkDir::new(&test_root)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|e| e.path().extension().is_some_and(|x| x == "el"))
                    .filter(|e| {
                        !e.path().components().any(|c| {
                            let s = c.as_os_str().to_string_lossy();
                            s.ends_with("-resources") || s == "manual" || s == "infra"
                        })
                    })
                    .filter(|e| {
                        let p = e.path().to_string_lossy();
                        !p.contains("/cl-lib-tests.el")
                            && !p.contains("/cl-macs-tests.el")
                            && !p.contains("/comp-tests.el")
                            && !p.contains("/comp-cstr-tests.el")
                            && !p.contains("/completion-tests.el")
                            && !p.contains("/cus-edit-tests.el")
                            && !p.contains("/custom-tests.el")
                            && !p.contains("/dom-tests.el")
                            && !p.contains("/backquote-tests.el")
                            && !p.contains("/bytecomp-tests.el")
                    })
                    .map(|e| e.path().to_path_buf())
                    .collect()
            };
            files.sort();
            eprintln!(
                "Discovered {} .el test files under {test_root}",
                files.len()
            );
            let jsonl_path = std::env::var("EMACS_TEST_RESULT_PATH")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::path::PathBuf::from("target/emacs-test-results.jsonl"));
            if let Some(parent) = jsonl_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut jsonl =
                std::io::BufWriter::new(std::fs::File::create(&jsonl_path).expect("create jsonl"));
            eprintln!("Writing per-test results to {}", jsonl_path.display());
            run_worker_pool(&files, &root, &mut jsonl)
        })
        .expect("spawn");
    handle.join().expect("join");
}
/// Drive a pool of `emacs_test_worker` subprocesses over the file list,
/// write results to `jsonl`, and print an aggregate summary.
#[allow(dead_code)]
fn run_worker_pool(
    files: &[std::path::PathBuf],
    root: &str,
    jsonl: &mut std::io::BufWriter<std::fs::File>,
) {
    use std::io::Write;
    use std::sync::mpsc;
    let n_workers = std::env::var("EMACS_TEST_WORKERS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get().clamp(1, 8))
                .unwrap_or(4)
        });
    let per_test_ms = std::env::var("EMACS_TEST_PER_TEST_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(8_000);
    let per_file_ms = std::env::var("EMACS_TEST_PER_FILE_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(120_000);
    eprintln!("Spawning {n_workers} worker subprocess(es)");
    let (task_tx, task_rx) = mpsc::channel::<(usize, std::path::PathBuf)>();
    let task_rx = std::sync::Arc::new(std::sync::Mutex::new(task_rx));
    for (idx, f) in files.iter().enumerate() {
        task_tx.send((idx, f.clone())).unwrap();
    }
    drop(task_tx);
    let (out_tx, out_rx) = mpsc::channel::<FileOutcome>();
    let mut handles = Vec::new();
    for wid in 0..n_workers {
        let task_rx = std::sync::Arc::clone(&task_rx);
        let out_tx = out_tx.clone();
        let root = root.to_string();
        handles.push(std::thread::spawn(move || {
            worker_manager(wid, task_rx, out_tx, root, per_test_ms, per_file_ms);
        }));
    }
    drop(out_tx);
    let mut grand = ErtRunStats::default();
    let mut files_done = 0;
    let mut files_load_failed = 0;
    let mut files_timed_out = 0;
    let mut files_crashed = 0;
    let total_files = files.len();
    for outcome in out_rx {
        files_done += 1;
        match outcome {
            FileOutcome::Ok {
                file_index,
                rel,
                jsonl_lines,
                summary,
                elapsed_ms,
            } => {
                for line in &jsonl_lines {
                    let _ = writeln!(jsonl, "{line}");
                }
                let t = summary.passed
                    + summary.failed
                    + summary.errored
                    + summary.skipped
                    + summary.panicked
                    + summary.timed_out;
                if summary.loaded == 0 && summary.total_forms == 0 {
                    files_load_failed += 1;
                    eprintln!("[{}/{total_files}] {rel}: load failed", file_index + 1,);
                } else {
                    eprintln!(
                        "[{}/{total_files}] {rel}: loaded {}/{} forms, ERT {} pass / {} fail / {} error / {} skip / {} panic / {} timeout (of {t})",
                        file_index + 1,
                        summary.loaded,
                        summary.total_forms,
                        summary.passed,
                        summary.failed,
                        summary.errored,
                        summary.skipped,
                        summary.panicked,
                        summary.timed_out,
                    );
                    eprintln!("    timing: {elapsed_ms}ms total");
                }
                grand.passed += summary.passed;
                grand.failed += summary.failed;
                grand.errored += summary.errored;
                grand.skipped += summary.skipped;
                grand.panicked += summary.panicked;
                grand.timed_out += summary.timed_out;
            }
            FileOutcome::Timeout { file_index, rel } => {
                eprintln!(
                    "[{}/{total_files}] {rel}: TIMEOUT — worker killed & respawned",
                    file_index + 1,
                );
                let _ = writeln!(
                    jsonl,
                    r#"{{"file":"{}","test":"<file>","result":"timeout","ms":120000,"detail":""}}"#,
                    rel,
                );
                files_timed_out += 1;
            }
            FileOutcome::Crashed {
                file_index,
                rel,
                reason,
            } => {
                eprintln!(
                    "[{}/{total_files}] {rel}: CRASHED ({reason}) — respawned",
                    file_index + 1,
                );
                let _ = writeln!(
                    jsonl,
                    r#"{{"file":"{}","test":"<file>","result":"crash","ms":0,"detail":"{}"}}"#,
                    rel,
                    reason.replace('"', "'"),
                );
                files_crashed += 1;
            }
        }
    }
    for h in handles {
        let _ = h.join();
    }
    let total = grand.passed
        + grand.failed
        + grand.errored
        + grand.skipped
        + grand.panicked
        + grand.timed_out;
    let pct = if total > 0 {
        grand.passed * 100 / total
    } else {
        0
    };
    eprintln!(
        "\n=== Emacs test suite summary ===\n\
         Files:  {files_done} run, {files_load_failed} load-failed, {files_timed_out} timed out, {files_crashed} crashed\n\
         Tests:  {} pass / {} fail / {} error / {} skip / {} panic / {} timeout (of {total})\n\
         Pass rate: {pct}%",
        grand.passed, grand.failed, grand.errored, grand.skipped, grand.panicked, grand.timed_out,
    );
}
/// One manager thread owns one persistent worker subprocess. Pulls
/// files from the shared queue, writes to worker stdin, reads
/// `__SUMMARY__` + `__DONE__` from stdout. Only respawns on
/// timeout/crash. The child creates a fresh bootstrapped interpreter
/// for every file so ERT registrations and global Lisp state do not
/// leak between files.
#[allow(dead_code)]
fn worker_manager(
    wid: usize,
    task_rx: std::sync::Arc<
        std::sync::Mutex<std::sync::mpsc::Receiver<(usize, std::path::PathBuf)>>,
    >,
    out_tx: std::sync::mpsc::Sender<FileOutcome>,
    root: String,
    per_test_ms: u64,
    per_file_ms: u64,
) {
    use std::io::Write;
    use std::time::Duration;
    let worker_bin = worker_binary_path();
    let mut worker = match Worker::spawn(&worker_bin, per_test_ms) {
        Some(w) => w,
        None => {
            eprintln!("worker {wid}: initial spawn failed, manager exiting");
            return;
        }
    };
    loop {
        let (file_index, file) = {
            let rx = task_rx.lock().unwrap();
            match rx.recv() {
                Ok(x) => x,
                Err(_) => break,
            }
        };
        let path_str = file.to_string_lossy().to_string();
        let rel = file
            .strip_prefix(&root)
            .unwrap_or(&file)
            .display()
            .to_string();
        let stdin_ok = worker
            .child
            .stdin
            .as_mut()
            .map(|stdin| writeln!(stdin, "{}", path_str).and_then(|_| stdin.flush()))
            .and_then(Result::ok);
        if stdin_ok.is_none() {
            let _ = out_tx.send(FileOutcome::Crashed {
                file_index,
                rel,
                reason: "stdin write failed".into(),
            });
            worker = match Worker::spawn(&worker_bin, per_test_ms) {
                Some(w) => w,
                None => return,
            };
            continue;
        }
        let mut jsonl_lines: Vec<String> = Vec::new();
        let mut summary: Option<FileSummary> = None;
        let mut done = false;
        let mut crashed = false;
        let started = std::time::Instant::now();
        let deadline = started + Duration::from_millis(per_file_ms);
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match worker.lines_rx.recv_timeout(remaining) {
                Ok(Some(line)) => {
                    if line == "__DONE__" {
                        done = true;
                        break;
                    } else if let Some(rest) = line.strip_prefix("__SUMMARY__ ") {
                        summary = parse_summary(rest);
                    } else if line.starts_with('{') {
                        jsonl_lines.push(line);
                    }
                }
                Ok(None) => {
                    crashed = true;
                    break;
                }
                Err(_) => break,
            }
        }
        let outcome = if done {
            FileOutcome::Ok {
                file_index,
                rel,
                jsonl_lines,
                summary: summary.unwrap_or_default(),
                elapsed_ms: started.elapsed().as_millis(),
            }
        } else if crashed {
            worker = match Worker::spawn(&worker_bin, per_test_ms) {
                Some(w) => w,
                None => return,
            };
            FileOutcome::Crashed {
                file_index,
                rel,
                reason: "worker stdout EOF before __DONE__".into(),
            }
        } else {
            worker = match Worker::spawn(&worker_bin, per_test_ms) {
                Some(w) => w,
                None => return,
            };
            FileOutcome::Timeout { file_index, rel }
        };
        if out_tx.send(outcome).is_err() {
            break;
        }
    }
}
#[allow(dead_code)]
fn parse_summary(rest: &str) -> Option<FileSummary> {
    let parts: Vec<&str> = rest.split_whitespace().collect();
    match parts.len() {
        7 => Some(FileSummary {
            passed: parts[0].parse().ok()?,
            failed: parts[1].parse().ok()?,
            errored: parts[2].parse().ok()?,
            skipped: parts[3].parse().ok()?,
            panicked: parts[4].parse().ok()?,
            timed_out: 0,
            loaded: parts[5].parse().ok()?,
            total_forms: parts[6].parse().ok()?,
        }),
        n if n >= 8 => Some(FileSummary {
            passed: parts[0].parse().ok()?,
            failed: parts[1].parse().ok()?,
            errored: parts[2].parse().ok()?,
            skipped: parts[3].parse().ok()?,
            panicked: parts[4].parse().ok()?,
            timed_out: parts[5].parse().ok()?,
            loaded: parts[6].parse().ok()?,
            total_forms: parts[7].parse().ok()?,
        }),
        _ => None,
    }
}
#[allow(dead_code)]
fn worker_binary_path() -> std::path::PathBuf {
    let mut exe = std::env::current_exe().expect("current_exe");
    while exe.pop() {
        let candidate = exe.join("emacs_test_worker");
        if candidate.exists() {
            return candidate;
        }
        let candidate = exe.join("../emacs_test_worker");
        if candidate.exists() {
            return candidate.canonicalize().unwrap_or(candidate);
        }
    }
    std::path::PathBuf::from("./target/release/emacs_test_worker")
}
#[allow(dead_code)]
pub(super) fn spawn_worker(bin: &std::path::Path, per_test_ms: u64) -> Option<std::process::Child> {
    use std::process::{Command, Stdio};
    Command::new(bin)
        .arg("--per-test-ms")
        .arg(per_test_ms.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}
/// Load a curated short list of Emacs test files and run them. Suitable
/// for routine CI (vs the full `test_emacs_all_files_run` which takes
/// minutes). Reports pass/fail/error counts as a baseline metric.
/// Marked `#[ignore]` because load_full_bootstrap allocates a lot
/// (~200 MB, runs many .el files) and would OOM the parallel test
/// runner. Run with `cargo test --release -- --ignored test_emacs_test_files_run`.
#[test]
#[ignore]
fn test_emacs_test_files_run() {
    let Some(root) = emacs_source_root() else {
        return;
    };
    if !ensure_stdlib_files() {
        return;
    }
    let root = root.to_string();
    let env_files = std::env::var("EMACS_TEST_FILES").ok();
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let default_files = ["test/src/data-tests.el"];
            let file_list: Vec<String> = match env_files {
                Some(s) => {
                    s.split(':').filter(|s| !s.is_empty()).map(String::from).collect()
                }
                None => default_files.iter().map(|&s| s.to_string()).collect(),
            };
            let files: Vec<&str> = file_list.iter().map(String::as_str).collect();
            let mut grand = ErtRunStats::default();
            for file in files {
                let interp = make_stdlib_interp();
                load_full_bootstrap(&interp);
                let path = format!("{root}/{file}");
                let load_summary = probe_emacs_file(&interp, &path);
                let s = run_rele_ert_tests(&interp);
                let total = s.passed + s.failed + s.errored + s.skipped + s.panicked;
                match load_summary {
                    Some((ok, n)) => {
                        eprintln!(
                            "  {file}: loaded {ok}/{n} forms, ERT {} pass / {} fail / {} error / {} skip / {} panic (of {total})",
                            s.passed, s.failed, s.errored, s.skipped, s.panicked,
                        )
                    }
                    None => eprintln!("  {file}: not loadable"),
                }
                grand.passed += s.passed;
                grand.failed += s.failed;
                grand.errored += s.errored;
                grand.skipped += s.skipped;
                grand.panicked += s.panicked;
            }
            eprintln!(
                "TOTAL: {} pass / {} fail / {} error / {} skip / {} panic", grand.passed,
                grand.failed, grand.errored, grand.skipped, grand.panicked,
            );
        })
        .expect("spawn");
    handle.join().expect("join");
}
