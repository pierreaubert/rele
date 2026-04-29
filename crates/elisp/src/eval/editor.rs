#![allow(clippy::disallowed_methods)]
// Editor callbacks: buffer-string, insert, point, goto-char, find-file, etc.

use super::SyncRefCell as RwLock;
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::sync::Arc;

use super::{Environment, InterpreterState, MacroTable, eval, eval_progn};

pub(super) fn eval_buffer_string(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(obj_to_value(LispObject::string(&cb.buffer_string()))),
        // Fall back to thread-local StubBuffer (used by tests).
        None => Ok(obj_to_value(LispObject::string(
            &crate::buffer::with_current(|b| b.buffer_string()),
        ))),
    }
}
pub(super) fn eval_buffer_size(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(obj_to_value(LispObject::integer(cb.buffer_size() as i64))),
        None => Ok(obj_to_value(LispObject::integer(
            crate::buffer::with_current(|b| b.buffer_size()) as i64,
        ))),
    }
}
pub(super) fn eval_insert(
    args: Value,
    before_markers: bool,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    // (insert &rest ARGS) — Emacs accepts multiple args, each a string
    // or a character (integer).
    let mut current = value_to_obj(args);
    let mut chunks: Vec<String> = Vec::new();
    while let Some((arg, rest)) = current.destructure_cons() {
        let v = value_to_obj(eval(obj_to_value(arg), env, editor, macros, state)?);
        let s = match &v {
            LispObject::String(s) => s.clone(),
            LispObject::Integer(i) => char::from_u32(*i as u32)
                .map(|c| c.to_string())
                .unwrap_or_default(),
            LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
            _ => format!("{:?}", v),
        };
        chunks.push(s);
        current = rest;
    }
    let text_str: String = chunks.concat();
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.insert(&text_str);
    } else {
        drop(e);
        super::insert_with_overlay_hooks(&text_str, before_markers, env, editor, macros, state)?;
    }
    Ok(Value::nil())
}
pub(super) fn eval_point(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(obj_to_value(LispObject::integer(cb.point() as i64))),
        None => Ok(obj_to_value(LispObject::integer(
            crate::buffer::with_current(|b| b.point) as i64,
        ))),
    }
}
pub(super) fn eval_goto_char(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let pos_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let pos = value_to_obj(eval(obj_to_value(pos_arg), env, editor, macros, state)?);
    let pos_i = match pos {
        LispObject::Integer(i) => i,
        _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        // Preserve existing behaviour: pass through whatever the caller
        // gave us (the editor callback handles its own bounds). Test
        // `test_save_excursion_restores_point` relies on `(goto-char 0)`
        // round-tripping through `point` as 0.
        cb.goto_char(pos_i.max(0) as usize);
    } else {
        drop(e);
        // StubBuffer path: clamp to point-min (1) since char offsets
        // can't be 0 in real Emacs.
        crate::buffer::with_current_mut(|b| b.goto_char(pos_i.max(1) as usize));
    }
    Ok(Value::nil())
}
pub(super) fn eval_delete_char(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let n_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = value_to_obj(eval(obj_to_value(n_arg), env, editor, macros, state)?);
    let n = match n {
        LispObject::Integer(i) => i,
        _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.delete_char(n);
    } else {
        drop(e);
        let (start, end) = crate::buffer::with_current(|b| {
            if n >= 0 {
                let end = (b.point as i64 + n).min(b.point_max() as i64) as usize;
                (b.point, end)
            } else {
                let start = (b.point as i64 + n).max(b.point_min() as i64) as usize;
                (start, b.point)
            }
        });
        crate::buffer::with_registry_mut(|r| r.delete_current_region(start, end));
    }
    Ok(Value::nil())
}
pub(super) fn eval_forward_char(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let n_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = value_to_obj(eval(obj_to_value(n_arg), env, editor, macros, state)?);
    let n = match n {
        LispObject::Integer(i) => i,
        _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.forward_char(n);
    }
    Ok(Value::nil())
}
pub(super) fn eval_find_file(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let path_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let path = value_to_obj(eval(obj_to_value(path_arg), env, editor, macros, state)?);
    let path_str = match &path {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        let success = cb.find_file(&path_str);
        Ok(if success { Value::t() } else { Value::nil() })
    } else {
        let args = LispObject::cons(LispObject::string(&path_str), LispObject::nil());
        let result = crate::primitives_file::call_file_primitive("find-file", &args, state)
            .ok_or_else(|| ElispError::VoidFunction("find-file".to_string()))??;
        Ok(obj_to_value(result))
    }
}
pub(super) fn eval_save_buffer(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        let success = cb.save_buffer();
        Ok(if success { Value::t() } else { Value::nil() })
    } else {
        crate::buffer::with_current_mut(|buffer| {
            buffer.modified = false;
            buffer.modified_status = None;
        });
        Ok(Value::nil())
    }
}

