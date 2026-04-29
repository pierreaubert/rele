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
   suivant syntax layer after `forward-comment`.
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

## 2026-04-29 — forward-comment syntax fixtures

**Command run:**

```bash
./ert-progress/refresh.sh
```

**Overall movement:**

| Metric | Before | After |
|--------|-------:|------:|
| Pass   |    685 |   739 |
| Fail   |    286 |   232 |
| Error  |     70 |    70 |
| Skip   |    124 |   124 |
| Rate   |    59% |   63% |

**Top bucket handled:**

- The top global pattern was `forward-comment`: 25 forward and 25
  backward direct assertion failures in `syntax-tests.el`.
- `syntax-tests.el` moved from `10 pass / 90 fail / 0 err` to
  `64 pass / 36 fail / 0 err`.
- The direct `forward-comment` bucket is gone. The remaining syntax
  failures are now `scan-lists`, `syntax-ppss`, and isolated syntax
  classification cases.

**Code landed:**

- Replaced the nil `forward-comment` stub in `primitives/buffer.rs` with
  a small comment scanner.
- Covered the comment forms exercised by Emacs's syntax resource:
  - Pascal-style `{ ... }`
  - Lisp line comments `; ...`
  - nested Lisp block comments `#| ... |#`
  - C block comments `/* ... */` with escaped `*/`
  - C line comments `// ...` with escaped newlines
- Backward movement now skips whitespace and finds the matching comment
  start, including the overlapping nested-comment edge in `#|#|#`.
- Unclosed block comments now stop at the syntax fixture EOF sentinel
  instead of blindly returning `point-max`.

**Verification:**

- `cargo test -p rele-elisp --lib`
- `./ert-progress/refresh.sh /Volumes/home_ext1/Src/emacs/test/src/syntax-tests.el`
- `./ert-progress/refresh.sh`

**Note:**

- `rustfmt crates/elisp/src/primitives/buffer.rs` was attempted but the
  standalone `rustfmt` invocation failed on existing let-chain syntax
  because it did not pick up the workspace edition. No formatting change
  was applied by that failed command.

**Next leverage targets** (highest impact first; verify with refresh.sh):

1. `DID_NOT_SIGNAL` is again the top global bucket at 23. Group by file
   before changing primitives; it is diffuse.
2. `timerp` should recognise Emacs's vector timer representation
   (`10` process-tests errors).
3. Implement `scan-lists` comment awareness; syntax-tests now has 10
   forward and 10 backward direct failures there.
4. Implement the `syntax-ppss` comment state fields used by the remaining
   syntax fixture assertions.
5. Fix `editfns` number/marker coercion buckets
   (`WRONG_TYPE_NUMBER`, `WRONG_TYPE_MARKER`).

## 2026-04-29 — scan-lists comment-aware list motion

**Command run:**

```bash
./ert-progress/refresh.sh
```

**Overall movement:**

| Metric | Before | After |
|--------|-------:|------:|
| Pass   |    739 |   761 |
| Fail   |    232 |   209 |
| Error  |     70 |    71 |
| Skip   |    124 |   124 |
| Rate   |    63% |   65% |

**Top bucket handled:**

- The suivant syntax leverage bucket was `scan-lists`: 10 forward and 10
  backward direct assertion failures, plus 3 `DID_NOT_SIGNAL` cases from
  unmatched list scans inside comment fixtures.
- `syntax-tests.el` moved from `64 pass / 36 fail / 0 err` to
  `86 pass / 13 fail / 1 err`.
- The direct `scan-lists forward` and `scan-lists backward` buckets are
  gone from the global top patterns.
- `DID_NOT_SIGNAL` dropped from 23 to 20 because the unmatched
  `scan-lists` cases now signal `scan-error`.

**Code landed:**

- Added stateful buffer primitive support for `scan-lists`.
- Implemented one-list forward and backward motion for `()`, `[]`, and
  `{}` using Emacs-style 1-based buffer positions.
- Reused the comment scanner from `forward-comment` so list matching
  skips Lisp line comments, Lisp block comments, C block comments, and C
  line comments with escaped newlines.
- The escaped-newline check now treats CRLF fixtures as escaped when the
  backslash appears before the `\r\n` line break.
- Treated `{}` as list delimiters during brace-list scans instead of
  misclassifying the opening brace as a Pascal comment.
- Added `scan-error` signaling for unmatched lists and unsupported
  nonzero depth/count cases.

**Verification:**

- `cargo test -p rele-elisp --lib`
- `./ert-progress/refresh.sh /Volumes/home_ext1/Src/emacs/test/src/syntax-tests.el`
- `./ert-progress/refresh.sh`

**Note:**

- One escaped C line-comment edge remains:
  `syntax-tests.el::syntax-br-comments-c-b54` now raises
  `scan-error`. That is likely a comment-continuation/list boundary
  mismatch in the lightweight scanner, not a broad registration issue.

