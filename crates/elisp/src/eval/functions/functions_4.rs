//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use super::SyncRefCell as RwLock;
use super::{Environment, InterpreterState, MacroTable, eval};
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::sync::Arc;

use super::functions_5::{as_f64, is_loop_keyword};
use super::types::{Accumulator, Action, Iter};

fn define_loop_pattern(env: &Arc<RwLock<Environment>>, pattern: &LispObject) {
    match pattern {
        LispObject::Symbol(id) => {
            env.write()
                .define(&crate::obarray::symbol_name(*id), LispObject::nil());
        }
        LispObject::Cons(_) => {
            let car = pattern.first().unwrap_or_else(LispObject::nil);
            let cdr = pattern.rest().unwrap_or_else(LispObject::nil);
            define_loop_pattern(env, &car);
            define_loop_pattern(env, &cdr);
        }
        LispObject::Nil => {}
        _ => {}
    }
}

fn bind_loop_pattern(env: &Arc<RwLock<Environment>>, pattern: &LispObject, value: LispObject) {
    match pattern {
        LispObject::Symbol(id) => {
            env.write().set(&crate::obarray::symbol_name(*id), value);
        }
        LispObject::Cons(_) => {
            let pat_car = pattern.first().unwrap_or_else(LispObject::nil);
            let pat_cdr = pattern.rest().unwrap_or_else(LispObject::nil);
            if let Some((val_car, val_cdr)) = value.destructure_cons() {
                bind_loop_pattern(env, &pat_car, val_car);
                bind_loop_pattern(env, &pat_cdr, val_cdr);
            } else {
                bind_loop_pattern(env, &pat_car, LispObject::nil());
                bind_loop_pattern(env, &pat_cdr, LispObject::nil());
            }
        }
        LispObject::Nil => {}
        _ => {}
    }
}

