# Rust Emacs Roadmap: `rele-elisp`

This file tracks the execution plan for building the Emacs Lisp core of our
Rust Emacs clone. The editor can only feel Emacs-like once this crate can load
and run real Emacs Lisp reliably, so the roadmap is intentionally staged:

1. Make the interpreter correct, isolated, and useful with real stdlib code.
2. Build enough editor-facing primitives for packages and commands to run.
3. Add JIT speedups only after the interpreter path is stable.

## Success Criteria

Current status snapshot:
- Default `rele-elisp` tests are green: `475 passed`, `3 ignored`.
- Default and JIT-feature `rele-elisp` checks are warning-clean:
  `cargo check -p rele-elisp` and
  `cargo check -p rele-elisp --features jit`.
- JIT build visibility is green: `cargo test -p rele-elisp --features jit --no-run`.
- JIT runtime tests are green: `cargo test -p rele-elisp --features jit --lib`
  (`504 passed`, `3 ignored`).
- Loadup bootstrap is green: `106 / 106` files, `8039 / 8039` forms.
- Secondary require audit is green: `891 / 891` forms across `cl-lib`,
  `cl-macs`, `cl-extra`, `cl-seq`, `cl-print`, `subr-x`, `pcase`, `gv`, and
  `ert`.
- The pre-JIT baseline gate is scripted in
  `scripts/pre-jit-baseline.sh`: compile-only, default tests, JIT no-run,
  load audit, and require audit.
- JIT coverage is now measurable against real Emacs `.elc` bytecode:
  `cargo run -p rele-elisp --bin jit_audit` currently scans `638` bytecode
  literals after bootstrap, with `24` fully JIT-supported and a ranked
  unsupported-opcode histogram for the next expansion loop.
- JIT hot-path benches include synthetic VM/JIT pairs and an optional real
  `.elc` zero-arg bytecode sample:
  `cargo bench -p rele-elisp --features jit --bench jit_hotpath`.

Interpreter-ready means:
- `cargo test -p rele-elisp`
- `cargo test -p rele-elisp --lib`
- `cargo test -p rele-elisp --features jit --no-run`
- Bootstrap helpers can reliably load the core stdlib stack:
  `subr`, `cl-lib`, `macroexp`, `pcase`, `ert`
- ERT smoke tests are deterministic and isolated across interpreter instances
- Core command/package patterns work through the `EditorCallbacks` boundary

JIT-ready means:
- Tiered execution preserves interpreter semantics on redefinition, deopt,
  unwind, throw, dynamic binding, and optional/rest arg handling.
- JIT counters reflect real compiler state, invalidations, and deopts.
- Benchmarks show wins on real bytecode workloads, not just synthetic ops.

## Milestone 1: Green Interpreter Baseline

- [x] Keep the default crate test build green.
- [x] Keep `--features jit --no-run` green so JIT regressions stay visible.
- [x] Make targeted failures easy to reproduce with one-command tests.
- [x] Separate compile failures from runtime/bootstrap failures in CI/local
      gating via `scripts/pre-jit-baseline.sh`.

Exit condition:
- The crate builds and bootstrap/require failures are tracked as behavior gaps,
  not compile blockers.

## Milestone 2: Runtime Isolation

- [x] Audit remaining process-global interpreter state beyond the obarray.
- [x] Ensure ERT registrations do not leak between interpreters.
- [x] Move current keymap state off thread-local storage and into
      interpreter-local value cells.
- [x] Ensure current buffer / match data / feature state are isolated enough
      for parallel test execution.
- [x] Move source-level match data and EIEIO class registration into
      interpreter-owned state.
- [x] Remove ad hoc cleanup from tests where runtime-owned reset/isolation is
      the right fix.

Exit condition:
- ERT and bootstrap tests behave deterministically under `cargo test`.

## Milestone 3: Bootstrap as Runtime Capability

- [x] Keep bootstrap helpers in reusable runtime code, not test-only code.
- [x] Move stdlib staging to repo `tmp/` to match repo policy.
- [x] Expose a clear bootstrap API used by tests, audit tools, and future app
      integration.
- [x] Add a small “bootstrap health” suite that exercises the core load chain.
- [x] Keep `load_audit` green for the Emacs 30.2 loadup chain.
- [x] Keep `require_audit` green for the first secondary library ring.

Exit condition:
- The same bootstrap code path is shared by tests, audits, and future clients,
  and it is good enough to use as a regression gate.

## Milestone 4: Interpreter Compatibility Gaps

Prioritize missing semantics that unlock real Emacs libraries instead of adding
more one-off stubs.