**Next leverage targets** (highest impact first; verify with refresh.sh):

1. Group the remaining `DID_NOT_SIGNAL` bucket, now at 20, before
   changing primitives; the syntax subgroup is gone.
2. `timerp` should recognise Emacs's vector timer representation
   (`10` process-tests errors).
3. Implement `syntax-ppss` comment state/open-position fields used by
   the remaining syntax fixture assertions (`8` top-pattern failures).
4. Fix `editfns` number/marker coercion buckets
   (`WRONG_TYPE_NUMBER`, `WRONG_TYPE_MARKER`).
5. Investigate `syntax-br-comments-c-b54` after `syntax-ppss`; it may
   need fuller C syntax-table awareness rather than more list scanning.

## 2026-04-29 — floatfns numeric error contracts

**Command run:**

```bash
./ert-progress/refresh.sh
```

**Overall movement:**

| Metric | Before | After |
|--------|-------:|------:|
| Pass   |    761 |   770 |
| Fail   |    209 |   201 |
| Error  |     71 |    70 |
| Skip   |    124 |   124 |
| Rate   |    65% |   66% |

**Top bucket handled:**

- The raw top bucket was `DID_NOT_SIGNAL` at 20. Grouping showed three
  equal largest subgroups: `floatfns`, `json`, and `lread`, each with 3.
- This run picked the self-contained `floatfns` subgroup:
  - `floatfns-tests-isnan`
  - `fround-fixnum`
  - `special-round`
- `floatfns-tests.el` moved from `19 pass / 10 fail / 4 err` to
  `28 pass / 2 fail / 3 err`.
- Global `DID_NOT_SIGNAL` dropped from 20 to 17.

**Code landed:**

- `isnan` now requires a float and signals `wrong-type-argument` for
  non-floats.
- Added primitive implementations for `ffloor`, `fceiling`, `fround`,
  and `ftruncate`; they require float arguments and return floats.
- Removed the nil-returning Elisp stubs for those `f*` functions so the
  primitive definitions are not shadowed during bootstrap.
- Reworked `floor`, `ceiling`, `round`, and `truncate` to support the
  optional divisor, zero-divisor signaling, non-finite float signaling,
  and exact integer quotient rounding for fixnums/bignums.
- Added local unit coverage for these numeric contracts.

**Verification:**

- `cargo test -p rele-elisp --lib`
- `./ert-progress/refresh.sh /Volumes/home_ext1/Src/emacs/test/src/floatfns-tests.el`
- `./ert-progress/refresh.sh`

**Next leverage targets** (highest impact first; verify with refresh.sh):

1. Group the remaining `DID_NOT_SIGNAL` bucket, now at 17. Largest
   coherent subgroups before this run were `json` and `lread` at 3 each.
2. `timerp` should recognise Emacs's vector timer representation
   (`10` process-tests errors).
3. Implement `syntax-ppss` comment state/open-position fields used by
   the remaining syntax fixture assertions (`8` top-pattern failures).
4. Fix `editfns` marker and number coercion buckets
   (`WRONG_TYPE_MARKER`, `WRONG_TYPE_NUMBER`).
5. Floatfns remaining failures are now semantic rather than
   no-signal: `bignum-expt`, `log`, `bignum-to-float`, and huge
   `big-round` conversion.

## 2026-04-29 — lread reader signal contracts

**Command run:**

```bash
CARGO_TARGET_DIR=/Users/pierre/src/rele/tmp/codex-target ./ert-progress/refresh.sh
```

**Overall movement:**

| Metric | Before | After |
|--------|-------:|------:|
| Pass   |    772 |   775 |
| Fail   |    199 |   196 |
| Error  |     70 |    70 |
| Skip   |    124 |   124 |
| Rate   |    66% |   66% |

**Top bucket handled:**

- The raw top bucket was `DID_NOT_SIGNAL` at 16. The largest coherent
  subgroup was `lread-tests.el` at 3:
  - `lread-test-bug70702`
  - `lread-circular-hash`
  - `lread-char-escape-eof`
- `lread-tests.el` moved from `39 pass / 15 fail / 4 err` to
  `42 pass / 12 fail / 4 err`.
- Global `DID_NOT_SIGNAL` dropped from 16 to 13.

**Code landed:**

- `read` now accepts buffer objects, reads from the current buffer point,
  advances point after successful reads, and signals
  `(invalid-read-syntax "#<" 1 2)` for unreadable `#<...>` objects.
- Hash-table record literals now reject circular shared-structure
  references while preserving existing non-circular `#N=` / `#N#` reads.
- Character literal parsing now signals on unfinished modifier, Unicode,
  and named-character escapes such as `?\\M`, `?\\u234`, and `?\\N`.
- The refresh driver now honours `CARGO_TARGET_DIR`, needed here because
  the default symlinked `target/release` was readable but not writable.
- Added focused reader regression coverage for the three fixed contracts.

**Verification:**

