//! State-aware cl-* functions: those that funcall a PREDICATE or
//! KEY-FUNCTION argument. They can't live in `primitives_cl.rs`
//! because invoking a lisp function requires the full env/editor/
//! macros/state quartet — only `call_function` has that.
//!
//! Dispatched from `call_stateful_primitive` when none of the earlier
//! state-aware buckets match.

use crate::EditorCallbacks;
use crate::error::ElispResult;
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use super::SyncRefCell as RwLock;
use std::sync::Arc;

use super::functions::call_function;
use super::{Environment, InterpreterState, MacroTable};

/// Parse keyword args `&key :key :test :test-not :start :end :from-end`.
/// Unknown keywords are silently skipped so we tolerate forms with
/// options we don't implement.
#[derive(Default)]
struct CommonKeys {
    key_fn: Option<LispObject>,
    test_fn: Option<LispObject>,
    test_not_fn: Option<LispObject>,
    start: Option<i64>,
    end: Option<i64>,
    from_end: bool,
    count: Option<i64>,
    initial_value: Option<LispObject>,
    #[allow(dead_code)]
    has_initial_value: bool,
}

/// Walk cons-list args looking for `:KEYWORD VALUE` pairs. Also
/// returns the *positional* args (those before any keyword). Used by
/// most cl-* sequence functions.
fn split_keys(args: &LispObject, positional_count: usize) -> (Vec<LispObject>, CommonKeys) {
    let mut positional: Vec<LispObject> = Vec::with_capacity(positional_count);
    let mut cur = args.clone();
    for _ in 0..positional_count {
        match cur.destructure_cons() {
            Some((car, cdr)) => {
                positional.push(car);
                cur = cdr;
            }
            None => break,
        }
    }
    let mut keys = CommonKeys::default();
    while let Some((k, vs)) = cur.destructure_cons() {
        let key = k.as_symbol();
        let Some((v, rest2)) = vs.destructure_cons() else {
            break;
        };
        match key.as_deref() {
            Some(":key") => keys.key_fn = Some(v),
            Some(":test") => keys.test_fn = Some(v),
            Some(":test-not") => keys.test_not_fn = Some(v),
            Some(":start") => keys.start = v.as_integer(),
            Some(":end") => keys.end = v.as_integer(),
            Some(":from-end") => keys.from_end = !matches!(v, LispObject::Nil),
            Some(":count") => keys.count = v.as_integer(),
            Some(":initial-value") => {
                keys.initial_value = Some(v);
                keys.has_initial_value = true;
            }
            _ => {} // silently skip unknown keyword
        }
        cur = rest2;
    }
    (positional, keys)
}

fn to_vec(list: &LispObject) -> Vec<LispObject> {
    let mut out = Vec::new();
    let mut cur = list.clone();
    let mut steps = 0u64;
    while let Some((car, cdr)) = cur.destructure_cons() {
        out.push(car);
        cur = cdr;
        steps += 1;
        if steps > (1 << 24) {
            break;
        }
    }
    out
}

fn from_slice(items: &[LispObject]) -> LispObject {
    let mut out = LispObject::nil();
    for i in items.iter().rev() {
        out = LispObject::cons(i.clone(), out);
    }
    out
}

/// Invoke a lisp function on one arg and return the result as LispObject.
fn call1(
    func: &LispObject,
    arg: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let args = LispObject::cons(arg, LispObject::nil());
    let r = call_function(
        obj_to_value(func.clone()),
        obj_to_value(args),
        env,
        editor,
        macros,
        state,
    )?;
    Ok(value_to_obj(r))
}

fn call2(
    func: &LispObject,
    a: LispObject,
    b: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let args = LispObject::cons(a, LispObject::cons(b, LispObject::nil()));
    let r = call_function(
        obj_to_value(func.clone()),
        obj_to_value(args),
        env,
        editor,
        macros,
        state,
    )?;
    Ok(value_to_obj(r))
}

fn truthy(v: &LispObject) -> bool {
    !matches!(v, LispObject::Nil)
}

/// Apply keys.key_fn to x if set; otherwise return x.
fn keyed(
    keys: &CommonKeys,
    x: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    match &keys.key_fn {
        Some(f) => call1(f, x, env, editor, macros, state),
        None => Ok(x),
    }
}

/// Default equality test (eql/equal): we use our cross-type equal.
fn default_eq(a: &LispObject, b: &LispObject) -> bool {
    a == b
}

