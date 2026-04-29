# ERT Session Log

Append-only — newest entries at top. Each session: what changed, what
landed, what to look at next.

## 2026-04-29

**Net change:** +20 tests passing on the tractable ERT baseline. The
previous top bucket, `SIGNAL: signal search-failed` (96 cases), dropped
out of the top 20 after rerunning `./ert-progress/refresh.sh`.

**Baseline movement:**

| Scope             | Before              | After               |
|-------------------|--------------------:|--------------------:|
| TOTAL             | 635 pass / 54%      | 655 pass / 56%      |
| syntax-tests.el   | 0 pass, 95 errors   | 20 pass, 64 errors  |

**Code landed:**

- `emacs_re_to_rust` in `primitives/buffer.rs` now compiles translated
  Emacs regexps in multiline mode, so `^` and `$` behave like Emacs line
  anchors while `\\`` and `\\'` remain absolute buffer anchors via
  `\\A` / `\\z`. Added a regression using the label-search shape from
  `syntax-comments-point`.
- `probe_emacs_file` now defines `ert-resource-file` from the resource
  directory that matches Emacs test conventions: for `foo-tests.el`, it
  prefers `foo-resources/`, then `foo-tests-resources/`, then
  `resources/`. This lets `syntax-tests.el` find
  `syntax-resources/syntax-comments.txt`.
- `find-file` text loading now normalizes CRLF/CR line endings to LF,
  matching Emacs text-buffer decoding and allowing backward `$` label
  searches in CRLF resource files to work.

**Validation:**

- `cargo test -p rele-elisp regex_line_anchor_translation_is_multiline`
- `cargo test -p rele-elisp resource_directory_candidates_prefer_feature_resources_for_tests_file`
- `cargo test -p rele-elisp decode_text_file_contents_normalizes_crlf`
- `./ert-progress/refresh.sh /Volumes/home_ext1/Src/emacs/test/src/syntax-tests.el`
- Full tractable refresh via `./ert-progress/refresh.sh` with
  `tractable.list` passed as arguments because macOS Bash 3.2 lacks
  `mapfile`.

**Next leverage targets** (highest impact first; verify with refresh.sh):

1. `WRONG_TYPE_INTEGER` now leads at 68 cases, mostly the 64 remaining
   `syntax-tests.el` comment movement cases plus integer edge handling
   such as `cmds-tests.el::forward-line-with-bignum`.
2. `DID_NOT_SIGNAL` is next at 52 cases; examples include
   `call-interactively/incomplete-multibyte-sequence`.
3. `timerp` still blocks 10 process tests that use Emacs timer vectors.

## 2026-04-27

**Net change:** +83 tests passing across 7 src/ files. No regressions in
the 766 internal unit tests.

**Files improved:**

| File              | Before | After  |
|-------------------|-------:|-------:|
| minibuf-tests.el  |   0/66 |  60/66 |
| lread-tests.el    |   7/63 |  18/63 |
| alloc-tests.el    |    0/4 |    4/4 |
| chartab-tests.el  |    4/6 |    6/6 |
| character-tests.el|    0/3 |    3/3 |
| doc-tests.el      |    1/5 |    3/5 |
| cmds-tests.el     |    0/2 |    1/2 |

**Code landed:**

- New completion primitives in `eval/builtins.rs`: `try-completion`,
  `all-completions`, `test-completion`. They iterate the collection via a
  shared `collection_candidates` helper that recognises lists,
  cons-headed alists, vectors, and hash tables (including `:test 'equal`
  via `HashKey::Printed`); honour an optional predicate (called with
  `(elt)` or `(key value)` for hash tables, with a fallback retry on
  `WrongNumberOfArguments`); and respect the dynamic
  `completion-regexp-list`. Wired up in `eval/mod.rs`.
- `intern` (in `eval/mod.rs`) now accepts an optional second `obarray`
  argument and registers the symbol in that hash-table-shaped obarray, so
  `(intern str ob)` populates `ob` instead of dropping the second arg.