pub(super) fn eval_point_min(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(obj_to_value(LispObject::integer(cb.point_min() as i64))),
        None => Ok(obj_to_value(LispObject::integer(
            crate::buffer::with_current(|b| b.point_min()) as i64,
        ))),
    }
}

pub(super) fn eval_point_max(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(obj_to_value(LispObject::integer(cb.point_max() as i64))),
        None => Ok(obj_to_value(LispObject::integer(
            crate::buffer::with_current(|b| b.point_max()) as i64,
        ))),
    }
}

pub(super) fn eval_forward_line(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    // `(forward-line N)` — N is optional and defaults to 1.
    let args_obj = value_to_obj(args);
    let n = match args_obj.first() {
        Some(arg) => {
            let v = value_to_obj(eval(obj_to_value(arg), env, editor, macros, state)?);
            match v {
                LispObject::Integer(i) => i,
                LispObject::Nil => 1,
                _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
            }
        }
        None => 1,
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.forward_line(n);
    }
    Ok(Value::nil())
}

pub(super) fn eval_move_beginning_of_line(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.move_beginning_of_line();
    }
    Ok(Value::nil())
}

pub(super) fn eval_move_end_of_line(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.move_end_of_line();
    }
    Ok(Value::nil())
}

pub(super) fn eval_beginning_of_buffer(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.move_beginning_of_buffer();
    }
    Ok(Value::nil())
}

pub(super) fn eval_end_of_buffer(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.move_end_of_buffer();
    }
    Ok(Value::nil())
}

pub(super) fn eval_undo_primitive(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.undo();
    }
    Ok(Value::nil())
}

pub(super) fn eval_redo_primitive(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.redo();
    }
    Ok(Value::nil())
}

// ---- Kill ring ----

pub(super) fn eval_kill_line(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.kill_line();
    }
    Ok(Value::nil())
}

pub(super) fn eval_kill_word(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let n = match args_obj.first() {
        Some(arg) => {
            let v = value_to_obj(eval(obj_to_value(arg), env, editor, macros, state)?);
            match v {
                LispObject::Integer(i) => i,
                LispObject::Nil => 1,
                _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
            }
        }
        None => 1,
    };
    if let Some(cb) = editor.write().as_mut() {
        cb.kill_word(n);
    }
    Ok(Value::nil())
}

pub(super) fn eval_kill_region(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.kill_region();
    }
    Ok(Value::nil())
}

pub(super) fn eval_copy_region_as_kill(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.copy_region_as_kill();
    }
    Ok(Value::nil())
}

pub(super) fn eval_yank(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.yank();
    }
    Ok(Value::nil())
}

pub(super) fn eval_yank_pop(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.yank_pop();
    }
    Ok(Value::nil())
}

// ---- Rectangles ----

pub(super) fn eval_delete_rectangle(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.delete_rectangle();
    }
    Ok(Value::nil())
}

pub(super) fn eval_kill_rectangle(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.kill_rectangle();
    }
    Ok(Value::nil())
}

pub(super) fn eval_yank_rectangle(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.yank_rectangle();
    }
    Ok(Value::nil())
}

pub(super) fn eval_open_rectangle(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.open_rectangle();
    }
    Ok(Value::nil())
}

pub(super) fn eval_clear_rectangle(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.clear_rectangle();
    }
    Ok(Value::nil())
}

