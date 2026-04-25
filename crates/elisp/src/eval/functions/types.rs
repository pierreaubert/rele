//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use super::FALLBACK_STACK;
use super::SyncRefCell as RwLock;
use super::{Environment, InterpreterState, Macro, MacroTable, eval, eval_progn};
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{obj_to_value, value_to_obj};
use std::sync::Arc;

#[derive(Clone)]
pub(super) enum Accumulator {
    List(Vec<LispObject>),
    Num(f64),
    MaxMin(Option<f64>),
}
impl Accumulator {
    pub(super) fn list() -> Self {
        Accumulator::List(Vec::new())
    }
    pub(super) fn num() -> Self {
        Accumulator::Num(0.0)
    }
    pub(super) fn max_min() -> Self {
        Accumulator::MaxMin(None)
    }
    pub(super) fn into_value(self, integer_mode: bool) -> LispObject {
        match self {
            Accumulator::List(items) => {
                let mut out = LispObject::nil();
                for i in items.into_iter().rev() {
                    out = LispObject::cons(i, out);
                }
                out
            }
            Accumulator::Num(n) => {
                if integer_mode && n.fract() == 0.0 {
                    LispObject::integer(n as i64)
                } else {
                    LispObject::float(n)
                }
            }
            Accumulator::MaxMin(Some(n)) => {
                if integer_mode && n.fract() == 0.0 {
                    LispObject::integer(n as i64)
                } else {
                    LispObject::float(n)
                }
            }
            Accumulator::MaxMin(None) => LispObject::nil(),
        }
    }
}
#[derive(Clone)]
pub(super) enum Action {
    Do(LispObject),
    While(LispObject, bool),
    Collect(String, LispObject),
    Append(String, LispObject),
    Nconc(String, LispObject),
    Sum(String, LispObject),
    Count(String, LispObject),
    Max(String, LispObject),
    Min(String, LispObject),
    Return(LispObject),
    Always(LispObject),
    Never(LispObject),
    Thereis(LispObject),
}
/// RAII guard for `FALLBACK_STACK`. Pushes the name on construction
/// and pops on drop — so the stack unwinds correctly even if the
/// evaluation panics inside `catch_unwind` (leaving a stale entry
/// would poison that name for the rest of the interpreter's life).
pub(super) struct FallbackFrame {
    pub(super) name: String,
}
impl FallbackFrame {
    pub(super) fn new(name: &str) -> Self {
        FALLBACK_STACK.with(|st| st.borrow_mut().push(name.to_string()));
        Self {
            name: name.to_string(),
        }
    }
}
pub(super) enum Iter {
    InList(LispObject),
    OnList(LispObject),
    FromTo {
        cur: i64,
        end: i64,
        step: i64,
        inclusive: bool,
        descending: bool,
    },
    Assignment {
        init: LispObject,
        then: Option<LispObject>,
        first: bool,
        value: LispObject,
    },
    Across {
        items: Vec<LispObject>,
        idx: usize,
    },
    Elements {
        items: Vec<LispObject>,
        idx: usize,
    },
    HashKeys {
        items: Vec<LispObject>,
        idx: usize,
    },
    HashValues {
        items: Vec<LispObject>,
        idx: usize,
    },
    Repeat {
        remaining: i64,
    },
}
impl Iter {
    /// Advance this iterator one step, producing the next bound
    /// value (or None when exhausted).
    pub(super) fn next(
        &mut self,
        env: &Arc<RwLock<Environment>>,
        editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
        macros: &MacroTable,
        state: &InterpreterState,
    ) -> ElispResult<Option<LispObject>> {
        match self {
            Iter::InList(list) => {
                if let Some((car, cdr)) = list.clone().destructure_cons() {
                    *list = cdr;
                    Ok(Some(car))
                } else {
                    Ok(None)
                }
            }
            Iter::OnList(list) => {
                if list.destructure_cons().is_some() {
                    let current = list.clone();
                    *list = list.rest().unwrap_or(LispObject::nil());
                    Ok(Some(current))
                } else {
                    Ok(None)
                }
            }
            Iter::FromTo {
                cur,
                end,
                step,
                inclusive,
                descending,
            } => {
                let within = if *descending {
                    if *inclusive {
                        *cur >= *end
                    } else {
                        *cur > *end
                    }
                } else if *inclusive {
                    *cur <= *end
                } else {
                    *cur < *end
                };
                if !within {
                    return Ok(None);
                }
                let v = *cur;
                *cur = if *descending {
                    *cur - *step
                } else {
                    *cur + *step
                };
                Ok(Some(LispObject::integer(v)))
            }
            Iter::Assignment {
                init,
                then,
                first,
                value,
            } => {
                let out = if *first {
                    *first = false;
                    let v = value_to_obj(eval(
                        obj_to_value(init.clone()),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    *value = v.clone();
                    v
                } else {
                    let form = then.clone().unwrap_or_else(|| init.clone());
                    let v = value_to_obj(eval(obj_to_value(form), env, editor, macros, state)?);
                    *value = v.clone();
                    v
                };
                Ok(Some(out))
            }
            Iter::Across { items, idx } | Iter::Elements { items, idx } => {
                if *idx < items.len() {
                    let v = items[*idx].clone();
                    *idx += 1;
                    Ok(Some(v))
                } else {
                    Ok(None)
                }
            }
            Iter::HashKeys { items, idx } | Iter::HashValues { items, idx } => {
                if *idx < items.len() {
                    let v = items[*idx].clone();
                    *idx += 1;
                    Ok(Some(v))
                } else {
                    Ok(None)
                }
            }
            Iter::Repeat { remaining } => {
                if *remaining > 0 {
                    *remaining -= 1;
                    Ok(Some(LispObject::nil()))
                } else {
                    Ok(None)
                }
            }
        }
    }
}
