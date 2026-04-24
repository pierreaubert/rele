# rele-elisp Roadmap

This file tracks the execution plan for making `rele-elisp` viable as an
Emacs core in two stages:

1. Interpreter correctness and bootstrap breadth first.
2. JIT speedups only after the interpreter path is stable.

## Success Criteria

Interpreter-ready means:
- `cargo test -p rele-elisp`
- `cargo test -p rele-elisp --lib`
- `cargo test -p rele-elisp --features jit --no-run`
- Bootstrap helpers can reliably load the core stdlib stack:
  `subr`, `cl-lib`, `macroexp`, `pcase`, `ert`
- ERT smoke tests are deterministic and isolated across interpreter instances

JIT-ready means:
- Tiered execution preserves interpreter semantics on redefinition, deopt,
  unwind, throw, dynamic binding, and optional/rest arg handling.
- JIT counters reflect real compiler state, invalidations, and deopts.
- Benchmarks show wins on real bytecode workloads, not just synthetic ops.

## Milestone 1: Green Interpreter Baseline

- [ ] Keep the default crate test build green.
- [ ] Keep `--features jit --no-run` green so JIT regressions stay visible.
- [ ] Make targeted failures easy to reproduce with one-command tests.
- [ ] Separate compile failures from runtime/bootstrap failures in CI.

Exit condition:
- The crate builds and the remaining failures are runtime behavior gaps only.

## Milestone 2: Runtime Isolation

- [ ] Audit remaining process-global interpreter state beyond the obarray.
- [ ] Ensure ERT registrations do not leak between interpreters.
- [ ] Ensure current buffer / match data / feature state are isolated enough
      for parallel test execution.
- [ ] Remove ad hoc cleanup from tests where runtime-owned reset/isolation is
      the right fix.

Exit condition:
- ERT and bootstrap tests behave deterministically under `cargo test`.

## Milestone 3: Bootstrap as Runtime Capability

- [ ] Keep bootstrap helpers in reusable runtime code, not test-only code.
- [ ] Move stdlib staging to repo `tmp/` to match repo policy.
- [ ] Expose a clear bootstrap API used by tests, audit tools, and future app
      integration.
- [ ] Add a small “bootstrap health” suite that exercises the core load chain.

Exit condition:
- The same bootstrap code path is shared by tests, audits, and future clients.

## Milestone 4: Interpreter Compatibility Gaps

Prioritize missing semantics that unlock real Emacs libraries instead of adding
more one-off stubs.

Current high-value gap buckets:
- [ ] `function-put`
- [ ] `require` / feature loading behavior
- [ ] `after-load-alist`
- [ ] `cl-struct-define`
- [ ] `cl-generic-define` and related generic-function plumbing
- [ ] `def-edebug-elem-spec`
- [ ] `add-minor-mode`
- [ ] `make-composed-keymap`
- [ ] `easy-menu-do-define`
- [ ] Wrong-type regressions hit during `cl-*`, `oclosure`, and character data

Exit condition:
- Core stdlib files load with a small, explicit set of known unsupported areas.

## Milestone 5: Semantic Parity Hardening

- [ ] Add interpreter/VM parity tests for:
  - [ ] redefinition and invalidation-sensitive call paths
  - [ ] dynamic vs lexical binding
  - [ ] unwind-protect / catch / throw
  - [ ] optional and rest argument normalization
  - [ ] macro expansion after stdlib bootstrap
  - [ ] `load` / `require` / `provide`
- [ ] Add a few end-to-end tests around real loaded stdlib functions.

Exit condition:
- The interpreter is boring: predictable, covered, and easy to debug.

## Milestone 6: JIT Safety

- [ ] Use version-checked compiled lookup on the hot path.
- [ ] Track actual compiled entry count instead of inferred hotness.
- [ ] Add tests for:
  - [ ] redefinition invalidation
  - [ ] deopt fallback
  - [ ] eager compile vs hot compile parity
  - [ ] tier transitions over the same function

Exit condition:
- The JIT never keeps stale code running and always falls back safely.

## Milestone 7: JIT Coverage and Performance

- [ ] Profile real bytecode workloads after bootstrap.
- [ ] Expand opcode coverage based on measured hot functions.
- [ ] Add before/after benches for each meaningful hot-path JIT expansion.
- [ ] Keep fallback exact for unsupported cases.

Exit condition:
- The JIT earns its complexity with measured wins on real code.

## Immediate Next Queue

These are the next tasks to pick up in order:

1. Get the targeted interpreter tests stable:
   `test_cl_files_load_progress`, `test_emacs_ert_can_run_a_test`,
   `test_ert_run_per_test_timeout`.
2. Reduce remaining ERT leakage by replacing test-local cleanup with
   interpreter/runtime isolation.
3. Move bootstrap staging off `/tmp/elisp-stdlib` into repo `tmp/`.
4. Turn the current bootstrap failures into a short tracked matrix:
   file, failing form, error class, missing primitive/special-form/semantic.
5. Fix the highest-frequency bootstrap blockers before adding new stubs.

Current short list from targeted runs:
- `function-put` appears repeatedly in `cl-lib`, `cl-macs`, `gv`, and `seq`
- `require` still breaks parts of `gv`
- `def-edebug-elem-spec` still blocks parts of `gv`
- `cl-generic-define` / `cl-generic-define-method` still block `seq`
- `add-minor-mode`, `make-composed-keymap`, and `easy-menu-do-define`
  still appear during heavier stdlib loads

## Notes

- Prefer fixing semantics over adding compatibility stubs when the same gap
  appears across multiple stdlib files.
- Any performance-sensitive interpreter or JIT change should come with a bench
  per `PERFORMANCE.md`.