pub(super) fn eval_string_rectangle(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let text_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let text_val = value_to_obj(eval(obj_to_value(text_arg), env, editor, macros, state)?);
    let text = match &text_val {
        LispObject::String(text) => text.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    if let Some(cb) = editor.write().as_mut() {
        cb.string_rectangle(&text);
    }
    Ok(Value::nil())
}

// ---- Search / replace ----

fn eval_string_arg(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<String> {
    let args_obj = value_to_obj(args);
    let arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let value = value_to_obj(eval(obj_to_value(arg), env, editor, macros, state)?);
    match value {
        LispObject::String(text) => Ok(text),
        LispObject::Symbol(id) => Ok(crate::obarray::symbol_name(id)),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn eval_two_string_args(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<(String, String)> {
    let args_obj = value_to_obj(args);
    let first = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let second = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let first = value_to_obj(eval(obj_to_value(first), env, editor, macros, state)?);
    let second = value_to_obj(eval(obj_to_value(second), env, editor, macros, state)?);
    let first = match first {
        LispObject::String(text) => text,
        LispObject::Symbol(id) => crate::obarray::symbol_name(id),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let second = match second {
        LispObject::String(text) => text,
        LispObject::Symbol(id) => crate::obarray::symbol_name(id),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    Ok((first, second))
}

fn eval_args_to_list(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let mut current = value_to_obj(args);
    let mut values = Vec::new();
    while let Some((arg, rest)) = current.destructure_cons() {
        values.push(value_to_obj(eval(
            obj_to_value(arg),
            env,
            editor,
            macros,
            state,
        )?));
        current = rest;
    }
    let mut out = LispObject::nil();
    for value in values.into_iter().rev() {
        out = LispObject::cons(value, out);
    }
    Ok(out)
}

fn search_result(result: Option<usize>) -> Value {
    result
        .map(|pos| obj_to_value(LispObject::integer(pos as i64)))
        .unwrap_or_else(Value::nil)
}

pub(super) fn eval_search_forward(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    if editor.read().is_none() {
        let args = eval_args_to_list(args, env, editor, macros, state)?;
        let result = crate::primitives_buffer::call_buffer_primitive("search-forward", &args)
            .ok_or_else(|| ElispError::VoidFunction("search-forward".to_string()))??;
        return Ok(obj_to_value(result));
    }
    let needle = eval_string_arg(args, env, editor, macros, state)?;
    let result = editor
        .write()
        .as_mut()
        .and_then(|cb| cb.search_forward(&needle));
    Ok(search_result(result))
}

pub(super) fn eval_search_backward(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    if editor.read().is_none() {
        let args = eval_args_to_list(args, env, editor, macros, state)?;
        let result = crate::primitives_buffer::call_buffer_primitive("search-backward", &args)
            .ok_or_else(|| ElispError::VoidFunction("search-backward".to_string()))??;
        return Ok(obj_to_value(result));
    }
    let needle = eval_string_arg(args, env, editor, macros, state)?;
    let result = editor
        .write()
        .as_mut()
        .and_then(|cb| cb.search_backward(&needle));
    Ok(search_result(result))
}

pub(super) fn eval_re_search_forward(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    if editor.read().is_none() {
        let args = eval_args_to_list(args, env, editor, macros, state)?;
        let result = crate::primitives_buffer::call_buffer_primitive("re-search-forward", &args)
            .ok_or_else(|| ElispError::VoidFunction("re-search-forward".to_string()))??;
        return Ok(obj_to_value(result));
    }
    let pattern = eval_string_arg(args, env, editor, macros, state)?;
    let result = editor
        .write()
        .as_mut()
        .and_then(|cb| cb.re_search_forward(&pattern));
    Ok(search_result(result))
}

pub(super) fn eval_re_search_backward(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    if editor.read().is_none() {
        let args = eval_args_to_list(args, env, editor, macros, state)?;
        let result = crate::primitives_buffer::call_buffer_primitive("re-search-backward", &args)
            .ok_or_else(|| ElispError::VoidFunction("re-search-backward".to_string()))??;
        return Ok(obj_to_value(result));
    }
    let pattern = eval_string_arg(args, env, editor, macros, state)?;
    let result = editor
        .write()
        .as_mut()
        .and_then(|cb| cb.re_search_backward(&pattern));
    Ok(search_result(result))
}

pub(super) fn eval_replace_match(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    if editor.read().is_none() {
        let args = eval_args_to_list(args, env, editor, macros, state)?;
        let result = crate::primitives_buffer::call_buffer_primitive("replace-match", &args)
            .ok_or_else(|| ElispError::VoidFunction("replace-match".to_string()))??;
        return Ok(obj_to_value(result));
    }
    let replacement = eval_string_arg(args, env, editor, macros, state)?;
    let replaced = editor
        .write()
        .as_mut()
        .is_some_and(|cb| cb.replace_match(&replacement));
    Ok(if replaced { Value::t() } else { Value::nil() })
}

pub(super) fn eval_replace_string(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let (from, to) = eval_two_string_args(args, env, editor, macros, state)?;
    let count = editor
        .write()
        .as_mut()
        .map_or(0, |cb| cb.replace_string(&from, &to));
    Ok(obj_to_value(LispObject::integer(count as i64)))
}

pub(super) fn eval_query_replace(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let (from, to) = eval_two_string_args(args, env, editor, macros, state)?;
    let count = editor
        .write()
        .as_mut()
        .map_or(0, |cb| cb.query_replace(&from, &to));
    Ok(obj_to_value(LispObject::integer(count as i64)))
}

pub(super) fn eval_replace_regexp(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let (pattern, replacement) = eval_two_string_args(args, env, editor, macros, state)?;
    let count = editor
        .write()
        .as_mut()
        .map_or(0, |cb| cb.replace_regexp(&pattern, &replacement));
    Ok(obj_to_value(LispObject::integer(count as i64)))
}

pub(super) fn eval_query_replace_regexp(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let (pattern, replacement) = eval_two_string_args(args, env, editor, macros, state)?;
    let count = editor
        .write()
        .as_mut()
        .map_or(0, |cb| cb.query_replace_regexp(&pattern, &replacement));
    Ok(obj_to_value(LispObject::integer(count as i64)))
}

// ---- Case ----

pub(super) fn eval_upcase_word(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.upcase_word();
    }
    Ok(Value::nil())
}

pub(super) fn eval_downcase_word(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.downcase_word();
    }
    Ok(Value::nil())
}

// ---- Reorder ----

pub(super) fn eval_transpose_chars(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.transpose_chars();
    }
    Ok(Value::nil())
}

pub(super) fn eval_transpose_words(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.transpose_words();
    }
    Ok(Value::nil())
}

// ---- Mark ----

pub(super) fn eval_set_mark(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.set_mark();
    }
    Ok(Value::nil())
}

pub(super) fn eval_exchange_point_and_mark(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.exchange_point_and_mark();
    }
    Ok(Value::nil())
}

// ---- Buffer list ----

pub(super) fn eval_next_buffer(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.next_buffer();
    }
    Ok(Value::nil())
}