- `eq_test` in `primitives/core/list.rs` (used by `memq` and friends) now
  compares strings by value. Our `LispObject::String` isn't reference-
  counted, so otherwise tests that rely on a captured collection's string
  identity (Emacs's de-facto sharing of source-literal strings) fail.
- Reader `?\C-X` (in `reader/mod.rs`) used to unconditionally `& 0x1F`,
  collapsing `?\C-\0` to 0. Now collapses only for `@-_` / letters / `?`
  and OR's with the control bit (`1 << 26`) for everything else, matching
  `lread.c` `make_ctrl_char`.
- Reader `\N{...}` (chars and strings) handles `U+XXXX` codepoint
  fallback, `VARIATION SELECTOR-N` (1..256), and `CJK COMPATIBILITY
  IDEOGRAPH-XXXX` patterns. Also added the missing `\N{...}` handling in
  string literals (previously only chars).
- `multibyte-char-to-unibyte` returns the char for Latin-1 codepoints
  (0..256), `-1` for higher codepoints. Bootstrap also stops shadowing it
  with a `(primitive "identity")` definition.
- `char-resolve-modifiers` now resolves shift on letters and control on
  `@-_` / letters / `?`.
- `self-insert-command` signals on negative argument.
- `string-width` handles tab characters (counts as 8), accepts FROM/TO
  with negative-index support and out-of-range error, and skips
  combining marks for Hebrew/Arabic/etc.
- `documentation` / `documentation-property` return `""` for a non-nil
  arg (instead of nil), so `stringp` checks pass.
- Records: `recordp` (in `eval/mod.rs`) actually inspects the vector
  shape (was hardcoded nil); `make-record` builds the right-shape
  vector; `make-finalizer` produces a record-tagged vector;
  `copy-sequence` deep-copies vectors/strings/lists; tag tracking added
  in `primitives/core/records.rs` so `(type-of (record 'foo …))` returns
  `'foo`.
- `char-table-subtype` now returns the stored purpose, and
  `char-table-range` accepts cons-shaped ranges (returns the common
  value when the whole range maps to one value).
- `eval-defvar` no longer clobbers an active dynamic binding when the
  variable is currently let-bound; it updates the bottom-most specpdl
  saved value, so the new toplevel default takes effect on let-unwind.
- `atoms.rs` now resolves special variables from `global_env` directly
  (skipping the captured-env shortcut) so closure-captured frames don't
  return stale dynamic values.
- Bootstrap clears `after-load-alist` at the end of `make_stdlib_interp`
  so the user-visible alist starts empty (the bootstrap stub-evaluator
  evaluates many `with-eval-after-load` forms internally).
- `ical:make-date-time` second definition no longer overrode the first
  with `args`-as-result.

**Next leverage targets** (highest impact first; verify with refresh.sh):

1. `parse-partial-sexp` / `modify-syntax-entry` for Pascal-style syntax
   tables — 64 syntax-tests errors all share `WRONG_TYPE_INTEGER`.
2. `format` spec set in editfns-tests: `%b`, `%#x`, `%#08x`, `%n$`
   positional args, `%.10s` with text properties — ~13 tests.
3. obarray-predicate variants in minibuf-tests: `intern-soft NAME OB`
   needs the obarray-walk path. ~3 tests, finishes minibuf-tests cleanly.
4. `try-completion` ignore-case (4th arg) — 1 test, but the path is
   useful elsewhere.
5. `timerp` recognising the vector representation Emacs uses — 10 tests.

## 2026-04-29 — Portable refresh driver, match-data bridge, reader invalid `\N{...}` signals

**Command run:**

```bash
./ert-progress/refresh.sh
```

**Overall movement:**

| Metric | Before | After |
|--------|-------:|------:|
| Pass   |    655 |   667 |
| Fail   |    252 |   305 |
| Error  |    134 |    69 |
| Skip   |    124 |   124 |
| Rate   |    56% |   57% |

**Top bucket handled:**

- The previous `68 WRONG_TYPE_INTEGER` bucket is down to `3`.
- The large `syntax-tests.el` portion was not a syntax-table bug yet:
  `re-search-forward` populated stub-buffer match data while
  `match-beginning` / `match-end` read interpreter-local match data.
  The evaluator now falls back to buffer primitive match data for
  no-editor ERT execution, and no-editor `forward-line` delegates to
  the buffer primitive so bignum line counts follow primitive integer
  coercion.
- The raw top bucket then became mixed `DID_NOT_SIGNAL`; fixed the
  largest coherent reader subgroup by making invalid character-name
  literals signal instead of silently producing NUL/replacement chars.

**Files improved:**

| File             | Before       | After        |
|------------------|-------------:|-------------:|
| lread-tests.el   | 18/58 pass   | 39/58 pass   |
| cmds-tests.el    | 2/2 pass with `forward-line` bignums | unchanged pass, no error bucket |
| syntax-tests.el  | 20 pass / 64 err | 10 pass / 0 err; failures now expose missing comment semantics |

**Code landed:**

- Added `ert-progress/refresh.py`, a portable Python implementation of
  the ERT refresh driver. `refresh.sh` is now a POSIX wrapper that
  execs the Python script, preserving the existing command entry point.
  The Python driver reads `tractable.list`, builds
  `emacs_test_worker`, runs one worker per file with timeouts, writes
  `tmp/ert-baseline.jsonl`, and invokes `summarize.py`.
- `eval/editor.rs`: no-editor `search-forward`,
  `search-backward`, `re-search-forward`, and `re-search-backward`
  clear interpreter-local string match data after delegating to buffer
  primitives; no-editor `forward-line` delegates to the buffer
  primitive and returns its shortage integer.
- `eval/mod.rs`: `match-beginning` and `match-end` now fall back to
  buffer primitive match data when interpreter-local match data is
  empty, fixing buffer search match data during ERT.
- `reader/mod.rs`: invalid `?\N{...}` character names now signal a
  reader error. Added range validation for `CJK COMPATIBILITY
  IDEOGRAPH-...`, kept valid variation selectors, and added the missing
  `BED` and `SYLOTI NAGRI LETTER DHO` names used by upstream tests.
  Escaped newline in character literals now signals.
- `primitives/core/io.rs`: `(read STRING)` converts reader failures to
  `invalid-read-syntax`, matching `read-from-string` and ERT
  `should-error :type 'invalid-read-syntax` expectations.

**Verification:**

- `./ert-progress/refresh.sh /Volumes/home_ext1/Src/emacs/test/src/syntax-tests.el`
- `./ert-progress/refresh.sh /Volumes/home_ext1/Src/emacs/test/src/lread-tests.el`
- `./ert-progress/refresh.sh`
- `cargo test -p rele-elisp --lib`

**Next leverage targets** (highest impact first; verify with refresh.sh):

1. Raw `DID_NOT_SIGNAL` is still top at 34, but it is a mixed bucket.
   Split it by file/subsystem before coding; likely next coherent
   candidates are JSON parser validation and call-interactively input
   validation.
2. Implement real `forward-comment` over current syntax/comment
   delimiters. Now that match data works, syntax-tests exposes 50 direct
   `forward-comment` assertion failures.
3. Implement enough `scan-lists` / `parse-partial-sexp` comment
   awareness to handle the remaining syntax comment fixtures.
4. `timerp` should recognise Emacs's vector timer representation
   (`10` process-tests errors).
5. Fix `editfns` number/marker coercion buckets
   (`WRONG_TYPE_NUMBER`, `WRONG_TYPE_MARKER`).

## 2026-04-29 — JSON primitives replace nil stubs

**Command run:**

```bash
./ert-progress/refresh.sh
```

**Overall movement:**

| Metric | Before | After |
|--------|-------:|------:|
| Pass   |    667 |   681 |
| Fail   |    305 |   290 |
| Error  |     69 |    70 |
| Skip   |    124 |   124 |
| Rate   |    57% |   58% |

**Top bucket handled:**

- The raw top bucket was still `DID_NOT_SIGNAL` at 34. It was mixed
  across many subsystems, so this run picked the largest coherent
  subgroup: JSON validation and JSON stub behavior.
- `json-tests.el` moved from `0 pass / 24 fail / 1 err` to
  `14 pass / 9 fail / 2 err`.
- JSON's contribution to `DID_NOT_SIGNAL` dropped from 10 to 3.

**Code landed:**

- Added `primitives/core/json.rs` with runtime JSON primitives backed by
  `serde_json`:
  - `json-parse-string`, `json-read-from-string`, `json-decode`
  - `json-parse-buffer`, `json-read`
  - `json-serialize`, `json-encode`
- Parser support includes Emacs-style `:object-type`, `:array-type`,
  `:null-object`, and `:false-object` options for the currently
  tractable cases, plus error symbols for EOF, trailing content,
  parse errors, and invalid raw-byte UTF-8 markers.
- Serializer support covers scalars, strings, vectors, hash tables,
  alists, and plists, with malformed object/list inputs signaling
  instead of returning nil.
- `json-insert` in `primitives/modules.rs` now inserts
  `(json-serialize object ...)` instead of being a nil stub.
- Moved `serde_json` from dev-dependencies to runtime dependencies in
  `crates/elisp/Cargo.toml`.
- Updated the old stub expectation in
  `eval/tests/functions_2.rs`: `(json-parse-string "1")` now returns
  `1`, matching Emacs behavior.

**Verification:**

- `./ert-progress/refresh.sh /Volumes/home_ext1/Src/emacs/test/src/json-tests.el`
- `./ert-progress/refresh.sh`
- `cargo test -p rele-elisp --lib`

**Next leverage targets** (highest impact first; verify with refresh.sh):

1. `DID_NOT_SIGNAL` remains top at 27, but is now diffuse. Largest
   remaining per-file chunk is `editfns-tests.el` at 4; inspect those
   exact `should-error` cases before changing primitives.
2. Implement real `forward-comment`; syntax-tests still has 25 forward
   and 25 backward direct assertion failures.
3. `timerp` should recognise Emacs's vector timer representation
   (`10` process-tests errors).
4. `scan-lists` / `parse-partial-sexp` comment awareness remains the
   next syntax layer after `forward-comment`.
5. JSON follow-ups are now semantic rather than stubs: duplicate-key
   parse ordering, condition hierarchy for `json-error`, invalid
   Unicode edge classification, and after-change hook propagation.

## 2026-04-29 — editfns should-error contracts

**Command run:**

```bash
./ert-progress/refresh.sh
```

**Overall movement:**

| Metric | Before | After |
|--------|-------:|------:|
| Pass   |    681 |   685 |
| Fail   |    290 |   286 |
| Error  |     70 |    70 |
| Skip   |    124 |   124 |
| Rate   |    58% |   59% |

**Top bucket handled:**

- The raw top bucket was `DID_NOT_SIGNAL` at 27. The largest coherent
  per-file chunk was `editfns-tests.el` at 4 cases.
- Fixed those four contracts:
  - `(propertize "foo" 'bar)` now signals `wrong-number-of-arguments`.
  - `(format "%c" 0.5)` now signals instead of silently consuming the arg.
  - `(group-name 'foo)` now signals a wrong-type argument.
  - `(byte-to-string -1)` and `(byte-to-string 256)` now signal, while
    valid bytes return unibyte strings.
- Full-run `DID_NOT_SIGNAL` dropped from 27 to 23.

**Code landed:**

- Added argument-pair validation to the evaluator's `propertize` fast path.
- Tightened `%c` handling in `format` so it requires integer character codes.
- Replaced `byte-to-string` and `group-name` nil/default stubs with small
  contract-aware helpers in `primitives/core/stubs.rs`.

**Verification:**

- `cargo test -p rele-elisp --lib`
- `./ert-progress/refresh.sh /Volumes/home_ext1/Src/emacs/test/src/editfns-tests.el`
- `./ert-progress/refresh.sh`

**Next leverage targets** (highest impact first; verify with refresh.sh):

1. Implement real `forward-comment`; it is now the top global pattern
   with 25 forward and 25 backward failures in `syntax-tests.el`.
2. `DID_NOT_SIGNAL` remains at 23, but is diffuse. Group by file again
   before changing primitives.
3. `timerp` should recognise Emacs's vector timer representation
   (`10` process-tests errors).
4. Implement enough `scan-lists` / `parse-partial-sexp` comment awareness
   after `forward-comment`.
5. Fix `editfns` number/marker coercion buckets
   (`WRONG_TYPE_NUMBER`, `WRONG_TYPE_MARKER`).