/// Call the custom :test / :test-not predicate if set, else fall back
/// to default_eq.
fn matches_test(
    keys: &CommonKeys,
    a: &LispObject,
    b: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<bool> {
    if let Some(f) = &keys.test_not_fn {
        let r = call2(f, a.clone(), b.clone(), env, editor, macros, state)?;
        return Ok(!truthy(&r));
    }
    if let Some(f) = &keys.test_fn {
        let r = call2(f, a.clone(), b.clone(), env, editor, macros, state)?;
        return Ok(truthy(&r));
    }
    Ok(default_eq(a, b))
}

// ---- Higher-order sequence ops --------------------------------------

pub fn cl_find(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let key = pos.first().cloned().unwrap_or(LispObject::nil());
    let list = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let items = to_vec(&list);
    let range = apply_start_end(&items, &keys);
    let mut indices: Vec<usize> = range.collect();
    if keys.from_end {
        indices.reverse();
    }
    for i in indices {
        let item = items[i].clone();
        let k = keyed(&keys, item.clone(), env, editor, macros, state)?;
        if matches_test(&keys, &key, &k, env, editor, macros, state)? {
            return Ok(item);
        }
    }
    Ok(LispObject::nil())
}

pub fn cl_find_if(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let pred = pos.first().cloned().unwrap_or(LispObject::nil());
    let list = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let items = to_vec(&list);
    let range = apply_start_end(&items, &keys);
    let mut indices: Vec<usize> = range.collect();
    if keys.from_end {
        indices.reverse();
    }
    for i in indices {
        let item = items[i].clone();
        let k = keyed(&keys, item.clone(), env, editor, macros, state)?;
        let r = call1(&pred, k, env, editor, macros, state)?;
        if truthy(&r) {
            return Ok(item);
        }
    }
    Ok(LispObject::nil())
}

pub fn cl_find_if_not(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // find-if-not PRED ≡ find-if (not . PRED). We inline the negation.
    let (pos, keys) = split_keys(args, 2);
    let pred = pos.first().cloned().unwrap_or(LispObject::nil());
    let list = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let items = to_vec(&list);
    let range = apply_start_end(&items, &keys);
    let mut indices: Vec<usize> = range.collect();
    if keys.from_end {
        indices.reverse();
    }
    for i in indices {
        let item = items[i].clone();
        let k = keyed(&keys, item.clone(), env, editor, macros, state)?;
        let r = call1(&pred, k, env, editor, macros, state)?;
        if !truthy(&r) {
            return Ok(item);
        }
    }
    Ok(LispObject::nil())
}

pub fn cl_position_if(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let pred = pos.first().cloned().unwrap_or(LispObject::nil());
    let list = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let items = to_vec(&list);
    let range = apply_start_end(&items, &keys);
    let indices: Vec<usize> = if keys.from_end {
        range.rev().collect()
    } else {
        range.collect()
    };
    for i in indices {
        let item = items[i].clone();
        let k = keyed(&keys, item, env, editor, macros, state)?;
        let r = call1(&pred, k, env, editor, macros, state)?;
        if truthy(&r) {
            return Ok(LispObject::integer(i as i64));
        }
    }
    Ok(LispObject::nil())
}

pub fn cl_count_if(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let pred = pos.first().cloned().unwrap_or(LispObject::nil());
    let list = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let items = to_vec(&list);
    let mut n: i64 = 0;
    for i in apply_start_end(&items, &keys) {
        let item = items[i].clone();
        let k = keyed(&keys, item, env, editor, macros, state)?;
        let r = call1(&pred, k, env, editor, macros, state)?;
        if truthy(&r) {
            n += 1;
        }
    }
    Ok(LispObject::integer(n))
}

pub fn cl_member_if(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, _keys) = split_keys(args, 2);
    let pred = pos.first().cloned().unwrap_or(LispObject::nil());
    let list = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let mut cur = list;
    while let Some((car, cdr)) = cur.clone().destructure_cons() {
        let r = call1(&pred, car, env, editor, macros, state)?;
        if truthy(&r) {
            return Ok(cur);
        }
        cur = cdr;
    }
    Ok(LispObject::nil())
}

pub fn cl_member_if_not(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, _keys) = split_keys(args, 2);
    let pred = pos.first().cloned().unwrap_or(LispObject::nil());
    let list = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let mut cur = list;
    while let Some((car, cdr)) = cur.clone().destructure_cons() {
        let r = call1(&pred, car, env, editor, macros, state)?;
        if !truthy(&r) {
            return Ok(cur);
        }
        cur = cdr;
    }
    Ok(LispObject::nil())
}