pub(super) fn eval_previous_buffer(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.previous_buffer();
    }
    Ok(Value::nil())
}

// ---- View toggles ----

pub(super) fn eval_toggle_preview(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.toggle_preview();
    }
    Ok(Value::nil())
}

pub(super) fn eval_toggle_line_numbers(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.toggle_line_numbers();
    }
    Ok(Value::nil())
}

pub(super) fn eval_toggle_preview_line_numbers(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.toggle_preview_line_numbers();
    }
    Ok(Value::nil())
}

pub(super) fn eval_keyboard_quit(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    if let Some(cb) = editor.write().as_mut() {
        cb.keyboard_quit();
    }
    Ok(Value::nil())
}

pub(super) fn eval_set_current_buffer_major_mode(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let mode_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mode_val = value_to_obj(eval(obj_to_value(mode_arg), env, editor, macros, state)?);
    let mode = match &mode_val {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => {
            return Err(ElispError::WrongTypeArgument(
                "symbol-or-string".to_string(),
            ));
        }
    };
    if let Some(cb) = editor.write().as_mut() {
        cb.set_current_buffer_major_mode(&mode);
    }
    Ok(obj_to_value(LispObject::symbol(&mode)))
}

// ---- Buffer registry bridge (Phase 1) ----
//
// These shadow the stub-buffer implementations in
// `crate::primitives_buffer::*` when an editor callback is attached.
// The fall-through case (no editor) defers to the stub registry the
// way the rest of the interpreter does, so headless tests continue
// to work unchanged.

