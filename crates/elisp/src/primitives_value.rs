//! Value-native fast path for hot primitives.
//!
//! The main `primitives::call_primitive` dispatcher walks args as a
//! `LispObject` cons chain — which means `obj_to_value` at the boundary
//! (creating `ConsArcCell` heap allocations) and `destructure_cons` (mutex
//! lock per cons cell) for every call. For tight hot-paths like `(+ 1 2)`,
//! that overhead dominates.
//!
//! This module intercepts the hot primitives and dispatches them directly
//! on `Value`s. It:
//! - Walks the arg list as raw `ConsCell`s (no mutex, no allocation).
//! - Extracts fixnum args with `Value::as_fixnum`.
//! - Returns `Value`s directly (no `obj_to_value` round-trip).
//!
//! When the fast path can't handle the inputs (float args, heap-typed
//! args, unrecognized primitive), it returns `None` and the caller falls
//! back to the full `LispObject` path.

use crate::error::ElispResult;
use crate::value::Value;

/// Walk a cons-chain Value, collecting fixnums. Returns `None` if any
/// element isn't a fixnum or if the spine contains a `ConsArcCell` (those
/// need a mutex lock, defeating the fast-path purpose) or any other
/// non-cons tag.
fn collect_fixnum_args(args: Value) -> Option<Vec<i64>> {
    let mut result = Vec::new();
    let mut current = args;
    while !current.is_nil() {
        let ptr = current.as_heap_ptr()? as *const crate::gc::GcHeader;
        // SAFETY: we just confirmed it's a heap pointer. The GcHeader
        // layout is guaranteed by the allocator.
        let tag = unsafe { (*ptr).tag };
        if tag != crate::gc::ObjectTag::Cons {
            return None; // ConsArc or other — fall back to LispObject path
        }
        // SAFETY: tag == Cons, so this is a ConsCell with raw u64 car/cdr.
        let cell = ptr as *const crate::gc::ConsCell;
        let car_raw = unsafe { (*cell).car };
        let cdr_raw = unsafe { (*cell).cdr };
        let car = Value::from_raw(car_raw);
        let n = car.as_fixnum()?;
        result.push(n);
        current = Value::from_raw(cdr_raw);
    }
    Some(result)
}

/// Get exactly one Value arg from a cons chain. Returns `None` if the
/// chain is empty or contains a non-Cons heap cell.
fn one_arg(args: Value) -> Option<Value> {
    if args.is_nil() {
        return None;
    }
    let ptr = args.as_heap_ptr()? as *const crate::gc::GcHeader;
    let tag = unsafe { (*ptr).tag };
    if tag != crate::gc::ObjectTag::Cons {
        return None;
    }
    let cell = ptr as *const crate::gc::ConsCell;
    Some(Value::from_raw(unsafe { (*cell).car }))
}

/// Get two args from a cons chain. Returns `None` if we don't have
/// exactly two or more args, or if the spine uses `ConsArcCell`.
fn two_args(args: Value) -> Option<(Value, Value)> {
    let first_cons = args.as_heap_ptr()? as *const crate::gc::GcHeader;
    if unsafe { (*first_cons).tag } != crate::gc::ObjectTag::Cons {
        return None;
    }
    let cell1 = first_cons as *const crate::gc::ConsCell;
    let a = Value::from_raw(unsafe { (*cell1).car });
    let cdr = Value::from_raw(unsafe { (*cell1).cdr });
    let second_cons = cdr.as_heap_ptr()? as *const crate::gc::GcHeader;
    if unsafe { (*second_cons).tag } != crate::gc::ObjectTag::Cons {
        return None;
    }
    let cell2 = second_cons as *const crate::gc::ConsCell;
    let b = Value::from_raw(unsafe { (*cell2).car });
    Some((a, b))
}

