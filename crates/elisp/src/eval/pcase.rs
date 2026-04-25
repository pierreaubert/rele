//! `pcase` pattern matching engine.
//!
//! Implements pattern matching for `(pcase EXPR (PAT BODY...)...)`.
//! Each pattern is matched against a value, producing an optional set
//! of bindings on success. Patterns supported:
//!
//! - `_`           — wildcard, always matches
//! - SYMBOL        — variable capture (binds SYMBOL to value)
//! - `'VAL`        — literal match via `equal`
//! - `(pred FN)`   — predicate match
//! - `(guard EXPR)` — guard expression
//! - `(and PAT...)` — all sub-patterns must match
//! - `(or PAT...)`  — any sub-pattern matches
//! - `(let PAT EXPR)` — match PAT against evaluated EXPR
//! - `(app FN PAT)` — match PAT against `(funcall FN val)`
//! - `(cl-type TYPE)` — type check via `cl-typep`
//! - `` `(A . B) `` — cons destructure (backquoted)
//! - `[PAT...]`    — vector destructure
//! - INTEGER / STRING — literal equality

use super::SyncRefCell as RwLock;
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::sync::Arc;

use super::functions::call_function;
use super::{Environment, InterpreterState, MacroTable, eval, eval_progn};

/// A successful pattern match produces a list of (name, value) bindings.
type Bindings = Vec<(String, LispObject)>;

