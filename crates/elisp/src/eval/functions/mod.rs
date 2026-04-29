#![allow(clippy::disallowed_methods)]
//! Auto-generated module structure

// Re-export from parent eval module so sub-files can use `super::X`
pub(super) use super::builtins;
pub(super) use super::dynamic::{bind_param_dynamic, unwind_specpdl};
pub(super) use super::environment::Environment;
pub(super) use super::eval;
pub(super) use super::macro_table::{Macro, MacroTable};
pub(super) use super::special_forms;
pub(super) use super::special_forms::eval_progn;
pub(super) use super::state_cl;
pub(super) use super::sync_cell::SyncRefCell;
pub(super) use super::types::InterpreterState;

pub mod fallbackframe_traits;
#[allow(clippy::module_inception)]
pub mod functions;
pub mod functions_2;
pub mod functions_3;
pub mod functions_4;
pub mod functions_5;
pub mod types;

pub(crate) use functions::{
    SetOperation, assign_symbol_value, deliver_variable_watchers, resolve_variable_alias,
    set_function_cell_checked,
};
pub(crate) use functions_2::{eval_apply, eval_funcall, eval_funcall_form};
pub(crate) use functions_3::{apply_lambda, call_function};
pub(in crate::eval) use functions_4::eval_cl_loop;