pub(in crate::eval) fn eval_cl_loop(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let clauses_obj = value_to_obj(args);
    let parent = Arc::new(env.read().clone());
    let loop_env = Arc::new(RwLock::new(Environment::with_parent(parent)));
    let mut bindings: Vec<(LispObject, Iter)> = Vec::new();
    let mut actions: Vec<Action> = Vec::new();
    let mut initially: Vec<LispObject> = Vec::new();
    let mut finally: Vec<LispObject> = Vec::new();
    let mut finally_return: Option<LispObject> = None;
    use std::collections::HashMap;
    let mut accs: HashMap<String, (Accumulator, bool)> = HashMap::new();
    let mut default_collect_name: Option<String> = None;
    let mut cur = clauses_obj;
    while let Some((first, rest)) = cur.clone().destructure_cons() {
        let word = first.as_symbol().unwrap_or_default();
        cur = rest;
        match word.as_str() {
            "named" => {
                let (_, r) = cur
                    .clone()
                    .destructure_cons()
                    .unwrap_or((LispObject::nil(), LispObject::nil()));
                cur = r;
            }
            "for" | "as" => {
                let (var_obj, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                define_loop_pattern(&loop_env, &var_obj);
                let (kind_obj, r2) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r2;
                let kind = kind_obj.as_symbol().unwrap_or_default();
                match kind.as_str() {
                    "in" | "in-ref" => {
                        let (list_form, r3) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        let list = value_to_obj(eval(
                            obj_to_value(list_form),
                            env,
                            editor,
                            macros,
                            state,
                        )?);
                        bindings.push((var_obj, Iter::InList(list)));
                    }
                    "on" => {
                        let (list_form, r3) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        let list = value_to_obj(eval(
                            obj_to_value(list_form),
                            env,
                            editor,
                            macros,
                            state,
                        )?);
                        bindings.push((var_obj, Iter::OnList(list)));
                    }
                    "to" | "upto" | "below" | "downto" | "above" => {
                        let (end_form, r3) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        let end =
                            value_to_obj(eval(obj_to_value(end_form), env, editor, macros, state)?)
                                .as_integer()
                                .unwrap_or(0);
                        bindings.push((
                            var_obj,
                            Iter::FromTo {
                                cur: 0,
                                end,
                                step: 1,
                                inclusive: matches!(kind.as_str(), "to" | "upto" | "downto"),
                                descending: matches!(kind.as_str(), "downto" | "above"),
                            },
                        ));
                    }
                    "from" | "upfrom" | "downfrom" => {
                        let (start_form, r3) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        let start = value_to_obj(eval(
                            obj_to_value(start_form),
                            env,
                            editor,
                            macros,
                            state,
                        )?)
                        .as_integer()
                        .unwrap_or(0);
                        let mut descending = kind.as_str() == "downfrom";
                        let mut inclusive = true;
                        let mut end: i64 = if descending { i64::MIN } else { i64::MAX };
                        let mut step: i64 = 1;
                        while let Some((next, r4)) = cur.clone().destructure_cons() {
                            let n = next.as_symbol().unwrap_or_default();
                            match n.as_str() {
                                "to" | "upto" => {
                                    inclusive = true;
                                    descending = false;
                                    let (e_form, r5) = r4
                                        .clone()
                                        .destructure_cons()
                                        .ok_or(ElispError::WrongNumberOfArguments)?;
                                    cur = r5;
                                    end = value_to_obj(eval(
                                        obj_to_value(e_form),
                                        env,
                                        editor,
                                        macros,
                                        state,
                                    )?)
                                    .as_integer()
                                    .unwrap_or(end);
                                }
                                "below" => {
                                    inclusive = false;
                                    descending = false;
                                    let (e_form, r5) = r4
                                        .clone()
                                        .destructure_cons()
                                        .ok_or(ElispError::WrongNumberOfArguments)?;
                                    cur = r5;
                                    end = value_to_obj(eval(
                                        obj_to_value(e_form),
                                        env,
                                        editor,
                                        macros,
                                        state,
                                    )?)
                                    .as_integer()
                                    .unwrap_or(end);
                                }
                                "downto" => {
                                    inclusive = true;
                                    descending = true;
                                    let (e_form, r5) = r4
                                        .clone()
                                        .destructure_cons()
                                        .ok_or(ElispError::WrongNumberOfArguments)?;
                                    cur = r5;
                                    end = value_to_obj(eval(
                                        obj_to_value(e_form),
                                        env,
                                        editor,
                                        macros,
                                        state,
                                    )?)
                                    .as_integer()
                                    .unwrap_or(end);
                                }
                                "above" => {
                                    inclusive = false;
                                    descending = true;
                                    let (e_form, r5) = r4
                                        .clone()
                                        .destructure_cons()
                                        .ok_or(ElispError::WrongNumberOfArguments)?;
                                    cur = r5;
                                    end = value_to_obj(eval(
                                        obj_to_value(e_form),
                                        env,
                                        editor,
                                        macros,
                                        state,
                                    )?)
                                    .as_integer()
                                    .unwrap_or(end);
                                }
                                "by" => {
                                    let (s_form, r5) = r4
                                        .clone()
                                        .destructure_cons()
                                        .ok_or(ElispError::WrongNumberOfArguments)?;
                                    cur = r5;
                                    step = value_to_obj(eval(
                                        obj_to_value(s_form),
                                        env,
                                        editor,
                                        macros,
                                        state,
                                    )?)
                                    .as_integer()
                                    .unwrap_or(1);
                                }
                                _ => break,
                            }
                        }
                        bindings.push((
                            var_obj,
                            Iter::FromTo {
                                cur: start,
                                end,
                                step,
                                inclusive,
                                descending,
                            },
                        ));
                    }
                    "=" => {
                        let (init_form, r3) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        let mut then_form: Option<LispObject> = None;
                        if let Some((tok, r4)) = cur.clone().destructure_cons()
                            && tok.as_symbol().as_deref() == Some("then") {
                                let (n_form, r5) = r4
                                    .clone()
                                    .destructure_cons()
                                    .ok_or(ElispError::WrongNumberOfArguments)?;
                                cur = r5;
                                then_form = Some(n_form);
                            }
                        bindings.push((
                            var_obj,
                            Iter::Assignment {
                                init: init_form,
                                then: then_form,
                                first: true,
                                value: LispObject::nil(),
                            },
                        ));
                    }
                    "across" => {
                        let (seq_form, r3) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        let seq =
                            value_to_obj(eval(obj_to_value(seq_form), env, editor, macros, state)?);
                        let items = match seq {
                            LispObject::Vector(v) => v.lock().clone(),
                            LispObject::String(s) => {
                                s.chars().map(|c| LispObject::integer(c as i64)).collect()
                            }
                            _ => {
                                let mut out = Vec::new();
                                let mut c = seq;
                                while let Some((car, cdr)) = c.destructure_cons() {
                                    out.push(car);
                                    c = cdr;
                                }
                                out
                            }
                        };
                        bindings.push((var_obj, Iter::Across { items, idx: 0 }));
                    }
                    "being" => {
                        let (qualifier, r3) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        let qn = qualifier.as_symbol().unwrap_or_default();
                        cur = r3;
                        let kind_tok = if qn == "the" || qn == "each" {
                            let (k, r4) = cur
                                .clone()
                                .destructure_cons()
                                .ok_or(ElispError::WrongNumberOfArguments)?;
                            cur = r4;
                            k.as_symbol().unwrap_or_default()
                        } else {
                            qn
                        };
                        if let Some((of_tok, r4)) = cur.clone().destructure_cons() {
                            let n = of_tok.as_symbol().unwrap_or_default();
                            if n == "of" || n == "in" {
                                cur = r4;
                            }
                        }
                        let (seq_form, r5) = cur
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r5;
                        let seq =
                            value_to_obj(eval(obj_to_value(seq_form), env, editor, macros, state)?);
                        let items: Vec<LispObject> = match (kind_tok.as_str(), &seq) {
                            ("hash-keys" | "hash-key", LispObject::HashTable(_)) => Vec::new(),
                            ("hash-values" | "hash-value", LispObject::HashTable(ht)) => {
                                ht.lock().data.values().cloned().collect()
                            }
                            _ => match seq {
                                LispObject::Vector(v) => v.lock().clone(),
                                LispObject::String(s) => {
                                    s.chars().map(|c| LispObject::integer(c as i64)).collect()
                                }
                                _ => {
                                    let mut out = Vec::new();
                                    let mut c = seq;
                                    while let Some((car, cdr)) = c.destructure_cons() {
                                        out.push(car);
                                        c = cdr;
                                    }
                                    out
                                }
                            },
                        };
                        bindings.push((var_obj, Iter::Elements { items, idx: 0 }));
                    }
                    _ => {}
                }
            }
            "with" => {
                let (var_obj, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                let var = var_obj
                    .as_symbol()
                    .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
                let mut init_value = LispObject::nil();
                if let Some((eq_tok, r2)) = cur.clone().destructure_cons()
                    && eq_tok.as_symbol().as_deref() == Some("=") {
                        let (init_form, r3) = r2
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        init_value = value_to_obj(eval(
                            obj_to_value(init_form),
                            env,
                            editor,
                            macros,
                            state,
                        )?);
                    }
                loop_env.write().define(&var, init_value);
            }
            "repeat" => {
                let (n_form, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                let n = value_to_obj(eval(obj_to_value(n_form), env, editor, macros, state)?)
                    .as_integer()
                    .unwrap_or(0);
                bindings.push((
                    LispObject::symbol("_repeat_"),
                    Iter::Repeat { remaining: n },
                ));
            }
            "while" | "until" => {
                let is_until = word == "until";
                let (cond, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                actions.push(Action::While(cond, is_until));
            }
            "always" => {
                let (form, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                actions.push(Action::Always(form));
            }
            "never" => {
                let (form, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                actions.push(Action::Never(form));
            }
            "thereis" => {
                let (form, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                actions.push(Action::Thereis(form));
            }
            "do" | "doing" => {
                let mut body: Vec<LispObject> = Vec::new();
                while let Some((form, r)) = cur.clone().destructure_cons() {
                    if is_loop_keyword(&form) {
                        break;
                    }
                    body.push(form);
                    cur = r;
                }
                let mut form = LispObject::symbol("progn");
                let mut chain = LispObject::nil();
                for f in body.into_iter().rev() {
                    chain = LispObject::cons(f, chain);
                }
                form = LispObject::cons(form, chain);
                actions.push(Action::Do(form));
            }
            "collect" | "collecting" | "append" | "appending" | "nconc" | "nconcing" | "sum"
            | "summing" | "count" | "counting" | "maximize" | "maximizing" | "minimize"
            | "minimizing" => {
                let (form, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                let mut acc_name = String::new();
                if let Some((into, r2)) = cur.clone().destructure_cons()
                    && into.as_symbol().as_deref() == Some("into") {
                        let (n_obj, r3) = r2
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r3;
                        acc_name = n_obj.as_symbol().unwrap_or_default();
                    }
                if acc_name.is_empty() {
                    acc_name = format!("__cl_loop_default_{}_", word);
                    default_collect_name = Some(acc_name.clone());
                }
                let kind = word.as_str();
                let acc = match kind {
                    "collect" | "collecting" | "append" | "appending" | "nconc" | "nconcing" => {
                        Accumulator::list()
                    }
                    "maximize" | "maximizing" | "minimize" | "minimizing" => Accumulator::max_min(),
                    _ => Accumulator::num(),
                };
                accs.entry(acc_name.clone()).or_insert((acc, true));
                actions.push(match kind {
                    "collect" | "collecting" => Action::Collect(acc_name, form),
                    "append" | "appending" => Action::Append(acc_name, form),
                    "nconc" | "nconcing" => Action::Nconc(acc_name, form),
                    "sum" | "summing" => Action::Sum(acc_name, form),
                    "count" | "counting" => Action::Count(acc_name, form),
                    "maximize" | "maximizing" => Action::Max(acc_name, form),
                    "minimize" | "minimizing" => Action::Min(acc_name, form),
                    _ => unreachable!(),
                });
            }
            "return" => {
                let (form, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                actions.push(Action::Return(form));
            }
            "initially" => {
                while let Some((form, r)) = cur.clone().destructure_cons() {
                    if is_loop_keyword(&form) {
                        break;
                    }
                    initially.push(form);
                    cur = r;
                }
            }
            "finally" => {
                if let Some((kw, r)) = cur.clone().destructure_cons() {
                    if kw.as_symbol().as_deref() == Some("return") {
                        let (form, r2) = r
                            .clone()
                            .destructure_cons()
                            .ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = r2;
                        finally_return = Some(form);
                        continue;
                    }
                    if kw.as_symbol().as_deref() == Some("do") {
                        cur = r;
                    }
                }
                while let Some((form, r)) = cur.clone().destructure_cons() {
                    if is_loop_keyword(&form) {
                        break;
                    }
                    finally.push(form);
                    cur = r;
                }
            }
            "if" | "when" | "unless" => {
                let (cond, r) = cur
                    .clone()
                    .destructure_cons()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                cur = r;
                if let Some((then_kw, r2)) = cur.clone().destructure_cons()
                    && (then_kw.as_symbol().as_deref() == Some("do")
                        || then_kw.as_symbol().as_deref() == Some("doing"))
                    {
                        cur = r2;
                        let mut body: Vec<LispObject> = Vec::new();
                        while let Some((form, r3)) = cur.clone().destructure_cons() {
                            if is_loop_keyword(&form) {
                                break;
                            }
                            body.push(form);
                            cur = r3;
                        }
                        let mut body_chain = LispObject::nil();
                        for form in body.into_iter().rev() {
                            body_chain = LispObject::cons(form, body_chain);
                        }
                        let progn = LispObject::cons(LispObject::symbol("progn"), body_chain);
                        let conditional = if word == "unless" {
                            LispObject::cons(
                                LispObject::symbol("if"),
                                LispObject::cons(
                                    cond,
                                    LispObject::cons(
                                        LispObject::nil(),
                                        LispObject::cons(progn, LispObject::nil()),
                                    ),
                                ),
                            )
                        } else {
                            LispObject::cons(
                                LispObject::symbol("if"),
                                LispObject::cons(cond, LispObject::cons(progn, LispObject::nil())),
                            )
                        };
                        actions.push(Action::Do(conditional));
                    }
            }
            "and" | "else" | "end" => {}
            _ => {}
        }
    }
    for form in &initially {
        eval(obj_to_value(form.clone()), &loop_env, editor, macros, state)?;
    }
    let mut early_return: Option<LispObject> = None;
    let mut thereis_found: Option<LispObject> = None;
    let mut always_ok = true;
    let mut never_ok = true;
    let mut steps: u64 = 0;
    const MAX_STEPS: u64 = 1 << 26;
    'outer: loop {
        if steps > MAX_STEPS {
            break;
        }
        steps += 1;
        for (pattern, iter) in bindings.iter_mut() {
            let next = iter.next(&loop_env, editor, macros, state)?;
            match next {
                Some(v) => {
                    if pattern.as_symbol().as_deref() != Some("_repeat_") {
                        bind_loop_pattern(&loop_env, pattern, v);
                    }
                }
                None => break 'outer,
            }
        }
        for action in actions.iter() {
            match action {
                Action::Do(form) => {
                    eval(obj_to_value(form.clone()), &loop_env, editor, macros, state)?;
                }
                Action::While(cond, is_until) => {
                    let v = value_to_obj(eval(
                        obj_to_value(cond.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    let truthy = !matches!(v, LispObject::Nil);
                    let should_break = if *is_until { truthy } else { !truthy };
                    if should_break {
                        break 'outer;
                    }
                }
                Action::Collect(name, form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    let entry = accs
                        .entry(name.clone())
                        .or_insert((Accumulator::list(), true));
                    if let Accumulator::List(items) = &mut entry.0 {
                        items.push(v);
                    }
                }
                Action::Append(name, form) | Action::Nconc(name, form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    let entry = accs
                        .entry(name.clone())
                        .or_insert((Accumulator::list(), true));
                    if let Accumulator::List(items) = &mut entry.0 {
                        let mut c = v;
                        while let Some((car, cdr)) = c.destructure_cons() {
                            items.push(car);
                            c = cdr;
                        }
                    }
                }
                Action::Sum(name, form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    let entry = accs
                        .entry(name.clone())
                        .or_insert((Accumulator::num(), true));
                    let n = as_f64(&v);
                    if let Accumulator::Num(acc) = &mut entry.0 {
                        *acc += n;
                    }
                    if matches!(v, LispObject::Float(_)) {
                        entry.1 = false;
                    }
                }
                Action::Count(name, form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    if !matches!(v, LispObject::Nil) {
                        let entry = accs
                            .entry(name.clone())
                            .or_insert((Accumulator::num(), true));
                        if let Accumulator::Num(acc) = &mut entry.0 {
                            *acc += 1.0;
                        }
                    }
                }
                Action::Max(name, form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    let n = as_f64(&v);
                    let entry = accs
                        .entry(name.clone())
                        .or_insert((Accumulator::max_min(), true));
                    if let Accumulator::MaxMin(slot) = &mut entry.0 {
                        *slot = Some(match *slot {
                            Some(cur) => cur.max(n),
                            None => n,
                        });
                    }
                    if matches!(v, LispObject::Float(_)) {
                        entry.1 = false;
                    }
                }
                Action::Min(name, form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    let n = as_f64(&v);
                    let entry = accs
                        .entry(name.clone())
                        .or_insert((Accumulator::max_min(), true));
                    if let Accumulator::MaxMin(slot) = &mut entry.0 {
                        *slot = Some(match *slot {
                            Some(cur) => cur.min(n),
                            None => n,
                        });
                    }
                    if matches!(v, LispObject::Float(_)) {
                        entry.1 = false;
                    }
                }
                Action::Return(form) => {
                    early_return = Some(value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?));
                    break 'outer;
                }
                Action::Always(form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    if matches!(v, LispObject::Nil) {
                        always_ok = false;
                        break 'outer;
                    }
                }
                Action::Never(form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    if !matches!(v, LispObject::Nil) {
                        never_ok = false;
                        break 'outer;
                    }
                }
                Action::Thereis(form) => {
                    let v = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        &loop_env,
                        editor,
                        macros,
                        state,
                    )?);
                    if !matches!(v, LispObject::Nil) {
                        thereis_found = Some(v);
                        break 'outer;
                    }
                }
            }
        }
    }
    {
        let mut w = loop_env.write();
        for (name, (acc, integer_mode)) in &accs {
            if default_collect_name.as_deref() == Some(name.as_str()) {
                continue;
            }
            w.define(name, acc.clone().into_value(*integer_mode));
        }
    }
    for form in &finally {
        eval(obj_to_value(form.clone()), &loop_env, editor, macros, state)?;
    }
    if let Some(v) = early_return {
        return Ok(obj_to_value(v));
    }
    if let Some(form) = finally_return {
        return eval(obj_to_value(form), &loop_env, editor, macros, state);
    }
    if let Some(v) = thereis_found {
        return Ok(obj_to_value(v));
    }
    if !always_ok {
        return Ok(obj_to_value(LispObject::nil()));
    }
    if !never_ok {
        return Ok(obj_to_value(LispObject::nil()));
    }
    let had_always = actions.iter().any(|a| matches!(a, Action::Always(_)));
    let had_never = actions.iter().any(|a| matches!(a, Action::Never(_)));
    if had_always || had_never {
        return Ok(obj_to_value(LispObject::t()));
    }
    if let Some(name) = default_collect_name
        && let Some((acc, integer_mode)) = accs.remove(&name) {
            return Ok(obj_to_value(acc.into_value(integer_mode)));
        }
    Ok(Value::nil())
}