pub(super) fn eval_get_buffer_create_bridge(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name_val = value_to_obj(eval(obj_to_value(name_arg), env, editor, macros, state)?);
    let name = match &name_val {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Err(ElispError::WrongTypeArgument("string".into())),
    };
    {
        let mut e = editor.write();
        if let Some(cb) = e.as_mut() {
            cb.get_or_create_buffer(&name);
            // Return the name as the buffer object representation —
            // matches how `prim_get_buffer_create` shapes its result
            // for the stub registry.
            return Ok(obj_to_value(LispObject::string(&name)));
        }
    }
    // Fall back to the stub buffer registry so non-editor uses keep
    // working unchanged.
    let id = crate::buffer::with_registry_mut(|r| r.create(&name));
    Ok(obj_to_value(LispObject::cons(
        LispObject::symbol("buffer"),
        LispObject::integer(id as i64),
    )))
}

pub(super) fn eval_set_buffer_bridge(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let target_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let target = value_to_obj(eval(obj_to_value(target_arg), env, editor, macros, state)?);
    let name = match &target {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        // A buffer object `(buffer . id)` — fall through to stub.
        LispObject::Cons(_) => {
            // Defer to the stub registry — `prim_set_buffer` semantics.
            let args_list = LispObject::cons(target.clone(), LispObject::nil());
            return Ok(obj_to_value(crate::primitives_window::prim_set_buffer(
                &args_list,
            )?));
        }
        _ => return Err(ElispError::WrongTypeArgument("buffer".into())),
    };
    {
        let mut e = editor.write();
        if let Some(cb) = e.as_mut() {
            cb.switch_to_buffer_by_name(&name);
            return Ok(obj_to_value(LispObject::string(&name)));
        }
    }
    let args_list = LispObject::cons(LispObject::string(&name), LispObject::nil());
    Ok(obj_to_value(crate::primitives_window::prim_set_buffer(
        &args_list,
    )?))
}

pub(super) fn eval_current_buffer_bridge(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    {
        let e = editor.read();
        if let Some(cb) = e.as_ref()
            && let Some(name) = cb.current_buffer_name()
        {
            return Ok(obj_to_value(LispObject::string(&name)));
        }
    }
    let id = crate::buffer::with_registry(|r| r.current_id());
    Ok(obj_to_value(LispObject::cons(
        LispObject::symbol("buffer"),
        LispObject::integer(id as i64),
    )))
}

pub(super) fn eval_buffer_list_bridge(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    {
        let e = editor.read();
        if let Some(cb) = e.as_ref() {
            let names = cb.list_buffer_names();
            if !names.is_empty() {
                let mut out = LispObject::nil();
                for name in names.into_iter().rev() {
                    out = LispObject::cons(LispObject::string(&name), out);
                }
                return Ok(obj_to_value(out));
            }
        }
    }
    let ids = crate::buffer::with_registry(|r| r.list());
    let mut out = LispObject::nil();
    for id in ids.into_iter().rev() {
        out = LispObject::cons(
            LispObject::cons(LispObject::symbol("buffer"), LispObject::integer(id as i64)),
            out,
        );
    }
    Ok(obj_to_value(out))
}

/// `(insert-directory FILE SWITCHES &optional WILDCARD FULL-DIRECTORY-P)`
/// — generate an `ls -l`-style listing for FILE and insert it into
/// the current buffer at point. Handles the bits dired's
/// `dired-insert-directory` actually exercises: long format (`-l`),
/// hidden entries (`-a`), and the `total N` header. We never shell
/// out to the system `ls`, so this works the same on every platform.
pub(super) fn eval_insert_directory(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let file_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let switches_arg = args_obj.nth(1).unwrap_or(LispObject::nil());
    let full_dir_p = args_obj.nth(3).map(|a| !a.is_nil()).unwrap_or(false);

    let file = value_to_obj(eval(obj_to_value(file_arg), env, editor, macros, state)?);
    let file_str = match &file {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".into())),
    };
    let switches = value_to_obj(eval(
        obj_to_value(switches_arg),
        env,
        editor,
        macros,
        state,
    )?);
    let switches_str = switches.as_string().cloned().unwrap_or_default();

    let listing = render_directory_listing(&file_str, &switches_str, full_dir_p);
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.insert(&listing);
    } else {
        drop(e);
        crate::buffer::with_current_mut(|b| b.insert(&listing));
    }
    Ok(Value::nil())
}