pub fn cl_some(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (cl-some PRED &rest LISTS)
    let pred = args.first().unwrap_or(LispObject::nil());
    let lists: Vec<Vec<LispObject>> = {
        let mut out = Vec::new();
        let mut cur = args.rest().unwrap_or(LispObject::nil());
        while let Some((list, rest)) = cur.destructure_cons() {
            out.push(to_vec(&list));
            cur = rest;
        }
        out
    };
    if lists.is_empty() {
        return Ok(LispObject::nil());
    }
    let len = lists.iter().map(Vec::len).min().unwrap_or(0);
    for i in 0..len {
        let args_list = {
            let mut out = LispObject::nil();
            for l in lists.iter().rev() {
                out = LispObject::cons(l[i].clone(), out);
            }
            out
        };
        let r = call_function(
            obj_to_value(pred.clone()),
            obj_to_value(args_list),
            env,
            editor,
            macros,
            state,
        )?;
        let obj = value_to_obj(r);
        if truthy(&obj) {
            return Ok(obj);
        }
    }
    Ok(LispObject::nil())
}

pub fn cl_every(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let pred = args.first().unwrap_or(LispObject::nil());
    let lists: Vec<Vec<LispObject>> = {
        let mut out = Vec::new();
        let mut cur = args.rest().unwrap_or(LispObject::nil());
        while let Some((list, rest)) = cur.destructure_cons() {
            out.push(to_vec(&list));
            cur = rest;
        }
        out
    };
    if lists.is_empty() {
        return Ok(LispObject::t());
    }
    let len = lists.iter().map(Vec::len).min().unwrap_or(0);
    for i in 0..len {
        let args_list = {
            let mut out = LispObject::nil();
            for l in lists.iter().rev() {
                out = LispObject::cons(l[i].clone(), out);
            }
            out
        };
        let r = call_function(
            obj_to_value(pred.clone()),
            obj_to_value(args_list),
            env,
            editor,
            macros,
            state,
        )?;
        if !truthy(&value_to_obj(r)) {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::t())
}

pub fn cl_notany(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let r = cl_some(args, env, editor, macros, state)?;
    Ok(LispObject::from(!truthy(&r)))
}

pub fn cl_notevery(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let r = cl_every(args, env, editor, macros, state)?;
    Ok(LispObject::from(!truthy(&r)))
}

pub fn cl_reduce(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (cl-reduce FN SEQ &key :initial-value :start :end :from-end :key)
    let (pos, keys) = split_keys(args, 2);
    let func = pos.first().cloned().unwrap_or(LispObject::nil());
    let seq = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let items = to_vec(&seq);
    let mut indices: Vec<usize> = apply_start_end(&items, &keys).collect();
    if keys.from_end {
        indices.reverse();
    }

    let mut acc: Option<LispObject> = keys.initial_value.clone();
    for i in indices {
        let item = items[i].clone();
        let k = keyed(&keys, item, env, editor, macros, state)?;
        acc = Some(match acc {
            None => k,
            Some(a) => {
                if keys.from_end {
                    call2(&func, k, a, env, editor, macros, state)?
                } else {
                    call2(&func, a, k, env, editor, macros, state)?
                }
            }
        });
    }
    Ok(acc.unwrap_or(LispObject::nil()))
}

pub fn cl_mapcar(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (cl-mapcar FN LIST &rest MORE-LISTS)
    let func = args.first().unwrap_or(LispObject::nil());
    let lists: Vec<Vec<LispObject>> = {
        let mut out = Vec::new();
        let mut cur = args.rest().unwrap_or(LispObject::nil());
        while let Some((l, rest)) = cur.destructure_cons() {
            out.push(to_vec(&l));
            cur = rest;
        }
        out
    };
    if lists.is_empty() {
        return Ok(LispObject::nil());
    }
    let len = lists.iter().map(Vec::len).min().unwrap_or(0);
    let mut results: Vec<LispObject> = Vec::with_capacity(len);
    for i in 0..len {
        let args_list = {
            let mut out = LispObject::nil();
            for l in lists.iter().rev() {
                out = LispObject::cons(l[i].clone(), out);
            }
            out
        };
        let r = call_function(
            obj_to_value(func.clone()),
            obj_to_value(args_list),
            env,
            editor,
            macros,
            state,
        )?;
        results.push(value_to_obj(r));
    }
    Ok(from_slice(&results))
}

pub fn cl_mapc(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // Like mapcar but returns first list (Emacs convention). We ignore
    // results.
    let first_list = args.nth(1).unwrap_or(LispObject::nil());
    let _ = cl_mapcar(args, env, editor, macros, state)?;
    Ok(first_list)
}

pub fn cl_mapcan(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // mapcar + nconc.
    let mapped = cl_mapcar(args, env, editor, macros, state)?;
    let mut all: Vec<LispObject> = Vec::new();
    let mut cur = mapped;
    while let Some((car, cdr)) = cur.destructure_cons() {
        for item in to_vec(&car) {
            all.push(item);
        }
        cur = cdr;
    }
    Ok(from_slice(&all))
}

pub fn cl_maplist(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (cl-maplist FN LIST) — like mapcar but passes tails.
    let func = args.first().unwrap_or(LispObject::nil());
    let list = args.nth(1).unwrap_or(LispObject::nil());
    let mut results: Vec<LispObject> = Vec::new();
    let mut cur = list;
    while let Some((_, _cdr)) = cur.clone().destructure_cons() {
        let args_list = LispObject::cons(cur.clone(), LispObject::nil());
        let r = call_function(
            obj_to_value(func.clone()),
            obj_to_value(args_list),
            env,
            editor,
            macros,
            state,
        )?;
        results.push(value_to_obj(r));
        cur = cur.rest().unwrap_or(LispObject::nil());
    }
    Ok(from_slice(&results))
}

pub fn cl_mapl(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let list = args.nth(1).unwrap_or(LispObject::nil());
    let _ = cl_maplist(args, env, editor, macros, state)?;
    Ok(list)
}

pub fn cl_mapcon(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // maplist + nconc.
    let mapped = cl_maplist(args, env, editor, macros, state)?;
    let mut all: Vec<LispObject> = Vec::new();
    let mut cur = mapped;
    while let Some((car, cdr)) = cur.destructure_cons() {
        for item in to_vec(&car) {
            all.push(item);
        }
        cur = cdr;
    }
    Ok(from_slice(&all))
}

pub fn cl_map(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (cl-map RESULT-TYPE FN SEQ &rest MORE-SEQS). We ignore RESULT-TYPE
    // and return a list (common for tests).
    let result_type = args.first().unwrap_or(LispObject::nil());
    let rest = args.rest().unwrap_or(LispObject::nil());
    let listified = cl_mapcar(&rest, env, editor, macros, state)?;
    // If requested type is 'string, convert to string.
    if result_type.as_symbol().as_deref() == Some("string") {
        let mut out = String::new();
        let mut cur = listified;
        while let Some((car, cdr)) = cur.destructure_cons() {
            if let Some(n) = car.as_integer() {
                if let Some(c) = char::from_u32(n as u32) {
                    out.push(c);
                }
            }
            cur = cdr;
        }
        return Ok(LispObject::string(&out));
    }
    Ok(listified)
}

pub fn cl_remove_if(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let pred = pos.first().cloned().unwrap_or(LispObject::nil());
    let list = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let items = to_vec(&list);
    let mut out: Vec<LispObject> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let in_range = range_contains(&items, &keys, i);
        let keep = if in_range {
            let k = keyed(&keys, item.clone(), env, editor, macros, state)?;
            let r = call1(&pred, k, env, editor, macros, state)?;
            !truthy(&r)
        } else {
            true
        };
        if keep {
            out.push(item.clone());
        }
    }
    Ok(from_slice(&out))
}

