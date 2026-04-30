//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use super::SyncRefCell as RwLock;
use super::{Environment, InterpreterState, Macro, MacroTable, eval};
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::sync::Arc;

use super::functions_2::{
    stateful_add_to_list, stateful_apply, stateful_cl_generic_define,
    stateful_cl_generic_define_method, stateful_cl_generic_generalizers,
    stateful_def_edebug_elem_spec, stateful_defvar_1, stateful_eval, stateful_featurep,
    stateful_funcall, stateful_function_get, stateful_function_put, stateful_get, stateful_provide,
    stateful_put, stateful_require, stateful_setplist, symbol_id_including_constants,
    symbol_name_including_constants,
};
use super::functions_3::{apply_lambda, call_function};
use super::functions_5::{
    stateful_autoload_do_load, stateful_buffer_local_value, stateful_default_toplevel_value,
    stateful_ert_get_test, stateful_ert_test_boundp, stateful_indirect_function,
    stateful_load_history_filename_element,
};
use super::types::FallbackFrame;
use std::collections::HashSet;

/// Phase 7a: **stateful primitives** — functions that are traditionally
/// implemented as special forms in the source-level dispatch (because
/// they need env/macros/state access) but are *semantically* regular
/// functions with evaluated arguments. When a bytecode function from
/// the VM (or any other caller that resolves through the symbol's
/// function cell) invokes `defalias`, `fset`, `eval`, `funcall`,
/// `apply`, etc., we route through this table instead of the regular
/// stateless `primitives::call_primitive` — the caller has already
/// evaluated the arguments, so we must not re-evaluate.
///
/// Returns `Some(result)` when `name` matches; `None` when the caller
/// should fall through to the regular primitive dispatch.
pub(crate) fn call_stateful_primitive(
    name: &str,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> Option<ElispResult<LispObject>> {
    match name {
        "defalias" => Some(stateful_defalias(args, macros, state)),
        "fset" => Some(stateful_fset(args, state)),
        "eval" => Some(stateful_eval(args, env, editor, macros, state)),
        "funcall" | "funcall-interactively" => {
            Some(stateful_funcall(args, env, editor, macros, state))
        }
        "apply" => Some(stateful_apply(args, env, editor, macros, state)),
        "put" => Some(stateful_put(args, state)),
        "get" => Some(stateful_get(args, state)),
        "provide" => Some(stateful_provide(args, state)),
        "featurep" => Some(stateful_featurep(args, state)),
        "require" => Some(stateful_require(args, env, editor, macros, state)),
        "function-put" => Some(stateful_function_put(args, state)),
        "function-get" => Some(stateful_function_get(args, state)),
        "setplist" => Some(stateful_setplist(args, state)),
        "def-edebug-elem-spec" => Some(stateful_def_edebug_elem_spec(args, state)),
        "defvar-1" => Some(stateful_defvar_1(args, env, state)),
        "cl-generic-generalizers" => Some(stateful_cl_generic_generalizers(args, state)),
        "cl-generic-define" => Some(stateful_cl_generic_define(args)),
        "cl-generic-define-method" => Some(stateful_cl_generic_define_method(args, state)),
        "add-to-list" | "add-to-ordered-list" => Some(stateful_add_to_list(args, env, state)),
        "boundp" => Some(stateful_boundp(args, env, state)),
        "char-equal" => Some(stateful_char_equal(args, env, state)),
        "fboundp" => Some(stateful_fboundp(args, env, state, macros)),
        "intern" => Some(stateful_intern(args)),
        "intern-soft" => Some(stateful_intern_soft(args, env, state)),
        "set" => Some(stateful_set(args, env, editor, macros, state)),
        "easy-menu-do-define" => Some(stateful_easy_menu_do_define(
            args, env, editor, macros, state,
        )),
        "symbol-value" => Some(stateful_symbol_value(args, env, state)),
        "symbol-function" => Some(stateful_symbol_function(args, env, macros, state)),
        "default-value" => Some(stateful_default_value(args, state)),
        "default-boundp" => Some(stateful_default_boundp(args, state)),
        "set-default" => Some(stateful_set_default(args, env, editor, macros, state)),
        "make-local-variable" => Some(stateful_make_local_variable(args, env, state)),
        "make-variable-buffer-local" => Some(stateful_make_variable_buffer_local(args, state)),
        "local-variable-p" | "local-variable-if-set-p" => Some(stateful_local_variable_p(args)),
        "variable-binding-locus" => Some(stateful_variable_binding_locus(args)),
        "kill-local-variable" => Some(stateful_kill_local_variable(
            args, env, editor, macros, state,
        )),
        "kill-all-local-variables" => Some(stateful_kill_all_local_variables(
            args, env, editor, macros, state,
        )),
        "special-variable-p" => Some(stateful_special_variable_p(args, state)),
        "type-of" => Some(stateful_type_of(args, state)),
        "cl-type-of" => Some(stateful_cl_type_of(args, state)),
        "makunbound" => Some(stateful_makunbound(args, env, editor, macros, state)),
        "fmakunbound" => Some(stateful_fmakunbound(args, state)),
        "make-hash-table" => Some(stateful_make_hash_table(args, state)),
        "make-closure" => Some(stateful_make_closure(args)),
        "vector" => Some(stateful_vector(args, state)),
        "format" | "format-message" | "message" => {
            Some(stateful_format(args, env, editor, macros, state))
        }
        "read-char"
        | "read-char-exclusive"
        | "read-event"
        | "read-string"
        | "read-from-minibuffer"
        | "read-no-blanks-input"
        | "yes-or-no-p"
        | "y-or-n-p" => Some(stateful_interaction_read(
            name, args, env, editor, macros, state,
        )),
        "find-file-name-handler" => Some(stateful_find_file_name_handler(args, env, state)),
        "make-process" => Some(stateful_make_process(args, env, editor, macros, state)),
        "make-pipe-process" => Some(stateful_make_pipe_process(args)),
        "start-process" => Some(stateful_start_process(args)),
        "process-live-p" => Some(stateful_process_live_p(args)),
        "process-status" => Some(stateful_process_status(args)),
        "process-exit-status" => Some(stateful_process_exit_status(args)),
        "process-name" => Some(stateful_process_name(args)),
        "process-buffer" => Some(stateful_process_buffer(args)),
        "set-process-buffer" => Some(stateful_set_process_buffer(args)),
        "process-sentinel" => Some(stateful_process_sentinel(args)),
        "set-process-sentinel" => Some(stateful_set_process_sentinel(args)),
        "process-filter" => Some(stateful_process_filter(args)),
        "set-process-filter" => Some(stateful_set_process_filter(args)),
        "set-process-query-on-exit-flag" | "process-kill-without-query" => {
            Some(Ok(args.first().unwrap_or_else(LispObject::nil)))
        }
        "process-send-string" | "send-string" => Some(stateful_process_send_string(args)),
        "accept-process-output" => Some(stateful_accept_process_output(
            args, env, editor, macros, state,
        )),
        "delete-process" => Some(stateful_delete_process(args)),
        "upcase-region" | "downcase-region" | "capitalize-region" => {
            Some(stateful_case_region(args, env, editor, macros, state))
        }
        "network-lookup-address-info" => Some(stateful_network_lookup_address_info(args)),
        "unify-charset" => Some(stateful_unify_charset(args)),
        "throw" => Some(stateful_throw(args)),
        "signal" => Some(stateful_signal(args)),
        "error" => Some(stateful_error(args, env, editor, macros, state)),
        "indirect-function" => Some(stateful_indirect_function(args, state)),
        "command-modes" => Some(stateful_command_modes(args, state)),
        "add-variable-watcher" => Some(stateful_add_variable_watcher(args, state)),
        "remove-variable-watcher" => Some(stateful_remove_variable_watcher(args, state)),
        "get-variable-watchers" => Some(stateful_get_variable_watchers(args, state)),
        "default-toplevel-value" => Some(stateful_default_toplevel_value(args, state)),
        "set-default-toplevel-value" => Some(stateful_set_default_toplevel_value(args, state)),
        "buffer-local-toplevel-value" => Some(stateful_buffer_local_toplevel_value(args)),
        "set-buffer-local-toplevel-value" => Some(stateful_set_buffer_local_toplevel_value(args)),
        "buffer-local-value" => Some(stateful_buffer_local_value(args, state)),
        "ert-get-test" => Some(stateful_ert_get_test(args, state)),
        "ert-test-boundp" => Some(stateful_ert_test_boundp(args, state)),
        "autoload-do-load" => Some(stateful_autoload_do_load(args, state)),
        "load-history-filename-element" => {
            Some(stateful_load_history_filename_element(args, state))
        }
        "documentation" => Some(stateful_documentation(args, state)),
        "documentation-property" => Some(stateful_documentation_property(args, state)),
        "describe-function" => Some(stateful_describe_function(args, state)),
        "describe-buffer-bindings" => Some(stateful_describe_buffer_bindings(
            args, env, editor, macros, state,
        )),
        "frame-or-buffer-changed-p" => Some(stateful_frame_or_buffer_changed_p(
            args, env, editor, macros, state,
        )),
        "any" => Some(stateful_any(args, env, editor, macros, state)),
        "substitute-command-keys" => Some(stateful_substitute_command_keys(args, state)),
        _ => {
            if let Some(r) =
                super::super::keymap_forms::call_stateful_keymap_primitive(name, args, state)
            {
                return Some(r);
            }
            if let Some(r) = super::super::metadata::call_metadata_primitive(name, args, state) {
                return Some(r);
            }
            if let Some(r) =
                crate::primitives_category::call_category_primitive(name, args, Some(state))
            {
                return Some(r);
            }
            if let Some(r) = crate::primitives_buffer::call_buffer_primitive(name, args) {
                return Some(r);
            }
            if let Some(r) = crate::primitives_window::call_window_primitive(name, args) {
                return Some(r);
            }
            if let Some(r) = crate::primitives_file::call_file_primitive(name, args, state) {
                return Some(r);
            }
            if let Some(r) = crate::primitives_eieio::call_eieio_primitive(name, args, Some(state))
            {
                return Some(r);
            }
            if let Some(r) =
                super::state_cl::call_stateful_cl(name, args, env, editor, macros, state)
            {
                return Some(r.map(value_to_obj));
            }
            if let Some(r) = crate::primitives_cl::call_cl_primitive(name, args) {
                return Some(r);
            }
            None
        }
    }
}

fn signal_with_data(symbol: &str, data: LispObject) -> ElispError {
    ElispError::Signal(Box::new(crate::error::SignalData {
        symbol: LispObject::symbol(symbol),
        data,
    }))
}

fn signal_error(message: &str) -> ElispError {
    signal_with_data(
        "error",
        LispObject::cons(LispObject::string(message), LispObject::nil()),
    )
}

fn docstring_from_body(body: LispObject) -> Option<String> {
    body.first().and_then(|doc| doc.as_string().cloned())
}

fn docstring_from_function(def: &LispObject) -> Option<String> {
    match def {
        LispObject::BytecodeFn(bytecode) => bytecode.docstring.clone(),
        LispObject::Primitive(_) => Some(String::new()),
        LispObject::Cons(_) => {
            let head = def.car()?.as_symbol()?;
            match head.as_str() {
                "lambda" => docstring_from_body(def.rest()?.rest().unwrap_or_else(LispObject::nil)),
                "closure" => {
                    let rest = def.rest()?;
                    docstring_from_body(rest.rest()?.rest().unwrap_or_else(LispObject::nil))
                }
                "macro" => def
                    .nth(1)
                    .and_then(|lambda| docstring_from_function(&lambda)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn stateful_documentation(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let target = args.first().unwrap_or_else(LispObject::nil);
    if target.is_nil() {
        return Ok(LispObject::nil());
    }

    if let Some(id) = target.as_symbol_id() {
        let prop_doc = state.get_plist(id, crate::obarray::intern("function-documentation"));
        if let Some(doc) = prop_doc.as_string() {
            return Ok(LispObject::string(doc));
        }
        if let Some(def) = state.get_function_cell(id)
            && let Some(doc) = docstring_from_function(&def)
        {
            return Ok(LispObject::string(&doc));
        }
        return Ok(LispObject::nil());
    }

    Ok(docstring_from_function(&target)
        .map(|doc| LispObject::string(&doc))
        .unwrap_or_else(LispObject::nil))
}

fn stateful_documentation_property(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let Some(sym) = args.first().and_then(|value| value.as_symbol_id()) else {
        return Ok(LispObject::nil());
    };
    let Some(prop) = args.nth(1).and_then(|value| value.as_symbol_id()) else {
        return Ok(LispObject::nil());
    };
    let value = state.get_plist(sym, prop);
    if value.is_nil() && crate::obarray::symbol_name(prop) == "function-documentation" {
        let doc_args = LispObject::cons(LispObject::Symbol(sym), LispObject::nil());
        return stateful_documentation(&doc_args, state);
    }
    Ok(value)
}

fn stateful_describe_function(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let target = args.first().unwrap_or_else(LispObject::nil);
    if let Some(id) = target.as_symbol_id()
        && state.get_function_cell(id).is_none()
    {
        return Ok(LispObject::nil());
    }
    Ok(LispObject::nil())
}

fn key_event_description(event: &LispObject) -> String {
    match event {
        LispObject::Integer(n) => {
            let mut code = *n;
            let meta = (code & 0x0800_0000) != 0;
            if meta {
                code &= !0x0800_0000;
            }
            let base = match code {
                1..=26 => format!("C-{}", char::from_u32((code as u32) + 96).unwrap_or('?')),
                32 => "SPC".to_string(),
                127 => "DEL".to_string(),
                _ => char::from_u32(code as u32)
                    .filter(|ch| !ch.is_control())
                    .map(|ch| ch.to_string())
                    .unwrap_or_else(|| code.to_string()),
            };
            if meta { format!("M-{base}") } else { base }
        }
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => {
            let name = crate::obarray::symbol_name(*id);
            if name.len() == 1 || name.contains('-') {
                name
            } else {
                format!("<{name}>")
            }
        }
        _ => event.princ_to_string(),
    }
}

fn key_description(key: &LispObject) -> String {
    match key {
        LispObject::Vector(items) => items
            .lock()
            .iter()
            .map(key_event_description)
            .collect::<Vec<_>>()
            .join(" "),
        _ => key_event_description(key),
    }
}

fn command_key_description(command: &str, state: &InterpreterState) -> String {
    let command_obj = LispObject::symbol(command);
    let keys = super::super::keymap_forms::command_keys_for_state(state, command_obj);
    keys.first()
        .map(key_description)
        .unwrap_or_else(|| format!("M-x {command}"))
}

fn substitute_command_keys_text(input: &str, state: &InterpreterState) -> String {
    let mut out = String::new();
    let mut i = 0;
    let mut literal_next = false;
    while i < input.len() {
        let rest = &input[i..];
        if let Some(after) = rest.strip_prefix("\\=") {
            literal_next = true;
            i = input.len() - after.len();
        } else if rest.starts_with("\\[")
            && let Some(end) = rest.find(']')
        {
            let original = &rest[..=end];
            if literal_next {
                out.push_str(original);
                literal_next = false;
            } else {
                let command = &rest[2..end];
                out.push_str(&command_key_description(command, state));
            }
            i += end + 1;
        } else if let Some(ch) = rest.chars().next() {
            out.push(ch);
            literal_next = false;
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    out
}

fn stateful_substitute_command_keys(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let input = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(input
        .as_string()
        .map(|text| LispObject::string(&substitute_command_keys_text(text, state)))
        .unwrap_or(input))
}

fn variable_value(
    name: &str,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> LispObject {
    env.read()
        .get(name)
        .or_else(|| state.get_value_cell(crate::obarray::intern(name)))
        .unwrap_or_else(LispObject::nil)
}

fn stateful_char_equal(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let a = args
        .first()
        .and_then(|value| value.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("character".into()))?;
    let b = args
        .nth(1)
        .and_then(|value| value.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("character".into()))?;
    if a == b {
        return Ok(LispObject::t());
    }
    if variable_value("case-fold-search", env, state).is_nil() {
        return Ok(LispObject::nil());
    }
    let Some(a) = char::from_u32(a as u32) else {
        return Ok(LispObject::nil());
    };
    let Some(b) = char::from_u32(b as u32) else {
        return Ok(LispObject::nil());
    };
    Ok(LispObject::from(
        a.to_lowercase().to_string() == b.to_lowercase().to_string(),
    ))
}

fn stateful_interaction_read(
    name: &str,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    if !variable_value("inhibit-interaction", env, state).is_nil() {
        return Err(signal_with_data("inhibited-interaction", LispObject::nil()));
    }
    run_hook_by_name("minibuffer-setup-hook", env, editor, macros, state)?;
    match name {
        "read-string" => Ok(read_string_result(args)),
        "read-from-minibuffer" => Ok(read_from_minibuffer_result(args)),
        "read-no-blanks-input" => Ok(read_no_blanks_input_result(args)),
        _ => Ok(LispObject::nil()),
    }
}

fn read_initial_string(value: LispObject) -> LispObject {
    match value {
        LispObject::String(_) => value,
        LispObject::Cons(_) => value.car().unwrap_or_else(LispObject::nil),
        _ => LispObject::nil(),
    }
}

fn run_hook_by_name(
    hook_name: &str,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<()> {
    let mut funcs = variable_value(hook_name, env, state);
    while let Some((func, rest)) = funcs.destructure_cons() {
        let callable = resolve_hook_function(&func, env, state).unwrap_or(func);
        call_function(
            obj_to_value(callable),
            obj_to_value(LispObject::nil()),
            env,
            editor,
            macros,
            state,
        )?;
        funcs = rest;
    }
    Ok(())
}

fn resolve_hook_function(
    func: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> Option<LispObject> {
    let symbol_id = func.as_symbol_id()?;
    state.get_function_cell(symbol_id).or_else(|| {
        let name = crate::obarray::symbol_name(symbol_id);
        env.read().get_function(&name)
    })
}

fn read_string_default(default: LispObject) -> Option<LispObject> {
    if default.is_nil() {
        None
    } else if let Some(first) = default.car() {
        Some(first)
    } else {
        Some(default)
    }
}

fn read_string_result(args: &LispObject) -> LispObject {
    read_string_default(args.nth(3).unwrap_or_else(LispObject::nil))
        .or_else(|| {
            let initial = read_initial_string(args.nth(1).unwrap_or_else(LispObject::nil));
            (!initial.is_nil()).then_some(initial)
        })
        .unwrap_or_else(|| LispObject::string(""))
}

fn read_from_minibuffer_result(args: &LispObject) -> LispObject {
    read_string_default(args.nth(5).unwrap_or_else(LispObject::nil))
        .or_else(|| {
            let initial = read_initial_string(args.nth(1).unwrap_or_else(LispObject::nil));
            (!initial.is_nil()).then_some(initial)
        })
        .unwrap_or_else(|| LispObject::string(""))
}

fn read_no_blanks_input_result(args: &LispObject) -> LispObject {
    let initial = read_initial_string(args.nth(1).unwrap_or_else(LispObject::nil));
    if initial.is_nil() {
        LispObject::string("")
    } else {
        initial
    }
}

struct EvalContext<'a> {
    env: &'a Arc<RwLock<Environment>>,
    editor: &'a Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &'a MacroTable,
    state: &'a InterpreterState,
}

fn stateful_describe_buffer_bindings(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let _buffer = args.first().unwrap_or_else(LispObject::nil);
    let ctx = EvalContext {
        env,
        editor,
        macros,
        state,
    };
    let mut lines = vec![String::from("key             binding\n")];
    let global_map = variable_value("global-map", env, state);
    let mut seen = HashSet::new();
    collect_described_key_bindings(&global_map, None, &mut lines, &ctx, &mut seen)?;
    crate::buffer::with_current_mut(|buffer| buffer.insert(&lines.concat()));
    Ok(LispObject::nil())
}

fn cons_identity(obj: &LispObject) -> Option<usize> {
    match obj {
        LispObject::Cons(cell) => Some(Arc::as_ptr(cell) as usize),
        _ => None,
    }
}

fn is_described_keymap(obj: &LispObject) -> bool {
    obj.car().and_then(|head| head.as_symbol()).as_deref() == Some("keymap")
}

fn described_key_text(key: &LispObject) -> String {
    match key {
        LispObject::String(s) => s.clone(),
        LispObject::Integer(n) => char::from_u32(*n as u32)
            .map(|ch| ch.to_string())
            .unwrap_or_else(|| n.to_string()),
        _ => key.prin1_to_string(),
    }
}

fn combine_described_key(prefix: Option<&str>, key: &LispObject) -> String {
    let key = described_key_text(key);
    match prefix {
        Some(prefix) if !prefix.is_empty() && !key.is_empty() => format!("{prefix} {key}"),
        _ => key,
    }
}

fn menu_item_filter(def: &LispObject) -> Option<LispObject> {
    let mut cur = def.rest()?.rest()?.rest()?;
    while let Some((key, rest)) = cur.destructure_cons() {
        let Some((value, next)) = rest.destructure_cons() else {
            return None;
        };
        if key.as_symbol().as_deref() == Some(":filter") {
            return Some(value);
        }
        cur = next;
    }
    None
}

fn described_binding_command(
    def: &LispObject,
    ctx: &EvalContext<'_>,
) -> ElispResult<Option<LispObject>> {
    if def.car().and_then(|head| head.as_symbol()).as_deref() != Some("menu-item") {
        return Ok((!def.is_nil()).then(|| def.clone()));
    }
    let command = def.nth(2).unwrap_or_else(LispObject::nil);
    if command.is_nil() {
        return Ok(None);
    }
    if let Some(filter) = menu_item_filter(def) {
        let result = value_to_obj(call_function(
            obj_to_value(filter),
            obj_to_value(LispObject::cons(command, LispObject::nil())),
            ctx.env,
            ctx.editor,
            ctx.macros,
            ctx.state,
        )?);
        if result.is_nil() {
            return Ok(None);
        }
        return Ok(Some(result));
    }
    Ok(Some(command))
}

fn described_binding_keymap(def: &LispObject) -> Option<LispObject> {
    if is_described_keymap(def) {
        return Some(def.clone());
    }
    if def.car().and_then(|head| head.as_symbol()).as_deref() == Some("menu-item")
        && let Some(command) = def.nth(2)
        && is_described_keymap(&command)
    {
        return Some(command);
    }
    def.destructure_cons()
        .and_then(|(_, tail)| is_described_keymap(&tail).then_some(tail))
}

fn described_command_text(command: &LispObject) -> Option<String> {
    match command {
        LispObject::Nil => None,
        LispObject::Symbol(_) | LispObject::String(_) | LispObject::Primitive(_) => {
            Some(command.princ_to_string())
        }
        LispObject::BytecodeFn(_) => Some(String::from("#<byte-code-function>")),
        LispObject::Cons(_) => command
            .car()
            .and_then(|head| head.as_symbol())
            .filter(|name| matches!(name.as_str(), "lambda" | "closure"))
            .map(|name| format!("#{name}")),
        _ => None,
    }
}

fn collect_described_key_bindings(
    map: &LispObject,
    prefix: Option<&str>,
    lines: &mut Vec<String>,
    ctx: &EvalContext<'_>,
    seen: &mut HashSet<usize>,
) -> ElispResult<()> {
    if !is_described_keymap(map) {
        return Ok(());
    }
    if let Some(ptr) = cons_identity(map)
        && !seen.insert(ptr)
    {
        return Ok(());
    }
    let mut cur = map.cdr().unwrap_or_else(LispObject::nil);
    while let Some((entry, rest)) = cur.destructure_cons() {
        if let Some(ptr) = cons_identity(&cur)
            && !seen.insert(ptr)
        {
            break;
        }
        if is_described_keymap(&entry) {
            collect_described_key_bindings(&entry, prefix, lines, ctx, seen)?;
        } else if let Some((key, value)) = entry.destructure_cons() {
            if key.as_symbol().as_deref() == Some(":parent") {
                collect_described_key_bindings(&value, prefix, lines, ctx, seen)?;
            } else {
                let key_text = combine_described_key(prefix, &key);
                if let Some(submap) = described_binding_keymap(&value) {
                    collect_described_key_bindings(&submap, Some(&key_text), lines, ctx, seen)?;
                } else if let Some(command) = described_binding_command(&value, ctx)?
                    && let Some(command_text) = described_command_text(&command)
                {
                    lines.push(format!("{key_text:<15} {command_text}\n"));
                }
            }
        }
        cur = rest;
    }
    Ok(())
}

fn stateful_any(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let predicate = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sequence = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut items = Vec::new();
    match sequence {
        LispObject::Cons(_) => {
            let mut cur = sequence;
            while let Some((item, rest)) = cur.destructure_cons() {
                items.push(item);
                cur = rest;
            }
        }
        LispObject::Vector(values) => items.extend(values.lock().iter().cloned()),
        LispObject::String(s) => {
            items.extend(
                s.chars()
                    .map(|ch| LispObject::integer(i64::from(u32::from(ch)))),
            );
        }
        LispObject::Nil => {}
        other => {
            return Err(ElispError::WrongTypeArgument(format!(
                "sequence: {other:?}"
            )));
        }
    }
    for item in items {
        let result = value_to_obj(call_function(
            obj_to_value(predicate.clone()),
            obj_to_value(LispObject::cons(item, LispObject::nil())),
            env,
            editor,
            macros,
            state,
        )?);
        if !result.is_nil() {
            return Ok(result);
        }
    }
    Ok(LispObject::nil())
}

fn stateful_case_region(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let noncontiguous = args.nth(2).unwrap_or_else(LispObject::nil);
    if noncontiguous.is_nil() {
        return Ok(LispObject::nil());
    }
    let extractor = variable_value("region-extract-function", env, state);
    if extractor.is_nil() {
        return Ok(LispObject::nil());
    }
    let call_args = LispObject::cons(LispObject::symbol("bounds"), LispObject::nil());
    let bounds = value_to_obj(call_function(
        obj_to_value(extractor),
        obj_to_value(call_args),
        env,
        editor,
        macros,
        state,
    )?);
    validate_region_bounds(&bounds)?;
    Ok(LispObject::nil())
}

fn validate_region_bounds(bounds: &LispObject) -> ElispResult<()> {
    let mut cur = bounds.clone();
    let mut seen = HashSet::new();
    while let LispObject::Cons(cell) = cur {
        let ptr = std::sync::Arc::as_ptr(&cell) as usize;
        if !seen.insert(ptr) {
            return Err(signal_error("Invalid region-extract-function result"));
        }
        let (item, rest) = {
            let borrowed = cell.lock();
            (borrowed.0.clone(), borrowed.1.clone())
        };
        let Some((beg, end)) = item.destructure_cons() else {
            return Err(signal_error("Invalid region-extract-function result"));
        };
        if beg.as_integer().is_none() || end.as_integer().is_none() {
            return Err(signal_error("Invalid region-extract-function result"));
        }
        cur = rest;
    }
    if cur.is_nil() {
        Ok(())
    } else {
        Err(signal_error("Invalid region-extract-function result"))
    }
}

const PROCESS_TAG: usize = 0;
const PROCESS_NAME: usize = 1;
const PROCESS_BUFFER: usize = 2;
const PROCESS_STATUS: usize = 3;
const PROCESS_EXIT_STATUS: usize = 4;
const PROCESS_SENTINEL: usize = 5;
const PROCESS_FILTER: usize = 6;
const PROCESS_PENDING_OUTPUT: usize = 7;
const PROCESS_PROMPT_COUNT: usize = 8;

fn list_from_vec(items: Vec<LispObject>) -> LispObject {
    items
        .into_iter()
        .rev()
        .fold(LispObject::nil(), |acc, item| LispObject::cons(item, acc))
}

fn plist_get(args: &LispObject, key: &str) -> Option<LispObject> {
    let mut cur = args.clone();
    while let Some((k, rest)) = cur.destructure_cons() {
        let Some((value, next)) = rest.destructure_cons() else {
            break;
        };
        if k.as_symbol().as_deref() == Some(key) {
            return Some(value);
        }
        cur = next;
    }
    None
}

fn first_file_handler(
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> Option<LispObject> {
    let handlers = env
        .read()
        .get("file-name-handler-alist")
        .or_else(|| state.get_value_cell(crate::obarray::intern("file-name-handler-alist")))?;
    let (entry, _) = handlers.destructure_cons()?;
    let (_, handler) = entry.destructure_cons()?;
    Some(handler)
}

fn object_memq(list: &LispObject, needle: &LispObject) -> bool {
    let mut current = list.clone();
    while let Some((item, rest)) = current.destructure_cons() {
        if item == *needle {
            return true;
        }
        current = rest;
    }
    false
}

fn file_handler_pattern_matches(pattern: &str, file: &str) -> bool {
    let rust_pattern = super::super::builtins::emacs_regex_to_rust(pattern);
    regex::Regex::new(&rust_pattern).is_ok_and(|re| re.is_match(file))
}

fn stateful_find_file_name_handler(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let file = args
        .first()
        .and_then(|value| value.as_string().cloned())
        .ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let operation = args.nth(1).unwrap_or_else(LispObject::nil);
    let inhibit_operation = variable_value("inhibit-file-name-operation", env, state);
    let inhibited_handlers = variable_value("inhibit-file-name-handlers", env, state);

    let mut handlers = variable_value("file-name-handler-alist", env, state);
    while let Some((entry, rest)) = handlers.destructure_cons() {
        if let Some((pattern, handler)) = entry.destructure_cons()
            && let Some(pattern) = pattern.as_string()
            && file_handler_pattern_matches(pattern, &file)
        {
            let inhibited =
                operation == inhibit_operation && object_memq(&inhibited_handlers, &handler);
            if !inhibited {
                return Ok(handler);
            }
        }
        handlers = rest;
    }
    Ok(LispObject::nil())
}

fn make_process_object(
    name: LispObject,
    buffer: LispObject,
    status: &str,
    sentinel: LispObject,
    pending_output: LispObject,
) -> LispObject {
    LispObject::Vector(std::sync::Arc::new(crate::eval::SyncRefCell::new(vec![
        LispObject::symbol("process"),
        name,
        buffer,
        LispObject::symbol(status),
        LispObject::integer(0),
        sentinel,
        LispObject::nil(),
        pending_output,
        LispObject::integer(0),
    ])))
}

fn process_slot(process: &LispObject, slot: usize) -> Option<LispObject> {
    let LispObject::Vector(vector) = process else {
        return None;
    };
    let values = vector.read();
    if values
        .get(PROCESS_TAG)
        .and_then(LispObject::as_symbol)
        .as_deref()
        != Some("process")
    {
        return None;
    }
    values.get(slot).cloned()
}

fn set_process_slot(process: &LispObject, slot: usize, value: LispObject) -> bool {
    let LispObject::Vector(vector) = process else {
        return false;
    };
    let mut values = vector.write();
    if values
        .get(PROCESS_TAG)
        .and_then(LispObject::as_symbol)
        .as_deref()
        != Some("process")
    {
        return false;
    }
    if let Some(target) = values.get_mut(slot) {
        *target = value;
        true
    } else {
        false
    }
}

fn stateful_make_process(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    if plist_get(args, ":stop").is_some_and(|value| !value.is_nil()) {
        return Err(signal_error("Invalid process option :stop"));
    }
    if plist_get(args, ":file-handler").is_some_and(|value| !value.is_nil())
        && let Some(handler) = first_file_handler(env, state)
    {
        let funcall_args = LispObject::cons(
            handler,
            LispObject::cons(LispObject::symbol("make-process"), args.clone()),
        );
        return stateful_funcall(&funcall_args, env, editor, macros, state);
    }
    let name = plist_get(args, ":name").unwrap_or_else(|| LispObject::string("process"));
    let buffer_arg = plist_get(args, ":buffer");
    let buffer = buffer_arg.clone().unwrap_or_else(LispObject::nil);
    let sentinel = plist_get(args, ":sentinel").unwrap_or_else(LispObject::nil);
    let pending = if buffer_arg.is_some() {
        LispObject::string("0> ")
    } else {
        LispObject::nil()
    };
    Ok(make_process_object(name, buffer, "run", sentinel, pending))
}

fn stateful_make_pipe_process(args: &LispObject) -> ElispResult<LispObject> {
    let name = plist_get(args, ":name").unwrap_or_else(|| LispObject::string("pipe"));
    let buffer = plist_get(args, ":buffer")
        .unwrap_or_else(|| LispObject::string(&format!("*{}*", name.princ_to_string())));
    let sentinel = plist_get(args, ":sentinel").unwrap_or_else(LispObject::nil);
    Ok(make_process_object(
        name,
        buffer,
        "run",
        sentinel,
        LispObject::nil(),
    ))
}

fn stateful_start_process(args: &LispObject) -> ElispResult<LispObject> {
    let name = args
        .first()
        .unwrap_or_else(|| LispObject::string("process"));
    let buffer = args.nth(1).unwrap_or_else(LispObject::nil);
    let pending = LispObject::string("0> ");
    Ok(make_process_object(
        name,
        buffer,
        "run",
        LispObject::nil(),
        pending,
    ))
}

fn stateful_process_live_p(args: &LispObject) -> ElispResult<LispObject> {
    let process = args.first().unwrap_or_else(LispObject::nil);
    Ok(LispObject::from(
        process_slot(&process, PROCESS_STATUS)
            .as_ref()
            .and_then(LispObject::as_symbol)
            .as_deref()
            == Some("run"),
    ))
}

fn stateful_process_status(args: &LispObject) -> ElispResult<LispObject> {
    let process = args.first().unwrap_or_else(LispObject::nil);
    Ok(process_slot(&process, PROCESS_STATUS).unwrap_or_else(LispObject::nil))
}

fn stateful_process_exit_status(args: &LispObject) -> ElispResult<LispObject> {
    let process = args.first().unwrap_or_else(LispObject::nil);
    Ok(process_slot(&process, PROCESS_EXIT_STATUS).unwrap_or_else(|| LispObject::integer(0)))
}

fn stateful_process_name(args: &LispObject) -> ElispResult<LispObject> {
    let process = args.first().unwrap_or_else(LispObject::nil);
    Ok(process_slot(&process, PROCESS_NAME).unwrap_or_else(LispObject::nil))
}

fn stateful_process_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let process = args.first().unwrap_or_else(LispObject::nil);
    Ok(process_slot(&process, PROCESS_BUFFER).unwrap_or_else(LispObject::nil))
}

fn stateful_set_process_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let process = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let buffer = args.nth(1).unwrap_or_else(LispObject::nil);
    set_process_slot(&process, PROCESS_BUFFER, buffer.clone());
    Ok(buffer)
}

fn stateful_process_sentinel(args: &LispObject) -> ElispResult<LispObject> {
    let process = args.first().unwrap_or_else(LispObject::nil);
    Ok(process_slot(&process, PROCESS_SENTINEL).unwrap_or_else(LispObject::nil))
}

fn stateful_set_process_sentinel(args: &LispObject) -> ElispResult<LispObject> {
    let process = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sentinel = args.nth(1).unwrap_or_else(LispObject::nil);
    set_process_slot(&process, PROCESS_SENTINEL, sentinel);
    Ok(process)
}

fn stateful_process_filter(args: &LispObject) -> ElispResult<LispObject> {
    let process = args.first().unwrap_or_else(LispObject::nil);
    Ok(process_slot(&process, PROCESS_FILTER).unwrap_or_else(LispObject::nil))
}

fn stateful_set_process_filter(args: &LispObject) -> ElispResult<LispObject> {
    let process = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let filter = args.nth(1).unwrap_or_else(LispObject::nil);
    set_process_slot(&process, PROCESS_FILTER, filter);
    Ok(process)
}

fn stateful_process_send_string(args: &LispObject) -> ElispResult<LispObject> {
    let process = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let input = args
        .nth(1)
        .and_then(|value| value.as_string().map(ToString::to_string))
        .unwrap_or_default();
    if process_slot(&process, PROCESS_TAG).is_none() {
        return Ok(LispObject::nil());
    }
    let pending = process_slot(&process, PROCESS_PENDING_OUTPUT)
        .and_then(|value| value.as_string().map(ToString::to_string))
        .unwrap_or_default();
    let count = process_slot(&process, PROCESS_PROMPT_COUNT)
        .and_then(|value| value.as_integer())
        .unwrap_or(0)
        + 1;
    let mut output = pending;
    output.push_str(input.trim_end_matches('\n'));
    output.push('\n');
    output.push_str(&format!("{count}> "));
    set_process_slot(
        &process,
        PROCESS_PENDING_OUTPUT,
        LispObject::string(&output),
    );
    set_process_slot(&process, PROCESS_PROMPT_COUNT, LispObject::integer(count));
    Ok(LispObject::nil())
}

fn stateful_accept_process_output(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let process = args.first().unwrap_or_else(LispObject::nil);
    if process_slot(&process, PROCESS_TAG).is_none() {
        return Ok(LispObject::nil());
    }
    let filter = process_slot(&process, PROCESS_FILTER).unwrap_or_else(LispObject::nil);
    let pending = process_slot(&process, PROCESS_PENDING_OUTPUT)
        .and_then(|value| value.as_string().map(ToString::to_string))
        .unwrap_or_default();
    if !pending.is_empty() {
        if filter.is_t() {
            return Ok(LispObject::nil());
        }
        let insert_form = LispObject::cons(
            LispObject::symbol("insert"),
            LispObject::cons(LispObject::string(&pending), LispObject::nil()),
        );
        eval(obj_to_value(insert_form), env, editor, macros, state)?;
        set_process_slot(&process, PROCESS_PENDING_OUTPUT, LispObject::nil());
        return Ok(LispObject::t());
    }
    if process_slot(&process, PROCESS_STATUS)
        .as_ref()
        .and_then(LispObject::as_symbol)
        .as_deref()
        == Some("run")
    {
        set_process_slot(&process, PROCESS_STATUS, LispObject::symbol("exit"));
        let sentinel = process_slot(&process, PROCESS_SENTINEL).unwrap_or_else(LispObject::nil);
        if !sentinel.is_nil() {
            let funcall_args = list_from_vec(vec![
                sentinel,
                process.clone(),
                LispObject::string("finished\n"),
            ]);
            stateful_funcall(&funcall_args, env, editor, macros, state)?;
            return Ok(LispObject::t());
        }
    }
    Ok(LispObject::nil())
}

fn stateful_delete_process(args: &LispObject) -> ElispResult<LispObject> {
    let process = args.first().unwrap_or_else(LispObject::nil);
    set_process_slot(&process, PROCESS_STATUS, LispObject::symbol("exit"));
    Ok(LispObject::nil())
}

fn valid_network_family(family: &LispObject) -> bool {
    family.is_nil()
        || family
            .as_symbol()
            .is_some_and(|name| matches!(name.as_str(), "ipv4" | "ipv6"))
}

fn parse_numeric_address_component(text: &str) -> Option<u64> {
    if text.is_empty() || text.starts_with('-') || text.starts_with('+') {
        return None;
    }
    let (digits, radix) =
        if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            (hex, 16)
        } else if text.len() > 1 && text.starts_with('0') {
            (text, 8)
        } else {
            (text, 10)
        };
    if digits.is_empty() {
        return None;
    }
    u64::from_str_radix(digits, radix).ok()
}

fn valid_numeric_ipv4(text: &str) -> bool {
    let parts = text.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 4 || parts.iter().any(|part| part.is_empty()) {
        return false;
    }
    if parts.len() == 1 {
        return parse_numeric_address_component(parts[0])
            .is_some_and(|value| value <= u32::MAX as u64);
    }
    parts.iter().all(|part| {
        parse_numeric_address_component(part).is_some_and(|value| value <= u8::MAX as u64)
    })
}

fn valid_numeric_ipv6(text: &str) -> bool {
    if !text.contains(':') || text.contains(":::") {
        return false;
    }
    let compressed = text.contains("::");
    if compressed && text.match_indices("::").count() > 1 {
        return false;
    }
    let parts = text.split(':').collect::<Vec<_>>();
    let non_empty = parts.iter().filter(|part| !part.is_empty()).count();
    if compressed {
        if non_empty >= 8 {
            return false;
        }
    } else if non_empty != 8 {
        return false;
    }
    parts.iter().filter(|part| !part.is_empty()).all(|part| {
        part.len() <= 4
            && part.chars().all(|ch| ch.is_ascii_hexdigit())
            && u16::from_str_radix(part, 16).is_ok()
    })
}

fn stateful_network_lookup_address_info(args: &LispObject) -> ElispResult<LispObject> {
    let address = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let family = args.nth(1).unwrap_or_else(LispObject::nil);
    let hints = args.nth(2).unwrap_or_else(LispObject::nil);
    let address_text = address
        .as_string()
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    if !valid_network_family(&family) {
        return Err(ElispError::WrongTypeArgument("address-family".to_string()));
    }
    if !hints.is_nil()
        && !hints
            .as_symbol()
            .is_some_and(|name| name.as_str() == "numeric")
    {
        return Err(ElispError::WrongTypeArgument(
            "network-lookup-hints".to_string(),
        ));
    }
    if hints.as_symbol().as_deref() == Some("numeric") {
        let family_name = family.as_symbol();
        let valid = match family_name.as_deref() {
            Some("ipv4") => valid_numeric_ipv4(address_text),
            Some("ipv6") => valid_numeric_ipv6(address_text),
            _ => valid_numeric_ipv4(address_text) || valid_numeric_ipv6(address_text),
        };
        if !valid {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::cons(address, LispObject::nil()))
}

fn stateful_unify_charset(args: &LispObject) -> ElispResult<LispObject> {
    let charset = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if charset.as_symbol().as_deref() == Some("ascii") {
        Err(signal_error("Can't unify charset ascii"))
    } else {
        Ok(LispObject::nil())
    }
}

/// `(throw TAG VALUE)` via funcall. Args pre-evaluated.
fn stateful_throw(args: &LispObject) -> ElispResult<LispObject> {
    let tag = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    Err(ElispError::Throw(Box::new(crate::error::ThrowData {
        tag,
        value,
    })))
}
/// `(signal ERROR-SYMBOL DATA)` via funcall. Args pre-evaluated.
fn stateful_signal(args: &LispObject) -> ElispResult<LispObject> {
    let symbol = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let data = args.nth(1).unwrap_or(LispObject::nil());
    Err(ElispError::Signal(Box::new(crate::error::SignalData {
        symbol,
        data,
    })))
}

fn stateful_type_of(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(class_name) = crate::primitives_eieio::object_class_name(&arg)
        && crate::primitives_eieio::get_class_for_state(state, &class_name).is_some()
    {
        return Ok(LispObject::symbol(&class_name));
    }
    if let LispObject::Vector(v) = &arg {
        let guard = v.lock();
        if let Some(first) = guard.first()
            && let Some(sym) = first.as_symbol()
            && (crate::primitives_eieio::get_class_for_state(state, &sym).is_some()
                || crate::primitives::core::records::is_record_tag(&sym))
        {
            return Ok(LispObject::symbol(&sym));
        }
    }
    crate::primitives::call_primitive("type-of", args)
}

fn stateful_cl_type_of(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(class_name) = crate::primitives_eieio::object_class_name(&arg)
        && crate::primitives_eieio::get_class_for_state(state, &class_name).is_some()
    {
        return Ok(LispObject::symbol(&class_name));
    }
    if let LispObject::Vector(v) = &arg {
        let guard = v.lock();
        if let Some(first) = guard.first()
            && let Some(sym) = first.as_symbol()
            && crate::primitives_eieio::get_class_for_state(state, &sym).is_some()
        {
            return Ok(LispObject::symbol(&sym));
        }
    }
    crate::primitives_cl::prim_cl_type_of(args)
}
/// `(error FMT &rest ARGS)` via funcall. Format the message with
/// pre-evaluated args, then signal `error` with it.
fn stateful_error(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let formatted = stateful_format(args, env, editor, macros, state)?;
    let msg = formatted.princ_to_string();
    Err(ElispError::Signal(Box::new(crate::error::SignalData {
        symbol: LispObject::symbol("error"),
        data: LispObject::cons(LispObject::string(&msg), LispObject::nil()),
    })))
}
fn stateful_vector(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let mut items: Vec<LispObject> = Vec::new();
    let mut cur = args.clone();
    while let Some((car, cdr)) = cur.destructure_cons() {
        items.push(car);
        cur = cdr;
    }
    let v = state.heap_vector_from_objects(&items);
    Ok(value_to_obj(v))
}
/// `(format FMT &rest ARGS)` via funcall path. Args are pre-evaluated;
/// we rewrap each as `(quote ARG)` so the existing source-level
/// `eval_format` can treat them as forms to eval — `quote` just
/// returns the wrapped value unchanged.
fn stateful_format(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let quote_sym = LispObject::symbol("quote");
    let mut wrapped: Vec<LispObject> = Vec::new();
    let mut cur = args.clone();
    while let Some((car, cdr)) = cur.destructure_cons() {
        if wrapped.is_empty() {
            wrapped.push(car);
        } else {
            let quoted =
                LispObject::cons(quote_sym.clone(), LispObject::cons(car, LispObject::nil()));
            wrapped.push(quoted);
        }
        cur = cdr;
    }
    let mut form = LispObject::nil();
    for item in wrapped.into_iter().rev() {
        form = LispObject::cons(item, form);
    }
    let result = super::builtins::eval_format(obj_to_value(form), env, editor, macros, state)?;
    Ok(value_to_obj(result))
}
fn stateful_boundp(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = crate::obarray::symbol_name(sym_id);
    if let Some(val) = env.read().get_id_local(sym_id) {
        return Ok(LispObject::from(!val.is_unbound_marker()));
    }
    if let Some(local) = current_buffer_local(&name) {
        return Ok(LispObject::from(!local.is_unbound_marker()));
    }
    Ok(LispObject::from(
        matches!(sym, LispObject::Nil | LispObject::T)
            || state
                .get_value_cell(sym_id)
                .is_some_and(|val| !val.is_unbound_marker())
            || env
                .read()
                .get(&name)
                .is_some_and(|val| !val.is_unbound_marker()),
    ))
}
fn stateful_fboundp(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
    macros: &MacroTable,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if matches!(sym, LispObject::Nil | LispObject::T) {
        return Ok(LispObject::nil());
    }
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = symbol_name_including_constants(&sym)?;
    let found = state.get_function_cell(sym_id).is_some()
        || env.read().get_function(&name).is_some()
        || macros.read().contains_key(&name);
    Ok(LispObject::from(found))
}
fn stateful_intern(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::String(s) => Ok(LispObject::symbol(&s)),
        LispObject::Symbol(_) => Ok(arg),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}
fn stateful_intern_soft(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = match &arg {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Ok(LispObject::nil()),
    };
    if env.read().get(&name).is_some() {
        return Ok(LispObject::symbol(&name));
    }
    let table = crate::obarray::GLOBAL_OBARRAY.read();
    for id in 0..table.symbol_count() as u32 {
        let sid = crate::obarray::SymbolId(id);
        if table.name(sid) == name {
            drop(table);
            let has_value = state.get_value_cell(sid).is_some();
            let has_fn = state.get_function_cell(sid).is_some();
            if has_value || has_fn {
                return Ok(LispObject::Symbol(sid));
            }
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::nil())
}
fn stateful_set(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    assign_symbol_value(
        sym_id,
        val.clone(),
        env,
        editor,
        macros,
        state,
        SetOperation::Set,
    )?;
    Ok(val)
}

/// `(easy-menu-do-define SYMBOL MAPS DOC MENU)` — Emacs implements
/// the menu structure dispatch in a defun in `easymenu.el` that
/// includes `(set symbol keymap)` so callers can later refer to
/// `dired-mode-foo-menu` etc. by name. We don't render menus, but
/// we still need that variable populated so loaders don't get
/// `void-variable` errors. Stash MENU under the symbol's value
/// cell and skip the rest.
pub(super) fn stateful_easy_menu_do_define(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().unwrap_or_else(LispObject::nil);
    let menu = args.nth(3).unwrap_or_else(LispObject::nil);
    if let Some(sym_id) = sym.as_symbol_id() {
        assign_symbol_value(sym_id, menu, env, editor, macros, state, SetOperation::Set)?;
    }
    Ok(LispObject::nil())
}

#[derive(Clone, Copy)]
pub(crate) enum SetOperation {
    Set,
    Setq,
    SetqLocal,
    SetDefault,
    Makunbound,
    Let,
    Unlet,
    Defvaralias,
}

impl SetOperation {
    fn symbol_name(self) -> &'static str {
        match self {
            SetOperation::Set | SetOperation::Setq => "set",
            SetOperation::SetqLocal => "set",
            SetOperation::SetDefault => "set",
            SetOperation::Makunbound => "makunbound",
            SetOperation::Let => "let",
            SetOperation::Unlet => "unlet",
            SetOperation::Defvaralias => "defvaralias",
        }
    }
}

pub(crate) fn resolve_variable_alias(
    sym_id: crate::obarray::SymbolId,
    state: &InterpreterState,
) -> crate::obarray::SymbolId {
    let alias_key = crate::obarray::intern("variable-alias");
    let mut current = sym_id;
    let mut seen = Vec::new();
    loop {
        if seen.contains(&current) {
            return current;
        }
        seen.push(current);
        let target = state.get_plist(current, alias_key);
        let Some(next) = target.as_symbol_id() else {
            return current;
        };
        current = next;
    }
}

fn update_closure_capture(obj: &LispObject, sym_id: crate::obarray::SymbolId, value: &LispObject) {
    let Some((head, rest)) = obj.destructure_cons() else {
        return;
    };
    if head.as_symbol().as_deref() != Some("closure") {
        return;
    }
    let Some(captured_alist) = rest.first() else {
        return;
    };
    let mut cur = captured_alist;
    while let Some((pair, next)) = cur.destructure_cons() {
        if let Some((key, _old_value)) = pair.destructure_cons()
            && key.as_symbol_id() == Some(sym_id)
        {
            pair.set_cdr(value.clone());
        }
        cur = next;
    }
}

fn sync_lexical_assignment_to_closures(
    sym_id: crate::obarray::SymbolId,
    value: &LispObject,
    env: &Arc<RwLock<Environment>>,
) {
    for (_id, binding) in env.read().local_binding_entries() {
        update_closure_capture(&binding, sym_id, value);
    }
}

#[allow(clippy::if_same_then_else)]
pub(crate) fn assign_symbol_value(
    mut sym_id: crate::obarray::SymbolId,
    mut value: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
    operation: SetOperation,
) -> ElispResult<LispObject> {
    if !matches!(operation, SetOperation::Defvaralias) {
        sym_id = resolve_variable_alias(sym_id, state);
    }
    let name = crate::obarray::symbol_name(sym_id);
    validate_symbol_value_assignment(sym_id, &name, &value)?;
    if name == "display-hourglass" {
        value = LispObject::from(!value.is_nil());
    }

    match operation {
        SetOperation::SetqLocal => {
            if state.specpdl.read().iter().any(|(id, _)| *id == sym_id) {
                if env.read().get_id_local(sym_id).is_some() && !Arc::ptr_eq(env, &state.global_env)
                {
                    env.write().set_id(sym_id, value.clone());
                    sync_lexical_assignment_to_closures(sym_id, &value, env);
                }
                state.global_env.write().set_id(sym_id, value.clone());
                state.set_value_cell(sym_id, value.clone());
            } else {
                set_current_buffer_local(&name, value.clone());
            }
        }
        SetOperation::SetDefault => {
            env.write().set_id(sym_id, value.clone());
            sync_lexical_assignment_to_closures(sym_id, &value, env);
            state.global_env.write().set_id(sym_id, value.clone());
            state.set_value_cell(sym_id, value.clone());
        }
        SetOperation::Set | SetOperation::Setq => {
            if state.special_vars.read().contains(&sym_id) {
                if matches!(operation, SetOperation::Set) && has_current_buffer_local(&name) {
                    set_current_buffer_local(&name, value.clone());
                    if state.specpdl.read().iter().any(|(id, _)| *id == sym_id) {
                        set_previous_buffer_local(&name, value.clone());
                    }
                } else if is_automatic_buffer_local(sym_id, state)
                    && state.specpdl.read().iter().any(|(id, _)| *id == sym_id)
                {
                    if dynamic_binding_buffer(sym_id, state).is_some_and(|id| {
                        crate::buffer::with_registry(|registry| registry.current_id()) != id
                    }) {
                        set_current_buffer_local(&name, value.clone());
                    } else {
                        if env.read().get_id_local(sym_id).is_some()
                            && !Arc::ptr_eq(env, &state.global_env)
                        {
                            env.write().set_id(sym_id, value.clone());
                            sync_lexical_assignment_to_closures(sym_id, &value, env);
                        }
                        state.global_env.write().set_id(sym_id, value.clone());
                        state.set_value_cell(sym_id, value.clone());
                    }
                } else if is_automatic_buffer_local(sym_id, state) {
                    set_current_buffer_local(&name, value.clone());
                } else if has_current_buffer_local(&name)
                    && !state.specpdl.read().iter().any(|(id, _)| *id == sym_id)
                {
                    set_current_buffer_local(&name, value.clone());
                } else {
                    if env.read().get_id_local(sym_id).is_some()
                        && !Arc::ptr_eq(env, &state.global_env)
                    {
                        env.write().set_id(sym_id, value.clone());
                        sync_lexical_assignment_to_closures(sym_id, &value, env);
                    }
                    state.global_env.write().set_id(sym_id, value.clone());
                    state.set_value_cell(sym_id, value.clone());
                }
            } else if env.read().get_id_local(sym_id).is_some()
                && !Arc::ptr_eq(env, &state.global_env)
            {
                env.write().set_id(sym_id, value.clone());
                sync_lexical_assignment_to_closures(sym_id, &value, env);
            } else if has_current_buffer_local(&name) || is_automatic_buffer_local(sym_id, state) {
                set_current_buffer_local(&name, value.clone());
            } else {
                env.write().set_id(sym_id, value.clone());
                sync_lexical_assignment_to_closures(sym_id, &value, env);
                state.global_env.write().set_id(sym_id, value.clone());
                state.set_value_cell(sym_id, value.clone());
            }
        }
        SetOperation::Makunbound => {
            deliver_variable_watchers(
                sym_id,
                &LispObject::nil(),
                operation,
                env,
                editor,
                macros,
                state,
            )?;
            if state.specpdl.read().iter().any(|(id, _)| *id == sym_id) {
                if env.read().get_id_local(sym_id).is_some() && !Arc::ptr_eq(env, &state.global_env)
                {
                    env.write().set_id(sym_id, LispObject::unbound_marker());
                }
                state
                    .global_env
                    .write()
                    .set_id(sym_id, LispObject::unbound_marker());
                state.set_value_cell(sym_id, LispObject::unbound_marker());
            } else if has_current_buffer_local(&name) {
                if has_current_buffer_local(&local_toplevel_key(&name)) {
                    set_current_buffer_local(&name, LispObject::unbound_marker());
                } else {
                    remove_current_buffer_local(&name);
                }
            } else {
                state.clear_value_cell(sym_id);
                state.global_env.write().unset_id(sym_id);
            }
        }
        SetOperation::Let | SetOperation::Unlet | SetOperation::Defvaralias => {}
    }

    if !matches!(operation, SetOperation::Makunbound) {
        deliver_variable_watchers(sym_id, &value, operation, env, editor, macros, state)?;
    }
    Ok(value)
}

fn dynamic_binding_buffer(
    id: crate::obarray::SymbolId,
    state: &InterpreterState,
) -> Option<crate::buffer::BufferId> {
    let key = crate::obarray::intern("rele-dynamic-binding-buffer");
    state.get_plist(id, key).as_integer().map(|id| id as usize)
}

fn validate_symbol_value_assignment(
    sym_id: crate::obarray::SymbolId,
    name: &str,
    value: &LispObject,
) -> ElispResult<()> {
    if is_constant_symbol_name(name) && value != &LispObject::Symbol(sym_id) {
        return Err(setting_constant_signal(sym_id));
    }
    if name == "gc-cons-threshold"
        && !matches!(value, LispObject::Integer(_) | LispObject::BigInt(_))
    {
        return Err(ElispError::WrongTypeArgument("integer".to_string()));
    }
    if name == "scroll-up-aggressively" {
        match value {
            LispObject::Nil => {}
            LispObject::Integer(n) if (0..=1).contains(n) => {}
            LispObject::Float(f) if (0.0..=1.0).contains(f) => {}
            _ => return Err(ElispError::WrongTypeArgument("fraction".to_string())),
        }
    }
    if name == "vertical-scroll-bar" && !value.is_nil() {
        match value.as_symbol().as_deref() {
            Some("left" | "right") => {}
            _ => {
                return Err(ElispError::WrongTypeArgument(
                    "vertical-scroll-bar".to_string(),
                ));
            }
        }
    }
    if name == "overwrite-mode" && !value.is_nil() {
        match value.as_symbol().as_deref() {
            Some("overwrite-mode-textual" | "overwrite-mode-binary") => {}
            _ => return Err(ElispError::WrongTypeArgument("overwrite-mode".to_string())),
        }
    }
    Ok(())
}

pub(crate) fn deliver_variable_watchers(
    sym_id: crate::obarray::SymbolId,
    value: &LispObject,
    operation: SetOperation,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<()> {
    let sym_id = if matches!(operation, SetOperation::Defvaralias) {
        sym_id
    } else {
        resolve_variable_alias(sym_id, state)
    };
    let key = crate::obarray::intern("variable-watchers");
    let watchers = list_to_vec(state.get_plist(sym_id, key));
    if watchers.is_empty() {
        return Ok(());
    }
    let name = crate::obarray::symbol_name(sym_id);
    let is_local_operation = !matches!(
        operation,
        SetOperation::SetDefault | SetOperation::Defvaralias
    ) && has_current_buffer_local(&name);
    let where_arg = if is_local_operation && editor.read().is_some() {
        crate::buffer::with_registry(|registry| {
            registry
                .get(registry.current_id())
                .map(|buffer| LispObject::string(&buffer.name))
                .unwrap_or_else(LispObject::nil)
        })
    } else {
        LispObject::nil()
    };
    for watcher in watchers {
        if watcher.is_nil() {
            continue;
        }
        let callback_args = vec_to_list(vec![
            LispObject::Symbol(sym_id),
            value.clone(),
            LispObject::symbol(operation.symbol_name()),
            where_arg.clone(),
        ]);
        call_variable_watcher(watcher, callback_args, env, editor, macros, state)?;
    }
    Ok(())
}

fn call_variable_watcher(
    watcher: LispObject,
    callback_args: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<()> {
    if let Some((head, rest)) = watcher.clone().destructure_cons()
        && let Some(kind) = head.as_symbol()
    {
        match kind.as_str() {
            "lambda" => {
                let params = rest.first().unwrap_or_else(LispObject::nil);
                let body = rest.rest().unwrap_or_else(LispObject::nil);
                if apply_watcher_body_directly(
                    &params,
                    &body,
                    &callback_args,
                    env,
                    editor,
                    macros,
                    state,
                )? {
                    return Ok(());
                }
                apply_lambda(
                    &params,
                    &body,
                    obj_to_value(callback_args),
                    env,
                    editor,
                    macros,
                    state,
                )?;
                return Ok(());
            }
            "closure" => {
                let rest = rest.rest().ok_or(ElispError::WrongNumberOfArguments)?;
                let params = rest.first().unwrap_or_else(LispObject::nil);
                let body = rest.rest().unwrap_or_else(LispObject::nil);
                if apply_watcher_body_directly(
                    &params,
                    &body,
                    &callback_args,
                    env,
                    editor,
                    macros,
                    state,
                )? {
                    return Ok(());
                }
                apply_lambda(
                    &params,
                    &body,
                    obj_to_value(callback_args),
                    env,
                    editor,
                    macros,
                    state,
                )?;
                return Ok(());
            }
            _ => {}
        }
    }
    let args = LispObject::cons(watcher, callback_args);
    let _ = stateful_funcall(&args, env, editor, macros, state)?;
    Ok(())
}

fn apply_watcher_body_directly(
    params: &LispObject,
    body: &LispObject,
    callback_args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<bool> {
    let first_form = body.first().unwrap_or_else(LispObject::nil);
    if !body.rest().unwrap_or_else(LispObject::nil).is_nil() {
        return Ok(false);
    }
    if let Some((head, rest)) = first_form.destructure_cons()
        && let Some(head_name) = head.as_symbol()
    {
        if head_name == "push" {
            let pushed = rest.first().unwrap_or_else(LispObject::nil);
            let place = rest.nth(1).unwrap_or_else(LispObject::nil);
            if let (Some(rest_param), Some(place_id)) =
                (rest_param_name(params), place.as_symbol_id())
                && pushed.as_symbol_id() == Some(rest_param)
            {
                let old = env.read().get_id(place_id).unwrap_or_else(LispObject::nil);
                env.write()
                    .set_id(place_id, LispObject::cons(callback_args.clone(), old));
                return Ok(true);
            }
        }
        if head_name == "setq" && only_rest_params(params) {
            let place = rest.first().unwrap_or_else(LispObject::nil);
            let value_expr = rest.nth(1).unwrap_or_else(LispObject::nil);
            if let Some(place_id) = place.as_symbol_id() {
                let value =
                    value_to_obj(eval(obj_to_value(value_expr), env, editor, macros, state)?);
                env.write().set_id(place_id, value);
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn rest_param_name(params: &LispObject) -> Option<crate::obarray::SymbolId> {
    let mut cur = params.clone();
    while let Some((param, rest)) = cur.destructure_cons() {
        if param.as_symbol().as_deref() == Some("&rest") {
            return rest.first().and_then(|arg| arg.as_symbol_id());
        }
        cur = rest;
    }
    None
}

fn only_rest_params(params: &LispObject) -> bool {
    let cur = params.clone();
    if let Some((param, rest)) = cur.destructure_cons() {
        if param.as_symbol().as_deref() == Some("&rest") {
            return rest.rest().map(|tail| tail.is_nil()).unwrap_or(true);
        }
        return false;
    }
    true
}

pub(crate) fn set_function_cell_checked(
    sym_id: crate::obarray::SymbolId,
    def: LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = crate::obarray::symbol_name(sym_id);
    if is_constant_symbol_name(&name) {
        return Err(setting_constant_signal(sym_id));
    }
    reject_cyclic_function_indirection(sym_id, &def, state)?;
    state.set_function_cell(sym_id, def.clone());
    Ok(def)
}

fn is_constant_symbol_name(name: &str) -> bool {
    matches!(
        name,
        "nil" | "t" | "initial-window-system" | "most-positive-fixnum" | "most-negative-fixnum"
    ) || name.starts_with(':')
}

fn setting_constant_signal(sym_id: crate::obarray::SymbolId) -> ElispError {
    ElispError::Signal(Box::new(crate::error::SignalData {
        symbol: LispObject::symbol("setting-constant"),
        data: LispObject::cons(LispObject::Symbol(sym_id), LispObject::nil()),
    }))
}

fn current_buffer_local(name: &str) -> Option<LispObject> {
    crate::buffer::with_registry(|registry| {
        registry
            .get(registry.current_id())
            .and_then(|buffer| buffer.locals.get(name).cloned())
    })
}

fn has_current_buffer_local(name: &str) -> bool {
    crate::buffer::with_registry(|registry| {
        registry
            .get(registry.current_id())
            .is_some_and(|buffer| buffer.locals.contains_key(name))
    })
}

fn set_current_buffer_local(name: &str, value: LispObject) {
    crate::buffer::with_registry_mut(|registry| {
        let current = registry.current_id();
        if let Some(buffer) = registry.get_mut(current) {
            buffer.locals.insert(name.to_string(), value);
        }
    });
}

fn local_toplevel_key(name: &str) -> String {
    format!("__rele_buffer_local_toplevel::{name}")
}

fn set_previous_buffer_local(name: &str, value: LispObject) {
    crate::buffer::with_registry_mut(|registry| {
        let Some(&previous) = registry.stack.iter().rev().nth(1) else {
            return;
        };
        if let Some(buffer) = registry.get_mut(previous) {
            buffer.locals.insert(name.to_string(), value);
        }
    });
}

fn remove_current_buffer_local(name: &str) -> Option<LispObject> {
    crate::buffer::with_registry_mut(|registry| {
        let current = registry.current_id();
        registry
            .get_mut(current)
            .and_then(|buffer| buffer.locals.remove(name))
    })
}

fn clear_current_buffer_locals() -> Vec<(String, LispObject)> {
    crate::buffer::with_registry_mut(|registry| {
        let current = registry.current_id();
        registry
            .get_mut(current)
            .map(|buffer| buffer.locals.drain().collect())
            .unwrap_or_default()
    })
}

fn is_automatic_buffer_local(id: crate::obarray::SymbolId, state: &InterpreterState) -> bool {
    let key = crate::obarray::intern("variable-buffer-local");
    !state.get_plist(id, key).is_nil()
}

fn stateful_make_local_variable(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = crate::obarray::symbol_name(sym_id);
    let value = current_buffer_local(&name)
        .or_else(|| env.read().get_id_local(sym_id))
        .or_else(|| state.get_value_cell(sym_id))
        .or_else(|| env.read().get(&name))
        .unwrap_or_else(LispObject::nil);
    set_current_buffer_local(&name, value);
    if name == "last-coding-system-used" {
        state.set_value_cell(
            sym_id,
            current_buffer_local(&name).unwrap_or_else(LispObject::nil),
        );
        state.global_env.write().set_id(
            sym_id,
            current_buffer_local(&name).unwrap_or_else(LispObject::nil),
        );
        env.write().unset_id(sym_id);
    }
    Ok(LispObject::Symbol(sym_id))
}

fn stateful_make_variable_buffer_local(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let key = crate::obarray::intern("variable-buffer-local");
    state.put_plist(sym_id, key, LispObject::t());
    Ok(LispObject::Symbol(sym_id))
}

fn stateful_local_variable_p(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = crate::obarray::symbol_name(sym_id);
    Ok(LispObject::from(has_current_buffer_local(&name)))
}

fn stateful_variable_binding_locus(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = crate::obarray::symbol_name(sym_id);
    if has_current_buffer_local(&name) {
        let buffer_name = crate::buffer::with_registry(|registry| {
            registry
                .get(registry.current_id())
                .map(|buffer| buffer.name.clone())
        });
        return Ok(buffer_name
            .map(|name| LispObject::string(&name))
            .unwrap_or_else(LispObject::nil));
    }
    Ok(LispObject::nil())
}

fn stateful_buffer_local_toplevel_value(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = crate::obarray::symbol_name(sym_id);
    current_buffer_local(&local_toplevel_key(&name))
        .or_else(|| current_buffer_local(&name))
        .ok_or(ElispError::VoidVariable(name))
}

fn stateful_set_buffer_local_toplevel_value(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let name = crate::obarray::symbol_name(sym_id);
    if !has_current_buffer_local(&name) {
        return Err(ElispError::VoidVariable(name));
    }
    let key = local_toplevel_key(&name);
    if has_current_buffer_local(&key) {
        set_current_buffer_local(&key, val.clone());
    } else {
        set_current_buffer_local(&name, val.clone());
    }
    Ok(val)
}

fn stateful_kill_local_variable(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    let name = crate::obarray::symbol_name(sym_id);
    if has_current_buffer_local(&name) {
        deliver_variable_watchers(
            sym_id,
            &LispObject::nil(),
            SetOperation::Makunbound,
            env,
            editor,
            macros,
            state,
        )?;
    }
    remove_current_buffer_local(&name);
    Ok(LispObject::Symbol(sym_id))
}

fn stateful_kill_all_local_variables(
    _args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let locals = crate::buffer::with_registry(|registry| {
        registry
            .get(registry.current_id())
            .map(|buffer| buffer.locals.clone())
            .unwrap_or_default()
    });
    for name in locals.keys() {
        let sym_id = crate::obarray::intern(name);
        deliver_variable_watchers(
            sym_id,
            &LispObject::nil(),
            SetOperation::Makunbound,
            env,
            editor,
            macros,
            state,
        )?;
    }
    let _removed = clear_current_buffer_locals();
    Ok(LispObject::nil())
}

fn stateful_set_default(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    assign_symbol_value(
        sym_id,
        val.clone(),
        env,
        editor,
        macros,
        state,
        SetOperation::SetDefault,
    )
}

fn stateful_default_value(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    Ok(state.get_value_cell(sym_id).unwrap_or_else(LispObject::nil))
}

fn stateful_default_boundp(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    Ok(LispObject::from(
        state
            .get_value_cell(sym_id)
            .is_some_and(|val| !val.is_unbound_marker()),
    ))
}

fn stateful_set_default_toplevel_value(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    let key = crate::obarray::intern("default-toplevel-value");
    state.put_plist(sym_id, key, val.clone());
    Ok(val)
}

fn stateful_symbol_value(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    let name = crate::obarray::symbol_name(sym_id);
    if state.special_vars.read().contains(&sym_id)
        && state.specpdl.read().iter().any(|(id, _)| *id == sym_id)
    {
        return match state.global_env.read().get_id(sym_id) {
            Some(val) if val.is_unbound_marker() => Err(ElispError::VoidVariable(name)),
            Some(val) => Ok(val),
            None => Err(ElispError::VoidVariable(name)),
        };
    }
    if state.special_vars.read().contains(&sym_id) {
        return match current_buffer_local(&name).or_else(|| state.global_env.read().get_id(sym_id))
        {
            Some(val) if val.is_unbound_marker() => Err(ElispError::VoidVariable(name)),
            Some(val) => Ok(val),
            None => Err(ElispError::VoidVariable(name)),
        };
    }
    if let Some(val) = env.read().get_id_local(sym_id) {
        if val.is_unbound_marker() {
            return Err(ElispError::VoidVariable(name));
        }
        return Ok(val);
    }
    match current_buffer_local(&name).or_else(|| state.get_value_cell(sym_id)) {
        Some(val) if val.is_unbound_marker() => Err(ElispError::VoidVariable(name)),
        Some(val) => Ok(val),
        None => Ok(LispObject::nil()),
    }
}
fn stateful_symbol_function(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    let name = symbol_name_including_constants(&sym)?;
    if name == "if" {
        return Ok(LispObject::primitive("if"));
    }
    if let Some(val) = state.get_function_cell(sym_id) {
        return Ok(val);
    }
    let func_val = env.read().get_function(&name);
    if let Some(val) = func_val {
        return Ok(val);
    }
    let macro_val = macros.read().get(&name).cloned();
    if let Some(m) = macro_val {
        let args_body = LispObject::cons(m.args, m.body);
        let lambda_form = LispObject::cons(LispObject::symbol("lambda"), args_body);
        return Ok(LispObject::cons(LispObject::symbol("macro"), lambda_form));
    }
    Ok(LispObject::nil())
}
fn stateful_special_variable_p(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&sym)?;
    Ok(LispObject::from(
        state.special_vars.read().contains(&sym_id),
    ))
}
fn stateful_makunbound(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(id) = sym.as_symbol_id() {
        let name = crate::obarray::symbol_name(id);
        if is_makunbound_forbidden(&name) {
            return Err(setting_constant_signal(id));
        }
        assign_symbol_value(
            id,
            LispObject::unbound_marker(),
            env,
            editor,
            macros,
            state,
            SetOperation::Makunbound,
        )?;
    }
    Ok(sym)
}
fn stateful_fmakunbound(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(id) = sym.as_symbol_id() {
        let name = crate::obarray::symbol_name(id);
        if is_constant_symbol_name(&name) {
            return Err(setting_constant_signal(id));
        }
        state.clear_function_cell(id);
    }
    Ok(sym)
}

fn is_makunbound_forbidden(name: &str) -> bool {
    matches!(
        name,
        "nil" | "t" | "initial-window-system" | "most-positive-fixnum" | "most-negative-fixnum"
    ) || name.starts_with(':')
}

fn list_to_vec(list: LispObject) -> Vec<LispObject> {
    let mut out = Vec::new();
    let mut cur = list;
    while let Some((car, cdr)) = cur.destructure_cons() {
        out.push(car);
        cur = cdr;
    }
    out
}

fn vec_to_list(items: Vec<LispObject>) -> LispObject {
    let mut out = LispObject::nil();
    for item in items.into_iter().rev() {
        out = LispObject::cons(item, out);
    }
    out
}

fn stateful_command_modes(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = symbol_id_including_constants(&sym)?;
    Ok(state.get_plist(id, crate::obarray::intern("command-modes")))
}

fn stateful_get_variable_watchers(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    Ok(state.get_plist(id, crate::obarray::intern("variable-watchers")))
}

fn stateful_add_variable_watcher(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let watcher = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    let key = crate::obarray::intern("variable-watchers");
    let mut watchers = list_to_vec(state.get_plist(id, key));
    if !watchers.iter().any(|item| item == &watcher) {
        watchers.push(watcher.clone());
    }
    state.put_plist(id, key, vec_to_list(watchers));
    Ok(watcher)
}

fn stateful_remove_variable_watcher(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let watcher = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let id = resolve_variable_alias(symbol_id_including_constants(&sym)?, state);
    let key = crate::obarray::intern("variable-watchers");
    let watchers = list_to_vec(state.get_plist(id, key))
        .into_iter()
        .filter(|item| item != &watcher)
        .collect();
    state.put_plist(id, key, vec_to_list(watchers));
    Ok(watcher)
}

/// `(make-hash-table &rest KEYWORDS)` when invoked via funcall path.
/// Args are pre-evaluated; we only read `:test`.
fn stateful_make_hash_table(
    args: &LispObject,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let mut test = crate::object::HashTableTest::Eql;
    let mut cur = args.clone();
    while let Some((key, rest)) = cur.destructure_cons() {
        if let Some(s) = key.as_symbol()
            && s == ":test"
            && let Some((val, rest2)) = rest.destructure_cons()
        {
            if let Some(t) = val.as_symbol() {
                test = match t.as_str() {
                    "eq" => crate::object::HashTableTest::Eq,
                    "eql" => crate::object::HashTableTest::Eql,
                    "equal" => crate::object::HashTableTest::Equal,
                    _ => crate::object::HashTableTest::Eql,
                };
            }
            cur = rest2;
            continue;
        }
        cur = rest;
    }
    let v = state.heap_hashtable(crate::object::LispHashTable::new(test));
    Ok(value_to_obj(v))
}

fn frame_buffer_snapshot(state: &InterpreterState) -> LispObject {
    let mut items = vec![
        LispObject::cons(LispObject::symbol("frame"), LispObject::integer(0)),
        LispObject::string("rele"),
    ];

    crate::buffer::with_registry(|registry| {
        let mut ids: Vec<_> = registry.buffers.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            let Some(buffer) = registry.get(id) else {
                continue;
            };
            if buffer.name.starts_with(' ') {
                continue;
            }
            items.push(crate::primitives_buffer::make_buffer_object(id));
            items.push(
                buffer
                    .locals
                    .get("buffer-read-only")
                    .cloned()
                    .unwrap_or_else(LispObject::nil),
            );
            items.push(
                buffer
                    .modified_status
                    .clone()
                    .unwrap_or_else(LispObject::nil),
            );
        }
    });
    items.push(LispObject::symbol("lambda"));

    value_to_obj(state.heap_vector_from_objects(&items))
}

fn frame_buffer_changed_with_internal_state(snapshot: LispObject) -> LispObject {
    LispObject::from(FRAME_BUFFER_STATE.with(|cell| {
        let mut current = cell.borrow_mut();
        if current
            .as_ref()
            .is_some_and(|existing| existing == &snapshot)
        {
            false
        } else {
            *current = Some(snapshot);
            true
        }
    }))
}

fn stateful_frame_or_buffer_changed_p(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    if args.nth(1).is_some() {
        return Err(ElispError::WrongNumberOfArguments);
    }

    let snapshot = frame_buffer_snapshot(state);
    let Some(variable) = args.first() else {
        return Ok(frame_buffer_changed_with_internal_state(snapshot));
    };
    if variable.is_nil() {
        return Ok(frame_buffer_changed_with_internal_state(snapshot));
    }

    let sym_id = variable
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let current = env.read().get_id(sym_id).unwrap_or_else(LispObject::nil);
    if matches!(current, LispObject::Vector(_)) && current == snapshot {
        return Ok(LispObject::nil());
    }

    assign_symbol_value(
        sym_id,
        snapshot,
        env,
        editor,
        macros,
        state,
        SetOperation::Set,
    )?;
    Ok(LispObject::t())
}

thread_local! {
    static FRAME_BUFFER_STATE: std::cell::RefCell < Option < LispObject >> =
        const { std::cell::RefCell::new(None) };
    #[doc = " Names currently inside `source_level_fallback`. If the same"] #[doc =
    " name re-enters, it means the source-level eval dispatch also"] #[doc =
    " funcalls through this name — definitely no implementation; bail"] #[doc =
    " with VoidFunction rather than infinite-looping."] pub(super) static FALLBACK_STACK :
    std::cell::RefCell < Vec < String >> = const { std::cell::RefCell::new(Vec::new()) };
}
pub(super) fn source_level_fallback(
    name: &str,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let already_active = FALLBACK_STACK.with(|st| st.borrow().iter().any(|n| n == name));
    if already_active {
        return Err(ElispError::VoidFunction(name.to_string()));
    }
    let quote_sym = LispObject::symbol("quote");
    let mut quoted_args: Vec<LispObject> = Vec::new();
    let mut cur = args.clone();
    while let Some((arg, rest)) = cur.destructure_cons() {
        let q = LispObject::cons(quote_sym.clone(), LispObject::cons(arg, LispObject::nil()));
        quoted_args.push(q);
        cur = rest;
    }
    let mut arg_list = LispObject::nil();
    for q in quoted_args.into_iter().rev() {
        arg_list = LispObject::cons(q, arg_list);
    }
    let form = LispObject::cons(LispObject::symbol(name), arg_list);
    let _guard = FallbackFrame::new(name);
    eval(obj_to_value(form), env, editor, macros, state)
}
/// `(make-closure PROTOTYPE &rest CLOSURE-VARS)` — Emacs byte-compiled
/// code uses this to construct closures at runtime. We don't have a
/// bytecode prototype model that matches Emacs's; for interpreted code,
/// the PROTOTYPE is usually a lambda already bound to the env that
/// produced it. Simplest correct-enough stub: return PROTOTYPE. Tests
/// that depend on the captured-vars mechanism will fail but no longer
/// "void function".
fn stateful_make_closure(args: &LispObject) -> ElispResult<LispObject> {
    let proto = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(proto)
}
/// `(defalias SYMBOL DEFINITION)` — set SYMBOL's function definition.
/// When DEFINITION is `(macro lambda ARGS . BODY)`, register as a
/// macro in the macros table; otherwise write the obarray function
/// cell. Args are pre-evaluated.
fn stateful_defalias(
    args: &LispObject,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let name_str = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    if let Some((car, rest)) = value.destructure_cons()
        && car.as_symbol().as_deref() == Some("macro")
        && let Some((lambda_sym, lambda_rest)) = rest.destructure_cons()
        && lambda_sym.as_symbol().as_deref() == Some("lambda")
    {
        let macro_args = lambda_rest.first().unwrap_or(LispObject::nil());
        let macro_body = lambda_rest.rest().unwrap_or(LispObject::nil());
        macros.write().insert(
            name_str.clone(),
            Macro {
                args: macro_args,
                body: macro_body,
            },
        );
        return Ok(LispObject::symbol(&name_str));
    }
    let id = crate::obarray::intern(&name_str);
    set_function_cell_checked(id, value, state)?;
    Ok(LispObject::symbol(&name_str))
}
/// `(fset SYMBOL DEFINITION)` — set SYMBOL's function cell. Like
/// `defalias` but doesn't check for the macro form. Args pre-evaluated.
fn stateful_fset(args: &LispObject, state: &InterpreterState) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let def = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = symbol_id_including_constants(&name)?;
    set_function_cell_checked(sym_id, def, state)
}

fn reject_cyclic_function_indirection(
    name: crate::obarray::SymbolId,
    def: &LispObject,
    state: &InterpreterState,
) -> ElispResult<()> {
    let mut current = match def {
        LispObject::Symbol(id) => *id,
        _ => return Ok(()),
    };
    let mut seen = std::collections::HashSet::new();
    while seen.insert(current) {
        if current == name {
            return Err(ElispError::Signal(Box::new(crate::error::SignalData {
                symbol: LispObject::symbol("cyclic-function-indirection"),
                data: LispObject::cons(LispObject::Symbol(name), LispObject::nil()),
            })));
        }
        match state.get_function_cell(current) {
            Some(LispObject::Symbol(next)) => current = next,
            _ => return Ok(()),
        }
    }
    Ok(())
}
