//! Cranelift-based JIT compiler for Emacs Lisp bytecode.
//!
//! Compiles hot bytecode functions to native machine code.
//! Feature-gated behind the `jit` feature flag.
//!
//! Architecture:
//! - A [`Profiler`] tracks invocation counts per bytecode function.
//!   When a function crosses the hot threshold, it is handed to the
//!   [`JitCompiler`] for compilation.
//! - The compiler lowers each bytecode opcode to Cranelift IR, emitting
//!   type-guard checks on the fast path and deoptimization exits that
//!   fall back to the bytecode VM when assumptions are violated (e.g.
//!   a function is redefined at runtime).

#[cfg(feature = "jit")]
mod compiler;
mod profiler;

#[cfg(feature = "jit")]
pub use compiler::{JitCompiler, NativeResult};
pub use profiler::Profiler;

/// Which execution tier a named function is currently running in.
///
/// Mirrors the `Tier` enum in `spec/quint/jit_runtime.qnt` and
/// `crates/elisp-spec-tests/src/replay.rs`. Exposed as a public
/// read-only snapshot via `Interpreter::jit_tier`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Source-walking interpreter / bytecode VM. Always safe.
    Interp,
    /// Native code produced by Cranelift, gated on a matching
    /// `def_version`. Falls back to `Interp` on deopt or on
    /// redefinition.
    Compiled,
}

/// Aggregate JIT counters + per-function version map snapshot.
/// Produced by `Interpreter::jit_stats` for tests + tooling.
#[derive(Debug, Clone, Default)]
pub struct JitStats {
    /// Number of bytecode functions that have crossed the hot
    /// threshold (`Profiler::hot_function_count`).
    pub hot_count: u64,
    /// Cumulative calls recorded by the profiler
    /// (`Profiler::total_calls`).
    pub total_calls: u64,
    /// Number of functions for which the compiler holds native code.
    /// Always 0 when the `jit` feature is disabled.
    pub compiled_count: u64,
}

/// Reasons `Interpreter::jit_compile(name)` might decline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JitError {
    /// The named symbol has no function cell (never `defun`'d).
    UnknownFunction(String),
    /// The function cell isn't a bytecode function — only bytecode
    /// is JIT-compilable today. Sources of "not bytecode": a Lisp
    /// lambda (compiles to source-level closure form), a primitive
    /// (already native), or an autoload stub.
    NotBytecode(String),
    /// The bytecode contained an opcode the compiler doesn't yet
    /// support. Caller should fall back to the VM.
    UnsupportedOpcode,
    /// The crate was built without the `jit` feature flag.
    JitDisabled,
}

impl std::fmt::Display for JitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JitError::UnknownFunction(n) => write!(f, "unknown function: {n}"),
            JitError::NotBytecode(n) => write!(f, "{n}: function cell is not bytecode"),
            JitError::UnsupportedOpcode => f.write_str("bytecode contains unsupported opcode"),
            JitError::JitDisabled => f.write_str("crate built without `jit` feature"),
        }
    }
}

impl std::error::Error for JitError {}

#[cfg(not(feature = "jit"))]
pub enum NativeResult {
    Ok(u64),
    Deoptimize,
}

/// No-op JIT compiler stub when the `jit` feature is disabled.
/// Allows callers to unconditionally hold a `JitCompiler` without
/// feature-gating every use site.
#[cfg(not(feature = "jit"))]
pub struct JitCompiler;

#[cfg(not(feature = "jit"))]
impl JitCompiler {
    pub fn new() -> Self {
        JitCompiler
    }

    /// Returns `false` -- native compilation is not available.
    pub fn is_available() -> bool {
        false
    }
}

#[cfg(not(feature = "jit"))]
impl Default for JitCompiler {
    fn default() -> Self {
        Self::new()
    }
}