pub fn cl_remove_if_not(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let pred = pos.first().cloned().unwrap_or(LispObject::nil());
    let list = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let items = to_vec(&list);
    let mut out: Vec<LispObject> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let in_range = range_contains(&items, &keys, i);
        let keep = if in_range {
            let k = keyed(&keys, item.clone(), env, editor, macros, state)?;
            let r = call1(&pred, k, env, editor, macros, state)?;
            truthy(&r)
        } else {
            true
        };
        if keep {
            out.push(item.clone());
        }
    }
    Ok(from_slice(&out))
}

fn apply_start_end(items: &[LispObject], keys: &CommonKeys) -> std::ops::Range<usize> {
    let s = keys.start.unwrap_or(0).max(0) as usize;
    let e = keys
        .end
        .map(|n| n.max(0) as usize)
        .unwrap_or(items.len())
        .min(items.len());
    let s = s.min(e);
    s..e
}

fn range_contains(items: &[LispObject], keys: &CommonKeys, i: usize) -> bool {
    apply_start_end(items, keys).contains(&i)
}

// ---- assoc / rassoc variants ---------------------------------------

pub fn cl_assoc(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let key = pos.first().cloned().unwrap_or(LispObject::nil());
    let alist = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let mut cur = alist;
    while let Some((pair, rest)) = cur.destructure_cons() {
        if let Some((pk, _)) = pair.destructure_cons() {
            let keyed_pk = keyed(&keys, pk.clone(), env, editor, macros, state)?;
            if matches_test(&keys, &key, &keyed_pk, env, editor, macros, state)? {
                return Ok(pair);
            }
        }
        cur = rest;
    }
    Ok(LispObject::nil())
}

pub fn cl_assoc_if(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let pred = pos.first().cloned().unwrap_or(LispObject::nil());
    let alist = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let mut cur = alist;
    while let Some((pair, rest)) = cur.destructure_cons() {
        if let Some((pk, _)) = pair.destructure_cons() {
            let k = keyed(&keys, pk.clone(), env, editor, macros, state)?;
            let r = call1(&pred, k, env, editor, macros, state)?;
            if truthy(&r) {
                return Ok(pair);
            }
        }
        cur = rest;
    }
    Ok(LispObject::nil())
}