Current high-value gap buckets:
- [x] `function-put` / `function-get`
- [x] `require` / `provide` / `featurep` through function-cell dispatch
- [x] Basic `after-load-alist`, `eval-after-load`, and
      `with-eval-after-load` behavior for loaded file hooks
- [ ] `cl-struct-define` (partial: custom constructors, predicates, tags, and
      metadata slot records now work; `cl--class-p` inheritance remains)
- [x] `cl-generic-define` / `cl-generic-define-method` bytecode entrypoints
- [x] `cl-generic` bootstrap generalizers, including `(head ...)` and
      `(eql ...)` specializers
- [ ] Full `cl-generic` method combination and dispatch fidelity
- [x] `def-edebug-elem-spec`
- [x] `defvar-1`
- [x] `add-minor-mode`
- [x] `make-composed-keymap`
- [x] `easy-menu-do-define`
- [x] `tool-bar-local-item`
- [x] Sparse char-table storage for high-codepoint `aref` / `aset`, range
      writes, parent links, and extra slots
- [x] Wrong-type regressions hit during `cl-*`, `oclosure`, and character data
      no longer block loadup or the first require audit ring.
- [x] Replace load-time metadata no-ops with real behavior where editor
      features depend on them:
      keymaps, coding systems, customization metadata, and advice.
- [x] Keymaps have a usable runtime representation for staged load paths:
      mutable bindings, parent fallback, composed maps, global/local maps,
      lookup, and `defvar-keymap` / `define-keymap` raw-form handling.
- [x] Finish keymap fidelity beyond load/runtime storage:
      canonical `kbd` / `key-parse`, remapping, prefix maps, menu entries, and
      `key-binding` / `where-is-internal` query behavior. Editor command
      dispatch integration now belongs to the server/app command bridge rather
      than the standalone elisp crate.
- [x] Turn the short-circuited generated table paths into explicit lazy data
      structures instead of evaluator pattern skips.

Exit condition:
- Core stdlib and the first secondary library ring load cleanly, and the
  remaining unsupported areas are explicit semantic-debt items rather than
  bootstrap blockers.

## Milestone 5: Semantic Parity Hardening

- [x] Add interpreter/VM parity tests for:
  - [x] redefinition and invalidation-sensitive call paths
  - [x] dynamic vs lexical binding
  - [x] unwind-protect / catch / throw
  - [x] optional and rest argument normalization
  - [x] macro expansion after stdlib bootstrap
  - [x] `load` / `require` / `provide`
- [x] Add a few end-to-end tests around real loaded stdlib functions.
- [x] Keep `load_audit` and `require_audit` as required gates while replacing
      stubs with semantics.
- [x] Add focused tests for the current bootstrap shortcuts before removing
      them, so semantic replacements do not accidentally widen behavior.

Current phase 5 gate:
- `tests/semantic_parity_phase5.rs` covers lambda optional/rest binding,
  `unwind-protect` cleanup through `throw`, top-level bytecode VM execution,
  bytecode-visible function redefinition, dynamic vs lexical binding,
  interpreter-local feature/match/EIEIO state, thread-worker-local buffer
  state, focused bootstrap shortcut shapes, real-file `load` / `require` /
  `provide` plus after-load hooks, Emacs-shaped macro expansion surviving full
  bootstrap, and a real bootstrapped `seq` function.

Exit condition:
- The interpreter is boring: predictable, covered, and easy to debug.

## Milestone 6: JIT Safety

- [x] Use version-checked compiled lookup on the hot path.
- [x] Track actual compiled entry count instead of inferred hotness.
- [x] Track invalidation and deopt counters in `JitStats`.
- [x] Use stable named-function JIT identities based on symbol IDs rather
      than cloned bytecode object addresses.
- [x] Add tests for:
  - [x] redefinition invalidation
  - [x] deopt fallback
  - [x] eager compile vs hot compile parity
  - [x] tier transitions over the same function

Exit condition:
- The JIT never keeps stale code running and always falls back safely.

## Milestone 7: JIT Coverage and Performance

- [x] Add a measured JIT/VM bytecode hot-path benchmark:
      `cargo bench -p rele-elisp --features jit --bench jit_hotpath`.
- [x] Keep fallback exact for unsupported cases and type-guard deopts.
- [x] Profile real bytecode workloads after bootstrap with `jit_audit`, including
      installed function cells plus bytecode literals parsed from real Emacs
      `.elc` files.
- [x] Expand opcode coverage based on measured hot functions:
      `stack-ref1` was the top unsupported opcode in the real `.elc` histogram,
      and `constant2` now covers large-constant bytecode functions.
