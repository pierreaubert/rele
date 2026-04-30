// Dynamic binding and special variable handling: unwind-specpdl, bind_param_dynamic.

use super::SyncRefCell as RwLock;
use crate::obarray::SymbolId;
use crate::object::LispObject;
use std::sync::Arc;

use super::{Environment, InterpreterState};

pub(super) fn unwind_specpdl(state: &InterpreterState, depth: usize) {
    let global = &state.global_env;
    let mut specpdl = state.specpdl.write();
    while specpdl.len() > depth {
        if let Some((id, old)) = specpdl.pop() {
            match old {
                Some(val) => global.write().set_id(id, val),
                None => global.write().unset_id(id),
            }
        }
    }
}
pub(super) fn bind_param_dynamic_id(
    id: SymbolId,
    value: LispObject,
    new_env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) {
    if state.special_vars.read().contains(&id) {
        let global = &state.global_env;
        let old = global.read().get_id(id);
        state.specpdl.write().push((id, old));
        new_env.write().define_id(id, value.clone());
        global.write().set_id(id, value);
    } else {
        new_env.write().define_id(id, value);
    }
}
