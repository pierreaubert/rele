// Editor callbacks: buffer-string, insert, point, goto-char, find-file, etc.

use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use parking_lot::RwLock;
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
            crate::buffer::with_current(|b| b.text.chars().count()) as i64,
        ))),
    }
}
pub(super) fn eval_insert(
    args: Value,
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
        crate::buffer::with_current_mut(|b| b.insert(&text_str));
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
        Ok(Value::nil())
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
        Ok(Value::nil())
    }
}

pub(super) fn eval_point_min(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(obj_to_value(LispObject::integer(cb.point_min() as i64))),
        // StubBuffer convention: point-min = 1 (Emacs-like).
        None => Ok(obj_to_value(LispObject::integer(1))),
    }
}

pub(super) fn eval_point_max(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(obj_to_value(LispObject::integer(cb.point_max() as i64))),
        None => Ok(obj_to_value(LispObject::integer(
            crate::buffer::with_current(|b| b.text.chars().count()) as i64 + 1,
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
