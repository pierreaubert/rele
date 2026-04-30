#![allow(clippy::disallowed_methods)]
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::obarray::{self, SymbolId};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub mod sync_cell;
pub use sync_cell::SyncRefCell;

/// Alias: drop-in replacement for `parking_lot::RwLock` that panics
/// instead of deadlocking on re-entrant access.
pub(crate) type RwLock<T> = SyncRefCell<T>;

pub use environment::{Environment, is_callable_value};
pub use macro_table::{Macro, MacroTable};
use thread_locals::REGEX_CACHE;
pub use thread_locals::{FeatureList, dec_eval_depth, inc_eval_depth};

macro_rules! eval_next {
    ($expr:expr, $env:expr, $editor:expr, $macros:expr, $state:expr) => {{
        inc_eval_depth()?;
        let result = eval($expr, $env, $editor, $macros, $state);
        dec_eval_depth();
        result
    }};
}

/// Dynamic binding stack entry: (variable, previous value or None if unbound).
pub(crate) type Specpdl = Arc<RwLock<Vec<(SymbolId, Option<LispObject>)>>>;
/// Set of variables declared special (dynamically bound) via `defvar`/`defconst`.
pub(crate) type SpecialVars = Arc<RwLock<HashSet<SymbolId>>>;

/// Autoload table: maps function names to the file that defines them.
pub(crate) type AutoloadTable = Arc<RwLock<HashMap<String, String>>>;

fn signal_error_data(items: impl IntoIterator<Item = LispObject>) -> ElispError {
    let mut data = LispObject::nil();
    let mut values: Vec<_> = items.into_iter().collect();
    while let Some(item) = values.pop() {
        data = LispObject::cons(item, data);
    }
    ElispError::Signal(Box::new(crate::error::SignalData {
        symbol: LispObject::symbol("error"),
        data,
    }))
}

fn interactive_spec_form(func: &LispObject) -> Option<LispObject> {
    let (head, rest) = func.destructure_cons()?;
    match head.as_symbol().as_deref()? {
        "lambda" => rest.rest()?.first(),
        "closure" => {
            let after_env = rest.rest()?;
            let after_params = after_env.rest()?;
            after_params.first()
        }
        _ => None,
    }
}

fn interactive_spec_string(func: &LispObject) -> Option<String> {
    let form = interactive_spec_form(func)?;
    let (head, rest) = form.destructure_cons()?;
    if head.as_symbol().as_deref()? != "interactive" {
        return None;
    }
    rest.first()?.as_string().cloned()
}

fn validate_interactive_spec(func: &LispObject) -> ElispResult<()> {
    let Some(spec) = interactive_spec_string(func) else {
        return Ok(());
    };
    let mut command_position = true;
    for ch in spec.chars() {
        if command_position {
            if ch == '\n' {
                continue;
            }
            if !ch.is_ascii() {
                let raw = ch as u32;
                let code = if (0xE000..=0xE0FF).contains(&raw) {
                    raw - 0xE000
                } else {
                    raw
                };
                let display = char::from_u32(code).unwrap_or(ch);
                return Err(signal_error_data([LispObject::string(&format!(
                    "Invalid control letter `{display}' (#o{code:o}, #x{code:04x}) in interactive calling string"
                ))]));
            }
            command_position = false;
        }
        if ch == '\n' {
            command_position = true;
        }
    }
    Ok(())
}

fn face_inherit_prop() -> SymbolId {
    obarray::intern("rele--face-inherit")
}

fn face_inherit_target(state: &InterpreterState, face: SymbolId) -> Option<SymbolId> {
    state.get_plist(face, face_inherit_prop()).as_symbol_id()
}

fn face_inheritance_reaches(state: &InterpreterState, start: SymbolId, target: SymbolId) -> bool {
    let mut current = start;
    let mut seen = HashSet::new();
    while seen.insert(current) {
        if current == target {
            return true;
        }
        let Some(next) = face_inherit_target(state, current) else {
            return false;
        };
        current = next;
    }
    false
}

fn update_face_inherit_attribute(
    state: &InterpreterState,
    values: &[LispObject],
) -> ElispResult<()> {
    let Some(face) = values.first().and_then(LispObject::as_symbol_id) else {
        return Ok(());
    };
    let mut i = 2usize;
    while i + 1 < values.len() {
        if values[i].as_symbol().as_deref() == Some(":inherit")
            && let Some(parent) = values[i + 1].as_symbol_id()
        {
            if face_inheritance_reaches(state, parent, face) {
                return Err(signal_error_data([
                    LispObject::string("Face inheritance results in inheritance cycle"),
                    LispObject::Symbol(parent),
                ]));
            }
            state.put_plist(face, face_inherit_prop(), LispObject::Symbol(parent));
        }
        i += 2;
    }
    Ok(())
}

// Type definitions and trait impls
pub mod interpreter_traits;
pub mod interpreterstate_traits;
pub mod types;

pub use types::*;

// Sub-modules for different evaluation contexts
mod atoms;
mod bootstrap_shortcuts;
mod builtins;
mod dispatch;
mod dynamic;
mod editor;
mod environment;
mod error_forms;
mod functions;
mod keymap_forms;
mod macro_table;
mod metadata;
mod pcase;
mod special_forms;
pub mod state_cl;
mod thread_locals;

// Re-export functions used internally and externally
use bootstrap_shortcuts::{
    get_or_create_runtime_char_table, is_cus_start_properties_let,
    is_generated_translation_table_let, register_generated_translation_table_let,
};
use builtins::{
    emacs_regex_to_rust, eval_dolist, eval_dotimes, eval_featurep, eval_format, eval_get,
    eval_mapc, eval_mapcar, eval_provide, eval_put, eval_require,
};
use editor::{
    current_editor_buffer_name, eval_beginning_of_buffer, eval_buffer_list_bridge,
    eval_buffer_size, eval_buffer_string, eval_clear_rectangle, eval_copy_region_as_kill,
    eval_current_buffer_bridge, eval_delete_char, eval_delete_rectangle, eval_downcase_word,
    eval_end_of_buffer, eval_exchange_point_and_mark, eval_find_file, eval_forward_char,
    eval_forward_line, eval_get_buffer_create_bridge, eval_goto_char, eval_insert,
    eval_insert_directory, eval_keyboard_quit, eval_kill_line, eval_kill_rectangle,
    eval_kill_region, eval_kill_word, eval_move_beginning_of_line, eval_move_end_of_line,
    eval_next_buffer, eval_open_rectangle, eval_point, eval_point_max, eval_point_min,
    eval_previous_buffer, eval_query_replace, eval_query_replace_regexp, eval_re_search_backward,
    eval_re_search_forward, eval_redo_primitive, eval_replace_match, eval_replace_regexp,
    eval_replace_string, eval_save_buffer, eval_save_current_buffer, eval_save_excursion,
    eval_search_backward, eval_search_forward, eval_set_buffer_bridge,
    eval_set_current_buffer_major_mode, eval_set_mark, eval_string_rectangle,
    eval_toggle_line_numbers, eval_toggle_preview, eval_toggle_preview_line_numbers,
    eval_transpose_chars, eval_transpose_words, eval_undo_primitive, eval_upcase_word, eval_yank,
    eval_yank_pop, eval_yank_rectangle, shadow_buffer_object_for_name,
    switch_editor_and_shadow_to_buffer,
};
use error_forms::{
    eval_catch, eval_condition_case, eval_error_fn, eval_signal, eval_throw, eval_unwind_protect,
    eval_user_error_fn,
};
pub(crate) use functions::{
    SetOperation, assign_symbol_value, deliver_variable_watchers, resolve_variable_alias,
    set_function_cell_checked,
};
use functions::{apply_lambda, eval_apply, eval_funcall, eval_funcall_form};
use special_forms::{
    eval_and, eval_cond, eval_defalias, eval_defconst, eval_defmacro, eval_defun, eval_defvar,
    eval_dlet, eval_if, eval_let, eval_let_star, eval_loop, eval_macroexpand,
    eval_make_local_variable, eval_make_variable_buffer_local, eval_or, eval_prog1, eval_prog2,
    eval_progn, eval_setq, eval_setq_local, eval_unless, eval_when, eval_while, expand_macro,
};

// Re-export pub(crate) functions that vm.rs needs
pub(crate) use functions::call_function;

/// Reusable bootstrap helpers (interpreter setup, stdlib loading, ERT
/// runner). Extracted from the test module so audit binaries and
/// production code can use them without reaching into test internals.
pub mod bootstrap;

// `emacs_test_worker` binary needs to access. The `#[test]` fns are
// still gated by their own attribute, so they only run under
// `cargo test`. The pub helpers compile in all modes.
pub mod tests;

