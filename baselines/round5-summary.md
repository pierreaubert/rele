# Round-5 post-merge ERT baseline summary

- Source tree: Emacs 29.3 (github emacs-mirror, branch `emacs-29.3`) at `/tmp/emacs-src`
- Main branch commit: `df766dd` (PR #31 merged; includes R1–R15, R17–R20)
- Harness: `cargo test -p rele-elisp --release --ignored test_emacs_all_files_run`
- Data file: `baselines/emacs-test-results-round5-post.jsonl`

## Run outcome

The harness completed (exit 0) but under severe per-file timeout pressure:

- **467** Emacs test files discovered
- **29** files loaded successfully
- **9** files load-failed (0/N forms parsed)
- **429** files exceeded the 15 s per-file wall-clock budget (`TIMEOUT — worker killed & respawned`)
- **0** files crashed

Only **~1.6%** of the baseline test volume was reached before timeouts; the
rest of the data is dominated by 411 `"<file>"` timeout markers.

## Aggregate counts (captured from /proc/PID/fd/3 during the run)

| metric | round-2 baseline | round-5 post |
|---|---:|---:|
| total JSONL rows | 5477 | 500 |
| pass | 268 | 3 |
| fail | 834 | 8 |
| error | 3766 | 70 |
| skip | 554 | 8 |
| timeout (per-test + per-file) | 49 | 411 |
| crash | 3 | 0 |
| panic | 3 | 0 |

## Diff vs round-2 baseline (`diff-emacs-results.sh`, path-normalised)

- Newly passing: **0**
- Newly failing / disappeared: **265**

All 265 "newly failing" rows are tests that passed in round-2 but did not
appear in the round-5 dataset — because the enclosing file timed out
before any ERT form could run. Only **3** tests actually executed to a
pass verdict in round-5.

## Top 20 failure categories (round-5 data)

| count | detail |
|---:|---|
| 411 | *(file-level timeout, no detail)* |
| 23 | `void function: tempo-define-template` |
| 8 | *(skipped, no detail)* |
| 7 | `void function: nil` |
| 5 | `void function: semantic-idle-scheduler-mode` |
| 3 | `((equal (match-string 0 str) str))` |
| 3 | `wrong type argument: expected string` |
| 3 | `wrong type argument: expected list` |
| 3 | `void function: vc-hg-annotate-extract-revision-at-line` |
| 2 | `wrong type argument: expected symbol` |
| 2 | `void function: battery--upower-state` |
| 2 | `wrong type argument: expected array` |
| 2 | `void function: sasl-find-mechanism` |
| 2 | `((equal (xterm-mouse-tracking-enable-sequence) "X"))` |
| 1 | `void function: ansi-color-apply-on-region` |
| 1 | `void function: ansi-color-apply` |
| 1 | `void function: battery-format` |
| 1 | `void function: semantic-fetch-tags` |
| 1 | `evaluation error: mapcan needs eval dispatch` |
| 1 | `signal wrong-type-argument: (timerp [timer nil nil nil …])` |

## Top 30 `void function: X` (round-5)

```
23 tempo-define-template
 7 nil
 5 semantic-idle-scheduler-mode
 3 vc-hg-annotate-extract-revision-at-line
 2 battery--upower-state
 2 sasl-find-mechanism
 1 ansi-color-apply-on-region
 1 ansi-color-apply
 1 battery-format
 1 semantic-fetch-tags
 1 ido-directory-too-big-p
 1 footnote-mode
 1 nsm-network-same-subnet
 1 ert-info
 1 smerge-mode
 1 vc-hg-annotate-time
 1 x-dnd-xm-read-targets-table
 1 x-dnd-do-direct-save
```

## Top `void variable: X` (round-5)

```
1 ido-mode
1 xterm-mouse-mode
```

## Run environment caveat

The harness's per-file wall-clock deadline is hard-coded to 15 seconds
(`crates/elisp/src/eval/tests.rs:6846`). Under the R21 run conditions
(many concurrent cargo/rustc processes from sibling agent worktrees,
9p shared FS) this budget was not enough for 92% of files. A single
worker processing `abbrev-tests.el` from stdin in isolation completed
in ~9 s on the same machine, suggesting the timeout is marginal rather
than intrinsically wrong; under a loaded machine it fires on almost
every file.

Additionally, a separate process (not identified) repeatedly
overwrote `crates/elisp/target/emacs-test-results.jsonl` with the
round-2 baseline content immediately after the harness exited — the
round-5 data was recovered by continuously snapshotting
`/proc/<harness-pid>/fd/3` into `/tmp/jsonl-final-r21.txt` during the
run, then copying that snapshot into `baselines/`.
