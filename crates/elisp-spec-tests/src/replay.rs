//! Quint trace replay harness.
//!
//! Reads JSON traces produced by `quint run --out-itf` and replays them
//! against a Rust model of the JIT runtime state machine, asserting that
//! invariants hold at every step.

use serde::Deserialize;
use serde_json::Value;
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

// ── ITF (Informal Trace Format) parsing ──────────────────────────────────────
//
// Quint's `--out-itf` emits state snapshots, not action labels. The format
// wraps integers as `{"#bigint": "N"}` and maps as `{"#map": [[k,v],...]}`.
// We parse each snapshot into a full `JitState`, then verify the Quint
// transition relation by searching for a Rust action whose post-state matches.

fn parse_bigint(v: &Value) -> i64 {
    v.get("#bigint")
        .and_then(Value::as_str)
        .expect("expected #bigint")
        .parse()
        .expect("bigint string not parseable as i64")
}

fn parse_map_pairs(v: &Value) -> &Vec<Value> {
    v.get("#map")
        .and_then(Value::as_array)
        .expect("expected #map")
}

fn parse_map_int(v: &Value) -> HashMap<String, i64> {
    parse_map_pairs(v)
        .iter()
        .map(|pair| {
            let arr = pair.as_array().expect("pair must be array");
            let key = arr[0].as_str().expect("map key must be string").to_string();
            (key, parse_bigint(&arr[1]))
        })
        .collect()
}

fn parse_map_bool(v: &Value) -> HashMap<String, bool> {
    parse_map_pairs(v)
        .iter()
        .map(|pair| {
            let arr = pair.as_array().expect("pair must be array");
            let key = arr[0].as_str().expect("map key must be string").to_string();
            (key, arr[1].as_bool().expect("expected bool"))
        })
        .collect()
}

fn parse_map_tier(v: &Value) -> HashMap<String, Tier> {
    parse_map_pairs(v)
        .iter()
        .map(|pair| {
            let arr = pair.as_array().expect("pair must be array");
            let key = arr[0].as_str().expect("map key must be string").to_string();
            let tier = match arr[1].as_str().expect("tier must be string") {
                "Interp" => Tier::Interp,
                "Compiled" => Tier::Compiled,
                other => panic!("unknown tier: {other}"),
            };
            (key, tier)
        })
        .collect()
}

fn parse_state(v: &Value) -> JitState {
    JitState {
        tier: parse_map_tier(&v["tier"]),
        def_version: parse_map_int(&v["defVersion"]),
        call_count: parse_map_int(&v["callCount"]),
        cache_version: parse_map_int(&v["cache"]),
        has_compiled: parse_map_bool(&v["hasCompiled"]),
        threshold: parse_bigint(&v["threshold"]),
    }
}

/// Parse a Quint `--out-itf` JSON file into the list of state snapshots.
pub fn parse_itf(path: &str) -> Vec<JitState> {
    let raw = std::fs::read_to_string(path).expect("cannot read ITF file");
    let root: Value = serde_json::from_str(&raw).expect("invalid JSON");
    root["states"]
        .as_array()
        .expect("states must be array")
        .iter()
        .map(parse_state)
        .collect()
}

fn state_matches(a: &JitState, b: &JitState) -> bool {
    a.tier == b.tier
        && a.def_version == b.def_version
        && a.call_count == b.call_count
        && a.cache_version == b.cache_version
        && a.has_compiled == b.has_compiled
        && a.threshold == b.threshold
}

/// Try each action on each function from `prev`; return the first whose
/// resulting state equals `next`, or `None` if no action reproduces the
/// transition (which would indicate a Rust/Quint mismatch).
pub fn verify_transition(prev: &JitState, next: &JitState) -> Option<String> {
    let funcs: Vec<String> = prev.tier.keys().cloned().collect();

    for f in &funcs {
        // call(f)
        let mut s = prev.clone();
        s.call(f);
        if state_matches(&s, next) {
            return Some(format!("call({f})"));
        }

        // redefine(f)
        let mut s = prev.clone();
        s.redefine(f);
        if state_matches(&s, next) {
            return Some(format!("redefine({f})"));
        }

        // deopt(f) — only valid when currently compiled
        if prev.has_compiled[f] {
            let mut s = prev.clone();
            s.deopt(f);
            if state_matches(&s, next) {
                return Some(format!("deopt({f})"));
            }
        }
    }
    None
}
