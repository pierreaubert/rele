// jit_runtime.qnt — Quint model of the rele-elisp JIT runtime orchestration.
//
// Models tier transitions (Interp ↔ Compiled), code cache management,
// deoptimization, and invalidation on function redefinition.
//
// Run with:
//   quint run --invariant safeExecution --max-steps 50 jit_runtime.qnt
//   quint run --invariant noStaleKeepsRunning --max-steps 50 jit_runtime.qnt
//
// For exhaustive checking (requires Apalache):
//   quint verify --invariant safeExecution jit_runtime.qnt
//   quint verify --invariant noStaleKeepsRunning jit_runtime.qnt

module jit_runtime {

  // --- Types ---

  type FuncId = str

  // We represent Tier as a string: "Interp" or "Compiled"
  // (avoids sum-type parsing issues across Quint versions)

  // --- State ---

  /// Current execution tier per function.
  var tier: FuncId -> str

  /// Definition version — bumped on every (defun name ...) or (fset name ...).
  var defVersion: FuncId -> int

  /// Profiler call counts per function.
  var callCount: FuncId -> int

  /// Compiled code cache: funcId → version it was compiled from.
  /// -1 means no compiled code exists.
  var cache: FuncId -> int

  /// Whether compiled code exists for a function.
  var hasCompiled: FuncId -> bool

  /// Compilation threshold (constant after init).
  var threshold: int

  /// The set of known function ids (bounded for model checking).
  pure val FUNCS: Set[str] = Set("f", "g", "h")

  // --- Init ---

  action init = all {
    tier' = FUNCS.mapBy(_ => "Interp"),
    defVersion' = FUNCS.mapBy(_ => 0),
    callCount' = FUNCS.mapBy(_ => 0),
    cache' = FUNCS.mapBy(_ => -1),
    hasCompiled' = FUNCS.mapBy(_ => false),
    threshold' = 3,
  }

  // --- Actions ---

  /// A function is called. Bump its counter; if it reaches threshold
  /// and is not yet compiled, trigger compilation.
  action call(f: FuncId): bool = {
    val newCount = callCount.get(f) + 1
    all {
      callCount' = callCount.set(f, newCount),
      if (newCount >= threshold and not(hasCompiled.get(f))) {
        // Trigger compilation from current version
        all {
          cache' = cache.set(f, defVersion.get(f)),
          hasCompiled' = hasCompiled.set(f, true),
          tier' = tier.set(f, "Compiled"),
          defVersion' = defVersion,
          threshold' = threshold,
        }
      } else if (hasCompiled.get(f) and cache.get(f) == defVersion.get(f)) {
        // Already compiled and up-to-date: run compiled
        all {
          tier' = tier.set(f, "Compiled"),
          cache' = cache,
          hasCompiled' = hasCompiled,
          defVersion' = defVersion,
          threshold' = threshold,
        }
      } else {
        // Not yet hot, or compiled code is stale: interpret
        all {
          tier' = tier.set(f, "Interp"),
          cache' = cache,
          hasCompiled' = hasCompiled,
          defVersion' = defVersion,
          threshold' = threshold,
        }
      },
    }
  }

  /// A function is redefined (defun/fset). Bump version, invalidate
  /// compiled code, reset profiler counter.
  action redefine(f: FuncId): bool = all {
    defVersion' = defVersion.set(f, defVersion.get(f) + 1),
    callCount' = callCount.set(f, 0),
    hasCompiled' = hasCompiled.set(f, false),
    cache' = cache.set(f, -1),
    tier' = tier.set(f, "Interp"),
    threshold' = threshold,
  }

  /// Deoptimization: compiled code bails out (type guard failure).
  /// Function falls back to interpreter for this invocation.
  /// Compiled code stays in cache (it's still valid, just hit a slow path).
  action deopt(f: FuncId): bool = all {
    hasCompiled.get(f),
    tier' = tier.set(f, "Interp"),
    defVersion' = defVersion,
    callCount' = callCount,
    cache' = cache,
    hasCompiled' = hasCompiled,
    threshold' = threshold,
  }

  /// Non-deterministic step: pick any function and any action.
  action step = {
    nondet f = FUNCS.oneOf()
    any {
      call(f),
      redefine(f),
      deopt(f),
    }
  }

  // --- Invariants ---

  /// Safety: if a function is running as Compiled, the cached version
  /// MUST match the current definition version.
  val safeExecution: bool =
    FUNCS.forall(f =>
      tier.get(f) == "Compiled" implies cache.get(f) == defVersion.get(f)
    )

  /// No stale code keeps running: after a redefine, the function must
  /// NOT remain in Compiled tier.
  val noStaleKeepsRunning: bool =
    FUNCS.forall(f =>
      hasCompiled.get(f) == false implies tier.get(f) == "Interp"
    )
}