/// Try to match `pattern` against `value`. Returns `Some(bindings)` on
/// success, `None` on mismatch.
pub(super) fn pcase_match(
    pattern: &LispObject,
    value: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Option<Bindings>> {
    match pattern {
        // Wildcard: _ always matches, no bindings.
        LispObject::Symbol(id) => {
            let name = crate::obarray::symbol_name(*id);
            match name.as_str() {
                "_" | "otherwise" => Ok(Some(Vec::new())),
                "t" => Ok(Some(Vec::new())),
                "nil" => {
                    // nil pattern matches only nil values
                    if value.is_nil() {
                        Ok(Some(Vec::new()))
                    } else {
                        Ok(None)
                    }
                }
                _ => {
                    // Named symbol: bind it to value (variable capture).
                    // But first check if it's a keyword — keywords are
                    // literal matches, not captures.
                    if name.starts_with(':') {
                        if let Some(sym) = value.as_symbol() {
                            if sym == name {
                                return Ok(Some(Vec::new()));
                            }
                        }
                        Ok(None)
                    } else {
                        Ok(Some(vec![(name, value.clone())]))
                    }
                }
            }
        }

        // Integer / float literal match
        LispObject::Integer(n) => {
            if value.as_integer() == Some(*n) {
                Ok(Some(Vec::new()))
            } else {
                Ok(None)
            }
        }
        LispObject::Float(f) => {
            if value.as_float() == Some(*f) {
                Ok(Some(Vec::new()))
            } else {
                Ok(None)
            }
        }

        // String literal match
        LispObject::String(s) => {
            if let Some(vs) = value.as_string() {
                if *vs == **s {
                    Ok(Some(Vec::new()))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }

        // Vector pattern: [PAT1 PAT2 ...]
        LispObject::Vector(arc) => {
            let pats = arc.lock();
            match value {
                LispObject::Vector(varc) => {
                    let vals = varc.lock();
                    if pats.len() != vals.len() {
                        return Ok(None);
                    }
                    let mut all_bindings = Vec::new();
                    for (pat, val) in pats.iter().zip(vals.iter()) {
                        match pcase_match(pat, val, env, editor, macros, state)? {
                            Some(b) => all_bindings.extend(b),
                            None => return Ok(None),
                        }
                    }
                    Ok(Some(all_bindings))
                }
                _ => Ok(None),
            }
        }

        // Cons pattern — the main compound pattern dispatch
        LispObject::Cons(_) => {
            let car = pattern.first().unwrap_or(LispObject::nil());
            let cdr = pattern.rest().unwrap_or(LispObject::nil());

            // (quote VAL) — literal equality
            if car.as_symbol().as_deref() == Some("quote") {
                let quoted = cdr.first().unwrap_or(LispObject::nil());
                if values_equal(value, &quoted) {
                    return Ok(Some(Vec::new()));
                }
                return Ok(None);
            }

            if let Some(sym) = car.as_symbol() {
                match sym.as_str() {
                    // (pred FN) — predicate
                    "pred" => {
                        let fn_form = cdr.first().unwrap_or(LispObject::nil());
                        return match_pred(&fn_form, value, env, editor, macros, state);
                    }

                    // (guard EXPR) — guard expression
                    "guard" => {
                        let expr = cdr.first().unwrap_or(LispObject::nil());
                        let result = eval(obj_to_value(expr), env, editor, macros, state)?;
                        if result.is_nil() {
                            return Ok(None);
                        }
                        return Ok(Some(Vec::new()));
                    }

                    // (and PAT...) — all must match
                    "and" => {
                        let mut all_bindings = Vec::new();
                        let mut cur = cdr;
                        while let Some((sub_pat, rest)) = cur.destructure_cons() {
                            match pcase_match(&sub_pat, value, env, editor, macros, state)? {
                                Some(b) => all_bindings.extend(b),
                                None => return Ok(None),
                            }
                            cur = rest;
                        }
                        return Ok(Some(all_bindings));
                    }

                    // (or PAT...) — first that matches
                    "or" => {
                        let mut cur = cdr;
                        while let Some((sub_pat, rest)) = cur.destructure_cons() {
                            if let Some(b) =
                                pcase_match(&sub_pat, value, env, editor, macros, state)?
                            {
                                return Ok(Some(b));
                            }
                            cur = rest;
                        }
                        return Ok(None);
                    }

                    // (let PAT EXPR) — match PAT against evaluated EXPR
                    "let" => {
                        let inner_pat = cdr.first().unwrap_or(LispObject::nil());
                        let expr = cdr.nth(1).unwrap_or(LispObject::nil());
                        let val =
                            value_to_obj(eval(obj_to_value(expr), env, editor, macros, state)?);
                        return pcase_match(&inner_pat, &val, env, editor, macros, state);
                    }

                    // (app FN PAT) — match PAT against (funcall FN value)
                    "app" => {
                        let fn_form = cdr.first().unwrap_or(LispObject::nil());
                        let inner_pat = cdr.nth(1).unwrap_or(LispObject::nil());
                        let fn_val = eval(obj_to_value(fn_form), env, editor, macros, state)?;
                        let result = call_function(
                            fn_val,
                            obj_to_value(LispObject::cons(value.clone(), LispObject::nil())),
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let result_obj = value_to_obj(result);
                        return pcase_match(&inner_pat, &result_obj, env, editor, macros, state);
                    }

                    // (cl-type TYPE) — type check
                    "cl-type" => {
                        let type_name = cdr.first().unwrap_or(LispObject::nil());
                        let type_sym = if let Some(s) = type_name.as_symbol() {
                            s
                        } else if let Some(q) = type_name.as_quote_content() {
                            q.as_symbol().unwrap_or_default()
                        } else {
                            return Ok(None);
                        };
                        if cl_typep_check(value, &type_sym) {
                            return Ok(Some(Vec::new()));
                        }
                        return Ok(None);
                    }

                    // (cl-struct STRUCT-TYPE &rest FIELD-PATS) — struct field match
                    "cl-struct" => {
                        return match_cl_struct(&cdr, value, env, editor, macros, state);
                    }

                    // (seq &rest PATS) — map/plist pattern (simplified)
                    "seq" | "map" => {
                        // Simplified: treat as always-match with no bindings
                        return Ok(Some(Vec::new()));
                    }

                    // Backquoted cons pattern: (\, X) or (\,@ X)
                    "," => {
                        // Unquote inside a backquoted pcase pattern.
                        // The sub-form is a pattern variable capture.
                        let sub = cdr.first().unwrap_or(LispObject::nil());
                        return pcase_match(&sub, value, env, editor, macros, state);
                    }

                    "`" => {
                        // Backquoted pattern — recursively destructure
                        let inner = cdr.first().unwrap_or(LispObject::nil());
                        return match_backquoted(&inner, value, env, editor, macros, state);
                    }

                    _ => {}
                }
            }

            // Cons destructuring: (PAT-CAR . PAT-CDR) matches a cons cell
            match value {
                LispObject::Cons(_) => {
                    let val_car = value.first().unwrap_or(LispObject::nil());
                    let val_cdr = value.rest().unwrap_or(LispObject::nil());
                    let mut bindings = Vec::new();
                    match pcase_match(&car, &val_car, env, editor, macros, state)? {
                        Some(b) => bindings.extend(b),
                        None => return Ok(None),
                    }
                    match pcase_match(&cdr, &val_cdr, env, editor, macros, state)? {
                        Some(b) => bindings.extend(b),
                        None => return Ok(None),
                    }
                    Ok(Some(bindings))
                }
                _ => Ok(None),
            }
        }

        // Nil pattern matches nil
        LispObject::Nil => {
            if value.is_nil() {
                Ok(Some(Vec::new()))
            } else {
                Ok(None)
            }
        }

        // Any other pattern type — no match
        _ => Ok(None),
    }
}

/// Match a `(pred FN)` pattern. FN can be a symbol, a lambda, or a
/// `(not FN)` form.
fn match_pred(
    fn_form: &LispObject,
    value: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Option<Bindings>> {
    // (pred (not FN)) — negate
    if let Some((car, cdr)) = fn_form.destructure_cons() {
        if car.as_symbol().as_deref() == Some("not") {
            let inner_fn = cdr.first().unwrap_or(LispObject::nil());
            let result = match_pred(&inner_fn, value, env, editor, macros, state)?;
            return Ok(if result.is_some() {
                None
            } else {
                Some(Vec::new())
            });
        }
    }

    let fn_val = eval(obj_to_value(fn_form.clone()), env, editor, macros, state)
        .unwrap_or(obj_to_value(fn_form.clone()));
    let args = LispObject::cons(value.clone(), LispObject::nil());
    match call_function(fn_val, obj_to_value(args), env, editor, macros, state) {
        Ok(result) => {
            if result.is_nil() {
                Ok(None)
            } else {
                Ok(Some(Vec::new()))
            }
        }
        Err(_) => Ok(None),
    }
}

/// Match a backquoted pattern — recursive structural match where
/// unquoted parts (`,PAT`) are pattern variables and everything else
/// is literal.
fn match_backquoted(
    pattern: &LispObject,
    value: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Option<Bindings>> {
    match pattern {
        LispObject::Cons(_) => {
            let car = pattern.first().unwrap_or(LispObject::nil());

            // (\, SUB) — unquote: SUB is a pcase pattern
            if car.as_symbol().as_deref() == Some(",") {
                let sub = pattern
                    .rest()
                    .and_then(|r| r.first())
                    .unwrap_or(LispObject::nil());
                return pcase_match(&sub, value, env, editor, macros, state);
            }

            // Otherwise structural cons match
            match value {
                LispObject::Cons(_) => {
                    let cdr = pattern.rest().unwrap_or(LispObject::nil());
                    let val_car = value.first().unwrap_or(LispObject::nil());
                    let val_cdr = value.rest().unwrap_or(LispObject::nil());
                    let mut bindings = Vec::new();
                    match match_backquoted(&car, &val_car, env, editor, macros, state)? {
                        Some(b) => bindings.extend(b),
                        None => return Ok(None),
                    }
                    match match_backquoted(&cdr, &val_cdr, env, editor, macros, state)? {
                        Some(b) => bindings.extend(b),
                        None => return Ok(None),
                    }
                    Ok(Some(bindings))
                }
                _ => Ok(None),
            }
        }
        // Non-cons in backquoted context: literal equality
        _ => {
            if values_equal(value, pattern) {
                Ok(Some(Vec::new()))
            } else {
                Ok(None)
            }
        }
    }
}

/// Match a `(cl-struct TYPE FIELD PAT ...)` pattern.
/// Checks the type tag at index 0 of a vector/record.
fn match_cl_struct(
    args: &LispObject,
    value: &LispObject,
    _env: &Arc<RwLock<Environment>>,
    _editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    _macros: &MacroTable,
    _state: &InterpreterState,
) -> ElispResult<Option<Bindings>> {
    let type_name = args.first().unwrap_or(LispObject::nil());
    let type_sym = type_name.as_symbol().unwrap_or_default();

    // Value must be a vector/record with matching type tag at index 0.
    match value {
        LispObject::Vector(varc) => {
            let vals = varc.lock();
            if vals.is_empty() {
                return Ok(None);
            }
            if vals[0].as_symbol().as_deref() != Some(&type_sym) {
                return Ok(None);
            }
            // TODO: match field patterns by looking up slot indices
            // from the struct definition. For now, just confirm the tag.
            Ok(Some(Vec::new()))
        }
        _ => Ok(None),
    }
}

/// Deep equality check matching Emacs `equal`.
fn values_equal(a: &LispObject, b: &LispObject) -> bool {
    match (a, b) {
        (LispObject::Nil, LispObject::Nil) => true,
        (LispObject::Integer(x), LispObject::Integer(y)) => x == y,
        (LispObject::Float(x), LispObject::Float(y)) => x == y,
        (LispObject::String(x), LispObject::String(y)) => **x == **y,
        (LispObject::Symbol(x), LispObject::Symbol(y)) => x == y,
        (LispObject::Cons(_), LispObject::Cons(_)) => {
            let a_car = a.first().unwrap_or(LispObject::nil());
            let a_cdr = a.rest().unwrap_or(LispObject::nil());
            let b_car = b.first().unwrap_or(LispObject::nil());
            let b_cdr = b.rest().unwrap_or(LispObject::nil());
            values_equal(&a_car, &b_car) && values_equal(&a_cdr, &b_cdr)
        }
        (LispObject::Vector(x), LispObject::Vector(y)) => {
            let xg = x.lock();
            let yg = y.lock();
            xg.len() == yg.len() && xg.iter().zip(yg.iter()).all(|(a, b)| values_equal(a, b))
        }
        _ => false,
    }
}

/// Simple `cl-typep` check for pcase `(cl-type TYPE)` patterns.
fn cl_typep_check(value: &LispObject, type_name: &str) -> bool {
    match type_name {
        "integer" | "fixnum" => value.as_integer().is_some(),
        "float" => value.as_float().is_some(),
        "number" => value.as_integer().is_some() || value.as_float().is_some(),
        "string" => value.as_string().is_some(),
        "symbol" => value.as_symbol().is_some(),
        "cons" | "pair" => matches!(value, LispObject::Cons(_)),
        "list" => value.is_nil() || matches!(value, LispObject::Cons(_)),
        "vector" => matches!(value, LispObject::Vector(_)),
        "array" => matches!(value, LispObject::Vector(_)) || value.as_string().is_some(),
        "hash-table" => matches!(value, LispObject::HashTable(_)),
        "null" => value.is_nil(),
        "boolean" => {
            value.is_nil()
                || matches!(value, LispObject::Symbol(id) if crate::obarray::symbol_name(*id) == "t")
        }
        "keyword" => {
            if let Some(s) = value.as_symbol() {
                s.starts_with(':')
            } else {
                false
            }
        }
        "atom" => !matches!(value, LispObject::Cons(_)),
        _ => false,
    }
}

/// Evaluate `(pcase EXPR CLAUSES...)` with full pattern matching.
pub(super) fn eval_pcase(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let expr_form = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let expr_val = value_to_obj(eval(obj_to_value(expr_form), env, editor, macros, state)?);
    let clauses = args_obj.rest().unwrap_or(LispObject::nil());

    let mut cur = clauses;
    while let Some((clause, rest)) = cur.destructure_cons() {
        let (pattern, body) = match clause.destructure_cons() {
            Some(pair) => pair,
            None => {
                cur = rest;
                continue;
            }
        };

        match pcase_match(&pattern, &expr_val, env, editor, macros, state)? {
            Some(bindings) => {
                // Create a new env with the bindings
                if bindings.is_empty() {
                    return eval_progn(obj_to_value(body), env, editor, macros, state);
                }
                let parent = Arc::new(env.read().clone());
                let new_env = Arc::new(RwLock::new(super::Environment::with_parent(parent)));
                for (name, val) in bindings {
                    new_env.write().define(&name, val);
                }
                return eval_progn(obj_to_value(body), &new_env, editor, macros, state);
            }
            None => {
                cur = rest;
            }
        }
    }

    // No clause matched
    Ok(Value::nil())
}

/// Evaluate `(pcase-let ((PAT EXPR)...) BODY...)` with destructuring.
/// When `sequential` is true (`pcase-let*`), each binding's init-form
/// sees the previous bindings. When false (`pcase-let`), all init-forms
/// are evaluated in the original environment.
pub(super) fn eval_pcase_let(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
    sequential: bool,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let bindings_form = args_obj.first().unwrap_or(LispObject::nil());
    let body = args_obj.rest().unwrap_or(LispObject::nil());

    let parent = Arc::new(env.read().clone());
    let new_env = Arc::new(RwLock::new(super::Environment::with_parent(parent)));

    let mut binding_cur = bindings_form;
    while let Some((binding, rest)) = binding_cur.destructure_cons() {
        if let Some((pat, val_rest)) = binding.destructure_cons() {
            let val_form = val_rest.first().unwrap_or(LispObject::nil());
            // pcase-let*: evaluate in new_env so prior bindings are visible.
            // pcase-let: evaluate in original env.
            let eval_env = if sequential { &new_env } else { env };
            let val = value_to_obj(eval(
                obj_to_value(val_form),
                eval_env,
                editor,
                macros,
                state,
            )?);

            match pcase_match(&pat, &val, eval_env, editor, macros, state)? {
                Some(binds) => {
                    for (name, v) in binds {
                        new_env.write().define(&name, v);
                    }
                }
                None => {
                    // Pattern didn't match — bind pattern as symbol if possible
                    if let Some(name) = pat.as_symbol() {
                        new_env.write().define(&name, val);
                    }
                }
            }
        }
        binding_cur = rest;
    }

    eval_progn(obj_to_value(body), &new_env, editor, macros, state)
}

/// Evaluate `(pcase-dolist (PAT LIST [RESULT]) BODY...)`.
pub(super) fn eval_pcase_dolist(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let spec = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().unwrap_or(LispObject::nil());
    let pat = spec.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list_form = spec.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let result_form = spec.nth(2);
    let list = value_to_obj(eval(obj_to_value(list_form), env, editor, macros, state)?);

    let mut cur = list;
    while let Some((item, rest)) = cur.destructure_cons() {
        if let Some(bindings) = pcase_match(&pat, &item, env, editor, macros, state)? {
            let parent = Arc::new(env.read().clone());
            let loop_env = Arc::new(RwLock::new(super::Environment::with_parent(parent)));
            for (name, value) in bindings {
                loop_env.write().define(&name, value);
            }
            eval_progn(obj_to_value(body.clone()), &loop_env, editor, macros, state)?;
        }
        cur = rest;
    }

    if let Some(result) = result_form {
        eval(obj_to_value(result), env, editor, macros, state)
    } else {
        Ok(Value::nil())
    }
}
