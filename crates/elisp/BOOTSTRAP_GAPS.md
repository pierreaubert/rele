# Bootstrap Gap Matrix

Snapshot from `load_audit` against Emacs 30.2 stdlib.
Bootstrap-chain snapshot: **105 / 105** loadup files now load at 100%.
`oclosure.el`, `nadvice.el`, `cl-generic.el`, `international/characters.el`,
`international/cp51932.el`, `international/eucjp-ms.el`, and the remaining
previously-partial loadup files now load completely.

## Summary by visible error class

| Seen | Error class | Root cause |
|-----:|-------------|------------|
| 0 | none | Loadup bootstrap is green |

## Per-file detail

| File | OK/Total | Pct | Blocking errors |
|------|----------|-----|-----------------|
| all loadup files | 100% | 100% | none |

## Secondary libraries (via `require_audit`)

**891 / 891** forms pass (100.0%) across cl-lib, cl-macs, cl-extra,
cl-seq, cl-print, subr-x, pcase, gv, ert.

| File | OK/Total | Pct | Errors |
|------|----------|-----|--------|
| cl-lib | 96/96 | 100% | |
| cl-extra | 69/69 | 100% | |
| cl-seq | 81/81 | 100% | |
| cl-print | 43/43 | 100% | |
| subr-x | 27/27 | 100% | |
| gv | 160/160 | 100% | |
| cl-macs | 181/181 | 100% | |
| pcase | 52/52 | 100% | |
| ert | 182/182 | 100% | |

## Highest-impact fix targets

### Recently fixed on this branch

- `function-put` / `function-get` are now interpreter-local function-property
  primitives.
- `provide`, `featurep`, and `require` now work through function-cell dispatch
  for bytecode callers.
- `def-edebug-elem-spec` records Edebug metadata on symbol plists.
- `defvar-1` handles byte-compiled top-level variable declarations.
- `add-minor-mode`, `make-composed-keymap`, `easy-menu-do-define`, and tool-bar
  helpers no longer block headless stdlib loading.
- `cl-generic-define` and `cl-generic-define-method` now exist on the
  function-cell path, removing the `seq.elc` void-function failures.
- `function-get` now matches Emacs by returning nil for non-symbol function
  names instead of raising `wrong-type-argument`.
- Stdlib staging now lives under repo `tmp/elisp-stdlib`, and audit binaries
  use the shared `eval::bootstrap` staging helpers instead of duplicating the
  copy/gunzip logic.
- Source-level `cl-defstruct` now honors `(:predicate nil)`, custom predicate
  names, and custom constructor arglists whose positional order differs from
  field storage order.
- `cl-defstruct` class metadata now stores slot-descriptor records instead of
  placeholder nil slots, and preserves pre-registered child tags during the
  `cl-structure-class` / `cl-structure-object` circular bootstrap.
- Bytecode-level `symbol-value`, `default-value`, `boundp`, and
  `default-boundp` now see interpreter value cells as well as environment
  bindings.
- `add-to-list` / `add-to-ordered-list` now mutate symbol values, and `set`
  mirrors value-cell writes into the root environment so bootstrap `defvar`
  bindings do not shadow later mutation.
- `put`, `get`, `boundp`, `fboundp`, and symbol-plist access now treat
  self-evaluating `t` / `nil` as symbols at the primitive boundary.
- `cl-preloaded.el` now loads completely (`94/94`), closing the circular
  `cl-structure-class` / `cl--class` bootstrap gap.
- `cl-defstruct (:include ...)` now inherits parent slot metadata, custom
  constructors support `&aux`, and `closurep` recognizes lexical closures.
- `oclosure-define` now registers enough class metadata, predicates, slot
  descriptors, and index tables for `oclosure.el` (`34/34`) and
  `nadvice.el` (`42/42`) to load completely.
- `cl-generic.el` now loads completely (`103/103`): the bootstrap path keeps a
  Rust-backed `cl-generic-generalizers` dispatcher for `head` / `eql`
  specializers and short-circuits `with-memoization` before GV expands through
  temporary `getter` / `setter` helpers.
- Sparse char-table storage now supports high-codepoint `aref` / `aset`,
  range writes, parent links, and extra slots. Runtime standard case/syntax/
  category tables are persistent, which brings `international/characters.el`
  to `250/250`.
- Generated translation-table `let` forms are now short-circuited during
  bootstrap when they only feed the stubbed `define-translation-table`, bringing
  `international/cp51932.el` and `international/eucjp-ms.el` to `2/2`.
- The remaining loadup blockers are closed: keymap validation accepts staged
  key definitions, full keymaps expose a char-table slot, compile-time-only
  require/toggle/custom metadata forms no-op safely, utf-8-emacs composition
  table writes no longer trip on unsupported raw character literals, and
  generated translation/coding metadata avoids expensive bytecode paths.
- The secondary require audit is now green: `cl-macs`, `pcase`, and `ert`
  finish all top-level forms after `pcase-defmacro`, `pcase-dolist`, and
  destructuring `cl-loop` support stopped treating pattern syntax as ordinary
  value evaluation.

1. **Keep loadup green while replacing stubs with semantics**
   - The bootstrap chain now loads all 105 files. Next work should preserve this
     as a regression gate while replacing metadata no-ops with real editor and
     runtime behavior where user-facing features depend on them.

2. **Keymap and symbol semantics**
   - The load path now accepts keymap definitions, but interactive dispatch
     still needs real key parsing, lookup, inheritance, remapping, and command
     integration before the editor can rely on these maps.

3. **Char-table array fidelity** (unlocks character metadata forms)
   - Done for loadup: `international/characters` now passes. Keep expanding the
     model only when code depends on exact `map-char-table` enumeration.