- [x] Add before/after benches for each meaningful hot-path JIT expansion:
      `jit_hotpath` now includes add, `constant2`, `stack-ref1`, and an optional
      real `.elc` zero-arg bytecode sample.

Exit condition:
- The first audit-driven JIT coverage loop is complete. Quick Criterion numbers
  show wins for add, `stack-ref1`, and the real `.elc` zero-arg sample; the
  `constant2` pair is currently neutral/slightly slower through the eval
  boundary, which is tracked as data for future tiering work rather than hidden.

## Immediate Next Queue

These are the next tasks to pick up in order:

1. [x] Get the targeted interpreter tests stable:
   `test_cl_files_load_progress`, `test_emacs_ert_can_run_a_test`,
   `test_ert_run_per_test_timeout`.
2. [x] Implement `function-put` / `function-get` as interpreter-local function
   property primitives.
3. [x] Route `provide`, `featurep`, and `require` through function-cell
   dispatch so bytecode can call them.
4. [x] Add load-time metadata/helper primitives:
   `def-edebug-elem-spec`, `defvar-1`, `add-minor-mode`,
   `make-composed-keymap`, `easy-menu-do-define`, and tool-bar helpers.
5. [x] Reduce remaining ERT leakage by replacing test-local cleanup with
   interpreter/runtime isolation.
6. [x] Move bootstrap staging off `/tmp/elisp-stdlib` into repo `tmp/`.
7. [x] Turn the current bootstrap failures into a short tracked matrix:
   file, failing form, error class, missing primitive/special-form/semantic.
8. [x] Finish the `cl-preloaded` circular bootstrap: it now loads `94/94`
   forms, including the built-in `t` type registration path.
9. [x] Fix the highest-frequency bootstrap blockers before adding new stubs.
10. [x] Finish loadup bootstrap: `106 / 106` files now load completely.
11. [x] Finish the first secondary require audit ring: `891 / 891` forms now
    load across `cl-*`, `subr-x`, `pcase`, `gv`, and `ert`.

Current next queue:
1. [x] Replace the first layer of keymap stubs with usable runtime semantics:
   `define-key`, `keymap-set`, parent maps, composed maps, lookup, global/local
   maps, and `defvar-keymap` / `define-keymap` loading behavior.
2. [x] Make current keymaps interpreter-local so `global-set-key`,
   `local-set-key`, `current-global-map`, `current-local-map`, and
   `key-binding` do not leak across interpreters.
3. [x] Finish keymap fidelity:
   canonical `kbd` / `key-parse` output, remapping, prefix maps, menu entries,
   and key lookup/query behavior.
4. [x] Finish the runtime isolation audit by moving match data and any EIEIO
   class registries that still depend on thread/process globals into
   interpreter-owned state.
5. [x] Replace coding-system and translation-table load shortcuts with explicit
   runtime metadata objects and lazy generated tables.
6. [x] Replace customization/advice metadata no-ops where loaded libraries later
   query that metadata (`defcustom`, `custom-declare-*`, `advice-add`).
7. [x] Add interpreter/VM parity gates for the currently bytecode-visible
   pre-JIT subset: top-level bytecode execution, function-cell dispatch,
   redefinition-sensitive calls, metadata primitives, and the load/require
   audit gates.
8. [x] Fix the JIT hot-call path to use version-checked compiled lookup, then add
   redefinition/deopt/tier-transition tests.
9. [x] Clean up split-module warnings and import surfaces so CI can tighten toward
   warning-free elisp builds.
10. [x] Build the next JIT coverage loop from measured post-bootstrap hot
    bytecode functions, not synthetic opcodes.

Pre-JIT handoff:
- The pre-JIT blocking track in Milestones 1-5 is complete for the standalone
  `rele-elisp` interpreter.
- Remaining compatibility debt is explicit and non-blocking for JIT safety work:
  deeper `cl-struct-define` inheritance fidelity, full `cl-generic` method
  combination/dispatch, and app/server command-dispatch integration on top of
  the elisp keymap data.
- Milestones 6 and 7 are complete enough for the next JIT phase: broaden VM/JIT
  parity for higher-level bytecode operations (`car`/`cdr`, calls, varrefs,
  stack mutation) while preserving the audit-and-benchmark loop.

## Notes

- Prefer fixing semantics over adding compatibility stubs when the same gap
  appears across multiple stdlib files.
- Any performance-sensitive interpreter or JIT change should come with a bench
  per `PERFORMANCE.md`.