fn render_directory_listing(path: &str, switches: &str, full_dir_p: bool) -> String {
    let long_format = switches.contains('l');
    let show_hidden = switches.contains('a') || switches.contains('A');
    let include_dot_dotdot = switches.contains('a');
    let reverse = switches.contains('r');

    let entries = if full_dir_p {
        match std::fs::read_dir(path) {
            Ok(rd) => rd
                .flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            Err(_) => return String::new(),
        }
    } else {
        // Single file mode: just `path` itself.
        let name = std::path::Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        vec![name]
    };

    let mut filtered: Vec<String> = entries
        .into_iter()
        .filter(|n| show_hidden || !n.starts_with('.') || n == "." || n == "..")
        .collect();
    if include_dot_dotdot && full_dir_p {
        if !filtered.iter().any(|n| n == ".") {
            filtered.push(".".to_string());
        }
        if !filtered.iter().any(|n| n == "..") {
            filtered.push("..".to_string());
        }
    }
    filtered.sort();
    if reverse {
        filtered.reverse();
    }

    if !long_format {
        // Plain mode: one entry per line, no metadata. Dired never
        // uses this path but other callers might.
        let mut out = String::new();
        for name in filtered {
            out.push_str(&name);
            out.push('\n');
        }
        return out;
    }

    // Long format: gather metadata, compute column widths, render.
    let base = path.trim_end_matches('/');
    let rows: Vec<DiredRow> = filtered
        .iter()
        .filter_map(|name| {
            let full = if full_dir_p {
                format!("{base}/{name}")
            } else {
                path.to_string()
            };
            let meta = std::fs::symlink_metadata(&full).ok()?;
            Some(make_row(name, &full, &meta))
        })
        .collect();

    let total_blocks: u64 = rows.iter().map(|r| r.blocks).sum();
    let nlink_w = max_w(&rows, |r| r.nlink.len());
    let owner_w = max_w(&rows, |r| r.owner.len());
    let group_w = max_w(&rows, |r| r.group.len());
    let size_w = max_w(&rows, |r| r.size.len());

    let mut out = String::new();
    if full_dir_p {
        // Emacs prefixes with `total NNN` (1K-blocks).
        out.push_str(&format!("  total {}\n", total_blocks));
    }
    for r in rows {
        out.push_str(&format!(
            "{} {:>nlink_w$} {:<owner_w$} {:<group_w$} {:>size_w$} {} {}{}\n",
            r.mode_str,
            r.nlink,
            r.owner,
            r.group,
            r.size,
            r.time_str,
            r.name,
            if let Some(t) = &r.symlink_target {
                format!(" -> {t}")
            } else {
                String::new()
            },
            nlink_w = nlink_w,
            owner_w = owner_w,
            group_w = group_w,
            size_w = size_w,
        ));
    }
    out
}

struct DiredRow {
    mode_str: String,
    nlink: String,
    owner: String,
    group: String,
    size: String,
    time_str: String,
    name: String,
    symlink_target: Option<String>,
    /// 1K blocks used by this entry, summed for the `total` header.
    blocks: u64,
}

fn make_row(display_name: &str, full_path: &str, meta: &std::fs::Metadata) -> DiredRow {
    let (nlink, uid, gid, _ino, _dev, mode_bits) = file_attrs(meta);
    DiredRow {
        mode_str: crate::primitives::file::format_mode_string(meta, mode_bits),
        nlink: nlink.to_string(),
        owner: uid_name(uid),
        group: gid_name(gid),
        size: meta.len().to_string(),
        time_str: format_mtime(meta.modified().ok()),
        name: display_name.to_string(),
        symlink_target: if meta.file_type().is_symlink() {
            std::fs::read_link(full_path)
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
        } else {
            None
        },
        // Round up to 1K blocks.
        blocks: meta.len().div_ceil(1024),
    }
}

fn max_w<T>(rows: &[T], f: impl Fn(&T) -> usize) -> usize {
    rows.iter().map(f).max().unwrap_or(0)
}

#[allow(clippy::cast_possible_wrap)]
fn file_attrs(meta: &std::fs::Metadata) -> (u64, u32, u32, u64, u64, u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        (
            meta.nlink(),
            meta.uid(),
            meta.gid(),
            meta.ino(),
            meta.dev(),
            meta.mode(),
        )
    }
    #[cfg(not(unix))]
    {
        let writable = !meta.permissions().readonly();
        let mode = if meta.is_dir() {
            if writable { 0o755 } else { 0o555 }
        } else if writable {
            0o644
        } else {
            0o444
        };
        (1, 0, 0, 0, 0, mode)
    }
}