pub fn cl_assoc_if_not(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let pred = pos.first().cloned().unwrap_or(LispObject::nil());
    let alist = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let mut cur = alist;
    while let Some((pair, rest)) = cur.destructure_cons() {
        if let Some((pk, _)) = pair.destructure_cons() {
            let k = keyed(&keys, pk.clone(), env, editor, macros, state)?;
            let r = call1(&pred, k, env, editor, macros, state)?;
            if !truthy(&r) {
                return Ok(pair);
            }
        }
        cur = rest;
    }
    Ok(LispObject::nil())
}

pub fn cl_rassoc(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let key = pos.first().cloned().unwrap_or(LispObject::nil());
    let alist = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let mut cur = alist;
    while let Some((pair, rest)) = cur.destructure_cons() {
        if let Some((_, pv)) = pair.destructure_cons() {
            let keyed_pv = keyed(&keys, pv.clone(), env, editor, macros, state)?;
            if matches_test(&keys, &key, &keyed_pv, env, editor, macros, state)? {
                return Ok(pair);
            }
        }
        cur = rest;
    }
    Ok(LispObject::nil())
}

pub fn cl_rassoc_if(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let pred = pos.first().cloned().unwrap_or(LispObject::nil());
    let alist = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let mut cur = alist;
    while let Some((pair, rest)) = cur.destructure_cons() {
        if let Some((_, pv)) = pair.destructure_cons() {
            let k = keyed(&keys, pv.clone(), env, editor, macros, state)?;
            let r = call1(&pred, k, env, editor, macros, state)?;
            if truthy(&r) {
                return Ok(pair);
            }
        }
        cur = rest;
    }
    Ok(LispObject::nil())
}

pub fn cl_rassoc_if_not(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let pred = pos.first().cloned().unwrap_or(LispObject::nil());
    let alist = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let mut cur = alist;
    while let Some((pair, rest)) = cur.destructure_cons() {
        if let Some((_, pv)) = pair.destructure_cons() {
            let k = keyed(&keys, pv.clone(), env, editor, macros, state)?;
            let r = call1(&pred, k, env, editor, macros, state)?;
            if !truthy(&r) {
                return Ok(pair);
            }
        }
        cur = rest;
    }
    Ok(LispObject::nil())
}

// ---- search / mismatch ---------------------------------------------

pub fn cl_search(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (cl-search SEQ1 SEQ2 &key :test :key :start1 :end1 :start2 :end2 :from-end)
    let (pos, keys) = split_keys(args, 2);
    let seq1 = pos.first().cloned().unwrap_or(LispObject::nil());
    let seq2 = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let a = to_vec(&seq1);
    let b = to_vec(&seq2);
    if a.is_empty() {
        return Ok(LispObject::integer(0));
    }
    if a.len() > b.len() {
        return Ok(LispObject::nil());
    }
    let range: Vec<usize> = if keys.from_end {
        (0..=b.len() - a.len()).rev().collect()
    } else {
        (0..=b.len() - a.len()).collect()
    };
    for i in range {
        let mut all_match = true;
        for j in 0..a.len() {
            let x = keyed(&keys, a[j].clone(), env, editor, macros, state)?;
            let y = keyed(&keys, b[i + j].clone(), env, editor, macros, state)?;
            if !matches_test(&keys, &x, &y, env, editor, macros, state)? {
                all_match = false;
                break;
            }
        }
        if all_match {
            return Ok(LispObject::integer(i as i64));
        }
    }
    Ok(LispObject::nil())
}

pub fn cl_mismatch(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let seq1 = pos.first().cloned().unwrap_or(LispObject::nil());
    let seq2 = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let a = to_vec(&seq1);
    let b = to_vec(&seq2);
    let n = a.len().min(b.len());
    for i in 0..n {
        let x = keyed(&keys, a[i].clone(), env, editor, macros, state)?;
        let y = keyed(&keys, b[i].clone(), env, editor, macros, state)?;
        if !matches_test(&keys, &x, &y, env, editor, macros, state)? {
            return Ok(LispObject::integer(i as i64));
        }
    }
    if a.len() == b.len() {
        Ok(LispObject::nil())
    } else {
        Ok(LispObject::integer(n as i64))
    }
}

// ---- tree-equal ----------------------------------------------------

