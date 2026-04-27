//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use super::spawn_worker;
#[allow(unused_imports)]
use super::*;

/// Per-file outcome sent from a worker manager back to the main loop.
#[allow(dead_code)]
pub(super) enum FileOutcome {
    /// Successfully processed; includes per-test JSONL lines already
    /// formatted by the worker, plus the summary row.
    Ok {
        file_index: usize,
        rel: String,
        jsonl_lines: Vec<String>,
        summary: FileSummary,
        elapsed_ms: u128,
    },
    /// Worker exceeded the wall-clock budget for this file.
    Timeout { file_index: usize, rel: String },
    /// Worker crashed (stdout EOF before __DONE__ / stdin write error).
    Crashed {
        file_index: usize,
        rel: String,
        reason: String,
    },
}
/// One worker: the child subprocess plus a dedicated reader thread
/// that forwards stdout lines into `lines_rx`. The worker process is
/// reused across many files, but it creates a fresh interpreter for
/// each file. Dropping the `Worker` kills the child, which closes its
/// stdout, which makes the reader thread exit.
#[allow(dead_code)]
pub(super) struct Worker {
    pub(super) child: std::process::Child,
    pub(super) lines_rx: std::sync::mpsc::Receiver<Option<String>>,
}
impl Worker {
    #[allow(dead_code)]
    pub(super) fn spawn(bin: &std::path::Path, per_test_ms: u64) -> Option<Self> {
        let mut child = spawn_worker(bin, per_test_ms)?;
        let stdout = child.stdout.take()?;
        let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
        std::thread::spawn(move || {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        if tx.send(Some(l)).is_err() {
                            return;
                        }
                    }
                    Err(_) => {
                        let _ = tx.send(None);
                        return;
                    }
                }
            }
            let _ = tx.send(None);
        });
        Some(Worker {
            child,
            lines_rx: rx,
        })
    }
}
/// Summary row from one `__SUMMARY__` line.
#[derive(Default, Clone, Copy)]
#[allow(dead_code)]
pub(super) struct FileSummary {
    pub(super) passed: usize,
    pub(super) failed: usize,
    pub(super) errored: usize,
    pub(super) skipped: usize,
    pub(super) panicked: usize,
    pub(super) timed_out: usize,
    pub(super) loaded: usize,
    pub(super) total_forms: usize,
}
