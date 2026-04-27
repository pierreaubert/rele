# ERT Session Log

Append-only — newest entries at top. Each session: what changed, what
landed, what to look at next.

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