/// Try to handle `name`(`args`) on the fast Value path. Returns `None` to
/// indicate the caller should fall back to the `LispObject`-based path.
pub fn try_call_primitive_value(name: &str, args: Value) -> Option<ElispResult<Value>> {
    match name {
        // Arithmetic (variadic over fixnums)
        "+" => {
            let xs = collect_fixnum_args(args)?;
            let sum: i64 = xs.iter().copied().fold(0i64, |a, b| a.wrapping_add(b));
            Some(Ok(Value::fixnum(sum)))
        }
        "-" => {
            let xs = collect_fixnum_args(args)?;
            if xs.is_empty() {
                Some(Ok(Value::fixnum(0)))
            } else if xs.len() == 1 {
                Some(Ok(Value::fixnum(xs[0].wrapping_neg())))
            } else {
                let first = xs[0];
                let rest_sum: i64 = xs[1..].iter().copied().fold(0i64, |a, b| a.wrapping_add(b));
                Some(Ok(Value::fixnum(first.wrapping_sub(rest_sum))))
            }
        }
        "*" => {
            let xs = collect_fixnum_args(args)?;
            let product: i64 = xs.iter().copied().fold(1i64, |a, b| a.wrapping_mul(b));
            Some(Ok(Value::fixnum(product)))
        }

        // Comparison (binary fixnum)
        "=" | "eql" => {
            let xs = collect_fixnum_args(args)?;
            if xs.len() < 2 {
                return Some(Ok(Value::t()));
            }
            let all_eq = xs.windows(2).all(|w| w[0] == w[1]);
            Some(Ok(if all_eq { Value::t() } else { Value::nil() }))
        }
        "<" => {
            let xs = collect_fixnum_args(args)?;
            if xs.len() < 2 {
                return Some(Ok(Value::t()));
            }
            let all = xs.windows(2).all(|w| w[0] < w[1]);
            Some(Ok(if all { Value::t() } else { Value::nil() }))
        }
        ">" => {
            let xs = collect_fixnum_args(args)?;
            if xs.len() < 2 {
                return Some(Ok(Value::t()));
            }
            let all = xs.windows(2).all(|w| w[0] > w[1]);
            Some(Ok(if all { Value::t() } else { Value::nil() }))
        }
        "<=" => {
            let xs = collect_fixnum_args(args)?;
            if xs.len() < 2 {
                return Some(Ok(Value::t()));
            }
            let all = xs.windows(2).all(|w| w[0] <= w[1]);
            Some(Ok(if all { Value::t() } else { Value::nil() }))
        }
        ">=" => {
            let xs = collect_fixnum_args(args)?;
            if xs.len() < 2 {
                return Some(Ok(Value::t()));
            }
            let all = xs.windows(2).all(|w| w[0] >= w[1]);
            Some(Ok(if all { Value::t() } else { Value::nil() }))
        }
        "/=" => {
            // /= requires strict inequality between each pair of adjacent args.
            let xs = collect_fixnum_args(args)?;
            let all = xs.windows(2).all(|w| w[0] != w[1]);
            Some(Ok(if all { Value::t() } else { Value::nil() }))
        }
        "1+" => {
            let n = one_arg(args)?.as_fixnum()?;
            Some(Ok(Value::fixnum(n.wrapping_add(1))))
        }
        "1-" => {
            let n = one_arg(args)?.as_fixnum()?;
            Some(Ok(Value::fixnum(n.wrapping_sub(1))))
        }

        // Cons / list (raw-Value form)
        "null" | "not" => {
            let v = one_arg(args)?;
            Some(Ok(if v.is_nil() { Value::t() } else { Value::nil() }))
        }
        "eq" => {
            let (a, b) = two_args(args)?;
            Some(Ok(if a.raw() == b.raw() { Value::t() } else { Value::nil() }))
        }

        // Type predicates
        "integerp" | "fixnump" => {
            let v = one_arg(args)?;
            Some(Ok(if v.as_fixnum().is_some() { Value::t() } else { Value::nil() }))
        }
        "zerop" => {
            let v = one_arg(args)?;
            match v.as_fixnum() {
                Some(n) => Some(Ok(if n == 0 { Value::t() } else { Value::nil() })),
                None => None, // non-fixnum → fall back
            }
        }
        "natnump" => {
            let v = one_arg(args)?;
            match v.as_fixnum() {
                Some(n) => Some(Ok(if n >= 0 { Value::t() } else { Value::nil() })),
                None => None,
            }
        }

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Instrumentation — useful to tell whether the fast path is actually firing.
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicU64, Ordering};
static FAST_HITS: AtomicU64 = AtomicU64::new(0);
static SLOW_HITS: AtomicU64 = AtomicU64::new(0);

pub fn inc_fast_hits() {
    FAST_HITS.fetch_add(1, Ordering::Relaxed);
}
pub fn inc_slow_hits() {
    SLOW_HITS.fetch_add(1, Ordering::Relaxed);
}
pub fn fast_hits() -> u64 {
    FAST_HITS.load(Ordering::Relaxed)
}
pub fn slow_hits() -> u64 {
    SLOW_HITS.load(Ordering::Relaxed)
}
pub fn reset_hit_counters() {
    FAST_HITS.store(0, Ordering::Relaxed);
    SLOW_HITS.store(0, Ordering::Relaxed);
}

// Silence unused-import lint when the module's only user is behind a cfg.
#[allow(dead_code)]
fn _unused_marker() -> ElispResult<Value> {
    Ok(Value::nil())
}