fn cl_tree_equal_rec(
    a: LispObject,
    b: LispObject,
    keys: &CommonKeys,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<bool> {
    match (a.destructure_cons(), b.destructure_cons()) {
        (Some((ac, ad)), Some((bc, bd))) => {
            if !cl_tree_equal_rec(ac, bc, keys, env, editor, macros, state)? {
                return Ok(false);
            }
            cl_tree_equal_rec(ad, bd, keys, env, editor, macros, state)
        }
        (None, None) => {
            let ka = keyed(keys, a, env, editor, macros, state)?;
            let kb = keyed(keys, b, env, editor, macros, state)?;
            matches_test(keys, &ka, &kb, env, editor, macros, state)
        }
        _ => Ok(false),
    }
}

pub fn cl_tree_equal(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let a = pos.first().cloned().unwrap_or(LispObject::nil());
    let b = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let eq = cl_tree_equal_rec(a, b, &keys, env, editor, macros, state)?;
    Ok(LispObject::from(eq))
}

// ---- substitute / nsubstitute --------------------------------------

pub fn cl_substitute(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (cl-substitute NEW OLD SEQ &key :test :key :count :start :end :from-end)
    let (pos, keys) = split_keys(args, 3);
    let new = pos.first().cloned().unwrap_or(LispObject::nil());
    let old = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let seq = pos.get(2).cloned().unwrap_or(LispObject::nil());
    let items = to_vec(&seq);
    let mut remaining = keys.count.unwrap_or(i64::MAX);
    let mut indices: Vec<usize> = apply_start_end(&items, &keys).collect();
    if keys.from_end {
        indices.reverse();
    }
    let mut out = items.clone();
    for i in indices {
        if remaining <= 0 {
            break;
        }
        let k = keyed(&keys, items[i].clone(), env, editor, macros, state)?;
        if matches_test(&keys, &old, &k, env, editor, macros, state)? {
            out[i] = new.clone();
            remaining -= 1;
        }
    }
    Ok(from_slice(&out))
}

pub fn cl_substitute_if(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 3);
    let new = pos.first().cloned().unwrap_or(LispObject::nil());
    let pred = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let seq = pos.get(2).cloned().unwrap_or(LispObject::nil());
    let items = to_vec(&seq);
    let mut remaining = keys.count.unwrap_or(i64::MAX);
    let mut indices: Vec<usize> = apply_start_end(&items, &keys).collect();
    if keys.from_end {
        indices.reverse();
    }
    let mut out = items.clone();
    for i in indices {
        if remaining <= 0 {
            break;
        }
        let k = keyed(&keys, items[i].clone(), env, editor, macros, state)?;
        let r = call1(&pred, k, env, editor, macros, state)?;
        if truthy(&r) {
            out[i] = new.clone();
            remaining -= 1;
        }
    }
    Ok(from_slice(&out))
}

pub fn cl_substitute_if_not(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 3);
    let new = pos.first().cloned().unwrap_or(LispObject::nil());
    let pred = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let seq = pos.get(2).cloned().unwrap_or(LispObject::nil());
    let items = to_vec(&seq);
    let mut remaining = keys.count.unwrap_or(i64::MAX);
    let mut indices: Vec<usize> = apply_start_end(&items, &keys).collect();
    if keys.from_end {
        indices.reverse();
    }
    let mut out = items.clone();
    for i in indices {
        if remaining <= 0 {
            break;
        }
        let k = keyed(&keys, items[i].clone(), env, editor, macros, state)?;
        let r = call1(&pred, k, env, editor, macros, state)?;
        if !truthy(&r) {
            out[i] = new.clone();
            remaining -= 1;
        }
    }
    Ok(from_slice(&out))
}

// ---- subst-if / sublis (tree substitution with predicate) -----------

fn subst_if_rec(
    new: &LispObject,
    pred: &LispObject,
    tree: LispObject,
    keys: &CommonKeys,
    negate: bool,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let k = keyed(keys, tree.clone(), env, editor, macros, state)?;
    let r = call1(pred, k, env, editor, macros, state)?;
    let hit = if negate { !truthy(&r) } else { truthy(&r) };
    if hit {
        return Ok(new.clone());
    }
    match tree.destructure_cons() {
        Some((car, cdr)) => Ok(LispObject::cons(
            subst_if_rec(new, pred, car, keys, negate, env, editor, macros, state)?,
            subst_if_rec(new, pred, cdr, keys, negate, env, editor, macros, state)?,
        )),
        None => Ok(tree),
    }
}

pub fn cl_subst_if(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 3);
    let new = pos.first().cloned().unwrap_or(LispObject::nil());
    let pred = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let tree = pos.get(2).cloned().unwrap_or(LispObject::nil());
    subst_if_rec(&new, &pred, tree, &keys, false, env, editor, macros, state)
}

pub fn cl_subst_if_not(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 3);
    let new = pos.first().cloned().unwrap_or(LispObject::nil());
    let pred = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let tree = pos.get(2).cloned().unwrap_or(LispObject::nil());
    subst_if_rec(&new, &pred, tree, &keys, true, env, editor, macros, state)
}