fn eval_let_star_binding_form(
    binding: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<(Option<String>, LispObject)> {
    if let Some(name) = binding.as_symbol() {
        let value = value_to_obj(eval(obj_to_value(binding), env, editor, macros, state)?);
        let bind_name = (name != "_").then_some(name);
        return Ok((bind_name, value));
    }
    if let Some((name_obj, rest)) = binding.destructure_cons() {
        if let Some(name) = name_obj.as_symbol() {
            let value_expr = rest.first().unwrap_or_else(|| name_obj.clone());
            let value = value_to_obj(eval(obj_to_value(value_expr), env, editor, macros, state)?);
            let bind_name = (name != "_").then_some(name);
            return Ok((bind_name, value));
        }
        let value = value_to_obj(eval(obj_to_value(name_obj), env, editor, macros, state)?);
        return Ok((None, value));
    }
    Ok((None, LispObject::nil()))
}

fn eval_if_or_when_let_star(
    args: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
    is_when: bool,
) -> ElispResult<Value> {
    let bindings = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.rest().unwrap_or(LispObject::nil());
    let parent = Arc::new(env.read().clone());
    let local = Arc::new(RwLock::new(Environment::with_parent(parent)));
    let mut cur = bindings;
    while let Some((binding, rest)) = cur.destructure_cons() {
        let (name, value) = eval_let_star_binding_form(binding, &local, editor, macros, state)?;
        if value.is_nil() {
            if is_when {
                return Ok(Value::nil());
            }
            let else_body = body.rest().unwrap_or(LispObject::nil());
            return eval_progn(obj_to_value(else_body), env, editor, macros, state);
        }
        if let Some(name) = name {
            local.write().define(&name, value);
        }
        cur = rest;
    }
    if is_when {
        eval_progn(obj_to_value(body), &local, editor, macros, state)
    } else {
        let then_form = body.first().unwrap_or_else(LispObject::nil);
        eval(obj_to_value(then_form), &local, editor, macros, state)
    }
}

// --- Functions from original mod.rs (eval_setf_one, eval_cl_defgeneric_or_method) ---
/// Expand and execute a single `(setf PLACE VALUE)` pair.
///
/// Real Emacs `setf` uses `gv.el` to expand any place form via
/// `gv-define-setter` declarations. We don't have gv.el; instead we
/// hard-code the most common place patterns. Anything we don't
/// recognise falls through to a best-effort `setq` of a symbol or a
/// silent no-op for unknown forms.
pub(super) fn eval_setf_one(
    place: LispObject,
    value_form: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    if place.as_symbol().is_some() {
        let form = LispObject::cons(
            LispObject::symbol("setq"),
            LispObject::cons(place, LispObject::cons(value_form, LispObject::nil())),
        );
        return eval(obj_to_value(form), env, editor, macros, state);
    }
    let (head, args) = match place.destructure_cons() {
        Some(p) => p,
        None => return Ok(Value::nil()),
    };
    let head_name = match head.as_symbol() {
        Some(s) => s,
        None => return Ok(Value::nil()),
    };
    let new_val = value_to_obj(eval(obj_to_value(value_form), env, editor, macros, state)?);
    let call = |fn_sym: &str, fn_args: LispObject| -> ElispResult<Value> {
        let form = LispObject::cons(LispObject::symbol(fn_sym), fn_args);
        eval(obj_to_value(form), env, editor, macros, state)
    };
    let quoted_new = LispObject::cons(
        LispObject::symbol("quote"),
        LispObject::cons(new_val.clone(), LispObject::nil()),
    );
    match head_name.as_str() {
        "car" => {
            let x = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
            call(
                "setcar",
                LispObject::cons(x, LispObject::cons(quoted_new, LispObject::nil())),
            )?;
            Ok(obj_to_value(new_val))
        }
        "cdr" => {
            let x = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
            call(
                "setcdr",
                LispObject::cons(x, LispObject::cons(quoted_new, LispObject::nil())),
            )?;
            Ok(obj_to_value(new_val))
        }
        "nth" => {
            let n = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
            let l = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
            let nthcdr_form = LispObject::cons(
                LispObject::symbol("nthcdr"),
                LispObject::cons(n, LispObject::cons(l, LispObject::nil())),
            );
            call(
                "setcar",
                LispObject::cons(nthcdr_form, LispObject::cons(quoted_new, LispObject::nil())),
            )?;
            Ok(obj_to_value(new_val))
        }
        "aref" => {
            let v = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
            let i = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
            call(
                "aset",
                LispObject::cons(
                    v,
                    LispObject::cons(i, LispObject::cons(quoted_new, LispObject::nil())),
                ),
            )?;
            Ok(obj_to_value(new_val))
        }
        "gethash" => {
            let k = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
            let h = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
            call(
                "puthash",
                LispObject::cons(
                    k,
                    LispObject::cons(quoted_new, LispObject::cons(h, LispObject::nil())),
                ),
            )?;
            Ok(obj_to_value(new_val))
        }
        "get" => {
            let s = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
            let p = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
            call(
                "put",
                LispObject::cons(
                    s,
                    LispObject::cons(p, LispObject::cons(quoted_new, LispObject::nil())),
                ),
            )?;
            Ok(obj_to_value(new_val))
        }
        "overlay-get" => {
            let overlay = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
            let key = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
            call(
                "overlay-put",
                LispObject::cons(
                    overlay,
                    LispObject::cons(key, LispObject::cons(quoted_new, LispObject::nil())),
                ),
            )?;
            Ok(obj_to_value(new_val))
        }
        "cl--find-class" | "cl-find-class" => {
            let name_form = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
            let name = value_to_obj(eval(obj_to_value(name_form), env, editor, macros, state)?);
            if let Some(name_str) = name.as_symbol() {
                let id = crate::obarray::intern(&name_str);
                let key = crate::obarray::intern("cl--class");
                state.put_plist(id, key, new_val.clone());
                crate::primitives_eieio::register_class_for_state(
                    state,
                    crate::primitives_eieio::Class {
                        name: name_str,
                        parent: None,
                        slots: Vec::new(),
                    },
                );
            }
            Ok(obj_to_value(new_val))
        }
        "symbol-value" => {
            let s = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
            call(
                "set",
                LispObject::cons(s, LispObject::cons(quoted_new, LispObject::nil())),
            )
        }
        "symbol-function" => {
            let s = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
            call(
                "fset",
                LispObject::cons(s, LispObject::cons(quoted_new, LispObject::nil())),
            )
        }
        "plist-get" => {
            let plist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
            let key = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
            let put_form = LispObject::cons(
                LispObject::symbol("plist-put"),
                LispObject::cons(
                    plist.clone(),
                    LispObject::cons(key, LispObject::cons(quoted_new, LispObject::nil())),
                ),
            );
            if plist.as_symbol().is_some() {
                let setq_form = LispObject::cons(
                    LispObject::symbol("setq"),
                    LispObject::cons(plist, LispObject::cons(put_form, LispObject::nil())),
                );
                eval(obj_to_value(setq_form), env, editor, macros, state)
            } else {
                eval(obj_to_value(put_form), env, editor, macros, state)
            }
        }
        _ => {
            let accessor_id = crate::obarray::intern(&head_name);
            let gv_setter = state.get_plist(accessor_id, crate::obarray::intern("gv-setter"));
            if let Some(setter_name) = gv_setter.as_symbol() {
                let mut setter_args = Vec::new();
                let mut cur = args.clone();
                while let Some((arg, rest)) = cur.destructure_cons() {
                    setter_args.push(arg);
                    cur = rest;
                }
                setter_args.push(quoted_new.clone());
                let setter_args = setter_args
                    .into_iter()
                    .rev()
                    .fold(LispObject::nil(), |acc, arg| LispObject::cons(arg, acc));
                call(&setter_name, setter_args)?;
                return Ok(obj_to_value(new_val));
            }
            let slot_key = crate::obarray::intern("cl-struct-slot-index");
            if let Some(slot) = state.get_plist(accessor_id, slot_key).as_integer() {
                let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
                call(
                    "aset",
                    LispObject::cons(
                        obj,
                        LispObject::cons(
                            LispObject::integer(slot),
                            LispObject::cons(quoted_new, LispObject::nil()),
                        ),
                    ),
                )?;
            }
            Ok(obj_to_value(new_val))
        }
    }
}
/// Shared implementation for `cl-defgeneric` and `cl-defmethod`.
/// Parses `(NAME ... ARGS ... BODY)` where ARGS is the first non-qualifier
/// list and BODY is everything after it. The arg list may contain
/// type-dispatch specs like `(obj symbol)` — we strip the type and keep
/// the bare arg name.
///
/// `is_method` is `true` for `cl-defmethod` and `false` for
/// `cl-defgeneric`. When it's a method AND a qualifier is present
/// (any symbol before the arg list, e.g. `:before`, `:after`, `:around`,
/// `:printer`), we must NOT install a `defun` — in Emacs's cl-generic,
/// qualified methods are auxiliary (combined via method combination)
/// and are never the sole dispatch target. If we installed them as a
/// plain `defun`, a subsequent call to the generic would run the
/// qualifier body (or, if the qualifier symbol leaked into the
/// function slot itself, signal `void-function: :printer` at call time).
pub(super) fn eval_cl_defgeneric_or_method(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
    is_method: bool,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name_obj = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = match name_obj.as_symbol() {
        Some(s) => s,
        None => {
            if let Some((car, _)) = name_obj.destructure_cons()
                && car.as_symbol().as_deref() == Some("setf")
            {
                return Ok(Value::nil());
            }
            return Err(ElispError::WrongTypeArgument("symbol".to_string()));
        }
    };
    let mut rest = args_obj.rest().unwrap_or(LispObject::nil());
    if is_method && name == "cl-generic-generalizers" {
        let id = crate::obarray::intern(&name);
        state.set_function_cell(id, LispObject::primitive("cl-generic-generalizers"));
        return Ok(obj_to_value(LispObject::symbol(&name)));
    }
    let mut had_qualifier = false;
    while let Some((head, tail)) = rest.destructure_cons() {
        if matches!(head, LispObject::Cons(_)) {
            break;
        }
        had_qualifier = true;
        rest = tail;
    }
    if is_method && had_qualifier {
        return Ok(Value::nil());
    }
    let (arglist, body) = match rest.destructure_cons() {
        Some((a, b)) => (a, b),
        None => (LispObject::nil(), LispObject::nil()),
    };
    let mut plain_args = Vec::new();
    let mut cur = arglist;
    while let Some((arg, tail)) = cur.destructure_cons() {
        let bare = match &arg {
            LispObject::Symbol(_) => arg.clone(),
            LispObject::Cons(_) => arg.first().unwrap_or(LispObject::nil()),
            _ => arg.clone(),
        };
        plain_args.push(bare);
        cur = tail;
    }
    let mut arg_list = LispObject::nil();
    for a in plain_args.into_iter().rev() {
        arg_list = LispObject::cons(a, arg_list);
    }
    let mut effective_body = body;
    if let Some((maybe_doc, tail)) = effective_body.destructure_cons()
        && maybe_doc.as_string().is_some()
        && !tail.is_nil()
    {
        effective_body = tail;
    }
    let defun_args = LispObject::cons(
        LispObject::symbol(&name),
        LispObject::cons(arg_list, effective_body),
    );
    eval_defun(obj_to_value(defun_args), env, editor, macros, state)
}

// --- Functions from original mod.rs (cl-defstruct) ---
/// Minimal `cl-defstruct` implementation. Parses
/// `(cl-defstruct NAME-OR-OPTS [DOCSTRING] FIELDS...)` and installs:
/// - `make-NAME` constructor (positional args)
/// - `NAME-p` predicate
/// - `NAME-FIELD` accessors
/// - `copy-NAME` copier
///
/// Records are vectors with `cl-struct-NAME` at index 0.
/// Ignores :constructor / :copier / :predicate overrides and :include.
pub(super) fn eval_cl_defstruct(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name_spec = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut custom_constructors: Vec<(String, Option<LispObject>)> = Vec::new();
    let mut conc_name: Option<String> = None;
    let mut predicate_name: Option<Option<String>> = None;
    let mut include_name: Option<String> = None;
    let mut type_vector = false;
    let name = match &name_spec {
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        LispObject::Cons(_) => {
            let hd = name_spec
                .first()
                .ok_or(ElispError::WrongNumberOfArguments)?;
            let n = hd
                .as_symbol()
                .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
            let mut opts = name_spec.rest().unwrap_or(LispObject::nil());
            while let Some((opt, rest_o)) = opts.destructure_cons() {
                if let Some((k, opt_rest)) = opt.destructure_cons()
                    && let Some(ks) = k.as_symbol()
                {
                    match ks.as_str() {
                        ":constructor" => {
                            if let Some((cname, _)) = opt_rest.destructure_cons()
                                && let Some(cn) = cname.as_symbol()
                                && cn != "nil"
                            {
                                custom_constructors.push((cn, opt_rest.nth(1)));
                            }
                        }
                        ":conc-name" => {
                            if let Some((cn, _)) = opt_rest.destructure_cons() {
                                if let Some(s) = cn.as_symbol() {
                                    conc_name = Some(s);
                                } else if let Some(s) = cn.as_string() {
                                    conc_name = Some(s.to_string());
                                }
                            }
                        }
                        ":predicate" => {
                            if let Some((pred, _)) = opt_rest.destructure_cons() {
                                predicate_name = Some(
                                    pred.as_symbol()
                                        .and_then(|s| if s == "nil" { None } else { Some(s) }),
                                );
                            }
                        }
                        ":include" => {
                            if let Some((parent, _)) = opt_rest.destructure_cons() {
                                include_name = parent.as_symbol();
                            }
                        }
                        ":type" => {
                            if let Some((type_name, _)) = opt_rest.destructure_cons() {
                                type_vector = type_name.as_symbol().as_deref() == Some("vector");
                            }
                        }
                        _ => {}
                    }
                }
                opts = rest_o;
            }
            n
        }
        _ => return Err(ElispError::WrongTypeArgument("symbol-or-cons".to_string())),
    };
    let mut rest = args_obj.rest().unwrap_or(LispObject::nil());
    if let Some((first, tail)) = rest.destructure_cons()
        && first.as_string().is_some()
    {
        rest = tail;
    }
    let mut field_names: Vec<String> = include_name
        .as_deref()
        .map(|parent| cl_struct_inherited_field_names(state, parent))
        .unwrap_or_default();
    let mut field_defaults: Vec<LispObject> = vec![LispObject::nil(); field_names.len()];
    let mut cur = rest;
    while let Some((field_spec, next)) = cur.destructure_cons() {
        let (fname, fdefault) = match &field_spec {
            LispObject::Symbol(id) => (crate::obarray::symbol_name(*id), LispObject::nil()),
            LispObject::Cons(_) => {
                let fst = field_spec
                    .first()
                    .ok_or(ElispError::WrongNumberOfArguments)?;
                match fst.as_symbol() {
                    Some(s) => {
                        let default = field_spec.nth(1).unwrap_or(LispObject::nil());
                        (s, default)
                    }
                    None => {
                        cur = next;
                        continue;
                    }
                }
            }
            _ => break,
        };
        field_names.push(fname);
        field_defaults.push(fdefault);
        cur = next;
    }
    let tag_name = name.clone();
    let n_fields = field_names.len();
    let slots_reg: Vec<crate::primitives_eieio::Slot> = field_names
        .iter()
        .map(|f| crate::primitives_eieio::Slot {
            name: f.clone(),
            initarg: Some(f.clone()),
            default: LispObject::nil(),
        })
        .collect();
    crate::primitives_eieio::register_class_for_state(
        state,
        crate::primitives_eieio::Class {
            name: name.clone(),
            parent: None,
            slots: slots_reg,
        },
    );
    {
        let tags_var = format!("cl-struct-{}-tags", name);
        let tags_sym = crate::obarray::intern(&tags_var);
        let existing = state
            .get_value_cell(tags_sym)
            .or_else(|| state.global_env.read().get(&tags_var))
            .unwrap_or(LispObject::nil());
        let tag_sym = LispObject::symbol(&name);
        let mut found = false;
        let mut cur = existing.clone();
        while let Some((head, tail)) = cur.destructure_cons() {
            if head == tag_sym {
                found = true;
                break;
            }
            cur = tail;
        }
        let tags = if found {
            existing
        } else {
            LispObject::cons(tag_sym, existing)
        };
        state.global_env.write().set(&tags_var, tags.clone());
        state.set_value_cell(tags_sym, tags);
    }
    if name != "cl-structure-object" {
        let root_tags_var = "cl-struct-cl-structure-object-tags";
        let root_tags_sym = crate::obarray::intern(root_tags_var);
        let existing = state
            .get_value_cell(root_tags_sym)
            .or_else(|| state.global_env.read().get(root_tags_var))
            .unwrap_or(LispObject::nil());
        let tag_sym = LispObject::symbol(&name);
        let mut found = false;
        let mut cur = existing.clone();
        while let Some((head, tail)) = cur.destructure_cons() {
            if head == tag_sym {
                found = true;
                break;
            }
            cur = tail;
        }
        if !found {
            let new_list = LispObject::cons(tag_sym, existing);
            state
                .global_env
                .write()
                .set(root_tags_var, new_list.clone());
            state.set_value_cell(root_tags_sym, new_list);
        }
    }
    let defaults_str = field_defaults
        .iter()
        .map(|d| d.princ_to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let field_kws = field_names
        .iter()
        .map(|f| format!(":{f}"))
        .collect::<Vec<_>>()
        .join(" ");
    let tag_init = if type_vector {
        String::new()
    } else {
        format!("(aset vec 0 '{tag_name})")
    };
    let field_offset = if type_vector { 0 } else { 1 };
    let ctor_slots = n_fields + field_offset;
    let ctor_body = format!(
        "(lambda (&rest args)\
         (let ((vec (make-vector {n} nil))\
               (defaults (list {defaults}))\
               (fields '({field_kws}))\
               (i 0))\
           {tag_init}\
           (let ((d defaults) (j {field_offset}))\
             (while d\
               (aset vec j (car d))\
               (setq d (cdr d) j (+ j 1))))\
           (if (and args (symbolp (car args))\
                    (memq (car args) fields))\
             (while args\
               (let ((kw (car args)) (rst (cdr args)))\
                 (if (and (symbolp kw) rst)\
                   (let ((idx 0) (found nil) (flds fields))\
                     (while flds\
                       (when (eq kw (car flds))\
                         (aset vec (+ idx {field_offset}) (car rst))\
                         (setq found t flds nil))\
                       (setq idx (+ idx 1) flds (cdr flds)))\
                     (setq args (cdr rst)))\
                   (setq args nil))))\
             (while (and args (< i {nf}))\
               (aset vec (+ i {field_offset}) (car args))\
               (setq args (cdr args))\
               (setq i (+ i 1))))\
           vec))",
        n = ctor_slots,
        nf = n_fields,
        defaults = defaults_str,
        field_kws = field_kws,
        tag_init = tag_init,
        field_offset = field_offset,
    );
    let ctor_expr = crate::read(&ctor_body)
        .map_err(|e| ElispError::EvalError(format!("cl-defstruct ctor parse: {e}")))?;
    let ctor_val = value_to_obj(eval(obj_to_value(ctor_expr), env, editor, macros, state)?);
    state.set_function_cell(
        crate::obarray::intern(&format!("make-{}", name)),
        ctor_val.clone(),
    );
    for (cn, arglist) in &custom_constructors {
        let custom_val = if let Some(arglist) = arglist {
            let mut assignments = String::new();
            let mut cur = arglist.clone();
            while let Some((param, rest)) = cur.destructure_cons() {
                if let Some(param_name) = param.as_symbol() {
                    if !param_name.starts_with('&')
                        && let Some(field_idx) = field_names.iter().position(|f| f == &param_name)
                    {
                        assignments.push_str(&format!(
                            "(aset vec {} {})",
                            field_idx + field_offset,
                            param_name
                        ));
                    }
                } else if let Some(param_name) = param.first().and_then(|obj| obj.as_symbol())
                    && let Some(field_idx) = field_names.iter().position(|f| f == &param_name)
                {
                    assignments.push_str(&format!(
                        "(aset vec {} {})",
                        field_idx + field_offset,
                        param_name
                    ));
                }
                cur = rest;
            }
            let custom_ctor_body = format!(
                "(lambda {arglist}\
                 (let ((vec (make-vector {n} nil))\
                       (defaults (list {defaults})))\
                   {tag_init}\
                   (let ((d defaults) (j {field_offset}))\
                     (while d\
                       (aset vec j (car d))\
                       (setq d (cdr d) j (+ j 1))))\
                   {assignments}\
                vec))",
                arglist = arglist.princ_to_string(),
                n = ctor_slots,
                defaults = defaults_str,
                tag_init = tag_init,
                field_offset = field_offset,
                assignments = assignments,
            );
            let custom_ctor_expr = crate::read(&custom_ctor_body).map_err(|e| {
                ElispError::EvalError(format!("cl-defstruct custom ctor parse: {e}"))
            })?;
            value_to_obj(eval(
                obj_to_value(custom_ctor_expr),
                env,
                editor,
                macros,
                state,
            )?)
        } else {
            ctor_val.clone()
        };
        state.set_function_cell(crate::obarray::intern(cn), custom_val);
    }
    {
        let slot_descs = if field_names.is_empty() {
            "(make-vector 0 nil)".to_string()
        } else {
            format!(
                "(vector {})",
                field_names
                    .iter()
                    .map(|field| format!("(record 'cl-slot-descriptor '{field} nil nil nil)"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        let class_obj_form = format!(
            "(cl--struct-new-class '{name} nil nil nil nil \
             {slot_descs} (make-hash-table :test 'eq) \
             'cl-struct-{name}-tags '{tag} nil)",
            name = name,
            slot_descs = slot_descs,
            tag = tag_name,
        );
        if let Ok(forms) = crate::read_all(&class_obj_form) {
            for form in forms {
                if let Ok(class_val) = eval(obj_to_value(form), env, editor, macros, state) {
                    let class_obj = value_to_obj(class_val);
                    let id = crate::obarray::intern(&name);
                    let key = crate::obarray::intern("cl--class");
                    state.put_plist(id, key, class_obj);
                }
            }
        }
    }
    if let Some(pred_name) = predicate_name.unwrap_or_else(|| Some(format!("{}-p", name))) {
        let pred_body = if type_vector {
            format!("(lambda (obj) (and (vectorp obj) (= (length obj) {n_fields})))")
        } else {
            format!(
                "(lambda (obj) \
                   (and (vectorp obj) (> (length obj) 0) \
                        (if (memq (aref obj 0) cl-struct-{name}-tags) t nil)))",
                name = name,
            )
        };
        let pred_expr = crate::read(&pred_body)
            .map_err(|e| ElispError::EvalError(format!("cl-defstruct pred parse: {e}")))?;
        let pred_val = value_to_obj(eval(obj_to_value(pred_expr), env, editor, macros, state)?);
        state.set_function_cell(crate::obarray::intern(&pred_name), pred_val);
    }
    let prefix = conc_name.unwrap_or_else(|| format!("{}-", name));
    for (i, field) in field_names.iter().enumerate() {
        let body = format!("(lambda (obj) (aref obj {}))", i + field_offset);
        let expr = crate::read(&body)
            .map_err(|e| ElispError::EvalError(format!("cl-defstruct accessor parse: {e}")))?;
        let val = value_to_obj(eval(obj_to_value(expr), env, editor, macros, state)?);
        let accessor_id = crate::obarray::intern(&format!("{}{}", prefix, field));
        state.set_function_cell(accessor_id, val);
        state.put_plist(
            accessor_id,
            crate::obarray::intern("cl-struct-slot-index"),
            LispObject::integer((i + field_offset) as i64),
        );
    }
    let copier_expr = crate::read("(lambda (obj) (copy-sequence obj))")
        .map_err(|e| ElispError::EvalError(format!("cl-defstruct copier parse: {e}")))?;
    let copier_val = value_to_obj(eval(obj_to_value(copier_expr), env, editor, macros, state)?);
    state.set_function_cell(
        crate::obarray::intern(&format!("copy-{}", name)),
        copier_val,
    );
    Ok(obj_to_value(LispObject::symbol(&name)))
}
pub(super) fn cl_struct_inherited_field_names(
    state: &InterpreterState,
    parent: &str,
) -> Vec<String> {
    let parent_id = crate::obarray::intern(parent);
    let class_key = crate::obarray::intern("cl--class");
    let class = state.get_plist(parent_id, class_key);
    if let LispObject::Vector(class_vec) = class {
        let slots = {
            let guard = class_vec.lock();
            guard.get(4).cloned()
        };
        if let Some(slots) = slots {
            let names = cl_slot_descriptor_names(&slots);
            if !names.is_empty() {
                return names;
            }
        }
    }
    match parent {
        "cl--class" => ["name", "docstring", "parents", "slots", "index-table"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}
pub(super) fn cl_slot_descriptor_names(slots: &LispObject) -> Vec<String> {
    match slots {
        LispObject::Vector(vec) => {
            let guard = vec.lock();
            guard.iter().filter_map(cl_slot_descriptor_name).collect()
        }
        LispObject::Nil => Vec::new(),
        LispObject::Cons(_) => {
            let mut names = Vec::new();
            let mut cur = slots.clone();
            while let Some((desc, rest)) = cur.destructure_cons() {
                if let Some(name) = cl_slot_descriptor_name(&desc) {
                    names.push(name);
                }
                cur = rest;
            }
            names
        }
        _ => Vec::new(),
    }
}
pub(super) fn cl_slot_descriptor_name(desc: &LispObject) -> Option<String> {
    match desc {
        LispObject::Vector(vec) => {
            let guard = vec.lock();
            guard.get(1).and_then(LispObject::as_symbol)
        }
        LispObject::Cons(_) => desc.nth(1).and_then(|name| name.as_symbol()),
        _ => None,
    }
}

// --- Functions from original mod.rs (eval, core evaluation) ---
pub(super) fn eval_oclosure_define(args: Value, state: &InterpreterState) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name_spec = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let (name, parent_name, predicate_name) = parse_oclosure_name_spec(&name_spec)?;
    let mut rest = args_obj.rest().unwrap_or_else(LispObject::nil);
    let docstring = if let Some((first, tail)) = rest.destructure_cons() {
        if first.as_string().is_some() {
            rest = tail;
            first
        } else {
            LispObject::nil()
        }
    } else {
        LispObject::nil()
    };
    let parent_class = if let Some(parent) = &parent_name {
        state.get_plist(
            crate::obarray::intern(parent),
            crate::obarray::intern("cl--class"),
        )
    } else {
        LispObject::nil()
    };
    let mut slotdescs = class_slots_as_vec(&parent_class);
    let mut cur = rest;
    while let Some((slot_spec, next)) = cur.destructure_cons() {
        if let Some(slot_name) = oclosure_slot_name(&slot_spec) {
            slotdescs.push(make_cl_slot_descriptor(&slot_name));
        }
        cur = next;
    }
    let slots_obj = list_from_vec(slotdescs.clone());
    let index_table = make_slot_index_table(&slotdescs);
    let parents = if parent_class.is_nil() {
        LispObject::nil()
    } else {
        LispObject::cons(parent_class.clone(), LispObject::nil())
    };
    let allparents = LispObject::cons(
        LispObject::symbol(&name),
        class_allparents(&parent_class)
            .or_else(|| {
                parent_name
                    .as_ref()
                    .map(|p| LispObject::cons(LispObject::symbol(p), LispObject::nil()))
            })
            .unwrap_or_else(LispObject::nil),
    );
    let class_obj = LispObject::Vector(Arc::new(SyncRefCell::new(vec![
        LispObject::symbol("oclosure--class"),
        LispObject::symbol(&name),
        docstring,
        parents,
        slots_obj,
        index_table,
        allparents,
    ])));
    let name_id = crate::obarray::intern(&name);
    state.put_plist(name_id, crate::obarray::intern("cl--class"), class_obj);
    let pred = predicate_name.unwrap_or_else(|| format!("{name}--internal-p"));
    state.set_function_cell(
        crate::obarray::intern(&pred),
        LispObject::primitive("ignore"),
    );
    state.put_plist(
        name_id,
        crate::obarray::intern("cl-deftype-satisfies"),
        LispObject::symbol(&pred),
    );
    for desc in &slotdescs {
        if let Some(slot_name) = cl_slot_descriptor_name(desc) {
            let accessor = format!("{name}--{slot_name}");
            if let Ok(lambda) = crate::read(&format!(
                "(lambda (obj) (oclosure--slot-value obj '{slot_name}))"
            )) {
                state.set_function_cell(crate::obarray::intern(&accessor), lambda);
            }
        }
    }
    Ok(obj_to_value(LispObject::symbol(&name)))
}
fn parse_oclosure_name_spec(
    spec: &LispObject,
) -> ElispResult<(String, Option<String>, Option<String>)> {
    match spec {
        LispObject::Symbol(id) => Ok((
            crate::obarray::symbol_name(*id),
            Some("oclosure".into()),
            None,
        )),
        LispObject::Cons(_) => {
            let name = spec
                .first()
                .and_then(|n| n.as_symbol())
                .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
            let mut parent = Some("oclosure".to_string());
            let mut predicate = None;
            let mut opts = spec.rest().unwrap_or_else(LispObject::nil);
            while let Some((opt, next)) = opts.destructure_cons() {
                if let Some((key, vals)) = opt.destructure_cons()
                    && let Some(key_name) = key.as_symbol()
                {
                    match key_name.as_str() {
                        ":parent" | ":include" => {
                            parent = vals.first().and_then(|v| v.as_symbol());
                        }
                        ":predicate" => {
                            predicate = vals.first().and_then(|v| v.as_symbol());
                        }
                        _ => {}
                    }
                }
                opts = next;
            }
            Ok((name, parent, predicate))
        }
        _ => Err(ElispError::WrongTypeArgument("symbol-or-cons".to_string())),
    }
}
fn oclosure_slot_name(spec: &LispObject) -> Option<String> {
    match spec {
        LispObject::Symbol(id) => Some(crate::obarray::symbol_name(*id)),
        LispObject::Cons(_) => spec.first().and_then(|name| name.as_symbol()),
        _ => None,
    }
}
fn make_cl_slot_descriptor(name: &str) -> LispObject {
    LispObject::Vector(Arc::new(SyncRefCell::new(vec![
        LispObject::symbol("cl-slot-descriptor"),
        LispObject::symbol(name),
        LispObject::nil(),
        LispObject::nil(),
        LispObject::nil(),
    ])))
}
fn class_slots_as_vec(class_obj: &LispObject) -> Vec<LispObject> {
    let Some(slots) = class_slot_object(class_obj) else {
        return Vec::new();
    };
    match slots {
        LispObject::Vector(vec) => vec.lock().clone(),
        LispObject::Cons(_) => {
            let mut out = Vec::new();
            let mut cur = slots;
            while let Some((slot, rest)) = cur.destructure_cons() {
                out.push(slot);
                cur = rest;
            }
            out
        }
        _ => Vec::new(),
    }
}
fn class_slot_object(class_obj: &LispObject) -> Option<LispObject> {
    match class_obj {
        LispObject::Vector(vec) => vec.lock().get(4).cloned(),
        _ => None,
    }
}
fn class_allparents(class_obj: &LispObject) -> Option<LispObject> {
    match class_obj {
        LispObject::Vector(vec) => vec.lock().get(6).cloned(),
        _ => None,
    }
}
fn make_slot_index_table(slotdescs: &[LispObject]) -> LispObject {
    let mut table = crate::object::LispHashTable::new(crate::object::HashTableTest::Eq);
    for (idx, desc) in slotdescs.iter().enumerate() {
        if let Some(name) = cl_slot_descriptor_name(desc) {
            table.put(&LispObject::symbol(&name), LispObject::integer(idx as i64));
        }
    }
    LispObject::HashTable(Arc::new(SyncRefCell::new(table)))
}
fn list_from_vec(items: Vec<LispObject>) -> LispObject {
    items
        .into_iter()
        .rev()
        .fold(LispObject::nil(), |acc, item| LispObject::cons(item, acc))
}

fn collect_dot_symbols(obj: &LispObject, out: &mut HashSet<String>) {
    match obj {
        LispObject::Symbol(id) => {
            let name = crate::obarray::symbol_name(*id);
            if name.starts_with('.') && name.len() > 1 {
                out.insert(name);
            }
        }
        LispObject::Cons(_) => {
            let mut cur = obj.clone();
            while let Some((head, rest)) = cur.destructure_cons() {
                collect_dot_symbols(&head, out);
                cur = rest;
            }
            if !cur.is_nil() {
                collect_dot_symbols(&cur, out);
            }
        }
        LispObject::Vector(items) => {
            for item in items.lock().iter() {
                collect_dot_symbols(item, out);
            }
        }
        _ => {}
    }
}

fn append_hook_function(existing: LispObject, func: LispObject) -> LispObject {
    if existing.is_nil() {
        return LispObject::cons(func, LispObject::nil());
    }
    let mut funcs = Vec::new();
    let mut cur = existing;
    while let Some((head, rest)) = cur.destructure_cons() {
        if head == func {
            return list_from_vec(funcs.into_iter().chain([head]).collect());
        }
        funcs.push(head);
        cur = rest;
    }
    funcs.push(func);
    list_from_vec(funcs)
}

fn remove_hook_function(existing: LispObject, func: &LispObject) -> LispObject {
    let mut funcs = Vec::new();
    let mut cur = existing;
    while let Some((head, rest)) = cur.destructure_cons() {
        if &head != func {
            funcs.push(head);
        }
        cur = rest;
    }
    list_from_vec(funcs)
}

fn current_hook_value(name: &str, env: &Arc<RwLock<Environment>>) -> LispObject {
    let local = crate::buffer::with_registry(|registry| {
        registry
            .get(registry.current_id())
            .and_then(|buffer| buffer.locals.get(name).cloned())
    });
    match local {
        Some(value) if !value.is_nil() => value,
        _ => env.read().get(name).unwrap_or_else(LispObject::nil),
    }
}

fn current_buffer_hook_value(name: &str) -> LispObject {
    crate::buffer::with_registry(|registry| {
        registry
            .get(registry.current_id())
            .and_then(|buffer| buffer.locals.get(name).cloned())
            .unwrap_or_else(LispObject::nil)
    })
}

fn set_hook_value(name: &str, value: LispObject, local: bool, env: &Arc<RwLock<Environment>>) {
    if local {
        crate::buffer::with_current_mut(|buffer| {
            buffer.locals.insert(name.to_string(), value);
        });
    } else {
        env.write().define(name, value);
    }
}

fn run_hook_symbol(
    hook_symbol: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    run_hook_symbol_with_args(hook_symbol, LispObject::nil(), env, editor, macros, state)
}

fn run_hook_symbol_with_args(
    hook_symbol: LispObject,
    args: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let Some(name) = hook_symbol.as_symbol() else {
        return Ok(Value::nil());
    };
    let mut funcs = current_hook_value(&name, env);
    while let Some((func, rest)) = funcs.destructure_cons() {
        let before = closure_capture_snapshot(&func);
        functions::call_function(
            obj_to_value(func.clone()),
            obj_to_value(args.clone()),
            env,
            editor,
            macros,
            state,
        )?;
        sync_changed_closure_captures_to_env(&func, &before, env, state);
        funcs = rest;
    }
    Ok(Value::nil())
}

fn add_hook_local_arg(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<bool> {
    if let Some(expr) = args.nth(3) {
        return Ok(!value_to_obj(eval(obj_to_value(expr), env, editor, macros, state)?).is_nil());
    }
    if let Some(expr) = args.nth(2) {
        let value = value_to_obj(eval(obj_to_value(expr), env, editor, macros, state)?);
        return Ok(value.as_symbol().as_deref() == Some(":local"));
    }
    Ok(false)
}

fn run_buffer_local_hook_symbol(
    hook_symbol: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    run_buffer_local_hook_symbol_with_args(
        hook_symbol,
        LispObject::nil(),
        env,
        editor,
        macros,
        state,
    )
}

fn run_buffer_local_hook_symbol_with_args(
    hook_symbol: LispObject,
    args: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let Some(name) = hook_symbol.as_symbol() else {
        return Ok(Value::nil());
    };
    let mut funcs = current_buffer_hook_value(&name);
    while let Some((func, rest)) = funcs.destructure_cons() {
        let before = closure_capture_snapshot(&func);
        functions::call_function(
            obj_to_value(func.clone()),
            obj_to_value(args.clone()),
            env,
            editor,
            macros,
            state,
        )?;
        sync_changed_closure_captures_to_env(&func, &before, env, state);
        funcs = rest;
    }
    Ok(Value::nil())
}

fn closure_captures(func: &LispObject) -> Option<LispObject> {
    let (head, rest) = func.destructure_cons()?;
    if head.as_symbol().as_deref() != Some("closure") {
        return None;
    }
    rest.first()
}

fn closure_capture_snapshot(func: &LispObject) -> Vec<(SymbolId, LispObject)> {
    let Some(captured) = closure_captures(func) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut cur = captured;
    while let Some((pair, rest)) = cur.destructure_cons() {
        if let Some((key, value)) = pair.destructure_cons()
            && let Some(id) = key.as_symbol_id()
        {
            out.push((id, value));
        }
        cur = rest;
    }
    out
}

fn sync_changed_closure_captures_to_env(
    func: &LispObject,
    before: &[(SymbolId, LispObject)],
    env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) {
    let Some(captured) = closure_captures(func) else {
        return;
    };
    let mut cur = captured;
    while let Some((pair, rest)) = cur.destructure_cons() {
        if let Some((key, value)) = pair.destructure_cons()
            && let Some(id) = key.as_symbol_id()
            && before
                .iter()
                .find(|(old_id, _)| *old_id == id)
                .is_none_or(|(_, old_value)| old_value != &value)
        {
            env.write().set_id(id, value.clone());
            state.set_closure_mutation(id, value);
        }
        cur = rest;
    }
}

struct OverlayModificationCall {
    func: LispObject,
    args: LispObject,
}

fn overlay_object(id: crate::buffer::OverlayId) -> LispObject {
    LispObject::cons(
        LispObject::symbol("overlay"),
        LispObject::integer(id as i64),
    )
}

fn hook_property_functions(value: &LispObject) -> Vec<LispObject> {
    let mut funcs = Vec::new();
    let mut cur = value.clone();
    while let Some((func, rest)) = cur.destructure_cons() {
        funcs.push(func);
        cur = rest;
    }
    if funcs.is_empty() && !value.is_nil() {
        funcs.push(value.clone());
    }
    funcs
}

fn collect_overlay_hooks_for_insert(
    pos: usize,
    len: usize,
    after: bool,
) -> Vec<OverlayModificationCall> {
    let buffer = crate::buffer::with_registry(|registry| registry.current_id());
    crate::buffer::with_registry(|registry| {
        registry
            .overlays
            .values()
            .filter(|overlay| overlay.buffer == buffer)
            .filter_map(|overlay| {
                let (start, end) = (overlay.start?, overlay.end?);
                let property = if start == end && start == pos {
                    Some(vec!["insert-in-front-hooks", "insert-behind-hooks"])
                } else if pos == start {
                    Some(vec!["insert-in-front-hooks"])
                } else if pos == end {
                    Some(vec!["insert-behind-hooks"])
                } else if start < pos && pos < end {
                    Some(vec!["modification-hooks"])
                } else {
                    None
                }?;
                Some((overlay.id, property, overlay.plist.clone()))
            })
            .flat_map(|(id, properties, plist)| {
                let mut calls = Vec::new();
                for property in properties {
                    let key = LispObject::symbol(property);
                    let Some((_, hook_value)) = plist.iter().find(|(k, _)| *k == key) else {
                        continue;
                    };
                    let args = if after {
                        list_from_vec(vec![
                            overlay_object(id),
                            LispObject::t(),
                            LispObject::integer(pos as i64),
                            LispObject::integer((pos + len) as i64),
                            LispObject::integer(0),
                        ])
                    } else {
                        list_from_vec(vec![
                            overlay_object(id),
                            LispObject::nil(),
                            LispObject::integer(pos as i64),
                            LispObject::integer(pos as i64),
                        ])
                    };
                    for func in hook_property_functions(hook_value) {
                        calls.push(OverlayModificationCall {
                            func,
                            args: args.clone(),
                        });
                    }
                }
                calls
            })
            .collect()
    })
}

fn collect_overlay_hooks_for_replace(
    start: usize,
    end: usize,
    new_len: usize,
    after: bool,
) -> Vec<OverlayModificationCall> {
    let old_len = end.saturating_sub(start);
    let buffer = crate::buffer::with_registry(|registry| registry.current_id());
    crate::buffer::with_registry(|registry| {
        registry
            .overlays
            .values()
            .filter(|overlay| overlay.buffer == buffer)
            .filter_map(|overlay| {
                let (ov_start, ov_end) = (overlay.start?, overlay.end?);
                if start >= ov_end || end <= ov_start {
                    return None;
                }
                Some((overlay.id, overlay.plist.clone()))
            })
            .flat_map(|(id, plist)| {
                let key = LispObject::symbol("modification-hooks");
                let Some((_, hook_value)) = plist.iter().find(|(k, _)| *k == key) else {
                    return Vec::new();
                };
                let args = if after {
                    list_from_vec(vec![
                        overlay_object(id),
                        LispObject::t(),
                        LispObject::integer(start as i64),
                        LispObject::integer((start + new_len) as i64),
                        LispObject::integer(old_len as i64),
                    ])
                } else {
                    list_from_vec(vec![
                        overlay_object(id),
                        LispObject::nil(),
                        LispObject::integer(start as i64),
                        LispObject::integer(end as i64),
                    ])
                };
                hook_property_functions(hook_value)
                    .into_iter()
                    .map(|func| OverlayModificationCall {
                        func,
                        args: args.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    })
}

fn call_overlay_modification_hooks(
    calls: Vec<OverlayModificationCall>,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<()> {
    let inhibit_id = crate::obarray::intern("inhibit-modification-hooks");
    let old = env.read().get_id_local(inhibit_id);
    let old_cell = state.get_value_cell(inhibit_id);
    env.write().set_id(inhibit_id, LispObject::t());
    state.set_value_cell(inhibit_id, LispObject::t());
    let mut result = Ok(());
    for call in calls {
        if let Err(err) = functions::call_function(
            obj_to_value(call.func),
            obj_to_value(call.args),
            env,
            editor,
            macros,
            state,
        ) {
            result = Err(err);
            break;
        }
    }
    match old {
        Some(value) => env.write().set_id(inhibit_id, value),
        None => env.write().unset_id(inhibit_id),
    }
    match old_cell {
        Some(value) => state.set_value_cell(inhibit_id, value),
        None => state.clear_value_cell(inhibit_id),
    }
    result.map(|_| ())
}

pub(super) fn insert_with_overlay_hooks(
    text: &str,
    before_markers: bool,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<()> {
    let pos = crate::buffer::with_current(|buffer| buffer.point);
    let len = text.chars().count();
    let before = collect_overlay_hooks_for_insert(pos, len, false);
    let after = collect_overlay_hooks_for_insert(pos, len, true);
    call_overlay_modification_hooks(before, env, editor, macros, state)?;
    crate::buffer::with_registry_mut(|registry| registry.insert_current(text, before_markers));
    call_overlay_modification_hooks(after, env, editor, macros, state)?;
    run_hook_symbol_with_args(
        LispObject::symbol("after-change-functions"),
        list_from_vec(vec![
            LispObject::integer(pos as i64),
            LispObject::integer((pos + len) as i64),
            LispObject::integer(0),
        ]),
        env,
        editor,
        macros,
        state,
    )
    .map(|_| ())
}

fn replace_region_with_overlay_hooks(
    start: usize,
    end: usize,
    replacement: &str,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<()> {
    let new_len = replacement.chars().count();
    let before = collect_overlay_hooks_for_replace(start, end, new_len, false);
    let after = collect_overlay_hooks_for_replace(start, end, new_len, true);
    call_overlay_modification_hooks(before, env, editor, macros, state)?;
    crate::buffer::with_registry_mut(|registry| {
        registry.delete_current_region(start, end);
        if let Some(buffer) = registry.get_mut(registry.current_id()) {
            buffer.goto_char(start);
        }
        registry.insert_current(replacement, false);
    });
    call_overlay_modification_hooks(after, env, editor, macros, state)
}
/// Type-check with eval access — used by `cl-check-type`. Delegates to
/// `prim_cl_typep` for the pure cases, but handles `(satisfies PRED)`
/// and combinators containing it by actually calling the predicate.
pub(super) fn check_type_with_eval(
    val: &LispObject,
    type_spec: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<bool> {
    if let Some((head, rest)) = type_spec.destructure_cons()
        && let Some(op) = head.as_symbol()
    {
        match op.as_str() {
            "satisfies" => {
                let Some((pred, _)) = rest.destructure_cons() else {
                    return Ok(false);
                };
                let call = LispObject::cons(
                    pred,
                    LispObject::cons(
                        LispObject::cons(
                            LispObject::symbol("quote"),
                            LispObject::cons(val.clone(), LispObject::nil()),
                        ),
                        LispObject::nil(),
                    ),
                );
                let r = eval(obj_to_value(call), env, editor, macros, state)?;
                return Ok(!value_to_obj(r).is_nil());
            }
            "and" => {
                let mut cur = rest;
                while let Some((t, next)) = cur.destructure_cons() {
                    if !check_type_with_eval(val, &t, env, editor, macros, state)? {
                        return Ok(false);
                    }
                    cur = next;
                }
                return Ok(true);
            }
            "or" => {
                let mut cur = rest;
                while let Some((t, next)) = cur.destructure_cons() {
                    if check_type_with_eval(val, &t, env, editor, macros, state)? {
                        return Ok(true);
                    }
                    cur = next;
                }
                return Ok(false);
            }
            "not" => {
                if let Some((t, _)) = rest.destructure_cons() {
                    return Ok(!check_type_with_eval(val, &t, env, editor, macros, state)?);
                }
                return Ok(false);
            }
            _ => {}
        }
    }
    let args = LispObject::cons(
        val.clone(),
        LispObject::cons(type_spec.clone(), LispObject::nil()),
    );
    match crate::primitives_cl::prim_cl_typep(&args)? {
        LispObject::Nil => Ok(false),
        _ => Ok(true),
    }
}
/// Tail-call trampoline: special error variant that carries the next
/// expression to evaluate. `eval` catches this and loops instead of
/// recursing. This keeps stack depth O(1) for chains of
/// progn/if/let/cond/when/unless. Using `Err` is safe because `TailEval`
/// is only produced by `tail_call()` and only caught by `eval()` — it
/// never escapes to user-visible code.
pub(super) fn eval(
    mut expr: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    loop {
        let limit = state
            .eval_ops_limit
            .load(std::sync::atomic::Ordering::Relaxed);
        if limit > 0 {
            let ops = state
                .eval_ops
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if ops >= limit {
                return Err(ElispError::EvalError(
                    "eval operation limit exceeded".into(),
                ));
            }
            if ops & 0x3FF == 0
                && let Some(dl) = state.deadline.get()
                && std::time::Instant::now() >= dl
            {
                return Err(ElispError::EvalError("hard eval limit".into()));
            }
        }
        inc_eval_depth()?;
        let result = eval_inner(expr, env, editor, macros, state);
        dec_eval_depth();
        match result {
            Err(ElispError::TailEval(next)) => {
                expr = next;
            }
            other => return other,
        }
    }
}
/// Signal a tail call: return the expression wrapped in `TailEval`.
/// The caller (`eval`) will loop instead of recursing.
#[inline]
fn tail_call(expr: Value) -> ElispResult<Value> {
    Err(ElispError::TailEval(expr))
}

fn char_index_to_byte(s: &str, char_index: usize) -> usize {
    s.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or(s.len())
}

fn byte_index_to_char(s: &str, byte_index: usize) -> usize {
    s[..byte_index.min(s.len())].chars().count()
}

fn has_too_large_repeat(pattern: &str) -> bool {
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i + 3 < chars.len() {
        if chars[i] == '\\' && chars[i + 1] == '{' {
            let mut j = i + 2;
            let mut digits = String::new();
            while j < chars.len() && chars[j].is_ascii_digit() {
                digits.push(chars[j]);
                j += 1;
            }
            if j + 1 < chars.len()
                && chars[j] == '\\'
                && chars[j + 1] == '}'
                && digits.parse::<usize>().is_ok_and(|count| count > 65_535)
            {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn invalid_regexp_signal(message: &str) -> ElispError {
    ElispError::Signal(Box::new(crate::error::SignalData {
        symbol: LispObject::symbol("invalid-regexp"),
        data: LispObject::cons(LispObject::string(message), LispObject::nil()),
    }))
}

fn encode_raw_bytes_as_private(s: &str) -> String {
    s.chars()
        .map(|c| {
            let code = c as u32;
            if (0x80..=0xff).contains(&code) {
                char::from_u32(0xE000 + code).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

fn decode_private_raw_bytes(s: &str) -> String {
    s.chars()
        .map(|c| {
            let code = c as u32;
            if (0xE080..=0xE0FF).contains(&code) {
                char::from_u32(code - 0xE000).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

// --- Functions from original mod.rs (eval_inner, special form dispatch) ---

pub(super) fn eval_inner(
    expr: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    if let Some(result) = atoms::eval_atom(expr, env, state) {
        return result;
    }
    let expr_obj = value_to_obj(expr);
    let (car, cdr) = expr_obj.destructure();
    match &car {
        LispObject::Symbol(id) => {
            let sym_owned: Option<String>;
            let sym_name: &str = match dispatch::hot_form_name(*id) {
                Some(s) => s,
                None => {
                    sym_owned = Some(crate::obarray::symbol_name(*id));
                    sym_owned.as_deref().unwrap()
                }
            };
            if let Some(result) =
                keymap_forms::eval_keymap_form(sym_name, &cdr, env, editor, macros, state)
            {
                return result;
            }
            if let Some(result) =
                metadata::eval_metadata_form(sym_name, &cdr, env, editor, macros, state)
            {
                return result;
            }
            match sym_name {
                "quote" => match cdr.first() {
                    Some(arg) => Ok(obj_to_value(arg)),
                    None if !cdr.is_nil() => Ok(obj_to_value(cdr)),
                    _ => Err(ElispError::WrongNumberOfArguments),
                },
                "`" => {
                    let form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let expanded = eval_backquote_form(form, 1, env, editor, macros, state)?;
                    Ok(obj_to_value(expanded))
                }
                "if" => eval_if(obj_to_value(cdr), env, editor, macros, state),
                "setq" => eval_setq(obj_to_value(cdr), env, editor, macros, state),
                "setq-local" => eval_setq_local(obj_to_value(cdr), env, editor, macros, state),
                "defun" => eval_defun(obj_to_value(cdr), env, editor, macros, state),
                "let" => {
                    if is_generated_translation_table_let(&cdr) {
                        register_generated_translation_table_let(&cdr, state);
                        Ok(obj_to_value(LispObject::nil()))
                    } else if is_cus_start_properties_let(&cdr) {
                        Ok(Value::nil())
                    } else {
                        eval_let(obj_to_value(cdr), env, editor, macros, state)
                    }
                }
                "progn" => {
                    special_forms::eval_progn_tco(obj_to_value(cdr), env, editor, macros, state)
                }
                "lambda" => {
                    let params = cdr.first().unwrap_or(LispObject::nil());
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    let captured = env.read().capture_as_alist();
                    Ok(obj_to_value(LispObject::closure_expr(
                        captured, params, body,
                    )))
                }
                "cond" => eval_cond(obj_to_value(cdr), env, editor, macros, state),
                "loop" => eval_loop(obj_to_value(cdr), env, editor, macros, state),
                "function" => {
                    let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    if let Some((head, rest)) = arg.destructure_cons()
                        && head.as_symbol().as_deref() == Some("lambda")
                    {
                        let params = rest.first().unwrap_or(LispObject::nil());
                        let body = rest.rest().unwrap_or(LispObject::nil());
                        let captured = env.read().capture_as_alist();
                        return Ok(obj_to_value(LispObject::closure_expr(
                            captured, params, body,
                        )));
                    }
                    Ok(obj_to_value(arg))
                }
                "apply" => eval_apply(obj_to_value(cdr), env, editor, macros, state),
                "funcall" => eval_funcall_form(obj_to_value(cdr), env, editor, macros, state),
                "buffer-string" => eval_buffer_string(editor),
                "buffer-size" => eval_buffer_size(editor),
                "point" => eval_point(editor),
                "point-min" => eval_point_min(editor),
                "point-max" => eval_point_max(editor),
                "internal--labeled-narrow-to-region" | "internal--labeled-widen" => {
                    let mut evaled = Vec::new();
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        evaled.push(value_to_obj(eval(
                            obj_to_value(arg),
                            env,
                            editor,
                            macros,
                            state,
                        )?));
                        cur = rest;
                    }
                    let mut args = LispObject::nil();
                    for value in evaled.into_iter().rev() {
                        args = LispObject::cons(value, args);
                    }
                    let result = crate::primitives_buffer::call_buffer_primitive(sym_name, &args)
                        .ok_or_else(|| ElispError::VoidFunction(sym_name.to_string()))??;
                    Ok(obj_to_value(result))
                }
                "goto-char" => eval_goto_char(obj_to_value(cdr), env, editor, macros, state),
                "delete-char" => eval_delete_char(obj_to_value(cdr), env, editor, macros, state),
                "forward-char" => eval_forward_char(obj_to_value(cdr), env, editor, macros, state),
                "forward-line" => eval_forward_line(obj_to_value(cdr), env, editor, macros, state),
                "move-beginning-of-line" => eval_move_beginning_of_line(editor),
                "move-end-of-line" => eval_move_end_of_line(editor),
                "beginning-of-buffer" => eval_beginning_of_buffer(editor),
                "end-of-buffer" => eval_end_of_buffer(editor),
                "primitive-undo" => eval_undo_primitive(editor),
                "primitive-redo" => eval_redo_primitive(editor),
                "editor--kill-line" => eval_kill_line(editor),
                "editor--kill-word" => {
                    eval_kill_word(obj_to_value(cdr), env, editor, macros, state)
                }
                "editor--kill-region" => eval_kill_region(editor),
                "editor--copy-region-as-kill" => eval_copy_region_as_kill(editor),
                "editor--yank" => eval_yank(editor),
                "editor--yank-pop" => eval_yank_pop(editor),
                "editor--delete-rectangle" => eval_delete_rectangle(editor),
                "editor--kill-rectangle" => eval_kill_rectangle(editor),
                "editor--yank-rectangle" => eval_yank_rectangle(editor),
                "editor--open-rectangle" => eval_open_rectangle(editor),
                "editor--clear-rectangle" => eval_clear_rectangle(editor),
                "editor--string-rectangle" => {
                    eval_string_rectangle(obj_to_value(cdr), env, editor, macros, state)
                }
                "search-forward" | "editor--search-forward" => {
                    eval_search_forward(obj_to_value(cdr), env, editor, macros, state)
                }
                "search-backward" | "editor--search-backward" => {
                    eval_search_backward(obj_to_value(cdr), env, editor, macros, state)
                }
                "re-search-forward" | "editor--re-search-forward" => {
                    eval_re_search_forward(obj_to_value(cdr), env, editor, macros, state)
                }
                "re-search-backward" | "editor--re-search-backward" => {
                    eval_re_search_backward(obj_to_value(cdr), env, editor, macros, state)
                }
                "replace-match" | "editor--replace-match" => {
                    eval_replace_match(obj_to_value(cdr), env, editor, macros, state)
                }
                "editor--replace-string" => {
                    eval_replace_string(obj_to_value(cdr), env, editor, macros, state)
                }
                "editor--query-replace" => {
                    eval_query_replace(obj_to_value(cdr), env, editor, macros, state)
                }
                "editor--replace-regexp" => {
                    eval_replace_regexp(obj_to_value(cdr), env, editor, macros, state)
                }
                "editor--query-replace-regexp" => {
                    eval_query_replace_regexp(obj_to_value(cdr), env, editor, macros, state)
                }
                "editor--upcase-word" => eval_upcase_word(editor),
                "editor--downcase-word" => eval_downcase_word(editor),
                "editor--transpose-chars" => eval_transpose_chars(editor),
                "editor--transpose-words" => eval_transpose_words(editor),
                "editor--set-mark" => eval_set_mark(editor),
                "editor--exchange-point-and-mark" => eval_exchange_point_and_mark(editor),
                "editor--next-buffer" => eval_next_buffer(editor),
                "editor--previous-buffer" => eval_previous_buffer(editor),
                "editor--toggle-preview" => eval_toggle_preview(editor),
                "editor--toggle-line-numbers" => eval_toggle_line_numbers(editor),
                "editor--toggle-preview-line-numbers" => eval_toggle_preview_line_numbers(editor),
                "editor--keyboard-quit" => eval_keyboard_quit(editor),
                "editor--set-current-buffer-major-mode" => eval_set_current_buffer_major_mode(
                    obj_to_value(cdr),
                    env,
                    editor,
                    macros,
                    state,
                ),
                "find-file" => eval_find_file(obj_to_value(cdr), env, editor, macros, state),
                "save-buffer" => eval_save_buffer(editor),
                "save-excursion" => {
                    eval_save_excursion(obj_to_value(cdr), env, editor, macros, state)
                }
                "save-current-buffer" => {
                    eval_save_current_buffer(obj_to_value(cdr), env, editor, macros, state)
                }
                "save-restriction" => {
                    let buffer_id = crate::buffer::with_registry(|r| r.current_id());
                    let saved = crate::buffer::with_registry(|r| {
                        r.get(buffer_id).map(|buf| {
                            (
                                buf.manual_restriction,
                                buf.restriction_layers.clone(),
                                buf.restriction,
                            )
                        })
                    });
                    let result =
                        builtins::eval_progn_value(obj_to_value(cdr), env, editor, macros, state);
                    if let Some((manual, layers, restriction)) = saved {
                        crate::buffer::with_registry_mut(|r| {
                            if let Some(buf) = r.get_mut(buffer_id) {
                                buf.manual_restriction = manual;
                                buf.restriction_layers = layers;
                                buf.restriction = restriction;
                            }
                        });
                    }
                    result
                }
                "with-restriction" => {
                    let start_form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let end_form = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let start =
                        value_to_obj(eval(obj_to_value(start_form), env, editor, macros, state)?)
                            .as_integer()
                            .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
                    let end =
                        value_to_obj(eval(obj_to_value(end_form), env, editor, macros, state)?)
                            .as_integer()
                            .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
                    let mut body = cdr
                        .rest()
                        .and_then(|r| r.rest())
                        .unwrap_or_else(LispObject::nil);
                    let mut label = None;
                    if body.first().and_then(|v| v.as_symbol()).as_deref() == Some(":label") {
                        let label_form = body.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                        let label_value = value_to_obj(eval(
                            obj_to_value(label_form),
                            env,
                            editor,
                            macros,
                            state,
                        )?);
                        label = label_value
                            .as_symbol()
                            .or_else(|| label_value.as_string().map(ToOwned::to_owned));
                        body = body
                            .rest()
                            .and_then(|r| r.rest())
                            .unwrap_or_else(LispObject::nil);
                    }
                    let buffer_id = crate::buffer::with_registry(|r| r.current_id());
                    let saved = crate::buffer::with_registry_mut(|r| {
                        let buf = r.get_mut(buffer_id)?;
                        let saved = (
                            buf.manual_restriction,
                            buf.restriction_layers.clone(),
                            buf.restriction,
                        );
                        let pmin = buf.point_min();
                        let pmax = buf.point_max();
                        let lo = (start.min(end).max(1) as usize).clamp(pmin, pmax);
                        let hi = (start.max(end).max(1) as usize).clamp(pmin, pmax);
                        buf.push_restriction_layer(label, (lo, hi));
                        Some(saved)
                    });
                    let result =
                        builtins::eval_progn_value(obj_to_value(body), env, editor, macros, state);
                    if let Some((manual, layers, restriction)) = saved {
                        crate::buffer::with_registry_mut(|r| {
                            if let Some(buf) = r.get_mut(buffer_id) {
                                buf.manual_restriction = manual;
                                buf.restriction_layers = layers;
                                buf.restriction = restriction;
                            }
                        });
                    }
                    result
                }
                "without-restriction" => {
                    let mut body = cdr.clone();
                    let mut label = None;
                    if body.first().and_then(|v| v.as_symbol()).as_deref() == Some(":label") {
                        let label_form = body.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                        let label_value = value_to_obj(eval(
                            obj_to_value(label_form),
                            env,
                            editor,
                            macros,
                            state,
                        )?);
                        label = label_value
                            .as_symbol()
                            .or_else(|| label_value.as_string().map(ToOwned::to_owned));
                        body = body
                            .rest()
                            .and_then(|r| r.rest())
                            .unwrap_or_else(LispObject::nil);
                    }
                    let buffer_id = crate::buffer::with_registry(|r| r.current_id());
                    let saved = crate::buffer::with_registry_mut(|r| {
                        let buf = r.get_mut(buffer_id)?;
                        let saved = (
                            buf.manual_restriction,
                            buf.restriction_layers.clone(),
                            buf.restriction,
                        );
                        buf.manual_restriction = None;
                        if let Some(label) = &label {
                            buf.remove_restriction_label(label);
                        } else {
                            buf.restriction_layers.clear();
                            buf.recompute_restriction();
                        }
                        Some(saved)
                    });
                    let result =
                        builtins::eval_progn_value(obj_to_value(body), env, editor, macros, state);
                    if let Some((manual, layers, restriction)) = saved {
                        crate::buffer::with_registry_mut(|r| {
                            if let Some(buf) = r.get_mut(buffer_id) {
                                buf.manual_restriction = manual;
                                buf.restriction_layers = layers;
                                buf.restriction = restriction;
                            }
                        });
                    }
                    result
                }
                "save-match-data" => {
                    let saved_data = state.match_data.read().clone();
                    let saved_str = state.match_string.read().clone();
                    let result =
                        builtins::eval_progn_value(obj_to_value(cdr), env, editor, macros, state);
                    state.set_match_data(saved_data, saved_str);
                    result
                }
                "current-buffer" => eval_current_buffer_bridge(editor),
                "set-buffer" => {
                    eval_set_buffer_bridge(obj_to_value(cdr), env, editor, macros, state)
                }
                "buffer-name" => {
                    let args = if let Some(arg) = cdr.first() {
                        let buffer =
                            value_to_obj(eval(obj_to_value(arg), env, editor, macros, state)?);
                        LispObject::cons(buffer, LispObject::nil())
                    } else {
                        LispObject::nil()
                    };
                    let result =
                        crate::primitives_buffer::call_buffer_primitive("buffer-name", &args)
                            .ok_or_else(|| ElispError::VoidFunction("buffer-name".to_string()))??;
                    Ok(obj_to_value(result))
                }
                "get-buffer" => {
                    let name = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let args = LispObject::cons(name.clone(), LispObject::nil());
                    let result =
                        crate::primitives_buffer::call_buffer_primitive("get-buffer", &args)
                            .ok_or_else(|| ElispError::VoidFunction("get-buffer".to_string()))??;
                    if result.is_nil() {
                        let name = match &name {
                            LispObject::String(name) => Some(name.clone()),
                            LispObject::Symbol(id) => Some(crate::obarray::symbol_name(*id)),
                            _ => None,
                        };
                        if let Some(name) = name {
                            let exists_in_editor = editor.read().as_ref().is_some_and(|callback| {
                                callback.list_buffer_names().iter().any(|n| n == &name)
                            });
                            if exists_in_editor {
                                return Ok(obj_to_value(shadow_buffer_object_for_name(&name)));
                            }
                        }
                    }
                    Ok(obj_to_value(result))
                }
                "get-buffer-create" => {
                    eval_get_buffer_create_bridge(obj_to_value(cdr), env, editor, macros, state)
                }
                "buffer-list" => eval_buffer_list_bridge(editor),
                "insert-directory" => {
                    eval_insert_directory(obj_to_value(cdr), env, editor, macros, state)
                }
                "buffer-live-p" => {
                    let buf = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let args = LispObject::cons(buf, LispObject::nil());
                    let result =
                        crate::primitives_buffer::call_buffer_primitive("buffer-live-p", &args)
                            .ok_or_else(|| ElispError::VoidFunction("buffer-live-p".to_string()))??;
                    Ok(obj_to_value(result))
                }
                "with-current-buffer" => {
                    let buf = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let previous = crate::buffer::with_registry(|registry| registry.current_id());
                    let previous_editor_name = current_editor_buffer_name(editor);
                    switch_editor_and_shadow_to_buffer(&buf, editor)?;
                    let run_pending_buffer_hook = crate::buffer::with_registry_mut(|registry| {
                        let current = registry.current_id();
                        let Some(buffer) = registry.get_mut(current) else {
                            return false;
                        };
                        let pending = buffer
                            .locals
                            .remove("rele--pending-buffer-list-update-hook")
                            .is_some_and(|value| !value.is_nil());
                        let indirect_pending = buffer.base_buffer.is_some()
                            && !buffer
                                .locals
                                .contains_key("rele--delivered-buffer-list-update-hook")
                            && buffer
                                .locals
                                .get("rele--inhibit-buffer-hooks")
                                .is_none_or(|value| value.is_nil());
                        if pending || indirect_pending {
                            buffer.locals.insert(
                                "rele--delivered-buffer-list-update-hook".to_string(),
                                LispObject::t(),
                            );
                            true
                        } else {
                            false
                        }
                    });
                    if run_pending_buffer_hook {
                        run_hook_symbol(
                            LispObject::symbol("buffer-list-update-hook"),
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                    }
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    let result = eval_progn(obj_to_value(body), env, editor, macros, state);
                    if let Some(name) = previous_editor_name {
                        let target = LispObject::string(&name);
                        let _ = switch_editor_and_shadow_to_buffer(&target, editor);
                    }
                    crate::buffer::with_registry_mut(|registry| {
                        registry.set_current(previous);
                    });
                    result
                }
                "with-temp-buffer" => {
                    crate::buffer::push_temp_buffer();
                    let result = eval_progn(obj_to_value(cdr), env, editor, macros, state);
                    crate::buffer::pop_buffer();
                    result
                }
                "with-temp-file" | "with-temp-message" => {
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    crate::buffer::push_temp_buffer();
                    let result = eval_progn(obj_to_value(body), env, editor, macros, state);
                    crate::buffer::pop_buffer();
                    result
                }
                "erase-buffer" => {
                    let mut e = editor.write();
                    if let Some(cb) = e.as_mut()
                        && let Some(name) = cb.current_buffer_name()
                    {
                        cb.set_buffer_text(&name, "");
                        drop(e);
                        let id = crate::buffer::with_registry_mut(|registry| {
                            let id = registry.create(&name);
                            registry.set_current(id);
                            id
                        });
                        crate::buffer::with_registry_mut(|registry| {
                            if let Some(buffer) = registry.get_mut(id) {
                                buffer.erase();
                            }
                        });
                        return Ok(Value::nil());
                    }
                    drop(e);
                    crate::buffer::with_current_mut(|b| b.erase());
                    Ok(Value::nil())
                }
                "buffer-substring" | "buffer-substring-no-properties" => {
                    let start = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?)
                    .as_integer()
                    .unwrap_or(1) as usize;
                    let end = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?)
                    .as_integer()
                    .unwrap_or(1) as usize;
                    let s = crate::buffer::with_current(|b| b.substring(start, end));
                    Ok(obj_to_value(LispObject::string(&s)))
                }
                "delete-region" => {
                    let start = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?)
                    .as_integer()
                    .unwrap_or(1) as usize;
                    let end = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?)
                    .as_integer()
                    .unwrap_or(1) as usize;
                    replace_region_with_overlay_hooks(start, end, "", env, editor, macros, state)?;
                    Ok(Value::nil())
                }
                "generate-new-buffer-name" => {
                    let name = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let args = LispObject::cons(name, LispObject::nil());
                    let result = crate::primitives_buffer::call_buffer_primitive(
                        "generate-new-buffer-name",
                        &args,
                    )
                    .ok_or_else(|| {
                        ElispError::VoidFunction("generate-new-buffer-name".to_string())
                    })??;
                    Ok(obj_to_value(result))
                }
                "generate-new-buffer" => {
                    let name = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let inhibit = if let Some(expr) = cdr.nth(1) {
                        !value_to_obj(eval(obj_to_value(expr), env, editor, macros, state)?)
                            .is_nil()
                    } else {
                        false
                    };
                    let args = LispObject::cons(
                        name,
                        LispObject::cons(LispObject::from(inhibit), LispObject::nil()),
                    );
                    let result = crate::primitives_buffer::call_buffer_primitive(
                        "generate-new-buffer",
                        &args,
                    )
                    .ok_or_else(|| ElispError::VoidFunction("generate-new-buffer".to_string()))??;
                    if let Some(id) = crate::primitives_buffer::buffer_object_id(&result) {
                        crate::buffer::with_registry_mut(|registry| {
                            if let Some(buffer) = registry.get_mut(id) {
                                buffer.locals.insert(
                                    "rele--inhibit-buffer-hooks".to_string(),
                                    LispObject::from(inhibit),
                                );
                            }
                        });
                    }
                    if !inhibit {
                        run_hook_symbol(
                            LispObject::symbol("buffer-list-update-hook"),
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                    }
                    Ok(obj_to_value(result))
                }
                "make-indirect-buffer" => {
                    let base = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let clone = if let Some(expr) = cdr.nth(2) {
                        value_to_obj(eval(obj_to_value(expr), env, editor, macros, state)?)
                    } else {
                        LispObject::nil()
                    };
                    let inhibit = if let Some(expr) = cdr.nth(3) {
                        !value_to_obj(eval(obj_to_value(expr), env, editor, macros, state)?)
                            .is_nil()
                    } else {
                        false
                    };
                    let args = LispObject::cons(
                        base,
                        LispObject::cons(
                            name,
                            LispObject::cons(
                                clone,
                                LispObject::cons(LispObject::from(inhibit), LispObject::nil()),
                            ),
                        ),
                    );
                    let result = crate::primitives_buffer::call_buffer_primitive(
                        "make-indirect-buffer",
                        &args,
                    )
                    .ok_or_else(|| {
                        ElispError::VoidFunction("make-indirect-buffer".to_string())
                    })??;
                    if let Some(id) = crate::primitives_buffer::buffer_object_id(&result) {
                        crate::buffer::with_registry_mut(|registry| {
                            if let Some(buffer) = registry.get_mut(id) {
                                buffer.locals.insert(
                                    "rele--inhibit-buffer-hooks".to_string(),
                                    LispObject::from(inhibit),
                                );
                            }
                        });
                    }
                    Ok(obj_to_value(result))
                }
                "kill-buffer" => {
                    let target = match cdr.first() {
                        Some(expr) => {
                            value_to_obj(eval(obj_to_value(expr), env, editor, macros, state)?)
                        }
                        None => crate::buffer::with_registry(|registry| {
                            crate::primitives_buffer::make_buffer_object(registry.current_id())
                        }),
                    };
                    let target_id = crate::primitives_buffer::resolve_buffer(&target)
                        .unwrap_or_else(|| crate::buffer::with_registry(|r| r.current_id()));
                    let target_name_for_editor = match &target {
                        LispObject::String(name) => Some(name.clone()),
                        LispObject::Symbol(id) => Some(crate::obarray::symbol_name(*id)),
                        _ => crate::buffer::with_registry(|registry| {
                            registry.get(target_id).map(|buffer| buffer.name.clone())
                        }),
                    };
                    let previous = crate::buffer::with_registry(|registry| registry.current_id());
                    let pushed_target = previous != target_id;
                    let inhibit = crate::buffer::with_registry_mut(|registry| {
                        if pushed_target {
                            registry.push_stack(target_id);
                        }
                        registry
                            .get(target_id)
                            .and_then(|buffer| {
                                buffer.locals.get("rele--inhibit-buffer-hooks").cloned()
                            })
                            .is_some_and(|value| !value.is_nil())
                    });
                    let auto_save_file = eval(
                        obj_to_value(LispObject::symbol("buffer-auto-save-file-name")),
                        env,
                        editor,
                        macros,
                        state,
                    )
                    .ok()
                    .and_then(|value| value_to_obj(value).as_string().cloned())
                    .or_else(|| {
                        eval(
                            obj_to_value(LispObject::symbol("auto-save")),
                            env,
                            editor,
                            macros,
                            state,
                        )
                        .ok()
                        .and_then(|value| value_to_obj(value).as_string().cloned())
                    });
                    if !inhibit {
                        run_buffer_local_hook_symbol(
                            LispObject::symbol("kill-buffer-query-functions"),
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        run_buffer_local_hook_symbol(
                            LispObject::symbol("kill-buffer-hook"),
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                    }
                    let delete_auto_save_setting = eval(
                        obj_to_value(LispObject::symbol("kill-buffer-delete-auto-save-files")),
                        env,
                        editor,
                        macros,
                        state,
                    )
                    .ok()
                    .map(value_to_obj)
                    .unwrap_or_else(LispObject::nil);
                    let test_prompt_answer = eval(
                        obj_to_value(LispObject::symbol("auto-save-answer")),
                        env,
                        editor,
                        macros,
                        state,
                    )
                    .ok()
                    .map(value_to_obj)
                    .unwrap_or_else(LispObject::nil);
                    let delete_auto_save = !auto_save_file.as_deref().unwrap_or("").is_empty()
                        && (!delete_auto_save_setting.is_nil() || !test_prompt_answer.is_nil());
                    if delete_auto_save {
                        let prompt_args = LispObject::cons(
                            LispObject::string("Delete auto-save file? "),
                            LispObject::nil(),
                        );
                        let answer = eval(
                            obj_to_value(LispObject::cons(
                                LispObject::symbol("yes-or-no-p"),
                                prompt_args,
                            )),
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        if !value_to_obj(answer).is_nil()
                            && let Some(path) = auto_save_file.as_deref()
                        {
                            let _ = std::fs::remove_file(path);
                        }
                    }
                    if pushed_target {
                        crate::buffer::with_registry_mut(|registry| registry.pop_stack());
                    }
                    let args = LispObject::cons(target, LispObject::nil());
                    let result =
                        crate::primitives_buffer::call_buffer_primitive("kill-buffer", &args)
                            .ok_or_else(|| ElispError::VoidFunction("kill-buffer".to_string()))??;
                    if !result.is_nil()
                        && let Some(name) = target_name_for_editor
                        && let Some(callback) = editor.write().as_mut()
                    {
                        callback.kill_buffer_by_name(&name);
                    }
                    crate::buffer::with_registry_mut(|registry| {
                        if registry.get(previous).is_some() {
                            registry.set_current(previous);
                        }
                    });
                    Ok(obj_to_value(result))
                }
                "file-name-quote" => {
                    let name_val = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let name_obj = value_to_obj(name_val);
                    let s = name_obj
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    Ok(obj_to_value(LispObject::string(&format!("/:{}", s))))
                }
                "file-name-unquote" => {
                    let name_val = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let name_obj = value_to_obj(name_val);
                    let s = name_obj
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let unquoted = s.strip_prefix("/:").unwrap_or(s);
                    Ok(obj_to_value(LispObject::string(unquoted)))
                }
                "insert" | "insert-before-markers" | "insert-before-markers-and-inherit" => {
                    let before_markers = sym_name != "insert";
                    eval_insert(
                        obj_to_value(cdr),
                        before_markers,
                        env,
                        editor,
                        macros,
                        state,
                    )
                }
                "prog1" => eval_prog1(obj_to_value(cdr), env, editor, macros, state),
                "prog2" => eval_prog2(obj_to_value(cdr), env, editor, macros, state),
                "and" => eval_and(obj_to_value(cdr), env, editor, macros, state),
                "or" => eval_or(obj_to_value(cdr), env, editor, macros, state),
                "when" => eval_when(obj_to_value(cdr), env, editor, macros, state),
                "unless" => eval_unless(obj_to_value(cdr), env, editor, macros, state),
                "when-let*" => eval_if_or_when_let_star(cdr, env, editor, macros, state, true),
                "if-let*" => eval_if_or_when_let_star(cdr, env, editor, macros, state, false),
                "while" => eval_while(obj_to_value(cdr), env, editor, macros, state),
                "let*" => eval_let_star(obj_to_value(cdr), env, editor, macros, state),
                "dlet" => eval_dlet(obj_to_value(cdr), env, editor, macros, state),
                "defvar" => eval_defvar(obj_to_value(cdr), env, editor, macros, state),
                "defcustom" => eval_defvar(obj_to_value(cdr), env, editor, macros, state),
                "defgroup" | "defface" => Ok(Value::nil()),
                "rx-define" => Ok(obj_to_value(cdr.first().unwrap_or_else(LispObject::nil))),
                "define-ccl-program" => {
                    let name = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    if let Some(id) = name.as_symbol_id() {
                        state.set_value_cell(id, LispObject::nil());
                    }
                    Ok(obj_to_value(name))
                }
                "define-translation-table" => {
                    Ok(obj_to_value(cdr.first().unwrap_or_else(LispObject::nil)))
                }
                "make-translation-table-from-alist" => Ok(Value::nil()),
                "let-when-compile" => Ok(Value::nil()),
                "isearch-define-mode-toggle" => Ok(Value::nil()),
                "define-minor-mode" => {
                    let name = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    if let Some(n) = name.as_symbol() {
                        let hook_name = format!("{n}-hook");
                        let mode_fn = format!(
                            "(lambda (&optional arg) \
                               (setq {n} (if (and arg (not (eq arg 1))) nil t)) \
                               (run-hooks '{hook_name}))"
                        );
                        if let Ok(func) = crate::read(&mode_fn) {
                            let func_val = eval(obj_to_value(func), env, editor, macros, state)?;
                            let id = crate::obarray::intern(&n);
                            state.set_function_cell(id, value_to_obj(func_val));
                        }
                        env.write().define(&n, LispObject::nil());
                        env.write().define(&hook_name, LispObject::nil());
                    }
                    Ok(obj_to_value(name))
                }
                "define-derived-mode" => {
                    let name = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let parent = cdr.nth(1).unwrap_or(LispObject::nil());
                    let docstring = cdr.nth(2);
                    if let Some(n) = name.as_symbol() {
                        let mode_name_str = docstring
                            .and_then(|d| d.as_string().map(|s| s.to_string()))
                            .unwrap_or_else(|| {
                                n.strip_suffix("-mode")
                                    .unwrap_or(&n)
                                    .replace('-', " ")
                                    .split_whitespace()
                                    .map(|w| {
                                        let mut c = w.chars();
                                        match c.next() {
                                            None => String::new(),
                                            Some(f) => f.to_uppercase().to_string() + c.as_str(),
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            });
                        let hook_name = format!("{n}-hook");
                        let parent_call = if parent.is_nil()
                            || parent.as_symbol().as_deref() == Some("fundamental-mode")
                        {
                            String::new()
                        } else if let Some(ps) = parent.as_symbol() {
                            format!("({ps})")
                        } else {
                            String::new()
                        };
                        let mode_fn = format!(
                            "(lambda () \
                               (kill-all-local-variables) \
                               {parent_call} \
                               (setq major-mode '{n}) \
                               (setq mode-name \"{mode_name_str}\") \
                               (editor--set-current-buffer-major-mode '{n}) \
                               (run-mode-hooks '{hook_name}))"
                        );
                        if let Ok(func) = crate::read(&mode_fn) {
                            let func_val = eval(obj_to_value(func), env, editor, macros, state)?;
                            let id = crate::obarray::intern(&n);
                            state.set_function_cell(id, value_to_obj(func_val));
                        }
                        env.write().define(&hook_name, LispObject::nil());
                        env.write().define(&n, LispObject::nil());
                        let map_name = format!("{n}-map");
                        let map_id = crate::obarray::intern(&map_name);
                        if state.get_value_cell(map_id).is_none() {
                            state.set_value_cell(map_id, LispObject::nil());
                        }
                    }
                    Ok(obj_to_value(name))
                }
                "defconst" => {
                    let name = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    if name.as_symbol().as_deref() == Some("pcase--subtype-bitsets") {
                        let id = name
                            .as_symbol_id()
                            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                        env.write().define_id(id, LispObject::nil());
                        state.set_value_cell(id, LispObject::nil());
                        Ok(obj_to_value(name))
                    } else {
                        eval_defconst(obj_to_value(cdr), env, editor, macros, state)
                    }
                }
                "defalias" => eval_defalias(obj_to_value(cdr), env, editor, macros, state),
                "ert-deftest" => {
                    let name_obj = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let name = name_obj
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    let after_args = cdr
                        .rest()
                        .and_then(|r| r.rest())
                        .unwrap_or(LispObject::nil());
                    let mut body = after_args;
                    let mut docstring = LispObject::nil();
                    if let Some((maybe_doc, tail)) = body.destructure_cons()
                        && maybe_doc.as_string().is_some()
                        && !tail.is_nil()
                    {
                        docstring = maybe_doc;
                        body = tail;
                    }
                    let mut tags = LispObject::nil();
                    #[allow(clippy::while_let_loop)]
                    loop {
                        match body.destructure_cons() {
                            Some((head, tail)) => {
                                let kw_name = head.as_symbol();
                                let is_kw = kw_name.as_deref().is_some_and(|s| s.starts_with(':'));
                                if is_kw {
                                    if kw_name.as_deref() == Some(":tags")
                                        && let Some((val_form, _)) = tail.destructure_cons()
                                        && let Ok(v) =
                                            eval(obj_to_value(val_form), env, editor, macros, state)
                                    {
                                        tags = value_to_obj(v);
                                    }
                                    body = tail.rest().unwrap_or(LispObject::nil());
                                } else {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    let captured = env.read().capture_as_alist();
                    let closure = LispObject::closure_expr(captured, LispObject::nil(), body);
                    let id = crate::obarray::intern(&name);
                    let test_key = crate::obarray::intern("ert--rele-test");
                    state.put_plist(id, test_key, closure.clone());
                    let test_struct_key = crate::obarray::intern("ert--test");
                    let test_obj =
                        crate::primitives::make_ert_test_obj(&name, tags, docstring, closure);
                    state.put_plist(id, test_struct_key, test_obj);
                    Ok(obj_to_value(LispObject::Symbol(id)))
                }
                "should" => {
                    let form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let result = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    if result.is_nil() {
                        return Err(ElispError::Signal(Box::new(crate::error::SignalData {
                            symbol: LispObject::symbol("ert-test-failed"),
                            data: LispObject::cons(form, LispObject::nil()),
                        })));
                    }
                    Ok(obj_to_value(result))
                }
                "should-not" => {
                    let form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let result = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    if !result.is_nil() {
                        return Err(ElispError::Signal(Box::new(crate::error::SignalData {
                            symbol: LispObject::symbol("ert-test-failed"),
                            data: LispObject::cons(form, LispObject::nil()),
                        })));
                    }
                    Ok(Value::nil())
                }
                "should-error" => {
                    let form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    match eval(obj_to_value(form), env, editor, macros, state) {
                        Ok(_) => Err(ElispError::Signal(Box::new(crate::error::SignalData {
                            symbol: LispObject::symbol("ert-test-failed"),
                            data: LispObject::cons(
                                LispObject::string("did not signal"),
                                LispObject::nil(),
                            ),
                        }))),
                        Err(ElispError::StackOverflow) => Err(ElispError::StackOverflow),
                        Err(ref e) if e.is_eval_ops_exceeded() => Err(e.clone()),
                        Err(ElispError::Signal(signal)) => {
                            Ok(obj_to_value(LispObject::cons(signal.symbol, signal.data)))
                        }
                        Err(error) => {
                            let signal = match error.to_signal() {
                                ElispError::Signal(signal) => signal,
                                _ => unreachable!("to_signal must return a signal"),
                            };
                            Ok(obj_to_value(LispObject::cons(signal.symbol, signal.data)))
                        }
                    }
                }
                "skip-unless" => {
                    let form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let result =
                        value_to_obj(eval(obj_to_value(form), env, editor, macros, state)?);
                    if result.is_nil() {
                        return Err(ElispError::Signal(Box::new(crate::error::SignalData {
                            symbol: LispObject::symbol("ert-test-skipped"),
                            data: LispObject::nil(),
                        })));
                    }
                    Ok(Value::t())
                }
                "skip-when" => {
                    let form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let result =
                        value_to_obj(eval(obj_to_value(form), env, editor, macros, state)?);
                    if !result.is_nil() {
                        return Err(ElispError::Signal(Box::new(crate::error::SignalData {
                            symbol: LispObject::symbol("ert-test-skipped"),
                            data: LispObject::nil(),
                        })));
                    }
                    Ok(Value::nil())
                }
                "ignore-errors" => {
                    let body = obj_to_value(cdr);
                    match eval_progn(body, env, editor, macros, state) {
                        Ok(v) => Ok(v),
                        Err(ElispError::Throw(_)) => {
                            Err(ElispError::EvalError("re-raise throw".to_string()))
                        }
                        Err(ElispError::StackOverflow) => Err(ElispError::StackOverflow),
                        Err(ref e) if e.is_eval_ops_exceeded() => Err(e.clone()),
                        Err(_) => Ok(Value::nil()),
                    }
                }
                "ert-info" => {
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    eval_progn(obj_to_value(body), env, editor, macros, state)
                }
                "ert-test-erts-file" => {
                    Err(ElispError::Signal(Box::new(crate::error::SignalData {
                        symbol: LispObject::symbol("ert-test-skipped"),
                        data: LispObject::cons(
                            LispObject::string("erts file tests not supported"),
                            LispObject::nil(),
                        ),
                    })))
                }
                "ert-with-message-capture" => {
                    let var = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    if let Some(name) = var.as_symbol() {
                        env.write().set(&name, LispObject::string(""));
                    }
                    eval_progn(obj_to_value(body), env, editor, macros, state)
                }
                "ert-simulate-command" => {
                    let cmd = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let cmd_obj = value_to_obj(cmd);
                    let (func, args) = if let Some((car, cdr_cmd)) = cmd_obj.destructure_cons() {
                        (car, cdr_cmd)
                    } else {
                        (cmd_obj, LispObject::nil())
                    };
                    functions::call_function(
                        obj_to_value(func),
                        obj_to_value(args),
                        env,
                        editor,
                        macros,
                        state,
                    )
                }
                "ert-with-buffer-selected" | "ert-with-buffer-renamed" => {
                    let _buf = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    eval_progn(obj_to_value(body), env, editor, macros, state)
                }
                "ert--skip-unless" => {
                    let form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let result =
                        value_to_obj(eval(obj_to_value(form), env, editor, macros, state)?);
                    if result.is_nil() {
                        return Err(ElispError::Signal(Box::new(crate::error::SignalData {
                            symbol: LispObject::symbol("ert-test-skipped"),
                            data: LispObject::nil(),
                        })));
                    }
                    Ok(Value::t())
                }
                "add-hook" => {
                    let hook = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let func = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let local = add_hook_local_arg(&cdr, env, editor, macros, state)?;
                    if let Some(name) = hook.as_symbol() {
                        let old = current_hook_value(&name, env);
                        set_hook_value(&name, append_hook_function(old, func), local, env);
                    }
                    Ok(Value::nil())
                }
                "remove-hook" => {
                    let hook = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let func = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let local = add_hook_local_arg(&cdr, env, editor, macros, state)?;
                    if let Some(name) = hook.as_symbol() {
                        let old = current_hook_value(&name, env);
                        set_hook_value(&name, remove_hook_function(old, &func), local, env);
                    }
                    Ok(Value::nil())
                }
                "run-hooks" | "run-mode-hooks" => {
                    let mut cur = cdr.clone();
                    while let Some((hook_sym, rest)) = cur.destructure_cons() {
                        let hook =
                            value_to_obj(eval(obj_to_value(hook_sym), env, editor, macros, state)?);
                        run_hook_symbol(hook, env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "ert-font-lock-test-file" | "ert-simulate-keys" => {
                    Err(ElispError::Signal(Box::new(crate::error::SignalData {
                        symbol: LispObject::symbol("ert-test-skipped"),
                        data: LispObject::cons(
                            LispObject::string("not supported in rele"),
                            LispObject::nil(),
                        ),
                    })))
                }
                "iter-lambda" => {
                    let params = cdr.first().unwrap_or(LispObject::nil());
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    let captured = env.read().capture_as_alist();
                    Ok(obj_to_value(LispObject::closure_expr(
                        captured,
                        params,
                        LispObject::cons(
                            LispObject::cons(
                                LispObject::symbol("iter--make-iterator"),
                                body.clone(),
                            ),
                            LispObject::nil(),
                        ),
                    )))
                }
                "iter-defun" => {
                    let name = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let name_sym = name
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
                    let arglist = cdr.nth(1).unwrap_or(LispObject::nil());
                    let mut body = cdr
                        .rest()
                        .and_then(|r| r.rest())
                        .unwrap_or(LispObject::nil());
                    if let Some((first, tail)) = body.destructure_cons()
                        && first.as_string().is_some()
                        && !tail.is_nil()
                    {
                        body = tail;
                    }
                    let captured = env.read().capture_as_alist();
                    let closure = LispObject::closure_expr(
                        captured,
                        arglist,
                        LispObject::cons(
                            LispObject::cons(LispObject::symbol("iter--make-iterator"), body),
                            LispObject::nil(),
                        ),
                    );
                    let id = crate::obarray::intern(&name_sym);
                    state.set_function_cell(id, closure);
                    Ok(obj_to_value(LispObject::symbol(&name_sym)))
                }
                "iter--make-iterator" => {
                    let body = obj_to_value(cdr);
                    let mut yields: Vec<LispObject> = Vec::new();
                    let mut final_val = LispObject::nil();
                    let body_obj = value_to_obj(body);
                    let mut cur = Some(body_obj);
                    'outer: while let Some(c) = cur {
                        if let Some((form, rest)) = c.destructure_cons() {
                            match eval(obj_to_value(form), env, editor, macros, state) {
                                Ok(v) => {
                                    final_val = value_to_obj(v);
                                }
                                Err(ElispError::Throw(ref td))
                                    if td.tag.as_symbol().as_deref() == Some("iter--yield") =>
                                {
                                    yields.push(td.value.clone());
                                }
                                Err(e) => return Err(e),
                            }
                            cur = Some(rest);
                        } else {
                            break 'outer;
                        }
                    }
                    let mut items = vec![LispObject::integer(0), final_val];
                    items.extend(yields);
                    let vec = LispObject::Vector(std::sync::Arc::new(
                        crate::eval::SyncRefCell::new(items),
                    ));
                    let iter_obj = LispObject::cons(LispObject::symbol("iter--state"), vec);
                    Ok(obj_to_value(iter_obj))
                }
                "iter-yield" => {
                    let val_expr = cdr.first().unwrap_or(LispObject::nil());
                    let val =
                        value_to_obj(eval(obj_to_value(val_expr), env, editor, macros, state)?);
                    Err(ElispError::Throw(Box::new(crate::error::ThrowData {
                        tag: LispObject::symbol("iter--yield"),
                        value: val,
                    })))
                }
                "iter-yield-from" => {
                    let iter_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let iter =
                        value_to_obj(eval(obj_to_value(iter_expr), env, editor, macros, state)?);
                    if let Some((tag, vec)) = iter.destructure_cons()
                        && tag.as_symbol().as_deref() == Some("iter--state")
                        && let LispObject::Vector(v) = vec
                    {
                        let guard = v.lock();
                        let idx = guard[0].as_integer().unwrap_or(0) as usize;
                        if let Some(i) = (idx + 2..guard.len()).next() {
                            return Err(ElispError::Throw(Box::new(crate::error::ThrowData {
                                tag: LispObject::symbol("iter--yield"),
                                value: guard[i].clone(),
                            })));
                        }
                        return Ok(obj_to_value(guard[1].clone()));
                    }
                    Ok(Value::nil())
                }
                "iter-next" => {
                    let iter_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let iter =
                        value_to_obj(eval(obj_to_value(iter_expr), env, editor, macros, state)?);
                    if let Some((tag, vec)) = iter.destructure_cons()
                        && tag.as_symbol().as_deref() == Some("iter--state")
                        && let LispObject::Vector(v) = vec
                    {
                        let mut guard = v.lock();
                        let idx = guard[0].as_integer().unwrap_or(0) as usize;
                        let n_yields = guard.len() - 2;
                        if idx < n_yields {
                            let val = guard[idx + 2].clone();
                            guard[0] = LispObject::integer((idx + 1) as i64);
                            return Ok(obj_to_value(val));
                        } else {
                            let final_val = guard[1].clone();
                            return Err(ElispError::Signal(Box::new(crate::error::SignalData {
                                symbol: LispObject::symbol("iter-end-of-sequence"),
                                data: final_val,
                            })));
                        }
                    }
                    Err(ElispError::Signal(Box::new(crate::error::SignalData {
                        symbol: LispObject::symbol("wrong-type-argument"),
                        data: LispObject::cons(LispObject::string("iterator"), LispObject::nil()),
                    })))
                }
                "iter-close" => {
                    let iter_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let iter =
                        value_to_obj(eval(obj_to_value(iter_expr), env, editor, macros, state)?);
                    if let Some((tag, vec)) = iter.destructure_cons()
                        && tag.as_symbol().as_deref() == Some("iter--state")
                        && let LispObject::Vector(v) = vec
                    {
                        let mut guard = v.lock();
                        let n = guard.len();
                        guard[0] = LispObject::integer(n as i64);
                    }
                    Ok(Value::nil())
                }
                "iter-do" => {
                    let spec = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    let var = spec.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let var_name = var
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
                    let iter_expr = spec.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let result_expr = spec.nth(2);
                    let iter =
                        value_to_obj(eval(obj_to_value(iter_expr), env, editor, macros, state)?);
                    if let Some((tag, vec)) = iter.destructure_cons()
                        && tag.as_symbol().as_deref() == Some("iter--state")
                        && let LispObject::Vector(v) = vec
                    {
                        let parent = Arc::new(env.read().clone());
                        let loop_env = Arc::new(RwLock::new(Environment::with_parent(parent)));
                        let guard = v.lock();
                        for i in 2..guard.len() {
                            loop_env.write().set(&var_name, guard[i].clone());
                            let _ = eval_progn(
                                obj_to_value(body.clone()),
                                &loop_env,
                                editor,
                                macros,
                                state,
                            );
                        }
                        drop(guard);
                        if let Some(result) = result_expr {
                            return eval(obj_to_value(result), &loop_env, editor, macros, state);
                        }
                    }
                    Ok(Value::nil())
                }
                "tempo-insert-template" => {
                    let template_sym = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let template_obj = value_to_obj(template_sym);
                    let elements = if let Some(name) = template_obj.as_symbol() {
                        env.read().get(&name).unwrap_or(LispObject::nil())
                    } else {
                        LispObject::nil()
                    };
                    let mut cur = elements;
                    while let Some((elt, rest)) = cur.destructure_cons() {
                        if let Some(s) = elt.as_string() {
                            crate::buffer::with_current_mut(|b| {
                                let pos = b.point;
                                b.text.insert_str(b.char_to_byte(pos), s);
                                b.point += s.chars().count();
                            });
                        }
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "tempo-complete-tag" => Ok(Value::nil()),
                "tempo-define-template" => {
                    let name_val = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let elements = eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let name_obj = value_to_obj(name_val);
                    let name_str = name_obj
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
                    let var_name = format!("tempo-template-{name_str}");
                    let elements_obj = value_to_obj(elements);
                    let var_id = crate::obarray::intern(&var_name);
                    state.set_value_cell(var_id, elements_obj.clone());
                    let insert_fn = format!(
                        "(lambda (&optional arg) \
                           (let ((elts {var_name})) \
                             (while elts \
                               (let ((e (car elts))) \
                                 (when (stringp e) (insert e))) \
                               (setq elts (cdr elts)))))"
                    );
                    if let Ok(func) = crate::read(&insert_fn)
                        && let Ok(func_val) = eval(obj_to_value(func), env, editor, macros, state)
                    {
                        state.set_function_cell(var_id, value_to_obj(func_val));
                    }
                    Ok(obj_to_value(LispObject::symbol(&var_name)))
                }
                "catch" => eval_catch(obj_to_value(cdr), env, editor, macros, state),
                "throw" => eval_throw(obj_to_value(cdr), env, editor, macros, state),
                "condition-case" => {
                    eval_condition_case(obj_to_value(cdr), env, editor, macros, state)
                }
                "signal" => eval_signal(obj_to_value(cdr), env, editor, macros, state),
                "unwind-protect" => {
                    eval_unwind_protect(obj_to_value(cdr), env, editor, macros, state)
                }
                "error" => eval_error_fn(obj_to_value(cdr), env, editor, macros, state),
                "user-error" => eval_user_error_fn(obj_to_value(cdr), env, editor, macros, state),
                "put" => eval_put(obj_to_value(cdr), env, editor, macros, state),
                "get" => eval_get(obj_to_value(cdr), env, editor, macros, state),
                "provide" => eval_provide(obj_to_value(cdr), env, editor, macros, state),
                "featurep" => eval_featurep(obj_to_value(cdr), env, editor, macros, state),
                "require" => eval_require(obj_to_value(cdr), env, editor, macros, state),
                "load" => builtins::eval_load(obj_to_value(cdr), env, editor, macros, state),
                "mapcar" => eval_mapcar(obj_to_value(cdr), env, editor, macros, state),
                "mapc" => eval_mapc(obj_to_value(cdr), env, editor, macros, state),
                "try-completion" => {
                    builtins::eval_try_completion(obj_to_value(cdr), env, editor, macros, state)
                }
                "all-completions" => {
                    builtins::eval_all_completions(obj_to_value(cdr), env, editor, macros, state)
                }
                "test-completion" => {
                    builtins::eval_test_completion(obj_to_value(cdr), env, editor, macros, state)
                }
                "dolist" => eval_dolist(obj_to_value(cdr), env, editor, macros, state),
                "dotimes" => eval_dotimes(obj_to_value(cdr), env, editor, macros, state),
                "maphash" => {
                    let func = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let table = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    if let LispObject::HashTable(ht) = &table {
                        let pairs: Vec<_> = ht
                            .lock()
                            .data
                            .iter()
                            .map(|(k, v)| (k.to_lisp_object(), v.clone()))
                            .collect();
                        for (key, val) in pairs {
                            let call_args =
                                LispObject::cons(key, LispObject::cons(val, LispObject::nil()));
                            functions::call_function(
                                obj_to_value(func.clone()),
                                obj_to_value(call_args),
                                env,
                                editor,
                                macros,
                                state,
                            )?;
                        }
                    }
                    Ok(Value::nil())
                }
                "mapcan" => {
                    let func_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let list_expr = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let func =
                        value_to_obj(eval(obj_to_value(func_expr), env, editor, macros, state)?);
                    let list =
                        value_to_obj(eval(obj_to_value(list_expr), env, editor, macros, state)?);
                    let mut results = Vec::new();
                    let mut current = list;
                    while let Some((car, rest)) = current.destructure_cons() {
                        let call_args = LispObject::cons(car, LispObject::nil());
                        let result = value_to_obj(functions::call_function(
                            obj_to_value(func.clone()),
                            obj_to_value(call_args),
                            env,
                            editor,
                            macros,
                            state,
                        )?);
                        let mut r = result;
                        while let Some((item, rest2)) = r.destructure_cons() {
                            results.push(item);
                            r = rest2;
                        }
                        current = rest;
                    }
                    let mut out = LispObject::nil();
                    for r in results.into_iter().rev() {
                        out = LispObject::cons(r, out);
                    }
                    Ok(obj_to_value(out))
                }
                "mapatoms" => {
                    let _func = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "call-interactively" => {
                    let func = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let func_obj = value_to_obj(func);
                    validate_interactive_spec(&func_obj)?;
                    functions::call_function(
                        obj_to_value(func_obj),
                        obj_to_value(LispObject::nil()),
                        env,
                        editor,
                        macros,
                        state,
                    )
                }
                "handler-bind" | "handler-bind-1" => {
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    eval_progn(obj_to_value(body), env, editor, macros, state)
                }
                "text-quoting-style" => Ok(obj_to_value(LispObject::symbol("grave"))),
                "let-alist" => {
                    let alist_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    let alist =
                        value_to_obj(eval(obj_to_value(alist_expr), env, editor, macros, state)?);
                    let parent = Arc::new(env.read().clone());
                    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent)));
                    let mut dot_symbols = HashSet::new();
                    collect_dot_symbols(&body, &mut dot_symbols);
                    for name in dot_symbols {
                        new_env.write().define(&name, LispObject::nil());
                    }
                    let mut cur = alist;
                    while let Some((entry, rest)) = cur.destructure_cons() {
                        if let Some((key, value)) = entry.destructure_cons()
                            && let Some(name) = key.as_symbol()
                        {
                            new_env.write().define(&format!(".{name}"), value);
                        }
                        cur = rest;
                    }
                    eval_progn(obj_to_value(body), &new_env, editor, macros, state)
                }
                "inline-quote" | "inline-letevals" => {
                    let body = cdr.rest().unwrap_or(cdr.clone());
                    eval_progn(obj_to_value(body), env, editor, macros, state)
                }
                "get-buffer-process" => {
                    let _buf = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "barf-if-buffer-read-only" => Ok(Value::nil()),
                "bounds-of-thing-at-point" => {
                    let _thing = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "thing-at-point" => {
                    let _thing = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "cursor-intangible-mode"
                | "cursor-sensor-mode"
                | "font-lock-mode"
                | "transient-mark-mode"
                | "indent-tabs-mode"
                | "auto-fill-mode"
                | "overwrite-mode"
                | "abbrev-mode" => Ok(Value::nil()),
                "declare"
                | "interactive"
                | "make-help-screen"
                | "declare-function"
                | "gv-define-expander"
                | "gv-define-setter"
                | "gv-define-simple-setter"
                | "cl-deftype"
                | "define-symbol-prop" => Ok(Value::nil()),
                "eval-after-load" => {
                    let file_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let file =
                        value_to_obj(eval(obj_to_value(file_expr), env, editor, macros, state)?);
                    let form_expr = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let form =
                        value_to_obj(eval(obj_to_value(form_expr), env, editor, macros, state)?);
                    let sym = obarray::intern("after-load-alist");
                    let old = state
                        .global_env
                        .read()
                        .get_id(sym)
                        .unwrap_or_else(LispObject::nil);
                    let entry =
                        LispObject::cons(file, LispObject::cons(form.clone(), LispObject::nil()));
                    let updated = LispObject::cons(entry, old);
                    state.global_env.write().set_id(sym, updated.clone());
                    state.set_value_cell(sym, updated);
                    Ok(obj_to_value(form))
                }
                "with-eval-after-load" => {
                    let file_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let file =
                        value_to_obj(eval(obj_to_value(file_expr), env, editor, macros, state)?);
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    let form = LispObject::cons(LispObject::symbol("progn"), body);
                    let sym = obarray::intern("after-load-alist");
                    let old = state
                        .global_env
                        .read()
                        .get_id(sym)
                        .unwrap_or_else(LispObject::nil);
                    let entry =
                        LispObject::cons(file, LispObject::cons(form.clone(), LispObject::nil()));
                    let updated = LispObject::cons(entry, old);
                    state.global_env.write().set_id(sym, updated.clone());
                    state.set_value_cell(sym, updated);
                    Ok(obj_to_value(form))
                }
                "defvar-local" => {
                    let var_name = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let id = var_name
                        .as_symbol_id()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    state.special_vars.write().insert(id);
                    let key = crate::obarray::intern("variable-buffer-local");
                    state.put_plist(id, key, LispObject::t());
                    if let Some(val_expr) = cdr.nth(1) {
                        let val = eval(obj_to_value(val_expr), env, editor, macros, state)?;
                        let value = value_to_obj(val);
                        state.set_value_cell(id, value.clone());
                        state.global_env.write().set_id(id, value);
                        state.put_plist(
                            id,
                            crate::obarray::intern("default-toplevel-value"),
                            state.get_value_cell(id).unwrap_or_else(LispObject::nil),
                        );
                    }
                    Ok(obj_to_value(var_name))
                }
                "fmakunbound" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    if let Some(name) = sym.as_symbol() {
                        macros.write().remove(&name);
                    }
                    Ok(obj_to_value(sym))
                }
                "makunbound" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let sym_id = symbol_id_including_constants(&sym)?;
                    let name = crate::obarray::symbol_name(sym_id);
                    if matches!(
                        name.as_str(),
                        "nil"
                            | "t"
                            | "initial-window-system"
                            | "most-positive-fixnum"
                            | "most-negative-fixnum"
                    ) || name.starts_with(':')
                    {
                        return Err(ElispError::Signal(Box::new(crate::error::SignalData {
                            symbol: LispObject::symbol("setting-constant"),
                            data: LispObject::cons(sym, LispObject::nil()),
                        })));
                    }
                    assign_symbol_value(
                        sym_id,
                        LispObject::unbound_marker(),
                        env,
                        editor,
                        macros,
                        state,
                        SetOperation::Makunbound,
                    )?;
                    Ok(obj_to_value(sym))
                }
                "garbage-collect" => {
                    let cons_total = crate::object::global_cons_count() as i64;
                    let sym_bytes = Value::symbol_id(obarray::intern("bytes-allocated").0);
                    let sym_gc = Value::symbol_id(obarray::intern("gc-count").0);
                    let sym_cons = Value::symbol_id(obarray::intern("cons-total").0);
                    let result = state.with_heap(|heap| {
                        heap.collect();
                        let allocated = heap.bytes_allocated() as i64;
                        let gc_count = heap.gc_count() as i64;
                        let bytes_pair =
                            heap.cons_value(sym_bytes.raw(), Value::fixnum(allocated).raw());
                        let gc_pair = heap.cons_value(sym_gc.raw(), Value::fixnum(gc_count).raw());
                        let cons_pair =
                            heap.cons_value(sym_cons.raw(), Value::fixnum(cons_total).raw());
                        let list3 = heap.cons_value(cons_pair.raw(), Value::nil().raw());
                        let list2 = heap.cons_value(gc_pair.raw(), list3.raw());
                        heap.cons_value(bytes_pair.raw(), list2.raw())
                    });
                    Ok(result)
                }
                "eval" => {
                    let form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let form = eval(obj_to_value(form), env, editor, macros, state)?;
                    eval(form, env, editor, macros, state)
                }
                "format" | "format-message" => {
                    eval_format(obj_to_value(cdr), env, editor, macros, state)
                }
                "message" => eval_format(obj_to_value(cdr), env, editor, macros, state),
                "1+" => {
                    let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let val = eval(obj_to_value(arg), env, editor, macros, state)?;
                    let val_obj = value_to_obj(val);
                    Ok(obj_to_value(crate::primitives::call_primitive(
                        "1+",
                        &LispObject::cons(val_obj, LispObject::nil()),
                    )?))
                }
                "1-" => {
                    let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let val = eval(obj_to_value(arg), env, editor, macros, state)?;
                    let val_obj = value_to_obj(val);
                    Ok(obj_to_value(crate::primitives::call_primitive(
                        "1-",
                        &LispObject::cons(val_obj, LispObject::nil()),
                    )?))
                }
                "defsubst" => eval_defun(obj_to_value(cdr), env, editor, macros, state),
                "define-inline" => eval_defun(obj_to_value(cdr), env, editor, macros, state),
                "cl-defun" => eval_defun(obj_to_value(cdr), env, editor, macros, state),
                "cl-defmacro" => eval_defmacro(obj_to_value(cdr), macros),
                "cl--find-class" | "cl-find-class" => {
                    let name = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    if let Some(name_str) = name.as_symbol() {
                        let id = crate::obarray::intern(&name_str);
                        let key = crate::obarray::intern("cl--class");
                        let v = state.get_plist(id, key);
                        return Ok(obj_to_value(v));
                    }
                    Ok(Value::nil())
                }
                "cl--class-allparents" | "cl--class-children" => {
                    let _ = eval(
                        obj_to_value(cdr.first().unwrap_or_else(LispObject::nil)),
                        env,
                        editor,
                        macros,
                        state,
                    );
                    Ok(Value::nil())
                }
                "define-error" => {
                    let name_form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let name =
                        value_to_obj(eval(obj_to_value(name_form), env, editor, macros, state)?);
                    let name_sym = name
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    let message = if let Some(m) = cdr.nth(1) {
                        value_to_obj(eval(obj_to_value(m), env, editor, macros, state)?)
                    } else {
                        LispObject::string(&name_sym)
                    };
                    let parent_list = if let Some(p) = cdr.nth(2) {
                        value_to_obj(eval(obj_to_value(p), env, editor, macros, state)?)
                    } else {
                        LispObject::cons(LispObject::symbol("error"), LispObject::nil())
                    };
                    let parents_as_list = if matches!(parent_list, LispObject::Cons(_)) {
                        parent_list
                    } else if parent_list.is_nil() {
                        LispObject::cons(LispObject::symbol("error"), LispObject::nil())
                    } else {
                        LispObject::cons(parent_list, LispObject::nil())
                    };
                    let conditions =
                        LispObject::cons(LispObject::symbol(&name_sym), parents_as_list);
                    let id = crate::obarray::intern(&name_sym);
                    let conds_id = crate::obarray::intern("error-conditions");
                    let msg_id = crate::obarray::intern("error-message");
                    state.put_plist(id, conds_id, conditions);
                    state.put_plist(id, msg_id, message);
                    Ok(obj_to_value(LispObject::Symbol(id)))
                }
                "cl-defstruct" | "defstruct" => {
                    eval_cl_defstruct(obj_to_value(cdr), env, editor, macros, state)
                }
                "cl-defgeneric" => eval_cl_defgeneric_or_method(
                    obj_to_value(cdr),
                    env,
                    editor,
                    macros,
                    state,
                    false,
                ),
                "cl-defmethod" => eval_cl_defgeneric_or_method(
                    obj_to_value(cdr),
                    env,
                    editor,
                    macros,
                    state,
                    true,
                ),
                "define-globalized-minor-mode" => {
                    let name = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    if let Some(n) = name.as_symbol() {
                        env.write().define(&n, LispObject::nil());
                        let id = crate::obarray::intern(&n);
                        state.set_function_cell(id, LispObject::primitive("ignore"));
                    }
                    Ok(Value::nil())
                }
                "cl-destructuring-bind" => {
                    let vars = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let value_form = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr
                        .rest()
                        .and_then(|r| r.rest())
                        .unwrap_or(LispObject::nil());
                    let value = eval(obj_to_value(value_form), env, editor, macros, state)?;
                    apply_lambda(&vars, &body, value, env, editor, macros, state)
                }
                "ert-with-temp-directory" | "ert-with-temp-file" => {
                    let is_dir = sym_name == "ert-with-temp-directory";
                    let name_obj = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let name_sym = name_obj
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
                    let mut body = cdr.rest().unwrap_or(LispObject::nil());
                    let mut prefix = String::from("rele-");
                    let mut suffix = String::new();
                    let mut text = String::new();
                    #[allow(clippy::while_let_loop)]
                    loop {
                        let (car, cdr2) = match body.destructure_cons() {
                            Some(p) => p,
                            None => break,
                        };
                        let kw = match car.as_symbol() {
                            Some(n) if n.starts_with(':') => n,
                            _ => break,
                        };
                        let (val_form, rest2) = match cdr2.destructure_cons() {
                            Some(p) => p,
                            None => break,
                        };
                        let evaled = eval(obj_to_value(val_form), env, editor, macros, state)
                            .unwrap_or(Value::nil());
                        let string_val = value_to_obj(evaled).as_string().map(|s| s.to_string());
                        match kw.as_str() {
                            ":prefix" => {
                                if let Some(s) = string_val {
                                    prefix = s;
                                }
                            }
                            ":suffix" => {
                                if let Some(s) = string_val {
                                    suffix = s;
                                }
                            }
                            ":text" => {
                                if let Some(s) = string_val {
                                    text = s;
                                }
                            }
                            _ => {}
                        }
                        body = rest2;
                    }
                    let tmp = std::env::temp_dir();
                    let nonce = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0);
                    let pid = std::process::id();
                    let path = tmp.join(format!("{prefix}{pid}-{nonce}{suffix}"));
                    let path_str = path.to_string_lossy().to_string();
                    if is_dir {
                        let _ = std::fs::create_dir_all(&path);
                    } else {
                        let _ = std::fs::write(&path, text.as_bytes());
                    }
                    let parent_env = Arc::new(env.read().clone());
                    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));
                    new_env
                        .write()
                        .define(&name_sym, LispObject::string(&path_str));
                    let result = eval_progn(obj_to_value(body), &new_env, editor, macros, state);
                    if is_dir {
                        let _ = std::fs::remove_dir_all(&path);
                    } else {
                        let _ = std::fs::remove_file(&path);
                    }
                    result
                }
                "cl-block" => {
                    let name_obj = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let name = name_obj
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    match eval_progn(obj_to_value(body), env, editor, macros, state) {
                        Ok(v) => Ok(v),
                        Err(ElispError::Throw(throw_data)) => {
                            if throw_data.tag.as_symbol().as_deref() == Some(&name) {
                                Ok(obj_to_value(throw_data.value))
                            } else {
                                Err(ElispError::Throw(throw_data))
                            }
                        }
                        Err(e) => Err(e),
                    }
                }
                "cl-return-from" => {
                    let name_obj = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let value = match cdr.nth(1) {
                        Some(v) => value_to_obj(eval(obj_to_value(v), env, editor, macros, state)?),
                        None => LispObject::nil(),
                    };
                    Err(ElispError::Throw(Box::new(crate::error::ThrowData {
                        tag: name_obj,
                        value,
                    })))
                }
                "cl-return" => {
                    let value = match cdr.first() {
                        Some(v) => value_to_obj(eval(obj_to_value(v), env, editor, macros, state)?),
                        None => LispObject::nil(),
                    };
                    Err(ElispError::Throw(Box::new(crate::error::ThrowData {
                        tag: LispObject::symbol("nil"),
                        value,
                    })))
                }
                "cl-case" | "cl-ecase" => {
                    let key_form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let key =
                        value_to_obj(eval(obj_to_value(key_form), env, editor, macros, state)?);
                    let mut clauses = cdr.rest().unwrap_or(LispObject::nil());
                    while let Some((clause, rest)) = clauses.destructure_cons() {
                        let keyset = clause.first().unwrap_or(LispObject::nil());
                        let body = clause.rest().unwrap_or(LispObject::nil());
                        let matched = match &keyset {
                            LispObject::T => true,
                            LispObject::Symbol(_) => {
                                let n = keyset.as_symbol().unwrap_or_default();
                                n == "otherwise" || (keyset == key)
                            }
                            LispObject::Cons(_) => {
                                let mut cur = keyset.clone();
                                let mut matched = false;
                                while let Some((car, cdr2)) = cur.destructure_cons() {
                                    if car == key {
                                        matched = true;
                                        break;
                                    }
                                    cur = cdr2;
                                }
                                matched
                            }
                            _ => keyset == key,
                        };
                        if matched {
                            return eval_progn(obj_to_value(body), env, editor, macros, state);
                        }
                        clauses = rest;
                    }
                    if sym_name == "cl-ecase" {
                        return Err(ElispError::Signal(Box::new(crate::error::SignalData {
                            symbol: LispObject::symbol("cl-ecase-no-match"),
                            data: LispObject::cons(key, LispObject::nil()),
                        })));
                    }
                    Ok(Value::nil())
                }
                "cl-typecase" | "cl-etypecase" => {
                    let key_form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let key =
                        value_to_obj(eval(obj_to_value(key_form), env, editor, macros, state)?);
                    let mut clauses = cdr.rest().unwrap_or(LispObject::nil());
                    while let Some((clause, rest)) = clauses.destructure_cons() {
                        let type_spec = clause.first().unwrap_or(LispObject::nil());
                        let body = clause.rest().unwrap_or(LispObject::nil());
                        let args = LispObject::cons(
                            key.clone(),
                            LispObject::cons(type_spec.clone(), LispObject::nil()),
                        );
                        let type_name = type_spec.as_symbol();
                        let matched = matches!(type_name.as_deref(), Some("t") | Some("otherwise"))
                            || matches!(
                                crate ::primitives_cl::prim_cl_typep(& args), Ok(r) if !
                                matches!(r, LispObject::Nil)
                            );
                        if matched {
                            return eval_progn(obj_to_value(body), env, editor, macros, state);
                        }
                        clauses = rest;
                    }
                    if sym_name == "cl-etypecase" {
                        return Err(ElispError::Signal(Box::new(crate::error::SignalData {
                            symbol: LispObject::symbol("cl-etypecase-no-match"),
                            data: LispObject::cons(key, LispObject::nil()),
                        })));
                    }
                    Ok(Value::nil())
                }
                "cl-flet" => {
                    let bindings = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    let parent = Arc::new(env.read().clone());
                    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent)));
                    let mut cur = bindings;
                    while let Some((binding, rest)) = cur.destructure_cons() {
                        let name = binding
                            .first()
                            .and_then(|n| n.as_symbol())
                            .unwrap_or_default();
                        let rest_of_binding = binding.rest().unwrap_or(LispObject::nil());
                        let params = rest_of_binding.first().unwrap_or(LispObject::nil());
                        let fbody = rest_of_binding.rest().unwrap_or(LispObject::nil());
                        let lambda = LispObject::cons(
                            LispObject::symbol("lambda"),
                            LispObject::cons(params, fbody),
                        );
                        new_env.write().define(&name, lambda);
                        cur = rest;
                    }
                    eval_progn(obj_to_value(body), &new_env, editor, macros, state)
                }
                "cl-labels" => {
                    let bindings = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    let parent = Arc::new(env.read().clone());
                    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent)));
                    let mut cur = bindings;
                    while let Some((binding, rest)) = cur.destructure_cons() {
                        let name = binding
                            .first()
                            .and_then(|n| n.as_symbol())
                            .unwrap_or_default();
                        let rest_of_binding = binding.rest().unwrap_or(LispObject::nil());
                        let params = rest_of_binding.first().unwrap_or(LispObject::nil());
                        let fbody = rest_of_binding.rest().unwrap_or(LispObject::nil());
                        let lambda = LispObject::cons(
                            LispObject::symbol("lambda"),
                            LispObject::cons(params, fbody),
                        );
                        new_env.write().define(&name, lambda);
                        cur = rest;
                    }
                    eval_progn(obj_to_value(body), &new_env, editor, macros, state)
                }
                "cl-letf" | "cl-letf*" => {
                    enum ClLetfSave {
                        Value(String, Option<LispObject>),
                        Function(SymbolId, Option<LispObject>, Vec<(SymbolId, LispObject)>),
                    }
                    let bindings = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    let mut saves: Vec<ClLetfSave> = Vec::new();
                    let mut cur = bindings;
                    while let Some((binding, rest)) = cur.destructure_cons() {
                        let place = binding.first().unwrap_or(LispObject::nil());
                        let val_expr = binding.nth(1).unwrap_or(LispObject::nil());
                        if let Some(sym) = place.as_symbol() {
                            saves.push(ClLetfSave::Value(sym.clone(), env.read().get(&sym)));
                            let val = value_to_obj(eval(
                                obj_to_value(val_expr),
                                env,
                                editor,
                                macros,
                                state,
                            )?);
                            env.write().set(&sym, val);
                        } else if let Some((head, tail)) = place.destructure_cons()
                            && head.as_symbol().as_deref() == Some("symbol-function")
                        {
                            let target_form = tail.first().unwrap_or(LispObject::nil());
                            let sym_id = if let Some(id) = target_form.as_symbol_id() {
                                id
                            } else if let Some((op, args)) = target_form.destructure_cons() {
                                if matches!(op.as_symbol().as_deref(), Some("quote" | "function")) {
                                    args.first().and_then(|arg| arg.as_symbol_id()).ok_or_else(
                                        || ElispError::WrongTypeArgument("symbol".to_string()),
                                    )?
                                } else {
                                    return Err(ElispError::WrongTypeArgument(
                                        "symbol".to_string(),
                                    ));
                                }
                            } else {
                                return Err(ElispError::WrongTypeArgument("symbol".to_string()));
                            };
                            let val = value_to_obj(eval(
                                obj_to_value(val_expr),
                                env,
                                editor,
                                macros,
                                state,
                            )?);
                            let before = closure_capture_snapshot(&val);
                            saves.push(ClLetfSave::Function(
                                sym_id,
                                state.get_function_cell(sym_id),
                                before,
                            ));
                            state.set_function_cell(sym_id, val);
                        }
                        cur = rest;
                    }
                    let result = eval_progn(obj_to_value(body), env, editor, macros, state);
                    for save in &saves {
                        if let ClLetfSave::Function(id, _, before) = save
                            && let Some(func) = state.get_function_cell(*id)
                        {
                            sync_changed_closure_captures_to_env(&func, before, env, state);
                        }
                    }
                    for save in saves.into_iter().rev() {
                        match save {
                            ClLetfSave::Value(name, old) => match old {
                                Some(v) => env.write().set(&name, v),
                                None => env.write().set(&name, LispObject::nil()),
                            },
                            ClLetfSave::Function(id, old, _) => match old {
                                Some(v) => state.set_function_cell(id, v),
                                None => state.clear_function_cell(id),
                            },
                        }
                    }
                    result
                }
                "cl-lexical-let" => eval_let(obj_to_value(cdr), env, editor, macros, state),
                "cl-lexical-let*" => eval_let_star(obj_to_value(cdr), env, editor, macros, state),
                "cl-macrolet" | "macrolet" => {
                    let bindings = cdr.first().unwrap_or(LispObject::nil());
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    let mut saved: Vec<(String, Option<Macro>)> = Vec::new();
                    let mut bcur = bindings;
                    while let Some((spec, brest)) = bcur.destructure_cons() {
                        if let Some(macro_name) = spec.first().and_then(|n| n.as_symbol()) {
                            let macro_args = spec.nth(1).unwrap_or(LispObject::nil());
                            let macro_body = spec
                                .rest()
                                .and_then(|r| r.rest())
                                .unwrap_or(LispObject::nil());
                            let old = macros.read().get(&macro_name).cloned();
                            saved.push((macro_name.clone(), old));
                            macros.write().insert(
                                macro_name,
                                Macro {
                                    args: macro_args,
                                    body: macro_body,
                                },
                            );
                        }
                        bcur = brest;
                    }
                    let result = eval_progn(obj_to_value(body), env, editor, macros, state);
                    for (name, old) in saved {
                        match old {
                            Some(m) => {
                                macros.write().insert(name, m);
                            }
                            None => {
                                macros.write().remove(&name);
                            }
                        }
                    }
                    result
                }
                "cl-symbol-macrolet" | "symbol-macrolet" => {
                    let bindings = cdr.first().unwrap_or(LispObject::nil());
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    let mut subs: Vec<(String, LispObject)> = Vec::new();
                    let mut bcur = bindings;
                    while let Some((spec, brest)) = bcur.destructure_cons() {
                        if let Some(sym_name) = spec.first().and_then(|n| n.as_symbol()) {
                            let expansion = spec.nth(1).unwrap_or(LispObject::nil());
                            subs.push((sym_name, expansion));
                        }
                        bcur = brest;
                    }
                    let expanded_body = symbol_macrolet_walk(body, &subs);
                    eval_progn(obj_to_value(expanded_body), env, editor, macros, state)
                }
                "cl-the" => {
                    let form = cdr.nth(1).unwrap_or(LispObject::nil());
                    eval(obj_to_value(form), env, editor, macros, state)
                }
                "cl-locally" | "locally" => {
                    eval_progn(obj_to_value(cdr), env, editor, macros, state)
                }
                "cl-psetq" | "psetq" => {
                    let mut pairs: Vec<(LispObject, Value)> = Vec::new();
                    let mut pcur = cdr;
                    while let Some((var, rest)) = pcur.destructure_cons() {
                        let val_form = rest.first().unwrap_or(LispObject::nil());
                        let val = eval(obj_to_value(val_form), env, editor, macros, state)?;
                        pairs.push((var, val));
                        pcur = rest.rest().unwrap_or(LispObject::nil());
                    }
                    let mut last = Value::nil();
                    for (var, val) in pairs {
                        let set_form = LispObject::cons(
                            LispObject::symbol("setq"),
                            LispObject::cons(
                                var,
                                LispObject::cons(
                                    LispObject::cons(
                                        LispObject::symbol("quote"),
                                        LispObject::cons(value_to_obj(val), LispObject::nil()),
                                    ),
                                    LispObject::nil(),
                                ),
                            ),
                        );
                        last = eval(obj_to_value(set_form), env, editor, macros, state)?;
                    }
                    Ok(last)
                }
                "cl-rotatef" | "rotatef" => {
                    let mut vars: Vec<LispObject> = Vec::new();
                    let mut vcur = cdr;
                    while let Some((v, rest)) = vcur.destructure_cons() {
                        vars.push(v);
                        vcur = rest;
                    }
                    if vars.len() < 2 {
                        return Ok(Value::nil());
                    }
                    let mut vals: Vec<Value> = Vec::new();
                    for v in &vars {
                        vals.push(eval(obj_to_value(v.clone()), env, editor, macros, state)?);
                    }
                    let first_val = vals[0];
                    for i in 0..vars.len() {
                        let new_val = if i < vars.len() - 1 {
                            vals[i + 1]
                        } else {
                            first_val
                        };
                        let set_form = LispObject::cons(
                            LispObject::symbol("setq"),
                            LispObject::cons(
                                vars[i].clone(),
                                LispObject::cons(
                                    LispObject::cons(
                                        LispObject::symbol("quote"),
                                        LispObject::cons(value_to_obj(new_val), LispObject::nil()),
                                    ),
                                    LispObject::nil(),
                                ),
                            ),
                        );
                        eval(obj_to_value(set_form), env, editor, macros, state)?;
                    }
                    Ok(Value::nil())
                }
                "cl-shiftf" | "shiftf" => {
                    let mut args_vec: Vec<LispObject> = Vec::new();
                    let mut acur = cdr;
                    while let Some((a, rest)) = acur.destructure_cons() {
                        args_vec.push(a);
                        acur = rest;
                    }
                    if args_vec.is_empty() {
                        return Ok(Value::nil());
                    }
                    let mut vals: Vec<Value> = Vec::new();
                    for a in &args_vec {
                        vals.push(eval(obj_to_value(a.clone()), env, editor, macros, state)?);
                    }
                    let result = vals[0];
                    for i in 0..args_vec.len() - 1 {
                        let new_val = vals[i + 1];
                        let set_form = LispObject::cons(
                            LispObject::symbol("setq"),
                            LispObject::cons(
                                args_vec[i].clone(),
                                LispObject::cons(
                                    LispObject::cons(
                                        LispObject::symbol("quote"),
                                        LispObject::cons(value_to_obj(new_val), LispObject::nil()),
                                    ),
                                    LispObject::nil(),
                                ),
                            ),
                        );
                        eval(obj_to_value(set_form), env, editor, macros, state)?;
                    }
                    Ok(result)
                }
                "cl-callf" | "callf" => {
                    let fn_form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let place = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let extra_args = cdr
                        .rest()
                        .and_then(|r| r.rest())
                        .unwrap_or(LispObject::nil());
                    let call_form =
                        LispObject::cons(fn_form, LispObject::cons(place.clone(), extra_args));
                    let setq_form = LispObject::cons(
                        LispObject::symbol("setq"),
                        LispObject::cons(place, LispObject::cons(call_form, LispObject::nil())),
                    );
                    eval(obj_to_value(setq_form), env, editor, macros, state)
                }
                "cl-incf" | "cl-decf" | "incf" | "decf" => {
                    let var = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let delta = cdr.nth(1).unwrap_or(LispObject::integer(1));
                    let op = if sym_name.ends_with("decf") { "-" } else { "+" };
                    let new_val = LispObject::cons(
                        LispObject::symbol(op),
                        LispObject::cons(var.clone(), LispObject::cons(delta, LispObject::nil())),
                    );
                    let setq_form = LispObject::cons(
                        LispObject::symbol("setq"),
                        LispObject::cons(var, LispObject::cons(new_val, LispObject::nil())),
                    );
                    eval(obj_to_value(setq_form), env, editor, macros, state)
                }
                "cl-assert" => {
                    let form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let val = value_to_obj(eval(
                        obj_to_value(form.clone()),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    if matches!(val, LispObject::Nil) {
                        return Err(ElispError::Signal(Box::new(crate::error::SignalData {
                            symbol: LispObject::symbol("cl-assertion-failed"),
                            data: LispObject::cons(form, LispObject::nil()),
                        })));
                    }
                    Ok(Value::nil())
                }
                "cl-check-type" => {
                    let form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let type_spec = cdr.nth(1).unwrap_or(LispObject::nil());
                    let val = value_to_obj(eval(obj_to_value(form), env, editor, macros, state)?);
                    let is_type =
                        check_type_with_eval(&val, &type_spec, env, editor, macros, state)?;
                    if !is_type {
                        return Err(ElispError::Signal(Box::new(crate::error::SignalData {
                            symbol: LispObject::symbol("wrong-type-argument"),
                            data: LispObject::cons(
                                type_spec,
                                LispObject::cons(val, LispObject::nil()),
                            ),
                        })));
                    }
                    Ok(Value::nil())
                }
                "cl-eval-when" => {
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    eval_progn(obj_to_value(body), env, editor, macros, state)
                }
                "cl-progv" => {
                    let syms_form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let vals_form = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr
                        .rest()
                        .and_then(|r| r.rest())
                        .unwrap_or(LispObject::nil());
                    let syms =
                        value_to_obj(eval(obj_to_value(syms_form), env, editor, macros, state)?);
                    let vals =
                        value_to_obj(eval(obj_to_value(vals_form), env, editor, macros, state)?);
                    let binding_buffer_key = crate::obarray::intern("rele-dynamic-binding-buffer");
                    #[allow(clippy::type_complexity)]
                    let mut saves: Vec<(
                        crate::obarray::SymbolId,
                        Option<LispObject>,
                        Option<LispObject>,
                        LispObject,
                        bool,
                    )> = Vec::new();
                    let mut s_cur = syms;
                    let mut v_cur = vals;
                    while let Some((s, s_rest)) = s_cur.destructure_cons() {
                        let (v, v_rest) = v_cur
                            .destructure_cons()
                            .unwrap_or((LispObject::nil(), LispObject::nil()));
                        if let Some(id) = s.as_symbol_id() {
                            let id = resolve_variable_alias(id, state);
                            let old_cell = state.get_value_cell(id);
                            saves.push((
                                id,
                                env.read().get_id_local(id),
                                old_cell.clone(),
                                state.get_plist(id, binding_buffer_key),
                                state.special_vars.read().contains(&id),
                            ));
                            state.special_vars.write().insert(id);
                            state.specpdl.write().push((id, old_cell));
                            let current_buffer_id =
                                crate::buffer::with_registry(|registry| registry.current_id());
                            state.put_plist(
                                id,
                                binding_buffer_key,
                                LispObject::integer(current_buffer_id as i64),
                            );
                            state.global_env.write().set_id(id, v.clone());
                            state.set_value_cell(id, v.clone());
                            env.write().set_id(id, v.clone());
                            deliver_variable_watchers(
                                id,
                                &v,
                                SetOperation::Let,
                                env,
                                editor,
                                macros,
                                state,
                            )?;
                        }
                        s_cur = s_rest;
                        v_cur = v_rest;
                    }
                    let result = eval_progn(obj_to_value(body), env, editor, macros, state);
                    for (id, old_env, old_cell, old_binding_buffer, was_special) in
                        saves.into_iter().rev()
                    {
                        match old_env {
                            Some(v) => env.write().set_id(id, v),
                            None => env.write().unset_id(id),
                        }
                        match old_cell {
                            Some(v) => {
                                state.set_value_cell(id, v.clone());
                                state.global_env.write().set_id(id, v);
                            }
                            None => {
                                state.clear_value_cell(id);
                                state.global_env.write().unset_id(id);
                            }
                        }
                        if !was_special {
                            state.special_vars.write().remove(&id);
                        }
                        state.put_plist(id, binding_buffer_key, old_binding_buffer);
                        let restored = state.get_value_cell(id).unwrap_or_else(LispObject::nil);
                        deliver_variable_watchers(
                            id,
                            &restored,
                            SetOperation::Unlet,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                    }
                    result
                }
                "cl-do" | "cl-do*" => {
                    let bindings = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let end_clause = cdr.nth(1).unwrap_or(LispObject::nil());
                    let body = cdr
                        .rest()
                        .and_then(|r| r.rest())
                        .unwrap_or(LispObject::nil());
                    let parent = Arc::new(env.read().clone());
                    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent)));
                    let mut triples: Vec<(String, Option<LispObject>, Option<LispObject>)> =
                        Vec::new();
                    let mut bcur = bindings;
                    while let Some((b, rest)) = bcur.destructure_cons() {
                        let name = b.first().and_then(|n| n.as_symbol()).unwrap_or_default();
                        let init = b.nth(1);
                        let step = b.nth(2);
                        triples.push((name, init, step));
                        bcur = rest;
                    }
                    for (name, init, _) in &triples {
                        let v = match init {
                            Some(e) => value_to_obj(eval(
                                obj_to_value(e.clone()),
                                &new_env,
                                editor,
                                macros,
                                state,
                            )?),
                            None => LispObject::nil(),
                        };
                        new_env.write().define(name, v);
                    }
                    let end_cond = end_clause.first().unwrap_or(LispObject::nil());
                    let result_forms = end_clause.rest().unwrap_or(LispObject::nil());
                    loop {
                        let done = value_to_obj(eval(
                            obj_to_value(end_cond.clone()),
                            &new_env,
                            editor,
                            macros,
                            state,
                        )?);
                        if !matches!(done, LispObject::Nil) {
                            return eval_progn(
                                obj_to_value(result_forms),
                                &new_env,
                                editor,
                                macros,
                                state,
                            );
                        }
                        let _ = eval_progn(
                            obj_to_value(body.clone()),
                            &new_env,
                            editor,
                            macros,
                            state,
                        )?;
                        let mut new_vals: Vec<(String, LispObject)> = Vec::new();
                        for (name, _, step) in &triples {
                            if let Some(step_form) = step {
                                let v = value_to_obj(eval(
                                    obj_to_value(step_form.clone()),
                                    &new_env,
                                    editor,
                                    macros,
                                    state,
                                )?);
                                new_vals.push((name.clone(), v));
                            }
                        }
                        for (name, v) in new_vals {
                            new_env.write().set(&name, v);
                        }
                    }
                }
                "cl-loop" => functions::eval_cl_loop(obj_to_value(cdr), env, editor, macros, state),
                "cl-multiple-value-bind" => {
                    let vars = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let value_form = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr
                        .rest()
                        .and_then(|r| r.rest())
                        .unwrap_or(LispObject::nil());
                    let value = eval(obj_to_value(value_form), env, editor, macros, state)?;
                    let value_list = LispObject::cons(value_to_obj(value), LispObject::nil());
                    apply_lambda(
                        &vars,
                        &body,
                        obj_to_value(value_list),
                        env,
                        editor,
                        macros,
                        state,
                    )
                }
                "defclass" => {
                    let name = match cdr.first().and_then(|o| o.as_symbol()) {
                        Some(n) => n,
                        None => return Ok(Value::nil()),
                    };
                    let parents = cdr.nth(1).unwrap_or(LispObject::nil());
                    let parent_name: Option<String> = parents
                        .destructure_cons()
                        .and_then(|(car, _)| car.as_symbol());
                    let slot_list = cdr.nth(2).unwrap_or(LispObject::nil());
                    let mut slots: Vec<crate::primitives_eieio::Slot> = Vec::new();
                    let mut cur = slot_list;
                    while let Some((slot_spec, rest)) = cur.destructure_cons() {
                        let (slot_name_obj, spec_rest) = match slot_spec.destructure_cons() {
                            Some(p) => p,
                            None => break,
                        };
                        if let Some(slot_name) = slot_name_obj.as_symbol() {
                            let mut initform = LispObject::nil();
                            let mut initarg: Option<String> = None;
                            let mut walk = spec_rest;
                            while let Some((k, vs)) = walk.destructure_cons() {
                                let key = k.as_symbol();
                                match key.as_deref() {
                                    Some(":initform") => {
                                        if let Some((v, rest2)) = vs.destructure_cons() {
                                            if let Ok(evaled) =
                                                eval(obj_to_value(v), env, editor, macros, state)
                                            {
                                                initform = value_to_obj(evaled);
                                            }
                                            walk = rest2;
                                            continue;
                                        }
                                    }
                                    Some(":initarg") => {
                                        if let Some((v, rest2)) = vs.destructure_cons() {
                                            if let Some(k2) = v.as_symbol() {
                                                initarg = Some(
                                                    k2.strip_prefix(':').unwrap_or(&k2).to_string(),
                                                );
                                            }
                                            walk = rest2;
                                            continue;
                                        }
                                    }
                                    _ => {}
                                }
                                if let Some((_, rest2)) = vs.destructure_cons() {
                                    walk = rest2;
                                } else {
                                    break;
                                }
                            }
                            slots.push(crate::primitives_eieio::Slot {
                                name: slot_name,
                                initarg,
                                default: initform,
                            });
                        }
                        cur = rest;
                    }
                    crate::primitives_eieio::register_class_for_state(
                        state,
                        crate::primitives_eieio::Class {
                            name: name.clone(),
                            parent: parent_name,
                            slots,
                        },
                    );
                    Ok(obj_to_value(LispObject::symbol(&name)))
                }
                "setf" => {
                    let mut last = Value::nil();
                    let mut cur = cdr.clone();
                    while let Some((place, rest)) = cur.destructure_cons() {
                        let value_form = rest.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        cur = rest.rest().unwrap_or(LispObject::nil());
                        last = eval_setf_one(place, value_form, env, editor, macros, state)?;
                    }
                    Ok(last)
                }
                "make-variable-buffer-local" => {
                    eval_make_variable_buffer_local(obj_to_value(cdr), env, editor, macros, state)
                }
                "make-hash-table" => {
                    let mut test = crate::object::HashTableTest::Eql;
                    let mut cur = cdr.clone();
                    while let Some((key, rest)) = cur.destructure_cons() {
                        let key_val =
                            value_to_obj(eval(obj_to_value(key), env, editor, macros, state)?);
                        if let Some(s) = key_val.as_symbol()
                            && s == ":test"
                            && let Some((val_expr, rest2)) = rest.destructure_cons()
                        {
                            let val = value_to_obj(eval(
                                obj_to_value(val_expr),
                                env,
                                editor,
                                macros,
                                state,
                            )?);
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
                    Ok(state.heap_hashtable(crate::object::LispHashTable::new(test)))
                }
                "gethash" => {
                    let key = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let table = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let default = if let Some(d) = cdr.nth(2) {
                        value_to_obj(eval(obj_to_value(d), env, editor, macros, state)?)
                    } else {
                        LispObject::nil()
                    };
                    if let LispObject::HashTable(ht) = &table {
                        Ok(obj_to_value(
                            ht.lock().get(&key).cloned().unwrap_or(default),
                        ))
                    } else {
                        Ok(obj_to_value(default))
                    }
                }
                "puthash" => {
                    let key = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let value = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let table_expr = cdr.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
                    let table =
                        value_to_obj(eval(obj_to_value(table_expr), env, editor, macros, state)?);
                    if let LispObject::HashTable(ht) = &table {
                        ht.lock().put(&key, value.clone());
                    }
                    Ok(obj_to_value(value))
                }
                "clrhash" => Ok(Value::nil()),
                "hash-table-p" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(LispObject::from(matches!(
                        arg,
                        LispObject::HashTable(_)
                    ))))
                }
                "hash-table-count" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    if let LispObject::HashTable(ht) = &arg {
                        Ok(obj_to_value(LispObject::integer(
                            ht.lock().data.len() as i64
                        )))
                    } else {
                        Ok(obj_to_value(LispObject::integer(0)))
                    }
                }
                "symbol-with-pos-p" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let args = LispObject::cons(arg, LispObject::nil());
                    Ok(obj_to_value(crate::primitives::call_primitive(
                        "symbol-with-pos-p",
                        &args,
                    )?))
                }
                "bare-symbol" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let args = LispObject::cons(arg, LispObject::nil());
                    Ok(obj_to_value(crate::primitives::call_primitive(
                        "remove-pos-from-symbol",
                        &args,
                    )?))
                }
                "vectorp" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(LispObject::from(matches!(
                        arg,
                        LispObject::Vector(_)
                    ))))
                }
                "recordp" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let is_record = matches!(&arg, LispObject::Vector(v)
                        if matches!(v.lock().first(), Some(LispObject::Symbol(_))));
                    Ok(obj_to_value(LispObject::from(is_record)))
                }
                "bool-vector-p" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(LispObject::from(
                        crate::primitives::core::is_bool_vector(&arg),
                    )))
                }
                "char-table-p" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(LispObject::from(
                        crate::primitives::core::is_char_table(&arg),
                    )))
                }
                "string-to-list" => {
                    let value = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let args = LispObject::cons(value, LispObject::nil());
                    Ok(obj_to_value(crate::primitives::call_primitive(
                        "string-to-list",
                        &args,
                    )?))
                }
                "cl-coerce" => {
                    let value = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let target = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    if target.as_symbol().as_deref() == Some("list")
                        && let Some(list) = crate::primitives::core::bool_vector_to_list(&value)
                    {
                        return Ok(obj_to_value(list));
                    }
                    let args = LispObject::cons(value, LispObject::cons(target, LispObject::nil()));
                    Ok(obj_to_value(crate::primitives::cl::prim_cl_coerce(&args)?))
                }
                "aref" => {
                    let array = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let idx = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let args = LispObject::cons(array, LispObject::cons(idx, LispObject::nil()));
                    Ok(obj_to_value(crate::primitives::call_primitive(
                        "aref", &args,
                    )?))
                }
                "aset" => {
                    if cdr.first().and_then(|array| array.as_symbol()).as_deref()
                        == Some("composition-function-table")
                    {
                        return Ok(Value::nil());
                    }
                    let idx_expr = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let array = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let idx_obj = match eval(obj_to_value(idx_expr), env, editor, macros, state) {
                        Ok(value) => value_to_obj(value),
                        Err(ElispError::VoidVariable(_)) => return Ok(Value::nil()),
                        Err(err) => return Err(err),
                    };
                    let val = value_to_obj(eval(
                        obj_to_value(cdr.nth(2).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let args = LispObject::cons(
                        array,
                        LispObject::cons(idx_obj, LispObject::cons(val, LispObject::nil())),
                    );
                    Ok(obj_to_value(crate::primitives::call_primitive(
                        "aset", &args,
                    )?))
                }
                "with-suppressed-warnings" | "dont-compile" => {
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    eval_progn(obj_to_value(body), env, editor, macros, state)
                }
                "defvaralias" | "define-obsolete-variable-alias" => {
                    let alias = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let base = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let alias_id = alias
                        .as_symbol_id()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    let base_id = base
                        .as_symbol_id()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    deliver_variable_watchers(
                        alias_id,
                        &LispObject::Symbol(base_id),
                        SetOperation::Defvaralias,
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let watcher_key = crate::obarray::intern("variable-watchers");
                    state.put_plist(alias_id, watcher_key, LispObject::nil());
                    let alias_key = crate::obarray::intern("variable-alias");
                    state.put_plist(alias_id, alias_key, LispObject::Symbol(base_id));
                    Ok(obj_to_value(LispObject::Symbol(alias_id)))
                }
                "define-obsolete-function-alias" | "set-advertised-calling-convention" => {
                    Ok(obj_to_value(cdr.first().unwrap_or_else(LispObject::nil)))
                }
                "push" => {
                    let val_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let place = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let val =
                        value_to_obj(eval(obj_to_value(val_expr), env, editor, macros, state)?);
                    let old = if let Some(place_id) = place.as_symbol_id() {
                        env.read().get_id(place_id).unwrap_or_else(LispObject::nil)
                    } else {
                        value_to_obj(eval(
                            obj_to_value(place.clone()),
                            env,
                            editor,
                            macros,
                            state,
                        )?)
                    };
                    let new = LispObject::cons(val, old);
                    let quoted_new = LispObject::cons(
                        LispObject::symbol("quote"),
                        LispObject::cons(new.clone(), LispObject::nil()),
                    );
                    eval_setf_one(place, quoted_new, env, editor, macros, state)?;
                    Ok(obj_to_value(new))
                }
                "pop" => {
                    let place = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let place_id = place
                        .as_symbol_id()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    let list = env.read().get_id(place_id).unwrap_or_else(LispObject::nil);
                    let car_val = list.first().unwrap_or(LispObject::nil());
                    let cdr_val = list.rest().unwrap_or(LispObject::nil());
                    env.write().set_id(place_id, cdr_val);
                    Ok(obj_to_value(car_val))
                }
                "symbol-value" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let id = arg
                        .as_symbol_id()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    let id = resolve_variable_alias(id, state);
                    let name = crate::obarray::symbol_name(id);
                    let buffer_local = crate::buffer::with_registry(|registry| {
                        registry
                            .get(registry.current_id())
                            .and_then(|buffer| buffer.locals.get(&name).cloned())
                    });
                    let value = buffer_local
                        .or_else(|| env.read().get_id_local(id))
                        .or_else(|| state.get_value_cell(id))
                        .unwrap_or_else(LispObject::nil);
                    Ok(obj_to_value(value))
                }
                "default-value" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let id = arg
                        .as_symbol_id()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    let id = resolve_variable_alias(id, state);
                    Ok(obj_to_value(
                        state.get_value_cell(id).unwrap_or_else(LispObject::nil),
                    ))
                }
                "default-boundp" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let id = arg
                        .as_symbol_id()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    let id = resolve_variable_alias(id, state);
                    Ok(obj_to_value(LispObject::from(
                        state
                            .get_value_cell(id)
                            .is_some_and(|v| !v.is_unbound_marker()),
                    )))
                }
                "set-default" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let val = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let id = sym
                        .as_symbol_id()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    let val = assign_symbol_value(
                        id,
                        val,
                        env,
                        editor,
                        macros,
                        state,
                        SetOperation::SetDefault,
                    )?;
                    Ok(obj_to_value(val))
                }
                "symbol-function" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = arg
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    if name == "if" {
                        return Ok(obj_to_value(LispObject::primitive("if")));
                    }
                    if let Some(val) = arg
                        .as_symbol_id()
                        .and_then(|id| state.get_function_cell(id))
                    {
                        Ok(obj_to_value(val))
                    } else if let Some(val) = env.read().get_function(&name) {
                        Ok(obj_to_value(val))
                    } else if let Some(m) = macros.read().get(&name).cloned() {
                        let sym_macro = Value::symbol_id(obarray::intern("macro").0);
                        let sym_lambda = Value::symbol_id(obarray::intern("lambda").0);
                        let args_val = obj_to_value(m.args);
                        let body_val = obj_to_value(m.body);
                        let result = state.with_heap(|heap| {
                            let args_body = heap.cons_value(args_val.raw(), body_val.raw());
                            let lambda_form = heap.cons_value(sym_lambda.raw(), args_body.raw());
                            heap.cons_value(sym_macro.raw(), lambda_form.raw())
                        });
                        Ok(result)
                    } else {
                        Ok(Value::nil())
                    }
                }
                "sort" => {
                    let list = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let pred = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let mut items = Vec::new();
                    let mut cur = list;
                    while let Some((car_val, cdr_val)) = cur.destructure_cons() {
                        items.push(car_val);
                        cur = cdr_val;
                    }
                    items.sort_by(|a, b| {
                        let call_args = LispObject::cons(
                            a.clone(),
                            LispObject::cons(b.clone(), LispObject::nil()),
                        );
                        let result = call_function(
                            obj_to_value(pred.clone()),
                            obj_to_value(call_args),
                            env,
                            editor,
                            macros,
                            state,
                        );
                        match result {
                            Ok(val) if !val.is_nil() => std::cmp::Ordering::Less,
                            _ => std::cmp::Ordering::Greater,
                        }
                    });
                    Ok(state.list_from_objects(items))
                }
                "nconc" => {
                    let mut lists = Vec::new();
                    let mut current = cdr.clone();
                    while let Some((arg_expr, rest)) = current.destructure_cons() {
                        lists.push(value_to_obj(eval(
                            obj_to_value(arg_expr),
                            env,
                            editor,
                            macros,
                            state,
                        )?));
                        current = rest;
                    }
                    if lists.is_empty() {
                        return Ok(Value::nil());
                    }
                    let mut result_idx = None;
                    for (i, l) in lists.iter().enumerate() {
                        if !l.is_nil() {
                            result_idx = Some(i);
                            break;
                        }
                    }
                    let result_idx = match result_idx {
                        Some(i) => i,
                        None => {
                            return Ok(obj_to_value(
                                lists.last().cloned().unwrap_or(LispObject::nil()),
                            ));
                        }
                    };
                    let result = lists[result_idx].clone();
                    let mut prev = lists[result_idx].clone();
                    for next in &lists[result_idx + 1..] {
                        let mut tail = prev.clone();
                        let mut steps: u64 = 0;
                        loop {
                            steps += 1;
                            if steps > (1 << 24) {
                                return Err(ElispError::EvalError(
                                    "nconc: list appears to be circular".to_string(),
                                ));
                            }
                            state.charge(1)?;
                            let cdr_val = tail.cdr().unwrap_or(LispObject::nil());
                            if !cdr_val.is_cons() {
                                break;
                            }
                            tail = cdr_val;
                        }
                        tail.set_cdr(next.clone());
                        prev = next.clone();
                    }
                    Ok(obj_to_value(result))
                }
                "nreverse" | "copy-sequence" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    if sym_name == "nreverse" {
                        let mut items = Vec::new();
                        let mut cur = arg;
                        let mut steps: u64 = 0;
                        while let Some((car_val, cdr_val)) = cur.destructure_cons() {
                            steps += 1;
                            if steps > (1 << 24) {
                                return Err(ElispError::EvalError(
                                    "nreverse: list appears to be circular".to_string(),
                                ));
                            }
                            state.charge(1)?;
                            items.push(car_val);
                            cur = cdr_val;
                        }
                        Ok(state.list_from_objects_reversed(items))
                    } else {
                        let args_list = LispObject::cons(arg, LispObject::nil());
                        let copied = crate::primitives::core::list::prim_copy_sequence(&args_list)?;
                        Ok(obj_to_value(copied))
                    }
                }
                "autoload" => {
                    let func_val = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    if let Some(file_expr) = cdr.nth(1) {
                        let file_val = eval(obj_to_value(file_expr), env, editor, macros, state)?;
                        let func_obj = value_to_obj(func_val);
                        let file_obj = value_to_obj(file_val);
                        if let (Some(func_name), Some(file_name)) =
                            (func_obj.as_symbol(), file_obj.as_string())
                        {
                            state.autoloads.write().insert(func_name, file_name.clone());
                        }
                    }
                    Ok(func_val)
                }
                "vector" => {
                    let mut items = Vec::new();
                    let mut current = cdr.clone();
                    while let Some((arg, rest)) = current.destructure_cons() {
                        items.push(value_to_obj(eval(
                            obj_to_value(arg),
                            env,
                            editor,
                            macros,
                            state,
                        )?));
                        current = rest;
                    }
                    Ok(state.heap_vector_from_objects(&items))
                }
                "make-vector" => {
                    let len_val = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let init_val = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let len = len_val.as_integer().unwrap_or(0).max(0) as usize;
                    let items = vec![init_val; len];
                    Ok(state.heap_vector_from_objects(&items))
                }
                "vconcat" => {
                    let mut items = Vec::new();
                    let mut current = cdr.clone();
                    while let Some((arg_expr, rest)) = current.destructure_cons() {
                        let arg =
                            value_to_obj(eval(obj_to_value(arg_expr), env, editor, macros, state)?);
                        match &arg {
                            LispObject::Vector(v) => {
                                items.extend(v.lock().iter().cloned());
                            }
                            LispObject::String(s) => {
                                for c in s.chars() {
                                    items.push(LispObject::integer(c as i64));
                                }
                            }
                            _ => {
                                let mut cur = arg;
                                while let Some((car, cdr_v)) = cur.destructure_cons() {
                                    items.push(car);
                                    cur = cdr_v;
                                }
                            }
                        }
                        current = rest;
                    }
                    Ok(state.heap_vector_from_objects(&items))
                }
                "byte-code" => {
                    let code_form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let consts_form = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let depth_form = cdr.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
                    let code_val =
                        value_to_obj(eval(obj_to_value(code_form), env, editor, macros, state)?);
                    let consts_val =
                        value_to_obj(eval(obj_to_value(consts_form), env, editor, macros, state)?);
                    let depth_val =
                        value_to_obj(eval(obj_to_value(depth_form), env, editor, macros, state)?);
                    let Some(code_str) = code_val.as_string() else {
                        return Ok(Value::nil());
                    };
                    let bytecode: Vec<u8> = code_str.chars().map(|c| c as u32 as u8).collect();
                    let constants: Vec<LispObject> = match consts_val {
                        LispObject::Vector(v) => v.lock().clone(),
                        _ => Vec::new(),
                    };
                    let maxdepth = depth_val.as_integer().unwrap_or(32) as usize;
                    let func = crate::object::BytecodeFunction {
                        argdesc: 0,
                        bytecode,
                        constants,
                        maxdepth,
                        docstring: None,
                        interactive: None,
                    };
                    match crate::vm::execute_bytecode(&func, &[], env, editor, macros, state) {
                        Ok(result) => Ok(obj_to_value(result)),
                        Err(e) if e.is_eval_ops_exceeded() => Err(e),
                        Err(e) => {
                            log::debug!("byte-code: execution error: {e}");
                            Ok(Value::nil())
                        }
                    }
                }
                "make-symbol" => {
                    let name_val = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let s = name_val
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    Ok(obj_to_value(LispObject::Symbol(
                        crate::obarray::make_uninterned(s),
                    )))
                }
                "fset" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let def = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let sym_id = symbol_id_including_constants(&sym)?;
                    let def = set_function_cell_checked(sym_id, def, state)?;
                    Ok(obj_to_value(def))
                }
                "purecopy" => {
                    let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    eval(obj_to_value(arg), env, editor, macros, state)
                }
                "intern" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let sym = match arg {
                        LispObject::String(ref s) => LispObject::symbol(s),
                        LispObject::Symbol(_) => arg.clone(),
                        _ => {
                            return Err(ElispError::WrongTypeArgument("string".to_string()));
                        }
                    };
                    // Optional second arg: obarray. We model obarrays as
                    // hash tables (`obarray-make` returns a hash table); if
                    // a hash-table obarray is given, register the symbol
                    // there so completion can iterate it.
                    if let Some(ob_expr) = cdr.nth(1) {
                        let ob =
                            value_to_obj(eval(obj_to_value(ob_expr), env, editor, macros, state)?);
                        if let LispObject::HashTable(ht) = &ob {
                            ht.lock().put(&sym, sym.clone());
                        }
                    }
                    Ok(obj_to_value(sym))
                }
                "intern-soft" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = match &arg {
                        LispObject::String(s) => s.clone(),
                        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
                        _ => return Ok(Value::nil()),
                    };
                    if env.read().get(&name).is_some() {
                        return Ok(obj_to_value(LispObject::symbol(&name)));
                    }
                    let table = crate::obarray::GLOBAL_OBARRAY.read();
                    for id in 0..table.symbol_count() as u32 {
                        let sid = crate::obarray::SymbolId(id);
                        if table.name(sid) == name {
                            drop(table);
                            let has_value = state.get_value_cell(sid).is_some();
                            let has_fn = state.get_function_cell(sid).is_some();
                            if has_value || has_fn {
                                return Ok(obj_to_value(LispObject::Symbol(sid)));
                            }
                            return Ok(Value::nil());
                        }
                    }
                    Ok(Value::nil())
                }
                "set" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let val = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let sym_id = sym
                        .as_symbol_id()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    let val = assign_symbol_value(
                        sym_id,
                        val,
                        env,
                        editor,
                        macros,
                        state,
                        SetOperation::Set,
                    )?;
                    Ok(obj_to_value(val))
                }
                "boundp" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let sym_id = symbol_id_including_constants(&sym)?;
                    let name = symbol_name_including_constants(&sym)?;
                    if let Some(val) = env.read().get_id_local(sym_id) {
                        return Ok(obj_to_value(LispObject::from(!val.is_unbound_marker())));
                    }
                    let local = crate::buffer::with_registry(|registry| {
                        registry
                            .get(registry.current_id())
                            .and_then(|buffer| buffer.locals.get(&name).cloned())
                    });
                    if let Some(val) = local {
                        return Ok(obj_to_value(LispObject::from(!val.is_unbound_marker())));
                    }
                    Ok(obj_to_value(LispObject::from(
                        matches!(sym, LispObject::Nil | LispObject::T)
                            || state
                                .get_value_cell(sym_id)
                                .is_some_and(|val| !val.is_unbound_marker())
                            || env
                                .read()
                                .get(&name)
                                .is_some_and(|val| !val.is_unbound_marker()),
                    )))
                }
                "bound-and-true-p" => {
                    let sym = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let sym_id = sym
                        .as_symbol_id()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    let name = crate::obarray::symbol_name(sym_id);
                    let value = env
                        .read()
                        .get_id_local(sym_id)
                        .or_else(|| {
                            crate::buffer::with_registry(|registry| {
                                registry
                                    .get(registry.current_id())
                                    .and_then(|buffer| buffer.locals.get(&name).cloned())
                            })
                        })
                        .or_else(|| state.get_value_cell(sym_id))
                        .or_else(|| env.read().get(&name));
                    let value = value
                        .filter(|value| !value.is_unbound_marker() && !value.is_nil())
                        .unwrap_or_else(LispObject::nil);
                    Ok(obj_to_value(value))
                }
                "variable-binding-locus" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let sym_id = symbol_id_including_constants(&sym)?;
                    let name = crate::obarray::symbol_name(sym_id);
                    let buffer_name = crate::buffer::with_registry(|registry| {
                        registry.get(registry.current_id()).and_then(|buffer| {
                            buffer
                                .locals
                                .contains_key(&name)
                                .then(|| buffer.name.clone())
                        })
                    });
                    let locus = if editor.read().is_none() {
                        None
                    } else {
                        buffer_name
                    };
                    Ok(obj_to_value(
                        locus
                            .map(|name| LispObject::string(&name))
                            .unwrap_or_else(LispObject::nil),
                    ))
                }
                "fboundp" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    if matches!(sym, LispObject::Nil | LispObject::T) {
                        return Ok(Value::nil());
                    }
                    let sym_id = symbol_id_including_constants(&sym)?;
                    let name = symbol_name_including_constants(&sym)?;
                    Ok(obj_to_value(LispObject::from(
                        state.get_function_cell(sym_id).is_some()
                            || env.read().get_function(&name).is_some(),
                    )))
                }
                "symbol-plist" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let sym_id = symbol_id_including_constants(&sym)?;
                    Ok(obj_to_value(state.full_plist(sym_id)))
                }
                "string-match-p" | "string-match" => {
                    let re_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let str_expr = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let re_val =
                        value_to_obj(eval(obj_to_value(re_expr), env, editor, macros, state)?);
                    let mut re_str = re_val
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                        .clone();
                    let text_val =
                        value_to_obj(eval(obj_to_value(str_expr), env, editor, macros, state)?);
                    let mut text = text_val
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                        .clone();
                    let re_multibyte = crate::object::string_is_multibyte(&re_str);
                    let text_multibyte = crate::object::string_is_multibyte(&text);
                    if text_multibyte && !re_multibyte {
                        re_str = encode_raw_bytes_as_private(&re_str);
                    } else if re_multibyte && !text_multibyte {
                        text = encode_raw_bytes_as_private(&text);
                    }
                    let start = if let Some(s) = cdr.nth(2) {
                        value_to_obj(eval(obj_to_value(s), env, editor, macros, state)?)
                            .as_integer()
                            .unwrap_or(0) as usize
                    } else {
                        0
                    };
                    let mut rust_re = emacs_regex_to_rust(&re_str);
                    let case_fold_id = crate::obarray::intern("case-fold-search");
                    let case_fold = env
                        .read()
                        .get_id(case_fold_id)
                        .or_else(|| state.get_value_cell(case_fold_id))
                        .is_some_and(|value| !value.is_nil());
                    if case_fold {
                        rust_re = format!("(?i:{rust_re})");
                    }
                    let set_data = sym_name == "string-match";
                    let start_byte = char_index_to_byte(&text, start);
                    if has_too_large_repeat(&re_str) {
                        return Err(invalid_regexp_signal("Regular expression too big"));
                    }
                    let re_opt = REGEX_CACHE.with(|cache| {
                        let mut c = cache.borrow_mut();
                        if let Some(re) = c.get(&rust_re) {
                            Some(re.clone())
                        } else {
                            regex::Regex::new(&rust_re).ok().inspect(|re| {
                                c.insert(rust_re.clone(), re.clone());
                            })
                        }
                    });
                    match re_opt {
                        Some(re) => {
                            if set_data && re.captures_len() > 1 {
                                if let Some(caps) = re.captures(&text[start_byte..]) {
                                    let mut data: Vec<Option<(usize, usize)>> =
                                        Vec::with_capacity(caps.len());
                                    for i in 0..caps.len() {
                                        data.push(caps.get(i).map(|m| {
                                            (
                                                byte_index_to_char(&text, start_byte + m.start()),
                                                byte_index_to_char(&text, start_byte + m.end()),
                                            )
                                        }));
                                    }
                                    state.set_match_data(data, Some(text.clone()));
                                    let m = caps.get(0).unwrap();
                                    Ok(obj_to_value(LispObject::integer(byte_index_to_char(
                                        &text,
                                        start_byte + m.start(),
                                    )
                                        as i64)))
                                } else {
                                    state.set_match_data(Vec::new(), None);
                                    Ok(Value::nil())
                                }
                            } else if let Some(m) = re.find(&text[start_byte..]) {
                                if set_data {
                                    state.set_match_data(
                                        vec![Some((
                                            byte_index_to_char(&text, start_byte + m.start()),
                                            byte_index_to_char(&text, start_byte + m.end()),
                                        ))],
                                        Some(text.clone()),
                                    );
                                }
                                Ok(obj_to_value(LispObject::integer(byte_index_to_char(
                                    &text,
                                    start_byte + m.start(),
                                )
                                    as i64)))
                            } else {
                                if set_data {
                                    state.set_match_data(Vec::new(), None);
                                }
                                Ok(Value::nil())
                            }
                        }
                        None => Ok(Value::nil()),
                    }
                }
                "match-beginning" => {
                    let n_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let n_obj =
                        value_to_obj(eval(obj_to_value(n_expr), env, editor, macros, state)?);
                    let n = n_obj.as_integer().unwrap_or(0) as usize;
                    if let Some((s, _)) = state.get_match_group(n) {
                        return Ok(obj_to_value(LispObject::integer(s as i64)));
                    }
                    if editor.read().is_none() {
                        let args = LispObject::cons(n_obj, LispObject::nil());
                        let result = crate::primitives_buffer::call_buffer_primitive(
                            "match-beginning",
                            &args,
                        )
                        .ok_or_else(|| ElispError::VoidFunction("match-beginning".to_string()))??;
                        Ok(obj_to_value(result))
                    } else {
                        Ok(Value::nil())
                    }
                }
                "match-end" => {
                    let n_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let n_obj =
                        value_to_obj(eval(obj_to_value(n_expr), env, editor, macros, state)?);
                    let n = n_obj.as_integer().unwrap_or(0) as usize;
                    if let Some((_, e)) = state.get_match_group(n) {
                        return Ok(obj_to_value(LispObject::integer(e as i64)));
                    }
                    if editor.read().is_none() {
                        let args = LispObject::cons(n_obj, LispObject::nil());
                        let result =
                            crate::primitives_buffer::call_buffer_primitive("match-end", &args)
                                .ok_or_else(|| ElispError::VoidFunction("match-end".to_string()))??;
                        Ok(obj_to_value(result))
                    } else {
                        Ok(Value::nil())
                    }
                }
                "match-string" => {
                    let n_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let n = value_to_obj(eval(obj_to_value(n_expr), env, editor, macros, state)?)
                        .as_integer()
                        .unwrap_or(0) as usize;
                    let src = if let Some(str_expr) = cdr.nth(1) {
                        let s =
                            value_to_obj(eval(obj_to_value(str_expr), env, editor, macros, state)?);
                        s.as_string().cloned()
                    } else {
                        state.get_match_string()
                    };
                    match (state.get_match_group(n), src) {
                        (Some((s, e)), Some(text)) => Ok(obj_to_value(LispObject::string(
                            text.get(s..e).unwrap_or(""),
                        ))),
                        _ => Ok(Value::nil()),
                    }
                }
                "match-data" => {
                    let data = state.match_data_as_objects();
                    Ok(state.list_from_objects(data))
                }
                "version-to-list" => {
                    let ver_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let ver =
                        value_to_obj(eval(obj_to_value(ver_expr), env, editor, macros, state)?);
                    let ver_str = ver
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let parts: Vec<LispObject> = ver_str
                        .split('.')
                        .map(|p| LispObject::integer(p.parse::<i64>().unwrap_or(0)))
                        .collect();
                    Ok(state.list_from_objects(parts))
                }
                "read-from-string" => {
                    let str_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let s = value_to_obj(eval(obj_to_value(str_expr), env, editor, macros, state)?);
                    let text = s
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let start = if let Some(start_expr) = cdr.nth(1) {
                        value_to_obj(eval(obj_to_value(start_expr), env, editor, macros, state)?)
                            .as_integer()
                            .unwrap_or(0) as usize
                    } else {
                        0
                    };
                    let sub = &text[start..];
                    let mut reader = crate::reader::Reader::new(sub);
                    match reader.read() {
                        Ok(obj) => {
                            let end_pos = start + reader.position();
                            Ok(state.heap_cons(
                                obj_to_value(obj),
                                obj_to_value(LispObject::Integer(end_pos as i64)),
                            ))
                        }
                        Err(e) => Err(ElispError::Signal(Box::new(crate::error::SignalData {
                            symbol: LispObject::symbol("invalid-read-syntax"),
                            data: LispObject::cons(
                                LispObject::string(&e.to_string()),
                                LispObject::nil(),
                            ),
                        }))),
                    }
                }
                "split-string" => {
                    let str_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let s = value_to_obj(eval(obj_to_value(str_expr), env, editor, macros, state)?);
                    let text = s
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                        .clone();
                    let separator = if let Some(sep_expr) = cdr.nth(1) {
                        let sep_val =
                            value_to_obj(eval(obj_to_value(sep_expr), env, editor, macros, state)?);
                        if sep_val.is_nil() {
                            None
                        } else {
                            sep_val.as_string().map(|s| s.to_string())
                        }
                    } else {
                        None
                    };
                    let omit_nulls = if let Some(omit_expr) = cdr.nth(2) {
                        let omit_val = eval(obj_to_value(omit_expr), env, editor, macros, state)?;
                        !omit_val.is_nil()
                    } else {
                        separator.is_none()
                    };
                    let parts: Vec<String> = match &separator {
                        None => text.split_whitespace().map(|s| s.to_string()).collect(),
                        Some(sep) => {
                            let rust_re = emacs_regex_to_rust(sep);
                            match regex::Regex::new(&rust_re) {
                                Ok(re) => re.split(&text).map(|s| s.to_string()).collect(),
                                Err(_) => text.split(sep.as_str()).map(|s| s.to_string()).collect(),
                            }
                        }
                    };
                    let parts: Vec<String> = if omit_nulls {
                        parts.into_iter().filter(|s| !s.is_empty()).collect()
                    } else {
                        parts
                    };
                    let string_values: Vec<Value> =
                        parts.into_iter().map(|p| state.heap_string(&p)).collect();
                    Ok(state.list_from_values(string_values))
                }
                "mapconcat" => {
                    let func = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let seq = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let sep = if let Some(s) = cdr.nth(2) {
                        value_to_obj(eval(obj_to_value(s), env, editor, macros, state)?)
                            .princ_to_string()
                    } else {
                        String::new()
                    };
                    let mut parts = Vec::new();
                    let mut cur = seq;
                    while let Some((car_val, rest)) = cur.destructure_cons() {
                        let call_args = LispObject::cons(car_val, LispObject::nil());
                        let r = call_function(
                            obj_to_value(func.clone()),
                            obj_to_value(call_args),
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        parts.push(value_to_obj(r).princ_to_string());
                        cur = rest;
                    }
                    Ok(obj_to_value(LispObject::string(&parts.join(&sep))))
                }
                "defmacro" => eval_defmacro(obj_to_value(cdr), macros),
                "macroexpand" => eval_macroexpand(obj_to_value(cdr), env, editor, macros, state),
                "eval-when-compile" | "eval-and-compile" => {
                    if sym_name == "eval-when-compile"
                        && cdr
                            .first()
                            .and_then(|form| form.first())
                            .and_then(|head| head.as_symbol())
                            .as_deref()
                            == Some("require")
                    {
                        Ok(Value::nil())
                    } else {
                        eval_progn(obj_to_value(cdr), env, editor, macros, state)
                    }
                }
                "file-exists-p" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    Ok(obj_to_value(LispObject::from(
                        std::path::Path::new(path.as_str()).exists(),
                    )))
                }
                "expand-file-name" => {
                    let name_val = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name_str = name_val
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                        .clone();
                    let expanded = if name_str.starts_with('/') || name_str.starts_with('~') {
                        name_str
                    } else {
                        let dir = cdr
                            .nth(1)
                            .and_then(|d| {
                                let v = value_to_obj(
                                    eval(obj_to_value(d), env, editor, macros, state).ok()?,
                                );
                                v.as_string().map(|s| s.to_string())
                            })
                            .unwrap_or_else(|| {
                                std::env::current_dir()
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_default()
                            });
                        format!("{}/{}", dir.trim_end_matches('/'), name_str)
                    };
                    Ok(obj_to_value(LispObject::string(&expanded)))
                }
                "file-name-directory" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    match std::path::Path::new(path.as_str()).parent() {
                        Some(p) if !p.as_os_str().is_empty() => Ok(obj_to_value(
                            LispObject::string(&format!("{}/", p.display())),
                        )),
                        _ => Ok(Value::nil()),
                    }
                }
                "file-name-nondirectory" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let name = std::path::Path::new(path.as_str())
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    Ok(obj_to_value(LispObject::string(&name)))
                }
                "file-readable-p" | "file-directory-p" | "file-regular-p" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let p = std::path::Path::new(path.as_str());
                    let result = match sym_name {
                        "file-readable-p" => p.exists(),
                        "file-directory-p" => p.is_dir(),
                        "file-regular-p" => p.is_file(),
                        _ => false,
                    };
                    Ok(obj_to_value(LispObject::from(result)))
                }
                "file-truename" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let resolved = std::fs::canonicalize(path.as_str())
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| path.to_string());
                    Ok(obj_to_value(LispObject::string(&resolved)))
                }
                "temporary-file-directory" => Ok(obj_to_value(LispObject::string("/tmp/"))),
                "directory-file-name" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let trimmed = path.trim_end_matches('/');
                    let result = if trimmed.is_empty() { "/" } else { trimmed };
                    Ok(obj_to_value(LispObject::string(result)))
                }
                "file-name-as-directory" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let result = if path.ends_with('/') {
                        path.to_string()
                    } else {
                        format!("{}/", path)
                    };
                    Ok(obj_to_value(LispObject::string(&result)))
                }
                "getenv" => {
                    let var = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = var
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    match std::env::var(name.as_str()) {
                        Ok(val) => Ok(obj_to_value(LispObject::string(&val))),
                        Err(_) => Ok(Value::nil()),
                    }
                }
                "system-name" => Ok(obj_to_value(LispObject::string("localhost"))),
                "user-login-name" | "user-real-login-name" => Ok(obj_to_value(LispObject::string(
                    &std::env::var("USER").unwrap_or_default(),
                ))),
                "emacs-pid" => Ok(obj_to_value(LispObject::integer(std::process::id() as i64))),
                "make-char-table" => {
                    let purpose = match cdr.first() {
                        Some(arg) => {
                            value_to_obj(eval(obj_to_value(arg), env, editor, macros, state)?)
                        }
                        None => LispObject::nil(),
                    };
                    let init = match cdr.nth(1) {
                        Some(arg) => {
                            value_to_obj(eval(obj_to_value(arg), env, editor, macros, state)?)
                        }
                        None => LispObject::nil(),
                    };
                    Ok(obj_to_value(crate::primitives::core::make_char_table(
                        purpose, init,
                    )))
                }
                "set-char-table-range" => {
                    let table = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let range = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let value = value_to_obj(eval(
                        obj_to_value(cdr.nth(2).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(crate::primitives::core::char_table_set_range(
                        &table, &range, value,
                    )?))
                }
                "char-table-range" => {
                    let table = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let code = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(crate::primitives::core::char_table_range(
                        &table, &code,
                    )?))
                }
                "char-table-subtype" => {
                    let table = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(crate::primitives::core::char_table_subtype(
                        &table,
                    )))
                }
                "set-char-table-parent" => {
                    let table = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let parent = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(
                        crate::primitives::core::char_table_set_parent(&table, parent)?,
                    ))
                }
                "char-table-parent" => {
                    let table = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(crate::primitives::core::char_table_parent(
                        &table,
                    )?))
                }
                "char-table-extra-slot" => {
                    let table = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let slot = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(
                        crate::primitives::core::char_table_extra_slot(&table, &slot)?,
                    ))
                }
                "set-char-table-extra-slot" => {
                    let table = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let slot = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let value = value_to_obj(eval(
                        obj_to_value(cdr.nth(2).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(
                        crate::primitives::core::char_table_set_extra_slot(&table, &slot, value)?,
                    ))
                }
                "map-char-table" => {
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "standard-case-table" | "current-case-table" => {
                    Ok(obj_to_value(get_or_create_runtime_char_table(
                        state,
                        "rele--standard-case-table",
                        "case-table",
                        true,
                    )))
                }
                "standard-syntax-table" | "syntax-table" => {
                    Ok(obj_to_value(get_or_create_runtime_char_table(
                        state,
                        "rele--standard-syntax-table",
                        "syntax-table",
                        false,
                    )))
                }
                "standard-category-table" => Ok(obj_to_value(get_or_create_runtime_char_table(
                    state,
                    "rele--standard-category-table",
                    "category-table",
                    false,
                ))),
                "set-standard-case-table" => {
                    let table = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    state
                        .set_value_cell(crate::obarray::intern("rele--standard-case-table"), table);
                    Ok(Value::nil())
                }
                "set-syntax-table" => {
                    let table = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    state.set_value_cell(
                        crate::obarray::intern("rele--standard-syntax-table"),
                        table,
                    );
                    Ok(Value::nil())
                }
                "char-syntax" => {
                    let _ = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(obj_to_value(LispObject::integer(32)))
                }
                "oclosure-define" => eval_oclosure_define(obj_to_value(cdr), state),
                "with-memoization" => {
                    let place = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let cached = match eval(obj_to_value(place), env, editor, macros, state) {
                        Ok(value) => value,
                        Err(ElispError::VoidVariable(_)) => Value::nil(),
                        Err(err) => return Err(err),
                    };
                    if !cached.is_nil() {
                        Ok(cached)
                    } else {
                        let body = cdr.rest().unwrap_or(LispObject::nil());
                        eval_progn(obj_to_value(body), env, editor, macros, state)
                    }
                }
                "pcase-defmacro" => Ok(obj_to_value(cdr.first().unwrap_or_else(LispObject::nil))),
                "advice-add" | "advice-remove" => Ok(Value::nil()),
                "kbd" | "key-parse" => {
                    if let Some(arg) = cdr.first() {
                        eval(obj_to_value(arg), env, editor, macros, state)
                    } else {
                        Ok(obj_to_value(LispObject::string("")))
                    }
                }
                "define-coding-system" | "set-language-info-alist" => {
                    if let Some(name_expr) = cdr.first() {
                        let _ = eval(obj_to_value(name_expr), env, editor, macros, state)?;
                    }
                    Ok(Value::nil())
                }
                "define-coding-system-alias" => {
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "coding-system-p" => {
                    let _ = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "check-coding-system" => eval(
                    obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                    env,
                    editor,
                    macros,
                    state,
                ),
                "coding-system-list" => Ok(Value::nil()),
                "find-coding-systems-region" => {
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "encode-coding-string" | "decode-coding-string" => {
                    let is_decode = car.as_symbol().as_deref() == Some("decode-coding-string");
                    let string_val = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    if let Some(rest) = cdr.rest() {
                        let mut cur = rest;
                        while let Some((arg, r)) = cur.destructure_cons() {
                            let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                            cur = r;
                        }
                    }
                    if is_decode {
                        let id = crate::obarray::intern("last-coding-system-used");
                        let name = crate::obarray::symbol_name(id);
                        let updated_local = crate::buffer::with_registry_mut(|registry| {
                            let current = registry.current_id();
                            registry.get_mut(current).is_some_and(|buffer| {
                                if let Some(slot) = buffer.locals.get_mut(&name) {
                                    *slot = LispObject::symbol("no-conversion");
                                    true
                                } else {
                                    false
                                }
                            })
                        });
                        if !updated_local {
                            let _ = assign_symbol_value(
                                id,
                                LispObject::symbol("no-conversion"),
                                env,
                                editor,
                                macros,
                                state,
                                SetOperation::Setq,
                            )?;
                        }
                    }
                    Ok(string_val)
                }
                "set-keyboard-coding-system" | "set-terminal-coding-system" => {
                    let _ = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "string-to-multibyte" | "string-to-unibyte" => {
                    let value = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let obj = value_to_obj(value);
                    if let Some(s) = obj.as_string() {
                        if car.as_symbol().as_deref() == Some("string-to-multibyte") {
                            let converted = if crate::object::string_is_multibyte(s) {
                                s.clone()
                            } else {
                                encode_raw_bytes_as_private(s)
                            };
                            crate::object::mark_string_multibyte(&converted, true);
                            Ok(obj_to_value(LispObject::string(&converted)))
                        } else {
                            let converted = decode_private_raw_bytes(s);
                            crate::object::mark_string_multibyte(&converted, false);
                            Ok(obj_to_value(LispObject::string(&converted)))
                        }
                    } else {
                        Ok(value)
                    }
                }
                "unibyte-string" => {
                    let mut vals = Vec::new();
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let val =
                            value_to_obj(eval(obj_to_value(arg), env, editor, macros, state)?);
                        vals.push(val);
                        cur = rest;
                    }
                    let mut args = LispObject::nil();
                    for val in vals.into_iter().rev() {
                        args = LispObject::cons(val, args);
                    }
                    Ok(obj_to_value(crate::primitives::call_primitive(
                        "unibyte-string",
                        &args,
                    )?))
                }
                "locale-coding-system" => Ok(Value::nil()),
                "set-language-environment"
                | "set-default-coding-systems"
                | "prefer-coding-system" => {
                    let _ = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "define-charset-internal" => {
                    if cdr.is_nil() {
                        return Err(ElispError::WrongNumberOfArguments);
                    }
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "put-charset-property" => {
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "charset-plist" => {
                    let _ = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "locate-library" => {
                    let name_val = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name_str = name_val
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                        .clone();
                    let load_path = state
                        .global_env
                        .read()
                        .get("load-path")
                        .unwrap_or(LispObject::nil());
                    let mut load_dirs = Vec::new();
                    let mut cur = load_path;
                    while let Some((dir, rest)) = cur.destructure_cons() {
                        if let Some(d) = dir.as_string() {
                            load_dirs.push(d.clone());
                        }
                        cur = rest;
                    }
                    let suffixes = [".elc", ".el", ""];
                    for suffix in &suffixes {
                        let full = format!("{}{}", name_str, suffix);
                        if std::path::Path::new(&full).exists() {
                            return Ok(obj_to_value(LispObject::string(&full)));
                        }
                        for d in &load_dirs {
                            let candidate = format!("{}/{}", d, full);
                            if std::path::Path::new(&candidate).exists() {
                                return Ok(obj_to_value(LispObject::string(&candidate)));
                            }
                        }
                    }
                    Ok(Value::nil())
                }
                "propertize" => {
                    let s = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let mut property_count = 0usize;
                    let mut cur = cdr.rest().unwrap_or_else(LispObject::nil);
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        property_count += 1;
                        cur = rest;
                    }
                    if !property_count.is_multiple_of(2) {
                        return Err(ElispError::WrongNumberOfArguments);
                    }
                    eval(obj_to_value(s), env, editor, macros, state)
                }
                "put-text-property"
                | "set-text-properties"
                | "add-text-properties"
                | "remove-text-properties" => {
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "get-text-property"
                | "text-properties-at"
                | "next-single-property-change"
                | "previous-single-property-change"
                | "text-property-any" => {
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "make-face" => {
                    let face = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    eval(obj_to_value(face), env, editor, macros, state)
                }
                "face-list" => Ok(Value::nil()),
                "set-face-attribute" | "internal-set-lisp-face-attribute" | "face-spec-set" => {
                    let mut cur = cdr.clone();
                    let mut values = Vec::new();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        values.push(value_to_obj(eval(
                            obj_to_value(arg),
                            env,
                            editor,
                            macros,
                            state,
                        )?));
                        cur = rest;
                    }
                    update_face_inherit_attribute(state, &values)?;
                    Ok(Value::nil())
                }
                "face-attribute"
                | "face-attribute-relative-p"
                | "internal-lisp-face-attribute-values" => {
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "display-supports-face-attributes-p" => {
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(obj_to_value(LispObject::t()))
                }
                "pcase" | "pcase-exhaustive" => {
                    pcase::eval_pcase(obj_to_value(cdr), env, editor, macros, state)
                }
                "pcase-let" => {
                    pcase::eval_pcase_let(obj_to_value(cdr), env, editor, macros, state, false)
                }
                "pcase-let*" => {
                    pcase::eval_pcase_let(obj_to_value(cdr), env, editor, macros, state, true)
                }
                "pcase-dolist" => {
                    pcase::eval_pcase_dolist(obj_to_value(cdr), env, editor, macros, state)
                }
                "define-button-type" => {
                    let name = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    eval(obj_to_value(name), env, editor, macros, state)
                }
                "make-local-variable" => {
                    eval_make_local_variable(obj_to_value(cdr), env, editor, macros, state)
                }
                "frame-list" | "window-list" => Ok(Value::nil()),
                "selected-frame" | "selected-window" => Ok(Value::nil()),
                "frame-parameter" => {
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                _ => {
                    if let Some(s) = car.as_symbol() {
                        let macro_table = macros.read();
                        if let Some(macro_) = macro_table.get(s.as_str()) {
                            let macro_ = macro_.clone();
                            drop(macro_table);
                            let expanded = expand_macro(&macro_, cdr, env, editor, macros, state)?;
                            return eval_next!(obj_to_value(expanded), env, editor, macros, state);
                        }
                    }
                    eval_funcall(
                        obj_to_value(car),
                        obj_to_value(cdr),
                        env,
                        editor,
                        macros,
                        state,
                    )
                }
            }
        }
        _ => eval_funcall(
            obj_to_value(car),
            obj_to_value(cdr),
            env,
            editor,
            macros,
            state,
        ),
    }
}

// --- Functions from original mod.rs (backquote, symbol-macrolet helpers) ---

/// Evaluate a backquoted form at the given nesting depth.
///
/// `depth` starts at 1 for the outermost backquote; a nested `` ` ``
/// inside the form raises it, and `,` / `,@` lowers it. An unquote
/// only fires (gets evaluated) when it brings the depth to 0.
///
/// This mirrors the semantics of Emacs's `backquote.el` but expands
/// and evaluates in a single pass, so the macro doesn't need to be
/// loaded from the stdlib.
pub(super) fn eval_backquote_form(
    form: LispObject,
    depth: u32,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let Some((car, cdr)) = form.destructure_cons() else {
        if let LispObject::Vector(v) = &form {
            let items: Vec<LispObject> = v.lock().clone();
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                if let Some((h, rest)) = item.destructure_cons()
                    && h.as_symbol().as_deref() == Some(",@")
                    && depth == 1
                {
                    let inner = rest.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let spliced =
                        value_to_obj(eval(obj_to_value(inner), env, editor, macros, state)?);
                    let mut cur = spliced;
                    while let Some((e, r)) = cur.destructure_cons() {
                        out.push(e);
                        cur = r;
                    }
                    continue;
                }
                out.push(eval_backquote_form(
                    item, depth, env, editor, macros, state,
                )?);
            }
            return Ok(LispObject::Vector(Arc::new(crate::eval::SyncRefCell::new(
                out,
            ))));
        }
        return Ok(form);
    };
    if let Some(sym) = car.as_symbol() {
        match sym.as_str() {
            "," => {
                let inner = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                if depth == 1 {
                    let val = eval(obj_to_value(inner), env, editor, macros, state)?;
                    return Ok(value_to_obj(val));
                }
                let expanded = eval_backquote_form(inner, depth - 1, env, editor, macros, state)?;
                return Ok(LispObject::cons(
                    LispObject::symbol(","),
                    LispObject::cons(expanded, LispObject::nil()),
                ));
            }
            ",@" => {
                if depth == 1 {
                    return Err(ElispError::EvalError(
                        "`,@` outside a list context".to_string(),
                    ));
                }
                let inner = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                let expanded = eval_backquote_form(inner, depth - 1, env, editor, macros, state)?;
                return Ok(LispObject::cons(
                    LispObject::symbol(",@"),
                    LispObject::cons(expanded, LispObject::nil()),
                ));
            }
            "`" => {
                let inner = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                let expanded = eval_backquote_form(inner, depth + 1, env, editor, macros, state)?;
                return Ok(LispObject::cons(
                    LispObject::symbol("`"),
                    LispObject::cons(expanded, LispObject::nil()),
                ));
            }
            _ => {}
        }
    }
    let mut out: Vec<LispObject> = Vec::new();
    let mut cur = LispObject::cons(car, cdr);
    let tail: LispObject = loop {
        match cur.destructure_cons() {
            None => break cur,
            Some((elem, rest)) => {
                if depth == 1 {
                    if let Some((h, r)) = elem.destructure_cons()
                        && h.as_symbol().as_deref() == Some(",@")
                    {
                        let inner = r.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        let spliced =
                            value_to_obj(eval(obj_to_value(inner), env, editor, macros, state)?);
                        let mut s = spliced;
                        while let Some((e, rr)) = s.destructure_cons() {
                            out.push(e);
                            s = rr;
                        }
                        if !s.is_nil() {
                            return Err(ElispError::EvalError(
                                ",@ spliced value was not a proper list".to_string(),
                            ));
                        }
                        cur = rest;
                        continue;
                    }
                    if elem.as_symbol().as_deref() == Some(",") {
                        let inner = rest.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        let tail_val = eval(obj_to_value(inner), env, editor, macros, state)?;
                        break value_to_obj(tail_val);
                    }
                }
                out.push(eval_backquote_form(
                    elem, depth, env, editor, macros, state,
                )?);
                cur = rest;
            }
        }
    };
    let mut result = if tail.is_nil() {
        LispObject::nil()
    } else {
        eval_backquote_form(tail, depth, env, editor, macros, state)?
    };
    for elem in out.into_iter().rev() {
        result = LispObject::cons(elem, result);
    }
    Ok(result)
}
/// Walk a form tree, replacing symbol occurrences according to
/// `cl-symbol-macrolet` substitutions. Respects shadowing:
/// `quote`, `function`, and binding forms (`let`, `let*`, `lambda`,
/// `defun`, `cl-flet`, `cl-labels`) suppress substitution for names
/// they bind.
pub(super) fn symbol_macrolet_walk(form: LispObject, subs: &[(String, LispObject)]) -> LispObject {
    if subs.is_empty() {
        return form;
    }
    match &form {
        LispObject::Symbol(id) => {
            let name = crate::obarray::symbol_name(*id);
            for (sub_name, expansion) in subs {
                if name == *sub_name {
                    return expansion.clone();
                }
            }
            form
        }
        LispObject::Cons(_) => {
            let car = form.first().unwrap_or(LispObject::nil());
            let cdr = form.rest().unwrap_or(LispObject::nil());
            if let Some(s) = car.as_symbol() {
                if s == "quote" || s == "function" {
                    return form;
                }
                match s.as_str() {
                    "let" | "let*" | "cl-letf" | "cl-letf*" | "cl-lexical-let"
                    | "cl-lexical-let*" | "dlet" => {
                        let bindings = cdr.first().unwrap_or(LispObject::nil());
                        let body = cdr.rest().unwrap_or(LispObject::nil());
                        let bound = collect_let_bound_names(&bindings);
                        let inner_subs = shadow_subs(subs, &bound);
                        let new_bindings = walk_let_bindings(bindings, subs);
                        let new_body = symbol_macrolet_walk_list(body, &inner_subs);
                        return LispObject::cons(car, LispObject::cons(new_bindings, new_body));
                    }
                    "lambda" | "defun" | "cl-defun" | "defsubst" => {
                        let (params, body) = if s == "lambda" {
                            (
                                cdr.first().unwrap_or(LispObject::nil()),
                                cdr.rest().unwrap_or(LispObject::nil()),
                            )
                        } else {
                            let name_form = cdr.first().unwrap_or(LispObject::nil());
                            let rest = cdr.rest().unwrap_or(LispObject::nil());
                            let params = rest.first().unwrap_or(LispObject::nil());
                            let body = rest.rest().unwrap_or(LispObject::nil());
                            return LispObject::cons(
                                car,
                                LispObject::cons(
                                    name_form,
                                    LispObject::cons(
                                        params.clone(),
                                        symbol_macrolet_walk_list(
                                            body,
                                            &shadow_subs(subs, &collect_param_names(&params)),
                                        ),
                                    ),
                                ),
                            );
                        };
                        let bound = collect_param_names(&params);
                        let inner_subs = shadow_subs(subs, &bound);
                        let new_body = symbol_macrolet_walk_list(body, &inner_subs);
                        return LispObject::cons(car, LispObject::cons(params, new_body));
                    }
                    _ => {}
                }
            }
            let new_car = symbol_macrolet_walk(car, subs);
            let new_cdr = symbol_macrolet_walk_list(cdr, subs);
            LispObject::cons(new_car, new_cdr)
        }
        _ => form,
    }
}
/// Walk let-bindings: substitute in init-forms but not in var names.
pub(super) fn walk_let_bindings(bindings: LispObject, subs: &[(String, LispObject)]) -> LispObject {
    match &bindings {
        LispObject::Cons(_) => {
            let car = bindings.first().unwrap_or(LispObject::nil());
            let cdr = bindings.rest().unwrap_or(LispObject::nil());
            let new_car = match &car {
                LispObject::Cons(_) => {
                    let var = car.first().unwrap_or(LispObject::nil());
                    let init = car.nth(1).unwrap_or(LispObject::nil());
                    LispObject::cons(
                        var,
                        LispObject::cons(symbol_macrolet_walk(init, subs), LispObject::nil()),
                    )
                }
                _ => car,
            };
            LispObject::cons(new_car, walk_let_bindings(cdr, subs))
        }
        _ => bindings,
    }
}
/// Collect variable names bound by a let-style binding list.
pub(super) fn collect_let_bound_names(bindings: &LispObject) -> Vec<String> {
    let mut names = Vec::new();
    let mut cur = bindings.clone();
    while let Some((binding, rest)) = cur.destructure_cons() {
        if let Some(name) = binding.as_symbol() {
            names.push(name);
        } else if let Some(name) = binding.first().and_then(|n| n.as_symbol()) {
            names.push(name);
        }
        cur = rest;
    }
    names
}
/// Collect parameter names from a lambda-style param list, stripping
/// `&optional`, `&rest`, `&key`, etc.
pub(super) fn collect_param_names(params: &LispObject) -> Vec<String> {
    let mut names = Vec::new();
    let mut cur = params.clone();
    while let Some((param, rest)) = cur.destructure_cons() {
        if let Some(name) = param.as_symbol() {
            if !name.starts_with('&') {
                names.push(name);
            }
        } else if let Some(name) = param.first().and_then(|n| n.as_symbol()) {
            names.push(name);
        }
        cur = rest;
    }
    names
}
/// Return subs with any names in `shadow` removed.
pub(super) fn shadow_subs(
    subs: &[(String, LispObject)],
    shadow: &[String],
) -> Vec<(String, LispObject)> {
    subs.iter()
        .filter(|(name, _)| !shadow.contains(name))
        .cloned()
        .collect()
}
pub(super) fn symbol_macrolet_walk_list(
    form: LispObject,
    subs: &[(String, LispObject)],
) -> LispObject {
    if subs.is_empty() {
        return form;
    }
    match &form {
        LispObject::Cons(_) => {
            let car = form.first().unwrap_or(LispObject::nil());
            let cdr = form.rest().unwrap_or(LispObject::nil());
            let new_car = symbol_macrolet_walk(car, subs);
            let new_cdr = symbol_macrolet_walk_list(cdr, subs);
            LispObject::cons(new_car, new_cdr)
        }
        LispObject::Nil => LispObject::nil(),
        _ => symbol_macrolet_walk(form, subs),
    }
}
pub(crate) fn symbol_id_including_constants(
    obj: &LispObject,
) -> ElispResult<crate::obarray::SymbolId> {
    match obj {
        LispObject::Symbol(id) => Ok(*id),
        LispObject::T => Ok(crate::obarray::intern("t")),
        LispObject::Nil => Ok(crate::obarray::intern("nil")),
        _ => Err(ElispError::WrongTypeArgument("symbol".to_string())),
    }
}
pub(crate) fn symbol_name_including_constants(obj: &LispObject) -> ElispResult<String> {
    Ok(crate::obarray::symbol_name(symbol_id_including_constants(
        obj,
    )?))
}