fn uid_name(uid: u32) -> String {
    #[cfg(unix)]
    {
        users::get_user_by_uid(uid)
            .map(|u| u.name().to_string_lossy().to_string())
            .unwrap_or_else(|| uid.to_string())
    }
    #[cfg(not(unix))]
    {
        let _ = uid;
        "owner".to_string()
    }
}

fn gid_name(gid: u32) -> String {
    #[cfg(unix)]
    {
        users::get_group_by_gid(gid)
            .map(|g| g.name().to_string_lossy().to_string())
            .unwrap_or_else(|| gid.to_string())
    }
    #[cfg(not(unix))]
    {
        let _ = gid;
        "group".to_string()
    }
}

fn format_mtime(mtime: Option<std::time::SystemTime>) -> String {
    use std::time::UNIX_EPOCH;
    let Some(mtime) = mtime else {
        return "?".into();
    };
    let dur = match mtime.duration_since(UNIX_EPOCH) {
        Ok(d) => d,
        Err(_) => return "?".into(),
    };
    // Format: "Mon DD HH:MM" if within last 6 months, else "Mon DD  YYYY".
    // Without bringing in chrono here (it's a *workspace* dep but we
    // want to avoid pulling it through rele-elisp), use a tiny manual
    // formatter — close enough for dired's column-alignment needs.
    let secs = dur.as_secs() as i64;
    let days = secs / 86_400;
    let hms_secs = secs % 86_400;
    let hour = hms_secs / 3600;
    let minute = (hms_secs % 3600) / 60;
    // Days since 1970-01-01 → year/month/day via simple conversion.
    let (year, month, day) = days_to_ymd(days);
    let now_secs = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(secs);
    let six_months = 6 * 30 * 86_400;
    let recent = now_secs - secs <= six_months;
    let month_name = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(month - 1) as usize];
    if recent {
        format!("{} {:>2} {:02}:{:02}", month_name, day, hour, minute)
    } else {
        format!("{} {:>2}  {}", month_name, day, year)
    }
}

/// Civil-date conversion from days-since-epoch to (year, month, day).
/// Uses Howard Hinnant's algorithm — pure integer math, no allocations.
fn days_to_ymd(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(m <= 2);
    (year, m, d)
}

#[allow(dead_code)]
pub(super) fn eval_kill_buffer_bridge(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let target = args_obj
        .first()
        .map(|a| value_to_obj(eval(obj_to_value(a), env, editor, macros, state).unwrap()))
        .unwrap_or(LispObject::nil());
    let name_opt = match &target {
        LispObject::String(s) => Some(s.clone()),
        LispObject::Symbol(id) => Some(crate::obarray::symbol_name(*id)),
        LispObject::Nil => editor
            .read()
            .as_ref()
            .and_then(|cb| cb.current_buffer_name()),
        _ => None,
    };
    if let Some(name) = name_opt {
        let mut e = editor.write();
        if let Some(cb) = e.as_mut() {
            return Ok(obj_to_value(LispObject::from(
                cb.kill_buffer_by_name(&name),
            )));
        }
    }
    let args_list = LispObject::cons(target, LispObject::nil());
    Ok(obj_to_value(crate::primitives_buffer::prim_kill_buffer(
        &args_list,
    )?))
}

/// (save-excursion BODY...)
/// Save point position, evaluate BODY, restore point. Returns result of last body form.
pub(super) fn eval_save_excursion(
    body: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let saved_point = {
        let e = editor.read();
        match e.as_ref() {
            Some(cb) => cb.point(),
            None => crate::buffer::with_current(|b| b.point),
        }
    };
    let result = eval_progn(body, env, editor, macros, state);
    // Restore point even if body signaled an error
    {
        let mut e = editor.write();
        if let Some(cb) = e.as_mut() {
            cb.goto_char(saved_point);
        } else {
            drop(e);
            crate::buffer::with_current_mut(|b| b.goto_char(saved_point));
        }
    }
    result
}

/// (save-current-buffer BODY...)
/// Save the current buffer, evaluate BODY, restore it. Since we only have a single buffer
/// right now this is equivalent to progn, but the structure is here for future expansion.
pub(super) fn eval_save_current_buffer(
    body: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    // With a single-buffer model, save-current-buffer is just progn.
    eval_progn(body, env, editor, macros, state)
}