/// (cl-sublis ALIST TREE &key :test :key) — replace each leaf LEAF
/// that matches `car` of an alist pair with the corresponding `cdr`.
fn sublis_rec(
    alist: &[(LispObject, LispObject)],
    tree: LispObject,
    keys: &CommonKeys,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let k = keyed(keys, tree.clone(), env, editor, macros, state)?;
    for (a, b) in alist {
        if matches_test(keys, &k, a, env, editor, macros, state)? {
            return Ok(b.clone());
        }
    }
    match tree.destructure_cons() {
        Some((car, cdr)) => Ok(LispObject::cons(
            sublis_rec(alist, car, keys, env, editor, macros, state)?,
            sublis_rec(alist, cdr, keys, env, editor, macros, state)?,
        )),
        None => Ok(tree),
    }
}

pub fn cl_sublis(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let (pos, keys) = split_keys(args, 2);
    let alist_obj = pos.first().cloned().unwrap_or(LispObject::nil());
    let tree = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let pairs: Vec<(LispObject, LispObject)> = to_vec(&alist_obj)
        .into_iter()
        .filter_map(|p| p.destructure_cons())
        .collect();
    sublis_rec(&pairs, tree, &keys, env, editor, macros, state)
}

// ---- merge / stable-sort --------------------------------------------

pub fn cl_merge(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (cl-merge TYPE SEQ1 SEQ2 PRED &key :key)
    let (pos, keys) = split_keys(args, 4);
    let _typ = pos.first().cloned().unwrap_or(LispObject::nil());
    let s1 = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let s2 = pos.get(2).cloned().unwrap_or(LispObject::nil());
    let pred = pos.get(3).cloned().unwrap_or(LispObject::nil());
    let a = to_vec(&s1);
    let b = to_vec(&s2);
    let mut out: Vec<LispObject> = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        let ka = keyed(&keys, a[i].clone(), env, editor, macros, state)?;
        let kb = keyed(&keys, b[j].clone(), env, editor, macros, state)?;
        let r = call2(&pred, kb, ka, env, editor, macros, state)?;
        if truthy(&r) {
            out.push(b[j].clone());
            j += 1;
        } else {
            out.push(a[i].clone());
            i += 1;
        }
    }
    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    Ok(from_slice(&out))
}

pub fn cl_stable_sort(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (cl-stable-sort SEQ PRED &key :key)
    let (pos, keys) = split_keys(args, 2);
    let seq = pos.first().cloned().unwrap_or(LispObject::nil());
    let pred = pos.get(1).cloned().unwrap_or(LispObject::nil());
    let items = to_vec(&seq);
    let n = items.len();
    // Pre-compute keyed values so we don't invoke :key per comparison.
    let mut decorated: Vec<(usize, LispObject, LispObject)> = Vec::with_capacity(n);
    for (i, it) in items.iter().enumerate() {
        let k = keyed(&keys, it.clone(), env, editor, macros, state)?;
        decorated.push((i, it.clone(), k));
    }
    // Stable insertion sort so pred is called at most O(n^2) times but
    // preserves original order for equal keys.
    for i in 1..n {
        let mut j = i;
        while j > 0 {
            let r = call2(
                &pred,
                decorated[j].2.clone(),
                decorated[j - 1].2.clone(),
                env,
                editor,
                macros,
                state,
            )?;
            if !truthy(&r) {
                break;
            }
            decorated.swap(j, j - 1);
            j -= 1;
        }
    }
    let sorted: Vec<LispObject> = decorated.into_iter().map(|(_, v, _)| v).collect();
    Ok(from_slice(&sorted))
}

// ---- Dispatch --------------------------------------------------------

