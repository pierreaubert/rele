//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

/// Per-test result, captured for JSONL emission.
#[derive(Clone, Debug)]
pub struct ErtTestResult {
    pub name: String,
    /// One of "pass", "fail", "error", "skip", "panic", "timeout".
    pub result: &'static str,
    /// Free-form detail (error message, signal symbol, etc.). Empty for passes.
    pub detail: String,
    pub duration_ms: u128,
    pub stub_hits: Vec<crate::primitives::core::stub_telemetry::StubHit>,
}
impl ErtTestResult {
    /// Encode as one JSON object (single line). We hand-roll to avoid
    /// pulling serde into the elisp crate proper — serde_json is a
    /// dev-dep but only used by the harness.
    pub fn to_jsonl(&self, file: &str) -> String {
        fn esc(s: &str) -> String {
            let mut out = String::with_capacity(s.len() + 2);
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if (c as u32) < 0x20 => {
                        out.push_str(&format!("\\u{:04x}", c as u32));
                    }
                    c => out.push(c),
                }
            }
            out
        }
        format!(
            r#"{{"file":"{}","test":"{}","result":"{}","ms":{},"detail":"{}","stubs":"{}"}}"#,
            esc(file),
            esc(&self.name),
            self.result,
            self.duration_ms,
            esc(&self.detail),
            esc(&crate::primitives::core::stub_telemetry::encode_stub_hits(
                &self.stub_hits,
            )),
        )
    }
}
#[derive(Default, Clone, Copy)]
pub struct ErtRunStats {
    pub passed: usize,
    pub failed: usize,
    pub errored: usize,
    pub skipped: usize,
    pub panicked: usize,
    pub timed_out: usize,
}
