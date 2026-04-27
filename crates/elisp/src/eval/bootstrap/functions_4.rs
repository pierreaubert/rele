//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use crate::eval::Interpreter;
use crate::object::LispObject;

use super::run_rele_ert_tests_detailed;
use super::types::{ErtRunStats, ErtTestResult};

pub(super) fn run_rele_ert_tests_detailed_inner(
    interp: &Interpreter,
    per_test_ms: u64,
) -> (ErtRunStats, Vec<ErtTestResult>) {
    use crate::obarray;
    let test_key = obarray::intern("ert--rele-test");
    let test_struct_key = obarray::intern("ert--test");
    let skipped_key = obarray::intern("ert-test-skipped");
    let failed_key = obarray::intern("ert-test-failed");
    let mut stats = ErtRunStats::default();
    let mut results: Vec<ErtTestResult> = Vec::new();
    let mut tests: Vec<(String, LispObject, LispObject)> = Vec::new();
    {
        let ob = obarray::GLOBAL_OBARRAY.read();
        let cells = interp.state.symbol_cells.read();
        for sym_idx in 0..ob.symbol_count() {
            let id = obarray::SymbolId(sym_idx as u32);
            // The global obarray is name-only. ERT registrations live in
            // this interpreter's SymbolCells, so stale names from other
            // interpreters are ignored here unless this interpreter also
            // registered the ERT plist entries.
            let thunk = cells.get_plist(id, test_key);
            if !thunk.is_nil() {
                let struct_obj = cells.get_plist(id, test_struct_key);
                tests.push((ob.name(id).to_string(), thunk, struct_obj));
            }
        }
    }
    let _ = test_struct_key;
    for (name, thunk, test_struct) in tests {
        let call = LispObject::cons(
            LispObject::symbol("funcall"),
            LispObject::cons(
                LispObject::cons(
                    LispObject::symbol("quote"),
                    LispObject::cons(thunk, LispObject::nil()),
                ),
                LispObject::nil(),
            ),
        );
        interp.reset_eval_ops();
        interp.state.clear_closure_mutations();
        interp.set_eval_ops_limit(50_000_000);
        if per_test_ms > 0 {
            interp.set_deadline(
                std::time::Instant::now() + std::time::Duration::from_millis(per_test_ms),
            );
        }
        crate::primitives::set_current_ert_test(test_struct.clone());
        let start = std::time::Instant::now();
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| interp.eval(call.clone())));
        let elapsed_ms = start.elapsed().as_millis();
        crate::primitives::set_current_ert_test(LispObject::nil());
        interp.clear_deadline();
        let was_timed_out = per_test_ms > 0 && elapsed_ms >= (per_test_ms as u128);
        let (result, detail) = match outcome {
            Ok(Ok(_)) => {
                stats.passed += 1;
                ("pass", String::new())
            }
            Ok(Err(crate::error::ElispError::Signal(sig))) => {
                let sym = sig.symbol.as_symbol_id();
                let sym_name = sig
                    .symbol
                    .as_symbol()
                    .unwrap_or_else(|| sig.symbol.prin1_to_string());
                let data_str = sig.data.prin1_to_string();
                if sym == Some(failed_key) {
                    stats.failed += 1;
                    let d = if data_str.is_empty() || data_str == "nil" {
                        "(assertion failed)".to_string()
                    } else {
                        data_str
                    };
                    ("fail", d)
                } else if sym == Some(skipped_key) {
                    stats.skipped += 1;
                    let d = if data_str.is_empty() || data_str == "nil" {
                        String::new()
                    } else {
                        format!("skip: {data_str}")
                    };
                    ("skip", d)
                } else {
                    stats.errored += 1;
                    if data_str.is_empty() || data_str == "nil" {
                        ("error", format!("signal {sym_name}"))
                    } else {
                        ("error", format!("signal {sym_name}: {data_str}"))
                    }
                }
            }
            Ok(Err(e)) => {
                let msg = e.to_string();
                if was_timed_out {
                    stats.timed_out += 1;
                    ("timeout", format!("exceeded {per_test_ms}ms wall-clock"))
                } else {
                    stats.errored += 1;
                    let detail = if msg.is_empty() {
                        format!("{e:?}")
                    } else {
                        msg
                    };
                    ("error", detail)
                }
            }
            Err(payload) => {
                stats.panicked += 1;
                let msg = if let Some(s) = payload.downcast_ref::<&'static str>() {
                    (*s).to_string()
                } else if let Some(s) = payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "panic: <unprintable payload>".to_string()
                };
                ("panic", msg)
            }
        };
        results.push(ErtTestResult {
            name,
            result,
            detail,
            duration_ms: elapsed_ms,
        });
    }
    interp.set_eval_ops_limit(0);
    (stats, results)
}
/// Backwards-compatible wrapper that drops the per-test detail.
pub fn run_rele_ert_tests(interp: &Interpreter) -> ErtRunStats {
    let (stats, results) = run_rele_ert_tests_detailed(interp);
    for r in &results {
        match r.result {
            "fail" => eprintln!("    FAIL: {}", r.name),
            "error" => eprintln!("    ERROR: {}: {}", r.name, r.detail),
            "panic" => eprintln!("    PANIC: {}", r.name),
            "timeout" => eprintln!("    TIMEOUT: {}", r.name),
            _ => {}
        }
    }
    stats
}