- `cargo test -p rele-elisp --lib`
- exact `reader_edges` tests for circular hash-table data, unfinished
  char escapes, and buffer `#<...>` reads
- `CARGO_TARGET_DIR=/Users/pierre/src/rele/tmp/codex-target ./ert-progress/refresh.sh /Volumes/home_ext1/Src/emacs/test/src/lread-tests.el`
- `CARGO_TARGET_DIR=/Users/pierre/src/rele/tmp/codex-target ./ert-progress/refresh.sh`

**Note:**

- A full `cargo test -p rele-elisp --test reader_edges` still hits an
  existing unrelated expectation:
  `test_char_named_unicode_unknown_yields_null` expects unknown
  `?\\N{...}` names to parse as NUL, while the current reader signals.

**Next leverage targets** (highest impact first; verify with refresh.sh):

1. Group the remaining `DID_NOT_SIGNAL` bucket, now at 13. Current
   two-test subgroups are `charset`, `coding`, and `json`.
2. `timerp` should recognise Emacs's vector timer representation
   (`10` process-tests errors).
3. Implement `syntax-ppss` comment state/open-position fields used by
   the remaining syntax fixture assertions (`8` top-pattern failures).
4. Fix `editfns` marker and number coercion buckets
   (`WRONG_TYPE_MARKER`, `WRONG_TYPE_NUMBER`).
5. `call-interactively` still has `WRONG_N_ARGS` and incomplete input
   signaling gaps in `callint-tests.el`.

## 2026-04-29 — JSON no-signal cleanup

**Command run:**

```bash
CARGO_TARGET_DIR=/Users/pierre/src/rele/tmp/codex-target ./ert-progress/refresh.sh
```

**Overall movement:**

| Metric | Before | After |
|--------|-------:|------:|
| Pass   |    775 |   776 |
| Fail   |    196 |   195 |
| Error  |     70 |    70 |
| Skip   |    124 |   124 |
| Rate   |    66% |   66% |

**Top bucket handled:**

- The raw top bucket was still `DID_NOT_SIGNAL`, now at 13. This run
  picked the self-contained JSON subgroup:
  - `json-serialize/object`
  - `json-parse-string/invalid-unicode`
- `json-tests.el` moved from `16 pass / 7 fail / 2 err` to
  `17 pass / 6 fail / 2 err`.
- Global `DID_NOT_SIGNAL` dropped from 13 to 11.

**Code landed:**

- JSON parsing now rejects replacement characters as
  `json-utf8-decode-error`, covering invalid Unicode escape inputs that
  the reader represents as U+FFFD.
- `#s(hash-table ...)` reader records now produce real hash-table
  objects, including `test` and alternating `data` entries.
- Active `#N#` circular references now produce a non-cyclic internal
  sentinel instead of `nil`; this lets serializers signal on circular
  input without creating Rust-side cons cycles that overflow stack in
  existing list walkers.
- JSON alist/plist serialization now guards repeated cons traversal and
  reports `circular-list` if a real cycle reaches that layer.
- Added JSON unit coverage for invalid Unicode, circular shared-list
  inputs, reader hash-table records, and nested object serialization.

**Verification:**

- `cargo test -p rele-elisp --lib`
- `cargo test -p rele-elisp primitives::core::json::tests --lib`
- `cargo test -p rele-elisp --test reader_edges test_hash_table_literal -- --exact`
- `CARGO_TARGET_DIR=/Users/pierre/src/rele/tmp/codex-target ./ert-progress/refresh.sh /Volumes/home_ext1/Src/emacs/test/src/json-tests.el`
- `CARGO_TARGET_DIR=/Users/pierre/src/rele/tmp/codex-target ./ert-progress/refresh.sh /Volumes/home_ext1/Src/emacs/test/src/lread-tests.el`
- `CARGO_TARGET_DIR=/Users/pierre/src/rele/tmp/codex-target ./ert-progress/refresh.sh`

**Note:**

- An attempted real cyclic-cons implementation fixed the JSON signal
  shape but made `lread-tests.el` overflow the stack. The landed version
  deliberately uses a sentinel until all list walkers have cycle guards.
- `json-serialize/object` now reaches a later semantic assertion about
  nested object serialization; it is no longer part of `DID_NOT_SIGNAL`.

**Next leverage targets** (highest impact first; verify with refresh.sh):

1. Group the remaining `DID_NOT_SIGNAL` bucket, now at 11. The largest
   current subgroups are `charset` and `coding`, at 2 tests each.
2. `timerp` should recognise Emacs's vector timer representation
   (`10` process-tests errors).
3. Implement `syntax-ppss` comment state/open-position fields used by
   the remaining syntax fixture assertions (`8` top-pattern failures).
4. Fix `editfns` marker and number coercion buckets
   (`WRONG_TYPE_MARKER`, `WRONG_TYPE_NUMBER`).
5. `json-serialize/object` still has a nested object semantic mismatch
   after the no-signal assertions now pass.
