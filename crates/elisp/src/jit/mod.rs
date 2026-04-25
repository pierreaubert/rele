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

use crate::object::BytecodeFunction;

/// One unsupported opcode discovered while scanning a bytecode function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnsupportedOpcode {
    /// Byte offset of the opcode inside the function's bytecode vector.
    pub pc: usize,
    /// Raw Emacs bytecode opcode.
    pub opcode: u8,
}

/// JIT coverage summary for one bytecode function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeJitCoverage {
    /// Number of opcodes scanned, excluding operand bytes.
    pub opcode_count: usize,
    /// Unsupported opcodes with their byte offsets.
    pub unsupported: Vec<UnsupportedOpcode>,
}

impl BytecodeJitCoverage {
    /// Returns true when every opcode in the function has a JIT lowering.
    pub fn is_fully_supported(&self) -> bool {
        self.unsupported.is_empty()
    }
}

/// Return the mnemonic used by the VM/JIT for an opcode.
///
/// This intentionally covers both supported and unsupported VM opcodes so audit
/// tools can print actionable coverage reports instead of raw byte values only.
pub fn opcode_name(opcode: u8) -> &'static str {
    match opcode {
        0..=5 => "stack-ref",
        6 => "stack-ref1",
        7 => "stack-ref2",
        8..=13 => "varref",
        14 => "varref1",
        15 => "varref2",
        16..=21 => "varset",
        22 => "varset1",
        23 => "varset2",
        24..=29 => "varbind",
        30 => "varbind1",
        31 => "varbind2",
        32..=37 => "call",
        38 => "call1",
        39 => "call2",
        40..=45 => "unbind",
        46 => "unbind1",
        47 => "unbind2",
        48 => "pophandler",
        49 => "pushconditioncase",
        50 => "pushcatch",
        56 => "nth",
        57 => "symbolp",
        58 => "consp",
        59 => "stringp",
        60 => "listp",
        61 => "eq",
        62 => "memq",
        63 => "not",
        64 => "car",
        65 => "cdr",
        66 => "cons",
        67 => "list1",
        68 => "list2",
        69 => "list3",
        70 => "list4",
        71 => "length",
        72 => "aref",
        73 => "aset",
        74 => "symbol-value",
        75 => "symbol-function",
        76 => "set",
        77 => "fset",
        78 => "get",
        79 => "substring",
        80 => "concat2",
        81 => "concat3",
        82 => "concat4",
        83 => "sub1",
        84 => "add1",
        85 => "eqlsign",
        86 => "gtr",
        87 => "lss",
        88 => "leq",
        89 => "geq",
        90 => "diff",
        91 => "negate",
        92 => "plus",
        93 => "max",
        94 => "min",
        95 => "mult",
        96 => "point",
        97 => "goto-char",
        98 => "point-legacy",
        99 => "goto-char-legacy",
        100 => "insert",
        101 => "point-max",
        102 => "point-min",
        103 => "char-after",
        104 => "following-char",
        105 => "preceding-char",
        106 => "current-column",
        107 => "indent-to",
        108 => "eobp",
        109 => "eolp",
        110 => "eobp-legacy",
        111 => "bolp",
        112 => "bobp",
        113 => "current-buffer",
        114 => "set-buffer",
        115 => "save-current-buffer",
        116 => "skip-chars-forward",
        117 => "skip-chars-backward",
        118 => "interactive-p",
        119 => "forward-char",
        120 => "forward-word",
        121 => "delete-region",
        122 => "forward-line",
        123 => "char-syntax",
        124 => "buffer-substring",
        125 => "delete-region",
        126 => "narrow-to-region",
        127 => "widen",
        128 => "end-of-line",
        129 => "constant2",
        130 => "goto",
        131 => "goto-if-nil",
        132 => "goto-if-not-nil",
        133 => "goto-if-nil-else-pop",
        134 => "goto-if-not-nil-else-pop",
        135 => "return",
        136 => "discard",
        137 => "dup",
        138 => "save-excursion",
        139 => "save-excursion-restore",
        140 => "save-restriction",
        141 => "catch",
        142 => "unwind-protect",
        143 => "condition-case",
        144 => "temp-output-buffer-setup",
        145 => "temp-output-buffer-show",
        146 => "mark-marker",
        147 => "set-marker",
        148 => "match-beginning",
        149 => "match-end",
        150 => "upcase",
        151 => "downcase",
        152 => "string=",
        153 => "string<",
        154 => "equal",
        155 => "nthcdr",
        156 => "elt",
        157 => "member",
        158 => "assq",
        159 => "nreverse",
        160 => "setcar",
        161 => "setcdr",
        162 => "car-safe",
        163 => "cdr-safe",
        164 => "nconc",
        165 => "quo",
        166 => "rem",
        167 => "numberp",
        168 => "integerp",
        169 => "rgoto",
        170 => "rgoto-if-nil",
        171 => "rgoto-if-not-nil",
        172 => "rgoto-if-nil-else-pop",
        173 => "rgoto-if-not-nil-else-pop",
        174 => "discard-n",
        175 => "stack-set",
        176 => "stack-set",
        177 => "stack-set",
        178 => "stack-set",
        179 => "stack-set2",
        180 => "stack-ref1",
        181 => "stack-ref2",
        182 => "discard-n",
        192..=255 => "constant",
        _ => "unknown",
    }
}

/// Returns true when the Cranelift JIT has a lowering for `opcode`.
pub fn is_jit_supported_opcode(opcode: u8) -> bool {
    matches!(
        opcode,
        0..=6
            | 61
            | 63
            | 83..=95
            | 129..=137
            | 192..=255
    )
}

/// Number of bytes occupied by an opcode including its operands.
///
/// Unknown opcodes are treated as single-byte instructions so audit reports can
/// keep scanning after VM/JIT gaps without panicking.
pub fn opcode_width(bytecode: &[u8], pc: usize) -> usize {
    let Some(&opcode) = bytecode.get(pc) else {
        return 0;
    };
    match opcode {
        6 | 14 | 22 | 30 | 38 | 46 | 169..=178 | 180 | 182 => 2,
        7 | 15 | 23 | 31 | 39 | 47 | 49 | 50 | 129..=134 | 141 | 143 | 179 | 181 => 3,
        _ => 1,
    }
}

/// Scan a bytecode function and report which opcodes still lack JIT coverage.
pub fn bytecode_jit_coverage(func: &BytecodeFunction) -> BytecodeJitCoverage {
    let mut pc = 0usize;
    let mut opcode_count = 0usize;
    let mut unsupported = Vec::new();
    while pc < func.bytecode.len() {
        let opcode = func.bytecode[pc];
        opcode_count += 1;
        if !is_jit_supported_opcode(opcode) {
            unsupported.push(UnsupportedOpcode { pc, opcode });
        }
        pc += opcode_width(&func.bytecode, pc).max(1);
    }
    BytecodeJitCoverage {
        opcode_count,
        unsupported,
    }
}

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
    /// Number of compiled entries invalidated because definitions changed.
    pub invalidation_count: u64,
    /// Number of native fast-path bailouts that fell back to the VM.
    pub deopt_count: u64,
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
