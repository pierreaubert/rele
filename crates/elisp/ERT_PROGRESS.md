# ERT Progress Dashboard

Tracking the rele-elisp interpreter's coverage of Emacs's ERT test suite. The
goal is to make every reasonably-supportable test in `<emacs>/test/src/` and
`<emacs>/test/lisp/` pass. Some categories are blocked by infrastructure we
don't model (native compilation, dynamic C modules, subprocesses, file
locking) and are listed under [Skip rationale](#skip-rationale) below.

## Workflow per session

1. Read this file to see current state and next leverage targets.
2. Run `./ert-progress/refresh.sh` to regenerate the baseline. It builds the
   worker, runs every file in `tractable.list`, and prints a per-file table
   plus a "top failure patterns" list. Raw results land in
   `tmp/ert-baseline.jsonl`.
3. Pick the highest-leverage pattern (one fix that unblocks many tests).
4. Implement the fix; run `cargo test -p rele-elisp` to check for
   regressions in the internal unit suite.
5. Re-run `./ert-progress/refresh.sh` and verify the pattern count dropped.
6. Update the [Per-file snapshot](#per-file-snapshot) and append an entry to
   `ert-progress/SESSIONS.md`.

The summary script is intentionally lossy — it categorises failures by
leading shape so similar bugs cluster together. Patterns labelled `ASSERT:`
are unique-shape asserts that don't generalise; `WRONG_TYPE_*` /
`VOID_FN:` / `VOID_VAR:` / `READER:` / `SIGNAL:` typically point at one bug
that a single fix will close.

## Per-file snapshot

Last refreshed: **2026-04-27**, target: `<emacs>/test/src/`.

| File                       | Pass | Fail | Err | Skip | Pct  | Notes |
|----------------------------|-----:|-----:|----:|-----:|-----:|-------|
| alloc-tests.el             |    4 |    0 |   0 |    0 | 100% | |
| buffer-tests.el            |  410 |    0 |   0 |    1 | 100% | |
| callint-tests.el           |    0 |    2 |   2 |    0 |   0% | call-interactively edge cases |
| casefiddle-tests.el        |    0 |    5 |   5 |    1 |   0% | case tables |
| category-tests.el          |    1 |    4 |   1 |    0 |  17% | category tables |
| character-tests.el         |    3 |    0 |   0 |    0 | 100% | |
| charset-tests.el           |    3 |   17 |   0 |    1 |  14% | charset infrastructure |
| chartab-tests.el           |    6 |    0 |   0 |    0 | 100% | |
| cmds-tests.el              |    1 |    0 |   1 |    0 |  50% | bignum forward-line |
| coding-tests.el            |    8 |   19 |   0 |    1 |  29% | coding systems |
| data-tests.el              |   74 |    3 |   0 |    2 |  94% | format edge cases |
| decompress-tests.el        |    0 |    0 |   0 |    1 |   0% | needs zlib |
| doc-tests.el               |    3 |    2 |   0 |    0 |  60% | autoloadp recognition |
| editfns-tests.el           |    6 |   37 |  14 |    0 |  11% | format spec, missing fns |
| eval-tests.el              |    ? |    ? |   ? |    ? |    ? | runs slow; not in last sweep |
| floatfns-tests.el          |    ? |    ? |   ? |    ? |    ? | not in last sweep |
| font-tests.el              |    0 |    2 |   0 |    0 |   0% | |
| image-tests.el             |    3 |    0 |   0 |    2 |  60% | |
| indent-tests.el            |    0 |    3 |   0 |    0 |   0% | |
| inotify-tests.el           |    0 |    0 |   0 |    3 |   0% | needs inotify |
| json-tests.el              |    0 |   24 |   1 |    0 |   0% | JSON encode/decode |
| keyboard-tests.el          |    0 |    3 |   0 |    0 |   0% | |
| keymap-tests.el            |    8 |   36 |   4 |    0 |  17% | keymap manipulation |
| lcms-tests.el              |    0 |    0 |   0 |    6 |   0% | needs lcms |
| lread-tests.el             |   18 |   35 |   5 |    0 |  31% | reader edge cases |
| marker-tests.el            |    4 |    6 |   2 |    0 |  33% | marker semantics |
| minibuf-tests.el           |   60 |    6 |   0 |    0 |  91% | obarray-predicate, ignore-case |
| process-tests.el           |    2 |    3 |  12 |   22 |   5% | subprocess (mostly skipped) |
| profiler-tests.el          |    0 |    0 |   1 |    1 |   0% | |
| search-tests.el            |    0 |    1 |   0 |    0 |   0% | |
| sqlite-tests.el            |    0 |    0 |   0 |   12 |   0% | needs sqlite |
| syntax-tests.el            |   20 |   16 |  64 |    0 |  20% | parse-partial-sexp |
| terminal-tests.el          |    0 |    0 |   1 |    0 |   0% | |
| textprop-tests.el          |    1 |    1 |   0 |    0 |  50% | |
| thread-tests.el            |    0 |    0 |   1 |   36 |   0% | needs threads |
| treesit-tests.el           |    1 |    2 |   0 |   35 |   3% | needs tree-sitter |
| undo-tests.el              |    0 |    4 |  14 |    0 |   0% | undo machinery |
| xdisp-tests.el             |    1 |    6 |   3 |    0 |  10% | display engine |
| xfaces-tests.el            |    1 |    2 |   0 |    0 |  33% | faces |
| xml-tests.el               |    0 |    1 |   0 |    0 |   0% | needs libxml |

## Top leverage targets (2026-04-27)

These are the failure patterns ranked by impact. A single fix at any of
these unblocks the listed count of tests at once. Verify the count is still
current by running `./ert-progress/refresh.sh` before tackling.

| Tests | Pattern | Likely cause |
|------:|---------|--------------|
| 64 | `WRONG_TYPE_INTEGER` in syntax-tests.el | `parse-partial-sexp` over Pascal-style syntax tables |
| 21 | `DID_NOT_SIGNAL` in lread-tests.el | reader doesn't validate as many invalid syntaxes as Emacs |
| 21 | `ASSERT: try-completion …` in keymap-tests.el | menu-bar / `where-is-internal` lookups |
| 14 | `ASSERT: ((equal (try-completion …))` (residual minibuf) | obarray-predicate, ignore-case |
| 13 | various format `%b` / `%#x` / `%n$` patterns in editfns-tests.el | `format` specifier set incomplete |
| 10 | `signal wrong-type-argument: (timerp …)` | `timerp` recognising vector timer objects |
|  6 | `(memq (quote ascii) charsets)` etc. in charset-tests.el | charset registry not populated |
|  5 | `where-is-internal` in keymap-tests.el | reverse keymap lookup |
|  3 | `bidi-find-overridden-directionality` | not implemented |

## Skip rationale

Files NOT in `tractable.list`, with reasons:

- `comp-tests.el` — Cranelift-based native compilation tests. Exercising
  the bytecode path is in scope; the emit-and-load-shared-object path is
  not. Excluded from automated runs.
- `emacs-module-tests.el` — Loads a `.so` written in C via `dlopen`. Out of
  scope without a C-compatible module ABI.
- `callproc-tests.el` — Spawns subprocesses (`call-process`). Out of scope.
- `filelock-tests.el` — `flock`/lock-file semantics. Out of scope.
- `fileio-tests.el` — Heavy filesystem-side-effect tests; deferred until we
  have a deterministic test fixture story.
- `fns-tests.el` — Loadable but extremely large; pull in to tractable.list
  once smaller files are at >90%.
- `emacs-tests.el` — Tests the `emacs` binary itself (seccomp/bwrap/etc.).
  Out of scope.

The two skip categories ("won't support" and "not yet") aren't separated in
the list above — when promoting a file from skip to tractable, just add it
to `tractable.list`.

## Files

- `ERT_PROGRESS.md` — this file. Update by hand each session.
- `ert-progress/tractable.list` — file list refresh.sh consumes.
- `ert-progress/refresh.sh` — refresh the baseline + print summary.
- `ert-progress/summarize.py` — JSONL → summary table.
- `ert-progress/SESSIONS.md` — append-only session log (newest first).
- `tmp/ert-baseline.jsonl` — latest worker output (regenerated).