/// Returns `Some(result)` if `name` is one of the stateful cl-* names.
pub fn call_stateful_cl(
    name: &str,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> Option<ElispResult<Value>> {
    let result = match name {
        "cl-find" => cl_find(args, env, editor, macros, state),
        "cl-find-if" => cl_find_if(args, env, editor, macros, state),
        "cl-find-if-not" => cl_find_if_not(args, env, editor, macros, state),
        "cl-position-if" => cl_position_if(args, env, editor, macros, state),
        "cl-position-if-not" => cl_find_if_not(args, env, editor, macros, state),
        "cl-count-if" => cl_count_if(args, env, editor, macros, state),
        "cl-count-if-not" => {
            // count-if-not ≡ len - count-if.
            cl_count_if(args, env, editor, macros, state).map(|r| {
                let count = r.as_integer().unwrap_or(0);
                let list = args.nth(1).unwrap_or(LispObject::nil());
                let total = to_vec(&list).len() as i64;
                LispObject::integer(total - count)
            })
        }
        "cl-member-if" => cl_member_if(args, env, editor, macros, state),
        "cl-member-if-not" => cl_member_if_not(args, env, editor, macros, state),
        "cl-some" => cl_some(args, env, editor, macros, state),
        "cl-every" => cl_every(args, env, editor, macros, state),
        "cl-notany" => cl_notany(args, env, editor, macros, state),
        "cl-notevery" => cl_notevery(args, env, editor, macros, state),
        "cl-reduce" => cl_reduce(args, env, editor, macros, state),
        "cl-mapcar" => cl_mapcar(args, env, editor, macros, state),
        "cl-mapc" => cl_mapc(args, env, editor, macros, state),
        "cl-mapcan" => cl_mapcan(args, env, editor, macros, state),
        "cl-maplist" => cl_maplist(args, env, editor, macros, state),
        "cl-mapl" => cl_mapl(args, env, editor, macros, state),
        "cl-mapcon" => cl_mapcon(args, env, editor, macros, state),
        "cl-map" => cl_map(args, env, editor, macros, state),
        "cl-remove-if" => cl_remove_if(args, env, editor, macros, state),
        "cl-remove-if-not" => cl_remove_if_not(args, env, editor, macros, state),
        "cl-delete-if" => cl_remove_if(args, env, editor, macros, state),
        "cl-delete-if-not" => cl_remove_if_not(args, env, editor, macros, state),
        "cl-assoc" => cl_assoc(args, env, editor, macros, state),
        "cl-assoc-if" => cl_assoc_if(args, env, editor, macros, state),
        "cl-assoc-if-not" => cl_assoc_if_not(args, env, editor, macros, state),
        "cl-rassoc" => cl_rassoc(args, env, editor, macros, state),
        "cl-rassoc-if" => cl_rassoc_if(args, env, editor, macros, state),
        "cl-rassoc-if-not" => cl_rassoc_if_not(args, env, editor, macros, state),
        "cl-search" => cl_search(args, env, editor, macros, state),
        "cl-mismatch" => cl_mismatch(args, env, editor, macros, state),
        "cl-tree-equal" => cl_tree_equal(args, env, editor, macros, state),
        "cl-substitute" => cl_substitute(args, env, editor, macros, state),
        "cl-substitute-if" => cl_substitute_if(args, env, editor, macros, state),
        "cl-substitute-if-not" => cl_substitute_if_not(args, env, editor, macros, state),
        "cl-nsubstitute" => cl_substitute(args, env, editor, macros, state),
        "cl-nsubstitute-if" => cl_substitute_if(args, env, editor, macros, state),
        "cl-nsubstitute-if-not" => cl_substitute_if_not(args, env, editor, macros, state),
        "cl-subst-if" => cl_subst_if(args, env, editor, macros, state),
        "cl-subst-if-not" => cl_subst_if_not(args, env, editor, macros, state),
        "cl-nsubst-if" => cl_subst_if(args, env, editor, macros, state),
        "cl-nsubst-if-not" => cl_subst_if_not(args, env, editor, macros, state),
        "cl-sublis" => cl_sublis(args, env, editor, macros, state),
        "cl-nsublis" => cl_sublis(args, env, editor, macros, state),
        "cl-merge" => cl_merge(args, env, editor, macros, state),
        "cl-stable-sort" => cl_stable_sort(args, env, editor, macros, state),
        _ => return None,
    };
    Some(result.map(obj_to_value))
}

pub const STATEFUL_CL_NAMES: &[&str] = &[
    "cl-find",
    "cl-find-if",
    "cl-find-if-not",
    "cl-position-if",
    "cl-position-if-not",
    "cl-count-if",
    "cl-count-if-not",
    "cl-member-if",
    "cl-member-if-not",
    "cl-some",
    "cl-every",
    "cl-notany",
    "cl-notevery",
    "cl-reduce",
    "cl-mapcar",
    "cl-mapc",
    "cl-mapcan",
    "cl-maplist",
    "cl-mapl",
    "cl-mapcon",
    "cl-map",
    "cl-remove-if",
    "cl-remove-if-not",
    "cl-delete-if",
    "cl-delete-if-not",
    "cl-assoc",
    "cl-assoc-if",
    "cl-assoc-if-not",
    "cl-rassoc",
    "cl-rassoc-if",
    "cl-rassoc-if-not",
    "cl-search",
    "cl-mismatch",
    "cl-tree-equal",
    "cl-substitute",
    "cl-substitute-if",
    "cl-substitute-if-not",
    "cl-nsubstitute",
    "cl-nsubstitute-if",
    "cl-nsubstitute-if-not",
    "cl-subst-if",
    "cl-subst-if-not",
    "cl-nsubst-if",
    "cl-nsubst-if-not",
    "cl-sublis",
    "cl-nsublis",
    "cl-merge",
    "cl-stable-sort",
];
