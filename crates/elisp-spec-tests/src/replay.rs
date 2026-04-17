//! Quint trace replay harness.
//!
//! Reads JSON traces produced by `quint run --out-itf` and replays them
//! against a Rust model of the JIT runtime state machine, asserting that
//! invariants hold at every step.

use serde::Deserialize;
use std::collections::HashMap;

/// JIT execution tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Interp,
    Compiled,
}

/// Rust-side mirror of the Quint JIT runtime state.
#[derive(Debug, Clone)]
pub struct JitState {
    pub tier: HashMap<String, Tier>,
    pub def_version: HashMap<String, i64>,
    pub call_count: HashMap<String, i64>,
    pub cache_version: HashMap<String, i64>,
    pub has_compiled: HashMap<String, bool>,
    pub threshold: i64,
}

impl JitState {
    pub fn new(funcs: &[&str], threshold: i64) -> Self {
        let mut state = Self {
            tier: HashMap::new(),
            def_version: HashMap::new(),
            call_count: HashMap::new(),
            cache_version: HashMap::new(),
            has_compiled: HashMap::new(),
            threshold,
        };
        for &f in funcs {
            state.tier.insert(f.to_string(), Tier::Interp);
            state.def_version.insert(f.to_string(), 0);
            state.call_count.insert(f.to_string(), 0);
            state.cache_version.insert(f.to_string(), -1);
            state.has_compiled.insert(f.to_string(), false);
        }
        state
    }

    pub fn call(&mut self, f: &str) {
        let count = self.call_count.get_mut(f).expect("unknown func");
        *count += 1;
        let new_count = *count;
        let has = self.has_compiled[f];
        let def_ver = self.def_version[f];

        if new_count >= self.threshold && !has {
            // Trigger compilation
            self.cache_version.insert(f.to_string(), def_ver);
            self.has_compiled.insert(f.to_string(), true);
            self.tier.insert(f.to_string(), Tier::Compiled);
        } else if has && self.cache_version[f] == def_ver {
            self.tier.insert(f.to_string(), Tier::Compiled);
        } else {
            self.tier.insert(f.to_string(), Tier::Interp);
        }
    }

    pub fn redefine(&mut self, f: &str) {
        *self.def_version.get_mut(f).expect("unknown func") += 1;
        self.call_count.insert(f.to_string(), 0);
        self.has_compiled.insert(f.to_string(), false);
        self.cache_version.insert(f.to_string(), -1);
        self.tier.insert(f.to_string(), Tier::Interp);
    }

    pub fn deopt(&mut self, f: &str) {
        assert!(self.has_compiled[f], "cannot deopt without compiled code");
        self.tier.insert(f.to_string(), Tier::Interp);
    }

    /// Invariant: compiled code version matches definition version.
    pub fn check_safe_execution(&self) -> bool {
        self.tier.iter().all(|(f, t)| {
            *t != Tier::Compiled || self.cache_version[f] == self.def_version[f]
        })
    }

    /// Invariant: no compiled code → must be in Interp tier.
    pub fn check_no_stale_keeps_running(&self) -> bool {
        self.has_compiled
            .iter()
            .all(|(f, &has)| has || self.tier[f] == Tier::Interp)
    }
}

/// A single step from a Quint ITF trace.
#[derive(Debug, Deserialize)]
pub struct TraceStep {
    pub action: String,
    pub func: String,
}

/// Replay a Quint-generated trace against the Rust state machine.
/// Returns the step index of the first invariant violation, or None if all pass.
pub fn replay_trace(steps: &[TraceStep], threshold: i64) -> Option<(usize, String)> {
    let funcs = ["f", "g", "h"];
    let mut state = JitState::new(&funcs, threshold);

    for (i, step) in steps.iter().enumerate() {
        match step.action.as_str() {
            "call" => state.call(&step.func),
            "redefine" => state.redefine(&step.func),
            "deopt" => {
                if !state.has_compiled[&step.func] {
                    continue; // skip invalid deopt in trace
                }
                state.deopt(&step.func);
            }
            other => return Some((i, format!("unknown action: {other}"))),
        }

        if !state.check_safe_execution() {
            return Some((i, format!("safeExecution violated at step {i}")));
        }
        if !state.check_no_stale_keeps_running() {
            return Some((i, format!("noStaleKeepsRunning violated at step {i}")));
        }
    }
    None
}
