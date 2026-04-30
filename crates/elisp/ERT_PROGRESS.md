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
Current total: **887 pass / 123 fail / 25 err / 128 skip** (`76%`).

| File                       | Pass | Fail | Err | Skip | Pct  | Notes |
|----------------------------|-----:|-----:|----:|-----:|-----:|-------|
| alloc-tests.el             |    4 |    0 |   0 |    0 | 100% | |
| buffer-tests.el            |  408 |    1 |   0 |    1 | 100% | |
| callint-tests.el           |    4 |    0 |   0 |    0 | 100% | call-interactively complete |
| casefiddle-tests.el        |    1 |    7 |   2 |    1 |   9% | case tables |
| category-tests.el          |    4 |    2 |   0 |    0 |  67% | lightweight category tables |
| character-tests.el         |    3 |    0 |   0 |    0 | 100% | |
| charset-tests.el           |   14 |    6 |   1 |    0 |  67% | lightweight charset tables |
| chartab-tests.el           |    6 |    0 |   0 |    0 | 100% | |
| cmds-tests.el              |    2 |    0 |   0 |    0 | 100% | |
| coding-tests.el            |   12 |   15 |   0 |    1 |  43% | coding systems |
| data-tests.el              |   74 |    3 |   0 |    2 |  94% | format edge cases |
| decompress-tests.el        |    0 |    0 |   0 |    1 |   0% | needs zlib |
| doc-tests.el               |    2 |    3 |   0 |    0 |  40% | documentation semantics need follow-up |
| editfns-tests.el           |   36 |   18 |   3 |    0 |  63% | field/text properties improved; format remains |
| eval-tests.el              |    ? |    ? |   ? |    ? |    ? | no results emitted in last sweep |
| floatfns-tests.el          |   28 |    2 |   3 |    0 |  85% | bignum edge cases |
| font-tests.el              |    2 |    0 |   0 |    0 | 100% | headless font parsing complete |
| image-tests.el             |    3 |    0 |   0 |    2 |  60% | |
| indent-tests.el            |    0 |    3 |   0 |    0 |   0% | |
| inotify-tests.el           |    0 |    0 |   0 |    3 |   0% | needs inotify |
| json-tests.el              |   17 |    7 |   1 |    0 |  68% | JSON encode/decode |
| keyboard-tests.el          |    1 |    2 |   0 |    0 |  33% | |
| keymap-tests.el            |   21 |   25 |   1 |    0 |  45% | keymap/help traversal improved |
| lcms-tests.el              |    0 |    0 |   0 |    6 |   0% | needs lcms |
| lread-tests.el             |   42 |   12 |   4 |    0 |  72% | reader edge cases |
| marker-tests.el            |    3 |    5 |   4 |    0 |  25% | marker semantics |
| minibuf-tests.el           |   62 |    4 |   0 |    0 |  94% | obarray-predicate, ignore-case |
| process-tests.el           |   12 |    0 |   0 |   27 |  31% | supportable headless cases pass |
| profiler-tests.el          |    0 |    0 |   1 |    1 |   0% | |
| search-tests.el            |    0 |    1 |   0 |    0 |   0% | |
| sqlite-tests.el            |    0 |    0 |   0 |   12 |   0% | needs sqlite |
| syntax-tests.el            |   98 |    0 |   2 |    0 |  98% | char-syntax edge cases |
| terminal-tests.el          |    0 |    0 |   1 |    0 |   0% | |
| textprop-tests.el          |    1 |    1 |   0 |    0 |  50% | |
| thread-tests.el            |    0 |    0 |   1 |   36 |   0% | needs threads |
| treesit-tests.el           |    1 |    2 |   0 |   35 |   3% | needs tree-sitter |
| undo-tests.el              |   16 |    0 |   1 |    0 |  94% | one new error after text-property pass |
| xdisp-tests.el             |    8 |    2 |   0 |    0 |  80% | bidi/display-property paths improved |
| xfaces-tests.el            |    2 |    1 |   0 |    0 |  67% | faces |
| xml-tests.el               |    0 |    1 |   0 |    0 |   0% | needs libxml |

### Targeted follow-up after snapshot

The full-suite total above has not been rerun after the latest
string/coding/window stub pass. Targeted runs after that pass show:

| File              | Result | Notes |
|-------------------|--------|-------|
| xdisp-tests.el    | 9 pass / 1 fail / 0 err / 0 skip | `read-string` now runs minibuffer setup hooks |
| terminal-tests.el | 1 pass / 0 fail / 0 err / 0 skip | single headless terminal object is live |
| coding-tests.el   | 14 pass / 13 fail / 0 err / 1 skip | `detect-coding-string` and unibyte string stubs removed from hit path |
| buffer-tests.el   | 408 pass / 1 fail / 0 err / 1 skip | `delete-file-internal` no longer hits a stub |
| undo-tests.el     | 16 pass / 0 fail / 1 err / 0 skip | `recent-auto-save-p` no longer hits a stub |

## Top leverage targets (2026-04-30)

These are the failure patterns ranked by impact. A single fix at any of
these unblocks the listed count of tests at once. Verify the count is still
current by running `./ert-progress/refresh.sh` before tackling.

| Tests | Pattern | Likely cause |
|------:|---------|--------------|
|  3 | `WRONG_TYPE_INTEGER` in floatfns-tests.el | bignum numeric edge cases |
|  2 | `WRONG_TYPE_STRING` in casefiddle tests | string-vs-char validation |
|  2 | `ASSERT: symbolp highest` in charset-tests.el | charset priority-list shape |
|  2 | `ASSERT: iso-charset ascii` in charset-tests.el | charset equivalence/declaration model |
|  2 | `ASSERT: keymap make-keymap` in keymap-tests.el | menu-vector table shape |
|  2 | `ASSERT: keymap lookup mixed case` in keymap-tests.el | menu-vector and key normalization |
|  2 | `WRONG_N_ARGS` in keymap-tests.el | keymaps-for-keymap/window argument handling |
|  2 | `ASSERT: marker buffer/window semantics` in marker-tests.el | marker/window-buffer compatibility |

## Runtime stub hits (2026-04-30)

`refresh.sh` records stub/no-op primitive calls per ERT test and ranks them
by failing/erroring tests affected. The last full refresh predates the
latest targeted pass, so rerun a full refresh before treating runtime hit
rankings as authoritative.

The source-derived inventory currently classifies `779` records:
`editing/regions=37`, `window/display=84`, `keymap/help=26`, `other=632`.
By status: `needs-classification=556`, `runtime-missing=218`,
`compat-identity=5`.

Removed from the source inventory in the latest pass:
`describe-buffer-bindings`, `detect-coding-string`, `delete-file-internal`,
`read-string`, `recent-auto-save-p`, `string-as-unibyte`,
`string-to-unibyte`, `string-make-unibyte`, `string-as-multibyte`,
`string-to-multibyte`, `string-make-multibyte`, `terminal-list`,
`terminal-live-p`, and `unibyte-char-to-multibyte`.

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
