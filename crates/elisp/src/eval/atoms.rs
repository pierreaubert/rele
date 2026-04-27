use super::{Environment, InterpreterState, RwLock};
use crate::error::{ElispError, ElispResult};
use crate::obarray::SymbolId;
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::sync::Arc;

pub(super) fn eval_atom(
    expr: Value,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> Option<ElispResult<Value>> {
    if expr.is_fixnum() || expr.is_float() || expr.is_nil() || expr.is_t() {
        return Some(Ok(expr));
    }

    if let Some(raw) = expr.as_symbol_id() {
        let sym_id = SymbolId(raw);
        let name = crate::obarray::symbol_name(sym_id);
        if name.starts_with(':') {
            return Some(Ok(expr));
        }
        if state.special_vars.read().contains(&sym_id) {
            if state.specpdl.read().iter().any(|(id, _)| *id == sym_id) {
                // Special variables use dynamic scope: always read from
                // global, never from the captured env. The env may hold a
                // stale value if this lookup is happening inside a closure
                // that captured an earlier `let` frame.
                let global = state.global_env.read();
                return Some(match global.get_id(sym_id) {
                    Some(val) if val.is_unbound_marker() => Err(ElispError::VoidVariable(name)),
                    Some(val) => Ok(obj_to_value(val)),
                    None => Err(ElispError::VoidVariable(name)),
                });
            }
            if let Some(local) = current_buffer_local(&name) {
                if local.is_unbound_marker() {
                    return Some(Err(ElispError::VoidVariable(name)));
                }
                return Some(Ok(obj_to_value(local)));
            }
            let global = state.global_env.read();
            return Some(match global.get_id(sym_id) {
                Some(val) if val.is_unbound_marker() => Err(ElispError::VoidVariable(name)),
                Some(val) => Ok(obj_to_value(val)),
                None => Err(ElispError::VoidVariable(name)),
            });
        }

        let env = env.read();
        if let Some(val) = env.get_id_local(sym_id) {
            if let Some(captured) = state.get_closure_mutation(sym_id)
                && captured != val
            {
                return Some(Ok(obj_to_value(captured)));
            }
            if val.is_unbound_marker() {
                return Some(Err(ElispError::VoidVariable(name)));
            }
            return Some(Ok(obj_to_value(val)));
        }
        if let Some(local) = current_buffer_local(&name) {
            if local.is_unbound_marker() {
                return Some(Err(ElispError::VoidVariable(name)));
            }
            return Some(Ok(obj_to_value(local)));
        }
        return Some(match state.get_value_cell(sym_id) {
            Some(val) if val.is_unbound_marker() => Err(ElispError::VoidVariable(name)),
            Some(val) => Ok(obj_to_value(val)),
            None => Err(ElispError::VoidVariable(name)),
        });
    }

    let expr_obj = value_to_obj(expr);
    match expr_obj {
        LispObject::String(_)
        | LispObject::Primitive(_)
        | LispObject::Vector(_)
        | LispObject::BytecodeFn(_)
        | LispObject::HashTable(_) => Some(Ok(obj_to_value(expr_obj))),
        LispObject::Cons(_) => None,
        _ => Some(Ok(expr)),
    }
}

fn current_buffer_local(name: &str) -> Option<LispObject> {
    crate::buffer::with_registry(|registry| {
        registry
            .get(registry.current_id())
            .and_then(|buffer| buffer.locals.get(name).cloned())
    })
}
