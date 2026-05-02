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

Last refreshed: **2026-05-02** (post keymap / sqlite / textprop cleanup), target: `ert-progress/tractable.list`.
Current total: **930 pass / 104 fail / 15 err / 116 skip** (`80%`).

| File                       | Pass | Fail | Err | Skip | Pct  | Notes |
|----------------------------|-----:|-----:|----:|-----:|-----:|-------|
| alloc-tests.el             |    4 |    0 |   0 |    0 | 100% | |
| buffer-tests.el            |  408 |    1 |   0 |    1 | 100% | |
| callint-tests.el           |    4 |    0 |   0 |    0 | 100% | call-interactively complete |
| casefiddle-tests.el        |    3 |    7 |   0 |    1 |  27% | char-valued casing fixed; case tables remain |
| category-tests.el          |    4 |    2 |   0 |    0 |  67% | lightweight category tables |
| character-tests.el         |    3 |    0 |   0 |    0 | 100% | |
| charset-tests.el           |   20 |    1 |   0 |    0 |  95% | priority/ISO/sort fixed; map-charset-chars remains |
| chartab-tests.el           |    6 |    0 |   0 |    0 | 100% | |
| cmds-tests.el              |    2 |    0 |   0 |    0 | 100% | |
| coding-tests.el            |   14 |   13 |   0 |    1 |  50% | coding systems |
| data-tests.el              |   74 |    3 |   0 |    2 |  94% | format edge cases |
| decompress-tests.el        |    0 |    0 |   0 |    1 |   0% | needs zlib |
| doc-tests.el               |    2 |    3 |   0 |    0 |  40% | documentation semantics need follow-up |
| editfns-tests.el           |   37 |   17 |   3 |    0 |  65% | text-property pass landed |
| eval-tests.el              |    ? |    ? |   ? |    ? |    ? | no results emitted in last sweep |
| floatfns-tests.el          |   30 |    2 |   1 |    0 |  91% | bignum expt/mod fixed; float precision/huge-round remain |
| font-tests.el              |    2 |    0 |   0 |    0 | 100% | headless font parsing complete |
| image-tests.el             |    3 |    0 |   0 |    2 |  60% | |
| indent-tests.el            |    0 |    3 |   0 |    0 |   0% | |
| inotify-tests.el           |    0 |    0 |   0 |    3 |   0% | needs inotify |
| json-tests.el              |   17 |    6 |   2 |    0 |  68% | JSON encode/decode |
| keyboard-tests.el          |    1 |    2 |   0 |    0 |  33% | |
| keymap-tests.el            |   29 |   18 |   0 |    0 |  62% | prompt + menu case normalization fixed |
| lcms-tests.el              |    0 |    0 |   0 |    6 |   0% | needs lcms |
| lread-tests.el             |   42 |   12 |   4 |    0 |  72% | reader edge cases |
| marker-tests.el            |   12 |    0 |   0 |    0 | 100% | detached/copy/window marker semantics complete |
| minibuf-tests.el           |   62 |    4 |   0 |    0 |  94% | obarray-predicate, ignore-case |
| process-tests.el           |   12 |    0 |   0 |   27 |  31% | supportable headless cases pass |
| profiler-tests.el          |    0 |    0 |   1 |    1 |   0% | |
| search-tests.el            |    0 |    1 |   0 |    0 |   0% | |
| sqlite-tests.el            |   10 |    1 |   1 |    0 |  83% | BLOB/unibyte round trips fixed; set/load-extension remain |
| syntax-tests.el            |   98 |    0 |   2 |    0 |  98% | char-syntax edge cases |
| terminal-tests.el          |    1 |    0 |   0 |    0 | 100% | |
| textprop-tests.el          |    2 |    1 |   0 |    0 |  67% | font-lock face-removal path fixed |
| thread-tests.el            |    0 |    0 |   1 |   36 |   0% | needs threads |
| treesit-tests.el           |    1 |    2 |   0 |   35 |   3% | needs tree-sitter |
| undo-tests.el              |   16 |    2 |   0 |    0 |  89% | undo replay drift |
| xdisp-tests.el             |    9 |    1 |   0 |    0 |  90% | bidi/display-property paths improved |
| xfaces-tests.el            |    2 |    1 |   0 |    0 |  67% | faces |
| xml-tests.el               |    0 |    1 |   0 |    0 |   0% | needs libxml |

## Top leverage targets (2026-05-02)

These are the failure patterns ranked by impact. A single fix at any of
these unblocks the listed count of tests at once. Verify the count is still
current by running `./ert-progress/refresh.sh` before tackling.

| Tests | Pattern | Likely cause |
|------:|---------|--------------|
|  2 | `SIGNAL: json-parse-error` in json-tests.el | JSON serialize/parse scalar round-trip |
|  2 | `ASSERT: keymap help describe-vector` in keymap-tests.el | shadow range description formatting |
|  1 | `overlay-modification-hooks` in buffer-tests.el | overlay change hook ordering |
|  1 | `casefiddle-tests-char-properties` and related case-table asserts | case-table property model |
|  1 | `charset-tests--map-charset-chars` | map-charset-chars range enumeration |

## Runtime stub hits (2026-05-02)

`refresh.sh` records stub/no-op primitive calls per ERT test and ranks them
by failing/erroring tests affected. The source-derived inventory currently
classifies `414` records (down from `778` before this session) — the
catch-all `other` bucket dropped from `632` to `~228`, and 14 dedicated
modules under `crates/elisp/src/primitives/core/` now host
markers / text-props / words / sexp / json / base64 / coding / abbrevs /
obarrays / selections / faces / terminal / processes / threads / timers /
sqlite / minibuf / misc-system / key-descriptions.

Top runtime stub hits from the latest full refresh:

| Bad tests | Hits | Bucket | Stub | Example |
|----------:|-----:|--------|------|---------|
| 1 | 6 | other | `text-char-description` | `keymap-tests.el::keymap-text-char-description` |
| 1 | 2 | other | `lossage-size` | `keyboard-tests.el::keyboard-lossage-size` |
| 1 | 1 | other | `get-load-suffixes` | `lread-tests.el::lread-tests--get-load-suffixes` |
| 1 | 1 | unknown | `find-operation-coding-system->ignore` | `coding-tests.el::coding-tests--find-operation-coding-system` |

The `text-char-description` hit is interesting: the source-derived
inventory thinks it's still a stub, but the primitive itself was
upgraded to a real impl in this session — re-check whether the test
hits the bytecode VM path which still lacks the C-equivalent
formatting.

Removed from the source inventory in the latest stub pass:
`describe-buffer-bindings`, `detect-coding-string`, `delete-file-internal`,
`frame-or-buffer-changed-p`,
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
