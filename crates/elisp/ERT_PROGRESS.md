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
   plus "top failure patterns" and "top runtime stub hits" lists. Raw results
   land in `tmp/ert-baseline.jsonl`; the source-derived stub inventory lands
   in `tmp/elisp-stub-inventory.tsv`.
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

Last refreshed: **2026-04-30**, target: `ert-progress/tractable.list`.
Current total: **837 pass / 166 fail / 31 err / 129 skip** (`72%`).

| File                       | Pass | Fail | Err | Skip | Pct  | Notes |
|----------------------------|-----:|-----:|----:|-----:|-----:|-------|
| alloc-tests.el             |    4 |    0 |   0 |    0 | 100% | |
| buffer-tests.el            |  408 |    1 |   0 |    1 | 100% | |
| callint-tests.el           |    4 |    0 |   0 |    0 | 100% | call-interactively complete |
| casefiddle-tests.el        |    1 |    7 |   2 |    1 |   9% | case tables |
| category-tests.el          |    1 |    4 |   1 |    0 |  17% | category tables |
| character-tests.el         |    3 |    0 |   0 |    0 | 100% | |
| charset-tests.el           |    5 |   15 |   0 |    1 |  24% | charset infrastructure |
| chartab-tests.el           |    6 |    0 |   0 |    0 | 100% | |
| cmds-tests.el              |    2 |    0 |   0 |    0 | 100% | |
| coding-tests.el            |   11 |   16 |   0 |    1 |  39% | coding systems |
| data-tests.el              |   74 |    3 |   0 |    2 |  94% | format edge cases |
| decompress-tests.el        |    0 |    0 |   0 |    1 |   0% | needs zlib |
| doc-tests.el               |    3 |    2 |   0 |    0 |  60% | autoloadp recognition |
| editfns-tests.el           |   20 |   32 |   5 |    0 |  35% | edit primitives improved; transpose/format remain |
| eval-tests.el              |    ? |    ? |   ? |    ? |    ? | no results emitted in last sweep |
| floatfns-tests.el          |   28 |    2 |   3 |    0 |  85% | bignum edge cases |
| font-tests.el              |    0 |    2 |   0 |    0 |   0% | |
| image-tests.el             |    3 |    0 |   0 |    2 |  60% | |
| indent-tests.el            |    0 |    3 |   0 |    0 |   0% | |
| inotify-tests.el           |    0 |    0 |   0 |    3 |   0% | needs inotify |
| json-tests.el              |   17 |    6 |   2 |    0 |  68% | JSON encode/decode |
| keyboard-tests.el          |    1 |    2 |   0 |    0 |  33% | |
| keymap-tests.el            |    8 |   35 |   4 |    0 |  17% | keymap manipulation |
| lcms-tests.el              |    0 |    0 |   0 |    6 |   0% | needs lcms |
| lread-tests.el             |   42 |   12 |   4 |    0 |  72% | reader edge cases |
| marker-tests.el            |    3 |    5 |   4 |    0 |  25% | marker semantics |
| minibuf-tests.el           |   61 |    5 |   0 |    0 |  92% | obarray-predicate, ignore-case |
| process-tests.el           |   12 |    0 |   0 |   27 |  31% | supportable headless cases pass |
| profiler-tests.el          |    0 |    0 |   1 |    1 |   0% | |
| search-tests.el            |    0 |    1 |   0 |    0 |   0% | |
| sqlite-tests.el            |    0 |    0 |   0 |   12 |   0% | needs sqlite |
| syntax-tests.el            |   98 |    2 |   0 |    0 |  98% | char-syntax edge cases |
| terminal-tests.el          |    0 |    0 |   1 |    0 |   0% | |
| textprop-tests.el          |    1 |    1 |   0 |    0 |  50% | |
| thread-tests.el            |    0 |    0 |   1 |   36 |   0% | needs threads |
| treesit-tests.el           |    1 |    2 |   0 |   35 |   3% | needs tree-sitter |
| undo-tests.el              |   17 |    0 |   0 |    0 | 100% | supportable cases pass; raw run still records two timeout entries |
| xdisp-tests.el             |    1 |    6 |   3 |    0 |  10% | display engine |
| xfaces-tests.el            |    2 |    1 |   0 |    0 |  67% | faces |
| xml-tests.el               |    0 |    1 |   0 |    0 |   0% | needs libxml |

## Top leverage targets (2026-04-30)

These are the failure patterns ranked by impact. A single fix at any of
these unblocks the listed count of tests at once. Verify the count is still
current by running `./ert-progress/refresh.sh` before tackling.

| Tests | Pattern | Likely cause |
|------:|---------|--------------|
|  3 | `WRONG_TYPE_STRING` in casefiddle/lread tests | string-vs-char validation |
|  3 | `WRONG_N_ARGS` in editfns-tests.el | transpose-regions arity/dispatch edge cases |
|  3 | `WRONG_TYPE_INTEGER` in floatfns-tests.el | bignum numeric edge cases |
|  3 | `VOID_VAR: help-mode-map` in keymap-tests.el | help-mode keymap bootstrap |
|  3 | `ASSERT: division by zero` in xdisp-tests.el | display geometry stubs |
|  2 | `ASSERT: ascii charset membership` in charset-tests.el | charset equivalence/declaration model |
|  2 | `ASSERT: keymap lookup/where-is` in keymap-tests.el | menu-vector and keymap traversal semantics |
|  2 | `ASSERT: marker buffer/window semantics` in marker-tests.el | marker/window-buffer compatibility |

## Runtime stub hits (2026-04-30)

`refresh.sh` now records stub/no-op primitive calls per ERT test and ranks
them by failing/erroring tests affected. Use this before chasing individual
assertions. The inventory currently classifies `1117` records:
`editing/regions=63`, `category/case-tables=91`, `window/display=255`,
`keymap/help=44`, `other=664`.

| Bad tests | Hits | Bucket | Stub | Example |
|----------:|-----:|--------|------|---------|
| 5 | 9 | editing/regions | `transpose-regions` | `editfns-tests.el::transpose-ascii-regions-test` |
| 4 | 62 | window/display | `force-mode-line-update->ignore` | `casefiddle-tests.el::casefiddle-tests-casing` |
| 4 | 5 | category/case-tables | `make-category-table` | `category-tests.el::category-tests-category-table` |
| 4 | 4 | window/display | `window-prev-buffers` | `editfns-tests.el::replace-buffer-contents-bug31837` |
| 4 | 4 | window/display | `window-dedicated-p->ignore` | `editfns-tests.el::replace-buffer-contents-bug31837` |
| 4 | 4 | window/display | `select-window` | `editfns-tests.el::replace-buffer-contents-bug31837` |
| 3 | 17 | editing/regions | `upcase-initials-region` | `casefiddle-tests.el::casefiddle-tests-casing` |
| 3 | 4 | keymap/help | `describe-function->ignore` | `marker-tests.el::marker-set-window-start-from-other-buffer` |
| 3 | 3 | window/display | `window-text-pixel-size` | `xdisp-tests.el::xdisp-tests--window-text-pixel-size` |
| 3 | 3 | window/display | `frame-char-width` | `xdisp-tests.el::xdisp-tests--window-text-pixel-size` |

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
- `ert-progress/stub_inventory.py` — source-derived stub/backlog report.
- `ert-progress/SESSIONS.md` — append-only session log (newest first).
- `tmp/ert-baseline.jsonl` — latest worker output (regenerated).
- `tmp/elisp-stub-inventory.tsv` — latest stub inventory (regenerated).
