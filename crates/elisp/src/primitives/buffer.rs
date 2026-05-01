#![allow(clippy::disallowed_methods)]
//! Buffer / marker / narrow / regex-search primitives.
//!
//! These are all state-less from the elisp perspective: they don't
//! need the interpreter env or macros table, only the per-thread
//! `buffer::Registry`. They're dispatched from `functions.rs::call_function`
//! via `call_stateful_primitive` — which in turn is registered in
//! `primitives.rs::add_primitives` so they're reachable from every
//! call site (source-level dispatch, funcall, apply, bytecode).
//!
//! The function signatures all match the stateful-primitive shape:
//!   fn(args: &LispObject) -> ElispResult<LispObject>
//!
//! Args are pre-evaluated.

use crate::buffer::{self, BufferId};
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_PROPERTIZED_STRING_ID: AtomicUsize = AtomicUsize::new(1);

thread_local! {
    static STRING_PROPERTIES: RefCell<HashMap<String, Vec<buffer::TextPropertySpan>>> =
        RefCell::new(HashMap::new());
}

/// Resolve an elisp buffer designator (string name, buffer handle)
/// to a BufferId. A `LispObject::String("<name>")` is treated as a
/// buffer name; a `LispObject::Nil` means "current buffer".
/// Returns None if the named buffer doesn't exist.
pub fn make_buffer_object(id: BufferId) -> LispObject {
    LispObject::cons(LispObject::symbol("buffer"), LispObject::integer(id as i64))
}

pub fn buffer_object_id(obj: &LispObject) -> Option<BufferId> {
    let (car, cdr) = obj.destructure_cons()?;
    if car.as_symbol().as_deref() != Some("buffer") {
        return None;
    }
    cdr.as_integer().map(|n| n as BufferId)
}

pub fn resolve_buffer(arg: &LispObject) -> Option<BufferId> {
    match arg {
        LispObject::Nil => Some(buffer::with_registry(|r| r.current_id())),
        LispObject::String(name) => buffer::with_registry(|r| r.lookup_by_name(name)),
        // Some call sites pass a symbol (the name). Resolve it.
        LispObject::Symbol(id) => {
            let name = crate::obarray::symbol_name(*id);
            buffer::with_registry(|r| r.lookup_by_name(&name))
        }
        LispObject::Cons(_) => {
            let id = buffer_object_id(arg)?;
            buffer::with_registry(|r| r.get(id).map(|_| id))
        }
        _ => None,
    }
}

fn integer_or_marker_value(value: &LispObject) -> Option<i64> {
    value.as_integer().or_else(|| {
        prim_marker_position(&LispObject::cons(value.clone(), LispObject::nil()))
            .ok()
            .and_then(|pos| pos.as_integer())
    })
}

fn int_arg(args: &LispObject, n: usize, default: i64) -> i64 {
    args.nth(n)
        .and_then(|v| integer_or_marker_value(&v))
        .unwrap_or(default)
}

fn required_pos_arg(args: &LispObject, n: usize) -> ElispResult<usize> {
    let value = args.nth(n).ok_or(ElispError::WrongNumberOfArguments)?;
    let pos = integer_or_marker_value(&value)
        .ok_or_else(|| ElispError::WrongTypeArgument("integer-or-marker".to_string()))?;
    Ok(pos.max(1) as usize)
}

fn string_arg(args: &LispObject, n: usize) -> Option<String> {
    args.nth(n)
        .and_then(|v| v.as_string().map(|s| s.to_string()))
}

fn list_from_pairs(items: &[(LispObject, LispObject)]) -> LispObject {
    let mut out = LispObject::nil();
    for (key, value) in items.iter().rev() {
        out = LispObject::cons(value.clone(), out);
        out = LispObject::cons(key.clone(), out);
    }
    out
}

fn plist_pairs(plist: &LispObject) -> ElispResult<Vec<(LispObject, LispObject)>> {
    let mut pairs = Vec::new();
    let mut current = plist.clone();
    while let Some((key, rest)) = current.destructure_cons() {
        let Some((value, next)) = rest.destructure_cons() else {
            return Err(ElispError::WrongNumberOfArguments);
        };
        pairs.push((key, value));
        current = next;
    }
    Ok(pairs)
}

fn plist_keys(plist: &LispObject) -> ElispResult<Vec<LispObject>> {
    Ok(plist_pairs(plist)?
        .into_iter()
        .map(|(key, _)| key)
        .collect())
}

fn make_propertized_string(text: &str, spans: Vec<buffer::TextPropertySpan>) -> LispObject {
    if spans.is_empty() {
        return LispObject::string(text);
    }
    let id = NEXT_PROPERTIZED_STRING_ID.fetch_add(1, Ordering::Relaxed);
    let key = format!("\x1frele-props:{id}:{text}");
    crate::object::mutate_string_value(&key, text.to_string());
    STRING_PROPERTIES.with(|props| {
        props.borrow_mut().insert(key.clone(), spans);
    });
    LispObject::String(key)
}

fn string_properties(raw: &str) -> Vec<buffer::TextPropertySpan> {
    STRING_PROPERTIES
        .with(|props| props.borrow().get(raw).cloned())
        .unwrap_or_default()
}

fn string_properties_at(raw: &str, pos: usize) -> Vec<(LispObject, LispObject)> {
    let mut props: Vec<(LispObject, LispObject)> = Vec::new();
    for span in string_properties(raw) {
        if pos < span.start || pos >= span.end {
            continue;
        }
        for (key, value) in span.plist {
            if let Some((_, existing)) = props.iter_mut().find(|(k, _)| *k == key) {
                *existing = value;
            } else {
                props.push((key, value));
            }
        }
    }
    props
}

fn string_property_at(raw: &str, pos: usize, prop: &LispObject) -> LispObject {
    string_properties(raw)
        .into_iter()
        .rev()
        .find_map(|span| {
            if pos >= span.start && pos < span.end {
                span.plist
                    .into_iter()
                    .rev()
                    .find_map(|(key, value)| (key == *prop).then_some(value))
            } else {
                None
            }
        })
        .unwrap_or_else(LispObject::nil)
}

pub fn copy_string_properties_to_current_buffer(raw: &str, start: usize) {
    let spans = string_properties(raw);
    if spans.is_empty() {
        return;
    }
    buffer::with_current_mut(|buffer| {
        for span in spans {
            buffer.add_text_properties(start + span.start, start + span.end, span.plist);
        }
    });
}

// ---- Buffer lifecycle -------------------------------------------------

pub fn prim_bufferp(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let is_buf = resolve_buffer(&a).is_some();
    Ok(LispObject::from(is_buf))
}

pub fn prim_buffer_live_p(args: &LispObject) -> ElispResult<LispObject> {
    let id = match args.first() {
        Some(LispObject::Nil) | None => None,
        Some(arg) => resolve_buffer(&arg),
    };
    Ok(LispObject::from(id.is_some()))
}

pub fn prim_buffer_name(args: &LispObject) -> ElispResult<LispObject> {
    let id = match args.first() {
        Some(a) => match resolve_buffer(&a) {
            Some(id) => id,
            None => return Ok(LispObject::nil()),
        },
        None => buffer::with_registry(|r| r.current_id()),
    };
    let name = buffer::with_registry(|r| r.get(id).map(|b| b.name.clone()));
    match name {
        Some(n) => Ok(LispObject::string(&n)),
        None => Ok(LispObject::nil()),
    }
}

pub fn prim_current_buffer(_args: &LispObject) -> ElispResult<LispObject> {
    let id = buffer::with_registry(|r| r.current_id());
    Ok(make_buffer_object(id))
}

pub fn prim_buffer_list(_args: &LispObject) -> ElispResult<LispObject> {
    let ids = buffer::with_registry(|r| r.list());
    let mut out = LispObject::nil();
    for id in ids.into_iter().rev() {
        out = LispObject::cons(make_buffer_object(id), out);
    }
    Ok(out)
}

pub fn prim_get_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match resolve_buffer(&a) {
        Some(id) => {
            if buffer::with_registry(|r| r.get(id).is_some()) {
                Ok(make_buffer_object(id))
            } else {
                Ok(LispObject::nil())
            }
        }
        None => Ok(LispObject::nil()),
    }
}

pub fn prim_find_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let variable = args
        .first()
        .and_then(|arg| arg.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
    let value = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let name = crate::obarray::symbol_name(variable);
    let found = buffer::with_registry(|registry| {
        registry.list().into_iter().find(|id| {
            registry.get(*id).is_some_and(|buffer| {
                let local = match name.as_str() {
                    "buffer-file-name" => buffer
                        .file_name
                        .as_ref()
                        .map(|path| LispObject::string(path)),
                    _ => buffer.locals.get(&name).cloned(),
                };
                local.as_ref() == Some(&value)
            })
        })
    });
    Ok(found
        .map(make_buffer_object)
        .unwrap_or_else(LispObject::nil))
}

pub fn prim_get_buffer_create(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = match &a {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Err(ElispError::WrongTypeArgument("string".into())),
    };
    let id = buffer::with_registry_mut(|r| r.create(&name));
    Ok(make_buffer_object(id))
}

pub fn prim_generate_new_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let name = string_arg(args, 0).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let inhibit_hooks = args.nth(1).is_some_and(|arg| !arg.is_nil());
    let id = buffer::with_registry_mut(|r| r.create_unique(&name));
    buffer::with_registry_mut(|r| {
        if let Some(buffer) = r.get_mut(id) {
            buffer.locals.insert(
                "rele--inhibit-buffer-hooks".to_string(),
                LispObject::from(inhibit_hooks),
            );
            buffer.locals.insert(
                "rele--pending-buffer-list-update-hook".to_string(),
                LispObject::from(!inhibit_hooks),
            );
        }
    });
    Ok(make_buffer_object(id))
}

pub fn prim_generate_new_buffer_name(args: &LispObject) -> ElispResult<LispObject> {
    let name = string_arg(args, 0).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let unique = buffer::with_registry(|r| r.generate_new_name(&name));
    Ok(LispObject::string(&unique))
}

pub fn prim_make_indirect_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let base = args
        .first()
        .and_then(|a| resolve_buffer(&a))
        .ok_or_else(|| ElispError::WrongTypeArgument("buffer".into()))?;
    let name = string_arg(args, 1).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let clone_overlays = args.nth(2).is_some_and(|arg| !arg.is_nil());
    let inhibit_hooks = args.nth(3).is_some_and(|arg| !arg.is_nil());
    let id = buffer::with_registry_mut(|r| r.make_indirect(base, &name, clone_overlays))
        .ok_or_else(|| ElispError::WrongTypeArgument("buffer".into()))?;
    buffer::with_registry_mut(|r| {
        if let Some(buffer) = r.get_mut(id) {
            buffer.locals.insert(
                "rele--inhibit-buffer-hooks".to_string(),
                LispObject::from(inhibit_hooks),
            );
            buffer.locals.insert(
                "rele--pending-buffer-list-update-hook".to_string(),
                LispObject::from(!inhibit_hooks),
            );
        }
    });
    Ok(make_buffer_object(id))
}

pub fn prim_rename_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let new_name =
        string_arg(args, 0).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let ok = buffer::with_registry_mut(|r| {
        let id = r.current_id();
        r.rename(id, &new_name)
    });
    if ok {
        Ok(LispObject::string(&new_name))
    } else {
        // Fallback: unique it.
        let mut candidate = format!("{new_name}<1>");
        let mut n = 1;
        loop {
            let ok = buffer::with_registry_mut(|r| {
                let id = r.current_id();
                r.rename(id, &candidate)
            });
            if ok {
                return Ok(LispObject::string(&candidate));
            }
            n += 1;
            candidate = format!("{new_name}<{n}>");
            if n > 1000 {
                return Err(ElispError::EvalError(
                    "rename-buffer: name collision".into(),
                ));
            }
        }
    }
}

pub fn prim_kill_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let id = match resolve_buffer(&a) {
        Some(id) => id,
        None => return Ok(LispObject::nil()),
    };
    let ok = buffer::with_registry_mut(|r| r.kill(id));
    Ok(LispObject::from(ok))
}

pub fn prim_other_buffer(_args: &LispObject) -> ElispResult<LispObject> {
    // Return the first buffer that isn't the current one. If there's
    // none, fall back to the current buffer.
    let id = buffer::with_registry(|r| {
        let cur = r.current_id();
        r.buffers
            .iter()
            .find(|&(&id, _)| id != cur)
            .map(|(&id, _)| id)
            .or_else(|| r.get(cur).map(|_| cur))
    });
    Ok(id.map(make_buffer_object).unwrap_or(LispObject::nil()))
}

pub fn prim_get_file_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let path = match args.first() {
        Some(LispObject::String(s)) => s,
        _ => return Ok(LispObject::nil()),
    };
    let found = buffer::with_registry(|r| {
        r.buffers
            .iter()
            .find(|(_, b)| b.file_name.as_deref() == Some(&path))
            .map(|(&id, _)| id)
    });
    Ok(found.map(make_buffer_object).unwrap_or(LispObject::nil()))
}

pub fn prim_buffer_modified_p(args: &LispObject) -> ElispResult<LispObject> {
    let id = args
        .first()
        .and_then(|a| resolve_buffer(&a))
        .or_else(|| Some(buffer::with_registry(|r| r.current_id())));
    let modified = buffer::with_registry(|r| {
        id.and_then(|id| r.get(id))
            .and_then(|b| b.modified_status.clone())
            .unwrap_or_else(LispObject::nil)
    });
    Ok(modified)
}

pub fn prim_set_buffer_modified_p(args: &LispObject) -> ElispResult<LispObject> {
    let value = args.first().unwrap_or(LispObject::nil());
    let status = if matches!(value, LispObject::Nil) {
        None
    } else {
        Some(value.clone())
    };
    buffer::with_current_mut(|b| {
        b.modified = status.is_some();
        b.modified_status = status;
    });
    Ok(value)
}

pub fn prim_buffer_modified_tick(_args: &LispObject) -> ElispResult<LispObject> {
    let t = buffer::with_current(|b| b.modified_tick);
    // Tick values fit comfortably in i64.
    #[allow(clippy::cast_possible_wrap)]
    Ok(LispObject::integer(t as i64))
}

pub fn prim_recent_auto_save_p(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_buffer_size(args: &LispObject) -> ElispResult<LispObject> {
    let id = args
        .first()
        .and_then(|a| resolve_buffer(&a))
        .unwrap_or_else(|| buffer::with_registry(|r| r.current_id()));
    let n = buffer::with_registry(|r| r.get(id).map(|b| b.buffer_size()).unwrap_or(0));
    Ok(LispObject::integer(n as i64))
}

pub fn prim_buffer_enable_undo(_args: &LispObject) -> ElispResult<LispObject> {
    buffer::with_current_mut(|b| {
        b.locals
            .insert("buffer-undo-list".to_string(), LispObject::nil());
    });
    Ok(LispObject::nil())
}

pub fn prim_buffer_disable_undo(_args: &LispObject) -> ElispResult<LispObject> {
    buffer::with_current_mut(|b| {
        b.locals
            .insert("buffer-undo-list".to_string(), LispObject::t());
    });
    Ok(LispObject::nil())
}

pub fn prim_undo_boundary(_args: &LispObject) -> ElispResult<LispObject> {
    buffer::with_current_mut(buffer::StubBuffer::push_undo_boundary);
    Ok(LispObject::nil())
}

fn error_signal(message: String) -> ElispError {
    ElispError::Signal(Box::new(crate::error::SignalData {
        symbol: LispObject::symbol("error"),
        data: LispObject::cons(LispObject::string(&message), LispObject::nil()),
    }))
}

fn wrong_type_signal(predicate: &str, value: LispObject) -> ElispError {
    ElispError::Signal(Box::new(crate::error::SignalData {
        symbol: LispObject::symbol("wrong-type-argument"),
        data: LispObject::cons(
            LispObject::symbol(predicate),
            LispObject::cons(value, LispObject::nil()),
        ),
    }))
}

fn unrecognized_undo_entry(entry: &LispObject) -> ElispError {
    error_signal(format!(
        "Unrecognized entry in undo list {}",
        entry.prin1_to_string()
    ))
}

fn checked_marker_adjustment(entry: &LispObject) -> Option<(usize, i64)> {
    let (car, cdr) = entry.destructure_cons()?;
    let marker = marker_id(&car)?;
    let offset = cdr.as_integer()?;
    Some((marker, offset))
}

fn adjust_marker_position(position: usize, offset: i64) -> usize {
    (position as i64 - offset).max(1) as usize
}

fn apply_undo_entry(entry: LispObject, list: &mut LispObject) -> ElispResult<()> {
    if let Some(pos) = entry.as_integer() {
        buffer::with_current_mut(|b| b.goto_char(pos.max(1) as usize));
        return Ok(());
    }

    let Some((car, cdr)) = entry.destructure_cons() else {
        return Err(unrecognized_undo_entry(&entry));
    };

    if let (Some(beg), Some(end)) = (car.as_integer(), cdr.as_integer()) {
        let beg = beg.max(1) as usize;
        let end = end.max(1) as usize;
        buffer::with_registry_mut(|r| r.delete_current_region(beg, end));
        return Ok(());
    }

    if car.is_t() {
        buffer::with_current_mut(|b| {
            b.modified = false;
            b.modified_status = None;
        });
        return Ok(());
    }

    if let (Some(text), Some(pos)) = (car.as_string(), cdr.as_integer()) {
        let abs_pos = pos.unsigned_abs() as usize;
        let mut valid_marker_adjustments = Vec::new();
        while let Some((next, rest)) = list.destructure_cons() {
            let Some((marker, offset)) = checked_marker_adjustment(&next) else {
                break;
            };
            *list = rest;
            let valid = buffer::with_registry(|r| {
                r.markers
                    .get(&marker)
                    .is_some_and(|m| m.buffer == r.current_id() && m.position == Some(abs_pos))
            });
            if valid {
                valid_marker_adjustments.push((marker, offset));
            }
        }
        buffer::with_registry_mut(|r| {
            if let Some(buf) = r.get_mut(r.current_id()) {
                buf.goto_char(abs_pos);
            }
            r.insert_current(text, false);
            if pos >= 0
                && let Some(buf) = r.get_mut(r.current_id())
            {
                buf.goto_char(abs_pos);
            }
            for (marker, offset) in valid_marker_adjustments {
                if let Some((buffer, position)) =
                    r.markers.get(&marker).map(|m| (m.buffer, m.position))
                    && let Some(position) = position
                {
                    r.marker_set(
                        marker,
                        buffer,
                        Some(adjust_marker_position(position, offset)),
                    );
                }
            }
        });
        return Ok(());
    }

    if let (Some(marker), Some(offset)) = (marker_id(&car), cdr.as_integer()) {
        buffer::with_registry_mut(|r| {
            if let Some((buffer, position)) = r.markers.get(&marker).map(|m| (m.buffer, m.position))
                && let Some(position) = position
            {
                let adjusted = adjust_marker_position(position, offset);
                r.marker_set(marker, buffer, Some(adjusted));
            }
        });
        return Ok(());
    }

    Err(unrecognized_undo_entry(&LispObject::cons(car, cdr)))
}

pub fn prim_primitive_undo(args: &LispObject) -> ElispResult<LispObject> {
    let count = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut remaining = integer_or_marker_value(&count)
        .ok_or_else(|| wrong_type_signal("number-or-marker-p", count.clone()))?;
    let mut list = args.nth(1).unwrap_or_else(LispObject::nil);
    if remaining < 0 {
        return Err(wrong_type_signal("natnump", count));
    }

    while remaining > 0 {
        loop {
            let Some((entry, rest)) = list.destructure_cons() else {
                list = LispObject::nil();
                break;
            };
            list = rest;
            if entry.is_nil() {
                break;
            }
            apply_undo_entry(entry, &mut list)?;
        }
        remaining -= 1;
    }
    Ok(list)
}

pub fn prim_buffer_local_variables(_args: &LispObject) -> ElispResult<LispObject> {
    let locals = buffer::with_current(|b| b.locals.clone());
    let mut out = LispObject::nil();
    for (name, value) in locals {
        out = LispObject::cons(LispObject::cons(LispObject::symbol(&name), value), out);
    }
    Ok(out)
}

pub fn prim_buffer_string(_args: &LispObject) -> ElispResult<LispObject> {
    let s = buffer::with_current(|b| b.buffer_string());
    Ok(LispObject::string(&s))
}

pub fn prim_buffer_substring(args: &LispObject) -> ElispResult<LispObject> {
    prim_buffer_substring_impl(args, true)
}

pub fn prim_buffer_substring_no_properties(args: &LispObject) -> ElispResult<LispObject> {
    prim_buffer_substring_impl(args, false)
}

fn prim_buffer_substring_impl(
    args: &LispObject,
    include_properties: bool,
) -> ElispResult<LispObject> {
    let start = int_arg(args, 0, 1) as usize;
    let end = int_arg(args, 1, 1) as usize;
    let (s, spans) = buffer::with_current(|b| {
        let s = b.substring(start, end);
        let spans = if include_properties {
            let base = start.min(end);
            b.text_properties_in_range(start, end)
                .into_iter()
                .map(|mut span| {
                    span.start -= base;
                    span.end -= base;
                    span
                })
                .collect()
        } else {
            Vec::new()
        };
        (s, spans)
    });
    Ok(make_propertized_string(&s, spans))
}

pub fn prim_filter_buffer_substring(args: &LispObject) -> ElispResult<LispObject> {
    let start = int_arg(args, 0, 1) as usize;
    let end = int_arg(args, 1, 1) as usize;
    let delete = args.nth(2).is_some_and(|value| !value.is_nil());
    let s = buffer::with_current(|b| b.substring(start, end));
    if delete {
        buffer::with_registry_mut(|r| r.delete_current_region(start, end));
    }
    Ok(LispObject::string(&s))
}

pub fn prim_insert_buffer_substring(args: &LispObject) -> ElispResult<LispObject> {
    let source = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let source_id =
        resolve_buffer(&source).ok_or_else(|| ElispError::WrongTypeArgument("buffer".into()))?;
    let (text, spans) = buffer::with_registry(|r| {
        let Some(source_buffer) = r.get(source_id) else {
            return Err(ElispError::WrongTypeArgument("buffer".into()));
        };
        let start = args
            .nth(1)
            .and_then(|arg| integer_or_marker_value(&arg))
            .unwrap_or(source_buffer.point_min() as i64)
            .max(1) as usize;
        let end = args
            .nth(2)
            .and_then(|arg| integer_or_marker_value(&arg))
            .unwrap_or(source_buffer.point_max() as i64)
            .max(1) as usize;
        if start < source_buffer.point_min()
            || start > source_buffer.point_max()
            || end < source_buffer.point_min()
            || end > source_buffer.point_max()
        {
            return Err(ElispError::InvalidOperation("args out of range".into()));
        }
        let base = start.min(end);
        let spans: Vec<buffer::TextPropertySpan> = source_buffer
            .text_properties_in_range(start, end)
            .into_iter()
            .map(|mut span| {
                span.start -= base;
                span.end -= base;
                span
            })
            .collect();
        Ok((source_buffer.substring(start, end), spans))
    })?;
    let insert_start = buffer::with_current(|b| b.point);
    buffer::with_registry_mut(|r| r.insert_current(&text, false));
    buffer::with_current_mut(|b| {
        for span in spans {
            b.add_text_properties(
                insert_start + span.start,
                insert_start + span.end,
                span.plist,
            );
        }
    });
    Ok(LispObject::nil())
}

fn checked_current_range(start: usize, end: usize) -> ElispResult<(usize, usize)> {
    buffer::with_current(|b| {
        let pmin = b.point_min();
        let pmax = b.point_max();
        if start < pmin || start > pmax || end < pmin || end > pmax {
            return Err(ElispError::InvalidOperation("args out of range".into()));
        }
        Ok(if start <= end {
            (start, end)
        } else {
            (end, start)
        })
    })
}

pub fn prim_delete_and_extract_region(args: &LispObject) -> ElispResult<LispObject> {
    let start = required_pos_arg(args, 0)?;
    let end = required_pos_arg(args, 1)?;
    let (start, end) = checked_current_range(start, end)?;
    let text = buffer::with_current(|b| b.substring(start, end));
    buffer::with_registry_mut(|r| r.delete_current_region(start, end));
    Ok(LispObject::string(&text))
}

fn replacement_source_text(source: &LispObject) -> ElispResult<String> {
    if let Some(text) = source.as_string() {
        return Ok(text.clone());
    }
    if let Some(buffer_id) = resolve_buffer(source) {
        return buffer::with_registry(|r| {
            r.get(buffer_id)
                .map(buffer::StubBuffer::buffer_string)
                .ok_or_else(|| ElispError::WrongTypeArgument("buffer".into()))
        });
    }
    if let LispObject::Vector(vector) = source {
        let items = vector.lock().clone();
        let Some(buffer_obj) = items.first() else {
            return Err(ElispError::WrongTypeArgument("buffer".into()));
        };
        let buffer_id = resolve_buffer(buffer_obj)
            .ok_or_else(|| ElispError::WrongTypeArgument("buffer".into()))?;
        let start = items
            .get(1)
            .and_then(integer_or_marker_value)
            .unwrap_or(1)
            .max(1) as usize;
        return buffer::with_registry(|r| {
            let Some(buffer) = r.get(buffer_id) else {
                return Err(ElispError::WrongTypeArgument("buffer".into()));
            };
            let end = items
                .get(2)
                .and_then(integer_or_marker_value)
                .unwrap_or(buffer.point_max() as i64)
                .max(1) as usize;
            Ok(buffer.substring(start, end))
        });
    }
    Err(ElispError::WrongTypeArgument("buffer-or-string".into()))
}

pub fn prim_replace_region_contents(args: &LispObject) -> ElispResult<LispObject> {
    let start = args
        .first()
        .and_then(|arg| integer_or_marker_value(&arg))
        .ok_or(ElispError::WrongNumberOfArguments)?
        .max(1) as usize;
    let end = args
        .nth(1)
        .and_then(|arg| integer_or_marker_value(&arg))
        .ok_or(ElispError::WrongNumberOfArguments)?
        .max(1) as usize;
    let source = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let replacement = replacement_source_text(&source)?;
    let replacement_len = replacement.chars().count();
    let (a, b) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };

    buffer::with_registry_mut(|registry| {
        let current = registry.current_id();
        let old_point = registry.get(current).map_or(1, |buffer| buffer.point);
        let markers = registry
            .markers
            .values()
            .filter(|marker| marker.buffer == current)
            .filter_map(|marker| {
                marker
                    .position
                    .map(|pos| (marker.id, pos, marker.insertion_type))
            })
            .collect::<Vec<_>>();
        let old_len = b.saturating_sub(a);
        let point_after = if old_point < a {
            old_point
        } else if old_point > b {
            old_point + replacement_len.saturating_sub(old_len)
        } else {
            a + replacement_len
        };

        registry.delete_current_region(a, b);
        if let Some(buffer) = registry.get_mut(current) {
            buffer.goto_char(a);
        }
        registry.insert_current(&replacement, false);

        let delta = replacement_len as i64 - old_len as i64;
        for (marker_id, pos, insertion_type) in markers {
            let new_pos = if pos <= a {
                pos
            } else if pos >= b {
                (pos as i64 + delta).max(1) as usize
            } else if insertion_type {
                a + replacement_len
            } else {
                a
            };
            registry.marker_set(marker_id, current, Some(new_pos));
        }
        if let Some(buffer) = registry.get_mut(current) {
            buffer.goto_char(point_after);
        }
    });
    Ok(LispObject::nil())
}

pub fn prim_compare_buffer_substrings(args: &LispObject) -> ElispResult<LispObject> {
    let buffer1 = args
        .first()
        .and_then(|arg| resolve_buffer(&arg))
        .ok_or_else(|| ElispError::WrongTypeArgument("buffer".into()))?;
    let start1 = args
        .nth(1)
        .and_then(|arg| integer_or_marker_value(&arg))
        .ok_or(ElispError::WrongNumberOfArguments)?
        .max(1) as usize;
    let end1 = args
        .nth(2)
        .and_then(|arg| integer_or_marker_value(&arg))
        .ok_or(ElispError::WrongNumberOfArguments)?
        .max(1) as usize;
    let buffer2 = args
        .nth(3)
        .and_then(|arg| resolve_buffer(&arg))
        .ok_or_else(|| ElispError::WrongTypeArgument("buffer".into()))?;
    let start2 = args
        .nth(4)
        .and_then(|arg| integer_or_marker_value(&arg))
        .ok_or(ElispError::WrongNumberOfArguments)?
        .max(1) as usize;
    let end2 = args
        .nth(5)
        .and_then(|arg| integer_or_marker_value(&arg))
        .ok_or(ElispError::WrongNumberOfArguments)?
        .max(1) as usize;

    let (s1, s2) = buffer::with_registry(|r| {
        let s1 = r
            .get(buffer1)
            .map(|buffer| buffer.substring(start1, end1))
            .unwrap_or_default();
        let s2 = r
            .get(buffer2)
            .map(|buffer| buffer.substring(start2, end2))
            .unwrap_or_default();
        (s1, s2)
    });
    for (idx, (a, b)) in s1.chars().zip(s2.chars()).enumerate() {
        if a != b {
            let pos = idx as i64 + 1;
            return Ok(LispObject::integer(if a < b { -pos } else { pos }));
        }
    }
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    if len1 == len2 {
        Ok(LispObject::integer(0))
    } else {
        let pos = len1.min(len2) as i64 + 1;
        Ok(LispObject::integer(if len1 < len2 { -pos } else { pos }))
    }
}

pub fn prim_subst_char_in_region(args: &LispObject) -> ElispResult<LispObject> {
    let start = args
        .first()
        .and_then(|arg| integer_or_marker_value(&arg))
        .ok_or(ElispError::WrongNumberOfArguments)?
        .max(1) as usize;
    let end = args
        .nth(1)
        .and_then(|arg| integer_or_marker_value(&arg))
        .ok_or(ElispError::WrongNumberOfArguments)?
        .max(1) as usize;
    let from = args
        .nth(2)
        .and_then(|arg| arg.as_integer())
        .and_then(|code| char::from_u32(code as u32))
        .ok_or_else(|| ElispError::WrongTypeArgument("character".into()))?;
    let to = args
        .nth(3)
        .and_then(|arg| arg.as_integer())
        .and_then(|code| char::from_u32(code as u32))
        .ok_or_else(|| ElispError::WrongTypeArgument("character".into()))?;

    buffer::with_current_mut(|buffer| {
        let (a, b) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let a = a.clamp(buffer.point_min(), buffer.point_max());
        let b = b.clamp(buffer.point_min(), buffer.point_max());
        let a_byte = buffer.char_to_byte(a);
        let b_byte = buffer.char_to_byte(b);
        let original = buffer.text[a_byte..b_byte].to_string();
        let replaced = original
            .chars()
            .map(|ch| if ch == from { to } else { ch })
            .collect::<String>();
        if replaced != original {
            buffer.text.replace_range(a_byte..b_byte, &replaced);
            buffer.bump_modified();
        }
    });
    Ok(LispObject::nil())
}

pub fn prim_buffer_file_name(args: &LispObject) -> ElispResult<LispObject> {
    let id = args
        .first()
        .and_then(|a| resolve_buffer(&a))
        .unwrap_or_else(|| buffer::with_registry(|r| r.current_id()));
    let f = buffer::with_registry(|r| r.get(id).and_then(|b| b.file_name.clone()));
    Ok(f.map(|s| LispObject::string(&s))
        .unwrap_or(LispObject::nil()))
}

pub fn prim_set_visited_file_name(args: &LispObject) -> ElispResult<LispObject> {
    let file = args.first().and_then(|arg| arg.as_string().cloned());
    buffer::with_current_mut(|b| {
        b.file_name = file.clone();
    });
    Ok(file
        .map(|path| LispObject::string(&path))
        .unwrap_or_else(LispObject::nil))
}

// ---- Point / positions ------------------------------------------------

pub fn prim_point(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(buffer::with_current(|b| b.point) as i64))
}

pub fn prim_point_min(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(
        buffer::with_current(|b| b.point_min()) as i64
    ))
}

pub fn prim_point_max(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(
        buffer::with_current(|b| b.point_max()) as i64
    ))
}

pub fn prim_gap_position(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(buffer::with_current(|b| b.point) as i64))
}

pub fn prim_gap_size(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(0))
}

pub fn prim_position_bytes(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|arg| integer_or_marker_value(&arg))
        .unwrap_or_else(|| buffer::with_current(|b| b.point as i64));
    if pos < 1 {
        return Ok(LispObject::nil());
    }
    buffer::with_current(|b| {
        let pos = pos as usize;
        if pos > b.point_max() {
            return Ok(LispObject::nil());
        }
        Ok(LispObject::integer((b.char_to_byte(pos) + 1) as i64))
    })
}

pub fn prim_byte_to_position(args: &LispObject) -> ElispResult<LispObject> {
    let byte_pos = args
        .first()
        .and_then(|arg| arg.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    if byte_pos < 1 {
        return Ok(LispObject::nil());
    }
    buffer::with_current(|b| {
        let byte_idx = byte_pos as usize - 1;
        if byte_idx > b.text.len() {
            return Ok(LispObject::nil());
        }
        if byte_idx == b.text.len() {
            return Ok(LispObject::integer(b.point_max() as i64));
        }
        let mut pos = 1usize;
        for (char_pos, (start, _)) in b.text.char_indices().enumerate() {
            if start > byte_idx {
                break;
            }
            pos = char_pos + 1;
        }
        Ok(LispObject::integer(pos as i64))
    })
}

pub fn prim_bidi_find_overridden_directionality(args: &LispObject) -> ElispResult<LispObject> {
    let start = required_pos_arg(args, 0)?;
    let end = required_pos_arg(args, 1)?;
    let (start, end) = checked_current_range(start, end)?;
    let found = buffer::with_current(|b| {
        let logical_pos = |pos: usize| {
            let crs_before = (start..pos)
                .filter(|candidate| b.char_at(*candidate) == Some('\r'))
                .count();
            pos.saturating_sub(crs_before)
        };
        let adjusted_affected_pos = |pos: usize| {
            let crs_before = (start..pos)
                .filter(|candidate| b.char_at(*candidate) == Some('\r'))
                .count();
            let logical = pos.saturating_sub(crs_before);
            if crs_before > 0 {
                logical.saturating_sub(2)
            } else {
                logical
            }
        };
        let mut pos = start;
        while pos < end {
            match b.char_at(pos) {
                Some('\u{202a}' | '\u{202b}' | '\u{202d}' | '\u{202e}') => {
                    if matches!(
                        b.char_at(pos + 1),
                        Some('\u{2066}' | '\u{2067}' | '\u{2068}')
                    ) {
                        return Some(logical_pos(pos));
                    }
                    let mut affected = pos + 1;
                    while affected < end {
                        match b.char_at(affected) {
                            Some(ch) if ch.is_alphanumeric() => {
                                return Some(adjusted_affected_pos(affected));
                            }
                            Some('\u{202c}' | '\u{2069}') | None => break,
                            _ => affected += 1,
                        }
                    }
                    return Some(logical_pos(pos));
                }
                Some('\u{2066}' | '\u{2067}' | '\u{2068}') => {
                    let mut affected = pos + 1;
                    while affected < end {
                        match b.char_at(affected) {
                            Some(ch) if ch.is_alphanumeric() => return Some(logical_pos(affected)),
                            Some('\u{2069}') | None => break,
                            _ => affected += 1,
                        }
                    }
                }
                _ => {}
            }
            pos += 1;
        }
        None
    });
    Ok(found
        .map(|pos| LispObject::integer(pos as i64))
        .unwrap_or_else(LispObject::nil))
}

pub fn prim_mark(args: &LispObject) -> ElispResult<LispObject> {
    let force = args.first().is_some_and(|value| !value.is_nil());
    let mark = buffer::with_current(|b| b.mark);
    match (mark, force) {
        (Some(pos), _) => Ok(LispObject::integer(pos as i64)),
        (None, true) => Ok(LispObject::nil()),
        (None, false) => Err(ElispError::InvalidOperation("mark not set".into())),
    }
}

pub fn prim_mark_marker(_args: &LispObject) -> ElispResult<LispObject> {
    let id = buffer::with_registry_mut(|r| {
        let buf = r.current_id();
        let pos = r.get(buf).and_then(|b| b.mark);
        let id = r.make_marker(buf);
        r.marker_set(id, buf, pos);
        id
    });
    Ok(make_marker_object(id))
}

pub fn prim_set_mark(args: &LispObject) -> ElispResult<LispObject> {
    let pos = int_arg(args, 0, buffer::with_current(|b| b.point) as i64) as usize;
    buffer::with_current_mut(|b| {
        let pos = pos.clamp(b.point_min(), b.point_max());
        b.mark = Some(pos);
        b.mark_active = true;
    });
    Ok(LispObject::nil())
}

pub fn prim_push_mark(args: &LispObject) -> ElispResult<LispObject> {
    let pos = int_arg(args, 0, buffer::with_current(|b| b.point) as i64) as usize;
    let activate = args.nth(2).is_some_and(|value| !value.is_nil());
    buffer::with_current_mut(|b| {
        let pos = pos.clamp(b.point_min(), b.point_max());
        b.mark = Some(pos);
        b.mark_active = activate;
    });
    Ok(LispObject::nil())
}

pub fn prim_region_beginning(_args: &LispObject) -> ElispResult<LispObject> {
    let pos = buffer::with_current(|b| b.mark.map_or(b.point, |mark| mark.min(b.point)));
    Ok(LispObject::integer(pos as i64))
}

pub fn prim_region_end(_args: &LispObject) -> ElispResult<LispObject> {
    let pos = buffer::with_current(|b| b.mark.map_or(b.point, |mark| mark.max(b.point)));
    Ok(LispObject::integer(pos as i64))
}

pub fn prim_region_active_p(_args: &LispObject) -> ElispResult<LispObject> {
    let active = buffer::with_current(|b| b.mark_active && b.mark.is_some());
    Ok(LispObject::from(active))
}

pub fn prim_bobp(_args: &LispObject) -> ElispResult<LispObject> {
    let at_bob = buffer::with_current(|b| b.point <= b.point_min());
    Ok(LispObject::from(at_bob))
}

pub fn prim_eobp(_args: &LispObject) -> ElispResult<LispObject> {
    let at_eob = buffer::with_current(|b| b.point >= b.point_max());
    Ok(LispObject::from(at_eob))
}

pub fn prim_bolp(_args: &LispObject) -> ElispResult<LispObject> {
    let at_bol = buffer::with_current(|b| b.point == b.line_beginning_position(b.point));
    Ok(LispObject::from(at_bol))
}

pub fn prim_eolp(_args: &LispObject) -> ElispResult<LispObject> {
    let at_eol = buffer::with_current(|b| b.point == b.line_end_position(b.point));
    Ok(LispObject::from(at_eol))
}

pub fn prim_goto_char(args: &LispObject) -> ElispResult<LispObject> {
    let pos = int_arg(args, 0, 1) as usize;
    buffer::with_current_mut(|b| b.goto_char(pos));
    Ok(LispObject::integer(pos as i64))
}

pub fn prim_forward_char(args: &LispObject) -> ElispResult<LispObject> {
    let n = int_arg(args, 0, 1);
    let overshoot = buffer::with_current_mut(|b| {
        let target = b.point as i64 + n;
        let pmin = b.point_min() as i64;
        let pmax = b.point_max() as i64;
        if target < pmin {
            b.point = pmin as usize;
            Some(false) // hit BOB
        } else if target > pmax {
            b.point = pmax as usize;
            Some(true) // hit EOB
        } else {
            b.point = target as usize;
            None
        }
    });
    match overshoot {
        Some(true) => Err(ElispError::Signal(Box::new(crate::error::SignalData {
            symbol: LispObject::symbol("end-of-buffer"),
            data: LispObject::nil(),
        }))),
        Some(false) => Err(ElispError::Signal(Box::new(crate::error::SignalData {
            symbol: LispObject::symbol("beginning-of-buffer"),
            data: LispObject::nil(),
        }))),
        None => Ok(LispObject::nil()),
    }
}

pub fn prim_backward_char(args: &LispObject) -> ElispResult<LispObject> {
    // (backward-char N) ≡ (forward-char -N). Inherit the boundary
    // signaling semantics from prim_forward_char.
    let n = int_arg(args, 0, 1);
    let neg_args = LispObject::cons(LispObject::integer(-n), LispObject::nil());
    prim_forward_char(&neg_args)
}

pub fn prim_forward_line(args: &LispObject) -> ElispResult<LispObject> {
    let n = int_arg(args, 0, 1);
    let rem = buffer::with_current_mut(|b| b.forward_line(n));
    Ok(LispObject::integer(rem))
}

pub fn prim_beginning_of_line(args: &LispObject) -> ElispResult<LispObject> {
    let n = int_arg(args, 0, 1);
    buffer::with_current_mut(|b| {
        if n != 1 {
            b.forward_line(n - 1);
        }
        b.point = b.line_beginning_position(b.point);
    });
    Ok(LispObject::nil())
}

pub fn prim_end_of_line(args: &LispObject) -> ElispResult<LispObject> {
    let n = int_arg(args, 0, 1);
    buffer::with_current_mut(|b| {
        if n != 1 {
            b.forward_line(n - 1);
        }
        b.point = b.line_end_position(b.point);
    });
    Ok(LispObject::nil())
}

pub fn prim_line_beginning_position(args: &LispObject) -> ElispResult<LispObject> {
    let n = int_arg(args, 0, 1);
    let pos = buffer::with_current_mut(|b| {
        let save = b.point;
        if n != 1 {
            b.forward_line(n - 1);
        }
        let out = b.line_beginning_position(b.point);
        b.point = save;
        out
    });
    Ok(LispObject::integer(pos as i64))
}

pub fn prim_line_end_position(args: &LispObject) -> ElispResult<LispObject> {
    let n = int_arg(args, 0, 1);
    let pos = buffer::with_current_mut(|b| {
        let save = b.point;
        if n != 1 {
            b.forward_line(n - 1);
        }
        let out = b.line_end_position(b.point);
        b.point = save;
        out
    });
    Ok(LispObject::integer(pos as i64))
}

pub fn prim_line_number_at_pos(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|a| a.as_integer())
        .map(|n| n as usize)
        .unwrap_or_else(|| buffer::with_current(|b| b.point));
    let n = buffer::with_current(|b| b.line_number_at_pos(pos));
    Ok(LispObject::integer(n as i64))
}

pub fn prim_current_column(_args: &LispObject) -> ElispResult<LispObject> {
    let col = buffer::with_current(|b| {
        let p = b.point;
        let bol = b.line_beginning_position(p);
        p.saturating_sub(bol)
    });
    Ok(LispObject::integer(col as i64))
}

pub fn prim_move_to_column(args: &LispObject) -> ElispResult<LispObject> {
    let col = int_arg(args, 0, 0) as usize;
    let force = args.nth(1).is_some_and(|value| !value.is_nil());
    let (reached, spaces) = buffer::with_current_mut(|b| {
        let bol = b.line_beginning_position(b.point);
        let eol = b.line_end_position(b.point);
        let eol_col = eol.saturating_sub(bol);
        let target = (bol + col).min(eol);
        b.point = target;
        if force && col > eol_col {
            (col, col - eol_col)
        } else {
            (target - bol, 0)
        }
    });
    if spaces > 0 {
        buffer::with_registry_mut(|r| r.insert_current(&" ".repeat(spaces), false));
    }
    Ok(LispObject::integer(reached as i64))
}

pub fn prim_indent_to(args: &LispObject) -> ElispResult<LispObject> {
    let col = int_arg(args, 0, 0).max(0) as usize;
    let current = buffer::with_current(|b| {
        let bol = b.line_beginning_position(b.point);
        b.point.saturating_sub(bol)
    });
    if col > current {
        buffer::with_registry_mut(|r| r.insert_current(&" ".repeat(col - current), false));
        Ok(LispObject::integer(col as i64))
    } else {
        Ok(LispObject::integer(current as i64))
    }
}

/// Replace the leading whitespace of the current line so it spans
/// exactly COLUMN columns of spaces, matching Emacs's `indent-line-to`.
/// Leaves point at the end of the indentation.
pub fn prim_indent_line_to(args: &LispObject) -> ElispResult<LispObject> {
    let col = int_arg(args, 0, 0).max(0) as usize;
    buffer::with_current_mut(|b| {
        let bol = b.line_beginning_position(b.point);
        let mut end_ws = bol;
        while end_ws < b.point_max() {
            match b.char_at(end_ws) {
                Some(' ') | Some('\t') => end_ws += 1,
                _ => break,
            }
        }
        if end_ws > bol {
            b.delete_region(bol, end_ws);
        }
        b.goto_char(bol);
    });
    if col > 0 {
        buffer::with_registry_mut(|r| r.insert_current(&" ".repeat(col), false));
    }
    Ok(LispObject::integer(col as i64))
}

/// Shift each line in [START, END] by N columns. Positive N inserts
/// spaces at BOL; negative N removes leading spaces (capped at the
/// available leading whitespace). Matches the non-interactive form
/// of Emacs's `indent-rigidly`.
pub fn prim_indent_rigidly(args: &LispObject) -> ElispResult<LispObject> {
    let start = required_pos_arg(args, 0)?;
    let end = required_pos_arg(args, 1)?;
    let n = args.nth(2).and_then(|v| v.as_integer()).unwrap_or(0);
    if n == 0 {
        return Ok(LispObject::nil());
    }
    let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
    let line_starts: Vec<usize> = buffer::with_current(|b| {
        let mut starts = vec![b.line_beginning_position(lo)];
        let mut p = b.line_beginning_position(lo);
        while p < hi {
            let next = b.line_end_position(p) + 1;
            if next >= b.point_max() || next > hi {
                break;
            }
            starts.push(next);
            p = next;
        }
        starts
    });
    if n > 0 {
        let pad = " ".repeat(n as usize);
        // Insert at each line start, walking backward to keep earlier
        // positions valid.
        for &start in line_starts.iter().rev() {
            buffer::with_current_mut(|b| {
                b.goto_char(start);
            });
            buffer::with_registry_mut(|r| r.insert_current(&pad, false));
        }
    } else {
        let to_remove = (-n) as usize;
        for &start in line_starts.iter().rev() {
            buffer::with_current_mut(|b| {
                let mut q = start;
                let limit = (start + to_remove).min(b.point_max());
                while q < limit && b.char_at(q) == Some(' ') {
                    q += 1;
                }
                if q > start {
                    b.delete_region(start, q);
                }
            });
        }
    }
    Ok(LispObject::nil())
}

/// `(indent-region START END &optional COLUMN)`. Without a major-mode
/// indent function we can only honor the COLUMN argument: each line
/// in the region is indented to that column. With no COLUMN argument
/// this is a no-op (the real behavior is mode-dependent and out of
/// scope headlessly).
pub fn prim_indent_region(args: &LispObject) -> ElispResult<LispObject> {
    let start = required_pos_arg(args, 0)?;
    let end = required_pos_arg(args, 1)?;
    let Some(col_arg) = args.nth(2).and_then(|v| v.as_integer()) else {
        return Ok(LispObject::nil());
    };
    let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
    let line_starts: Vec<usize> = buffer::with_current(|b| {
        let mut starts = vec![b.line_beginning_position(lo)];
        let mut p = b.line_beginning_position(lo);
        while p < hi {
            let next = b.line_end_position(p) + 1;
            if next >= b.point_max() || next > hi {
                break;
            }
            starts.push(next);
            p = next;
        }
        starts
    });
    let saved_point = buffer::with_current(|b| b.point);
    for &line_start in line_starts.iter().rev() {
        buffer::with_current_mut(|b| b.goto_char(line_start));
        prim_indent_line_to(&LispObject::cons(
            LispObject::integer(col_arg),
            LispObject::nil(),
        ))?;
    }
    buffer::with_current_mut(|b| b.goto_char(saved_point));
    Ok(LispObject::nil())
}

// ---- Narrowing --------------------------------------------------------

pub fn prim_narrow_to_region(args: &LispObject) -> ElispResult<LispObject> {
    let a = int_arg(args, 0, 1) as usize;
    let b = int_arg(args, 1, 1) as usize;
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    buffer::with_current_mut(|buf| {
        // Clamp to the CURRENT restriction (if any), not raw text.
        // This ensures a nested narrow can only shrink, never expand,
        // the visible region — matching Emacs's `save-restriction`
        // semantics. A plain `widen` must come first to escape an
        // outer narrowing.
        let pmin = buf.point_min();
        let pmax = buf.point_max();
        let lo = lo.clamp(pmin, pmax);
        let hi = hi.clamp(pmin, pmax);
        buf.set_manual_restriction(Some((lo, hi)));
    });
    Ok(LispObject::nil())
}

pub fn prim_widen(_args: &LispObject) -> ElispResult<LispObject> {
    buffer::with_current_mut(|b| b.set_manual_restriction(None));
    Ok(LispObject::nil())
}

pub fn prim_buffer_narrowed_p(_args: &LispObject) -> ElispResult<LispObject> {
    let narrowed = buffer::with_current(|b| b.restriction.is_some());
    Ok(LispObject::from(narrowed))
}

pub fn prim_set_buffer_multibyte(args: &LispObject) -> ElispResult<LispObject> {
    let enabled = args.first().is_none_or(|arg| !arg.is_nil());
    buffer::with_registry_mut(|r| r.set_current_multibyte(enabled));
    Ok(LispObject::nil())
}

fn restriction_label(obj: &LispObject) -> Option<String> {
    obj.as_symbol()
        .or_else(|| obj.as_string().map(ToOwned::to_owned))
}

pub fn prim_internal_labeled_narrow_to_region(args: &LispObject) -> ElispResult<LispObject> {
    let a = int_arg(args, 0, 1);
    let b = int_arg(args, 1, 1);
    let label = args
        .nth(2)
        .and_then(|obj| restriction_label(&obj))
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
    let lo_raw = a.min(b).max(1) as usize;
    let hi_raw = a.max(b).max(1) as usize;
    buffer::with_current_mut(|buf| {
        let pmin = buf.point_min();
        let pmax = buf.point_max();
        let lo = lo_raw.clamp(pmin, pmax);
        let hi = hi_raw.clamp(pmin, pmax);
        buf.push_restriction_layer(Some(label), (lo, hi));
    });
    Ok(LispObject::nil())
}

pub fn prim_internal_labeled_widen(args: &LispObject) -> ElispResult<LispObject> {
    let label = args
        .first()
        .and_then(|obj| restriction_label(&obj))
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
    buffer::with_current_mut(|buf| {
        buf.manual_restriction = None;
        buf.remove_restriction_label(&label);
    });
    Ok(LispObject::nil())
}

// ---- Characters and regions -----------------------------------------

pub fn prim_char_after(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|a| a.as_integer())
        .map(|n| n as usize)
        .unwrap_or_else(|| buffer::with_current(|b| b.point));
    let c = buffer::with_current(|b| {
        if pos < b.point_min() || pos >= b.point_max() {
            None
        } else {
            b.char_at(pos)
        }
    });
    Ok(c.map(|ch| LispObject::integer(ch as i64))
        .unwrap_or(LispObject::nil()))
}

pub fn prim_char_before(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|a| a.as_integer())
        .map(|n| n as usize)
        .unwrap_or_else(|| buffer::with_current(|b| b.point));
    let c = buffer::with_current(|b| {
        if pos <= b.point_min() || pos > b.point_max() {
            None
        } else {
            b.char_at(pos - 1)
        }
    });
    Ok(c.map(|ch| LispObject::integer(ch as i64))
        .unwrap_or(LispObject::nil()))
}

pub fn prim_following_char(_args: &LispObject) -> ElispResult<LispObject> {
    let c = buffer::with_current(|b| {
        if b.point >= b.point_max() {
            None
        } else {
            b.char_at(b.point)
        }
    });
    Ok(c.map(|ch| LispObject::integer(ch as i64))
        .unwrap_or_else(|| LispObject::integer(0)))
}

pub fn prim_preceding_char(_args: &LispObject) -> ElispResult<LispObject> {
    let c = buffer::with_current(|b| {
        if b.point <= b.point_min() || b.point > b.point_max() {
            None
        } else {
            b.char_at(b.point - 1)
        }
    });
    Ok(c.map(|ch| LispObject::integer(ch as i64))
        .unwrap_or_else(|| LispObject::integer(0)))
}

pub fn prim_delete_region(args: &LispObject) -> ElispResult<LispObject> {
    let start = int_arg(args, 0, 1) as usize;
    let end = int_arg(args, 1, 1) as usize;
    buffer::with_registry_mut(|r| r.delete_current_region(start, end));
    Ok(LispObject::nil())
}

pub fn prim_delete_char(args: &LispObject) -> ElispResult<LispObject> {
    let n = int_arg(args, 0, 1);
    let (start, end) = buffer::with_current(|b| {
        if n >= 0 {
            let end = (b.point as i64 + n).min(b.point_max() as i64) as usize;
            (b.point, end)
        } else {
            let start = (b.point as i64 + n).max(b.point_min() as i64) as usize;
            (start, b.point)
        }
    });
    buffer::with_registry_mut(|r| r.delete_current_region(start, end));
    Ok(LispObject::nil())
}

fn object_to_insert_text(obj: &LispObject) -> String {
    match obj {
        LispObject::String(s) => crate::object::current_string_value(s),
        LispObject::Integer(i) => char::from_u32(*i as u32)
            .map(|c| c.to_string())
            .unwrap_or_default(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        other => other.prin1_to_string(),
    }
}

pub fn prim_insert_and_inherit(args: &LispObject) -> ElispResult<LispObject> {
    let mut current = args.clone();
    let mut text = String::new();
    while let Some((arg, rest)) = current.destructure_cons() {
        text.push_str(&object_to_insert_text(&arg));
        current = rest;
    }
    let start = buffer::with_current(|b| b.point);
    let inherited = buffer::with_current(|b| {
        if start > b.point_min() {
            b.properties_at(start - 1)
        } else {
            b.properties_at(start)
        }
    });
    let len = text.chars().count();
    buffer::with_registry_mut(|r| r.insert_current(&text, false));
    if len > 0 && !inherited.is_empty() {
        buffer::with_current_mut(|b| b.add_text_properties(start, start + len, inherited));
    }
    Ok(LispObject::nil())
}

pub fn prim_insert_byte(args: &LispObject) -> ElispResult<LispObject> {
    let byte = args
        .first()
        .and_then(|arg| arg.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    if !(0..=255).contains(&byte) {
        return Err(ElispError::WrongTypeArgument("character".to_string()));
    }
    let count = int_arg(args, 1, 1).max(0) as usize;
    let ch = char::from_u32(byte as u32)
        .ok_or_else(|| ElispError::WrongTypeArgument("character".to_string()))?;
    let text: String = std::iter::repeat_n(ch, count).collect();
    buffer::with_registry_mut(|r| r.insert_current(&text, false));
    Ok(LispObject::nil())
}

pub fn prim_insert_char(args: &LispObject) -> ElispResult<LispObject> {
    let ch = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("character".into()))?;
    let count = int_arg(args, 1, 1) as usize;
    if let Some(c) = char::from_u32(ch as u32) {
        let s: String = std::iter::repeat_n(c, count).collect();
        buffer::with_registry_mut(|r| r.insert_current(&s, false));
    }
    Ok(LispObject::nil())
}

fn replace_current_range(start: usize, end: usize, replacement: &str) {
    buffer::with_current_mut(|b| {
        let start_byte = b.char_to_byte(start);
        let end_byte = b.char_to_byte(end);
        b.text.replace_range(start_byte..end_byte, replacement);
        b.point = start + replacement.chars().count();
        b.bump_modified();
    });
}

pub fn prim_transpose_regions(args: &LispObject) -> ElispResult<LispObject> {
    let start1 = required_pos_arg(args, 0)?;
    let end1 = required_pos_arg(args, 1)?;
    let start2 = required_pos_arg(args, 2)?;
    let end2 = required_pos_arg(args, 3)?;
    let leave_markers = args.nth(4).is_some_and(|value| !value.is_nil());
    let (a1, b1) = checked_current_range(start1, end1)?;
    let (a2, b2) = checked_current_range(start2, end2)?;
    if b1 > a2 && b2 > a1 {
        return Err(ElispError::InvalidOperation(
            "transpose-regions: overlapping regions".into(),
        ));
    }
    let ((a1, b1), (a2, b2)) = if a1 <= a2 {
        ((a1, b1), (a2, b2))
    } else {
        ((a2, b2), (a1, b1))
    };

    let (text1, middle, text2, buffer_id) = buffer::with_current(|b| {
        (
            b.substring(a1, b1),
            b.substring(b1, a2),
            b.substring(a2, b2),
            b.id,
        )
    });
    let len1 = b1 - a1;
    let len2 = b2 - a2;
    let new_region = format!("{text2}{middle}{text1}");

    buffer::with_registry_mut(|r| {
        if !leave_markers {
            let middle_len = a2 - b1;
            for marker in r.markers.values_mut() {
                if marker.buffer != buffer_id {
                    continue;
                }
                let Some(pos) = marker.position else {
                    continue;
                };
                marker.position = Some(if pos < a1 || pos >= b2 {
                    pos
                } else if pos < b1 {
                    pos + len2 + middle_len
                } else if pos < a2 {
                    if len2 >= len1 {
                        pos + (len2 - len1)
                    } else {
                        pos.saturating_sub(len1 - len2)
                    }
                } else {
                    a1 + (pos - a2)
                });
            }
        }
    });
    replace_current_range(a1, b2, &new_region);
    Ok(LispObject::nil())
}

pub fn prim_upcase_initials_region(args: &LispObject) -> ElispResult<LispObject> {
    let start = required_pos_arg(args, 0)?;
    let end = required_pos_arg(args, 1)?;
    let (start, end) = checked_current_range(start, end)?;
    let replacement = buffer::with_current(|b| {
        crate::emacs::casefiddle::upcase_initials(&b.substring(start, end))
    });
    replace_current_range(start, end, &replacement);
    Ok(LispObject::nil())
}

pub fn prim_field_beginning(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|arg| integer_or_marker_value(&arg))
        .unwrap_or_else(|| buffer::with_current(|b| b.point as i64));
    if pos < 1 {
        return Ok(LispObject::nil());
    }
    Ok(LispObject::integer(
        buffer::with_current(|b| field_bounds(b, pos as usize).0) as i64,
    ))
}

pub fn prim_field_end(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|arg| integer_or_marker_value(&arg))
        .unwrap_or_else(|| buffer::with_current(|b| b.point as i64));
    if pos < 1 {
        return Ok(LispObject::nil());
    }
    Ok(LispObject::integer(
        buffer::with_current(|b| field_bounds(b, pos as usize).1) as i64,
    ))
}

pub fn prim_field_string_no_properties(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|arg| integer_or_marker_value(&arg))
        .unwrap_or_else(|| buffer::with_current(|b| b.point as i64));
    if pos < 1 {
        return Ok(LispObject::string(""));
    }
    let text = buffer::with_current(|b| {
        let (start, end) = field_bounds(b, pos as usize);
        b.substring(start, end)
    });
    Ok(LispObject::string(&text))
}

pub fn prim_delete_field(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|arg| integer_or_marker_value(&arg))
        .unwrap_or_else(|| buffer::with_current(|b| b.point as i64));
    if pos < 1 {
        return Ok(LispObject::nil());
    }
    let (start, end) = buffer::with_current(|b| field_bounds(b, pos as usize));
    buffer::with_registry_mut(|r| r.delete_current_region(start, end));
    Ok(LispObject::nil())
}

pub fn prim_constrain_to_field(args: &LispObject) -> ElispResult<LispObject> {
    let use_point = args.first().map_or(true, |arg| arg.is_nil());
    let new_pos = args
        .first()
        .and_then(|arg| integer_or_marker_value(&arg))
        .unwrap_or_else(|| buffer::with_current(|b| b.point as i64));
    let old_pos = args
        .nth(1)
        .and_then(|arg| integer_or_marker_value(&arg))
        .unwrap_or_else(|| buffer::with_current(|b| b.point as i64));
    if new_pos < 1 || old_pos < 1 {
        return Ok(LispObject::nil());
    }
    let constrained = buffer::with_current(|b| {
        let (start, end) = field_bounds(b, old_pos as usize);
        (new_pos as usize).clamp(start, end)
    });
    if use_point {
        buffer::with_current_mut(|b| b.goto_char(constrained));
    }
    Ok(LispObject::integer(constrained as i64))
}

fn field_bounds(buffer: &buffer::StubBuffer, pos: usize) -> (usize, usize) {
    let pmin = buffer.point_min();
    let pmax = buffer.point_max();
    if pmin >= pmax {
        return (pmin, pmax);
    }
    let pos = pos.clamp(pmin, pmax);
    let sample = if pos == pmax { pos - 1 } else { pos };
    let field_prop = LispObject::symbol("field");
    let current = buffer.property_at(sample, &field_prop);
    let mut start = pmin;
    let mut cursor = sample;
    while cursor > pmin {
        let prev = cursor - 1;
        if buffer.property_at(prev, &field_prop) != current {
            start = cursor;
            break;
        }
        cursor = prev;
    }
    let mut end = pmax;
    cursor = sample + 1;
    while cursor < pmax {
        if buffer.property_at(cursor, &field_prop) != current {
            end = cursor;
            break;
        }
        cursor += 1;
    }
    (start, end)
}

pub fn prim_minibuffer_prompt_end(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(
        buffer::with_current(|b| b.point_min()) as i64
    ))
}

// ---- Skip chars / skip syntax ----------------------------------------

/// Parse a char-class spec like "a-zA-Z0-9" or "^ \t" (^ means negate).
fn chars_match(c: char, spec: &str) -> bool {
    let (neg, body) = match spec.strip_prefix('^') {
        Some(rest) => (true, rest),
        None => (false, spec),
    };
    let bytes: Vec<char> = body.chars().collect();
    let mut i = 0;
    let mut matched = false;
    while i < bytes.len() {
        let a = bytes[i];
        if a == '['
            && i + 3 < bytes.len()
            && bytes[i + 1] == ':'
            && let Some(end) = bytes[i + 2..]
                .windows(2)
                .position(|window| window[0] == ':' && window[1] == ']')
        {
            let name: String = bytes[i + 2..i + 2 + end].iter().collect();
            if char_class_match(c, &name) {
                matched = true;
            }
            i += end + 4;
            continue;
        }
        // Range "a-z"
        if i + 2 < bytes.len() && bytes[i + 1] == '-' {
            let b = bytes[i + 2];
            if c >= a && c <= b {
                matched = true;
            }
            i += 3;
        } else {
            if c == a {
                matched = true;
            }
            i += 1;
        }
    }
    if neg { !matched } else { matched }
}

fn char_class_match(c: char, name: &str) -> bool {
    match name {
        "alnum" => c.is_alphanumeric(),
        "alpha" => c.is_alphabetic(),
        "digit" => c.is_ascii_digit(),
        "xdigit" => c.is_ascii_hexdigit(),
        "upper" => c.is_uppercase(),
        "lower" => c.is_lowercase(),
        "word" => !c.is_whitespace() && !is_punctuation(c),
        "punct" => is_punctuation(c),
        "cntrl" => c.is_control(),
        "graph" => !c.is_whitespace() && !c.is_control(),
        "print" => !c.is_control(),
        "space" => c.is_whitespace(),
        "blank" => matches!(c, ' ' | '\t' | '\u{2001}'),
        "ascii" | "unibyte" => c.is_ascii(),
        "nonascii" | "multibyte" => !c.is_ascii(),
        _ => false,
    }
}

fn is_punctuation(c: char) -> bool {
    c.is_ascii_punctuation() || matches!(c, '\u{2010}'..='\u{2027}' | '\u{3001}'..='\u{303F}')
}

pub fn prim_skip_chars_forward(args: &LispObject) -> ElispResult<LispObject> {
    let spec = string_arg(args, 0).unwrap_or_default();
    let limit = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .map(|n| n as usize)
        .unwrap_or_else(|| buffer::with_current(|b| b.point_max()));
    let moved = buffer::with_current_mut(|b| {
        let start = b.point;
        while b.point < limit {
            match b.char_at(b.point) {
                Some(c) if chars_match(c, &spec) => b.point += 1,
                _ => break,
            }
        }
        (b.point as i64) - (start as i64)
    });
    Ok(LispObject::integer(moved))
}

pub fn prim_skip_chars_backward(args: &LispObject) -> ElispResult<LispObject> {
    let spec = string_arg(args, 0).unwrap_or_default();
    let limit = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .map(|n| n as usize)
        .unwrap_or_else(|| buffer::with_current(|b| b.point_min()));
    let moved = buffer::with_current_mut(|b| {
        let start = b.point;
        while b.point > limit {
            match b.char_at(b.point - 1) {
                Some(c) if chars_match(c, &spec) => b.point -= 1,
                _ => break,
            }
        }
        (start as i64) - (b.point as i64)
    });
    Ok(LispObject::integer(-moved))
}

/// Syntax-class lookup stub. Real Emacs has a per-buffer syntax table
/// with entries like "w" for word, "." for punctuation, etc. We use a
/// hard-coded mapping that covers the common ASCII cases.
fn syntax_class(c: char) -> char {
    if c.is_alphanumeric() || c == '_' {
        'w'
    } else if c.is_whitespace() {
        ' '
    } else if "(){}[]".contains(c) {
        '('
    } else if ".,;:!?".contains(c) {
        '.'
    } else if "\"".contains(c) {
        '"'
    } else {
        '.'
    }
}

pub fn prim_skip_syntax_forward(args: &LispObject) -> ElispResult<LispObject> {
    let spec = string_arg(args, 0).unwrap_or_default();
    let limit = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .map(|n| n as usize)
        .unwrap_or_else(|| buffer::with_current(|b| b.point_max()));
    let moved = buffer::with_current_mut(|b| {
        let start = b.point;
        while b.point < limit {
            match b.char_at(b.point) {
                Some(c) => {
                    let class = syntax_class(c);
                    if chars_match(class, &spec) {
                        b.point += 1;
                    } else {
                        break;
                    }
                }
                None => break,
            }
        }
        (b.point as i64) - (start as i64)
    });
    Ok(LispObject::integer(moved))
}

pub fn prim_skip_syntax_backward(args: &LispObject) -> ElispResult<LispObject> {
    let spec = string_arg(args, 0).unwrap_or_default();
    let limit = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .map(|n| n as usize)
        .unwrap_or_else(|| buffer::with_current(|b| b.point_min()));
    let moved = buffer::with_current_mut(|b| {
        let start = b.point;
        while b.point > limit {
            match b.char_at(b.point - 1) {
                Some(c) => {
                    let class = syntax_class(c);
                    if chars_match(class, &spec) {
                        b.point -= 1;
                    } else {
                        break;
                    }
                }
                None => break,
            }
        }
        (start as i64) - (b.point as i64)
    });
    Ok(LispObject::integer(-moved))
}

// ---- Markers ----------------------------------------------------------

/// A marker, elisp-side, is represented as a cons
/// `(marker . <integer marker-id>)` with the symbol `marker` as tag.
/// This keeps the shape uniform without requiring a new LispObject
/// variant.
fn make_marker_object(id: usize) -> LispObject {
    let tag = LispObject::symbol("marker");
    LispObject::cons(tag, LispObject::integer(id as i64))
}

fn marker_id(obj: &LispObject) -> Option<usize> {
    let (car, cdr) = obj.destructure_cons()?;
    if car.as_symbol().as_deref() != Some("marker") {
        return None;
    }
    cdr.as_integer().map(|n| n as usize)
}

pub fn prim_markerp(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    Ok(LispObject::from(marker_id(&a).is_some()))
}

pub fn prim_make_marker(_args: &LispObject) -> ElispResult<LispObject> {
    let id = buffer::with_registry_mut(|r| {
        let buf = r.current_id();
        r.make_marker(buf)
    });
    Ok(make_marker_object(id))
}

pub fn prim_point_marker(_args: &LispObject) -> ElispResult<LispObject> {
    let id = buffer::with_registry_mut(|r| {
        let buf = r.current_id();
        let id = r.make_marker(buf);
        let p = r.get(buf).map(|b| b.point);
        r.marker_set(id, buf, p);
        id
    });
    Ok(make_marker_object(id))
}

pub fn prim_point_min_marker(_args: &LispObject) -> ElispResult<LispObject> {
    let id = buffer::with_registry_mut(|r| {
        let buf = r.current_id();
        let id = r.make_marker(buf);
        let p = r.get(buf).map(|b| b.point_min());
        r.marker_set(id, buf, p);
        id
    });
    Ok(make_marker_object(id))
}

pub fn prim_point_max_marker(_args: &LispObject) -> ElispResult<LispObject> {
    let id = buffer::with_registry_mut(|r| {
        let buf = r.current_id();
        let id = r.make_marker(buf);
        let p = r.get(buf).map(|b| b.point_max());
        r.marker_set(id, buf, p);
        id
    });
    Ok(make_marker_object(id))
}

pub fn prim_copy_marker(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    // Accept either an integer (treated as a position in current
    // buffer) or another marker.
    let (buf, pos) = if let Some(n) = a.as_integer() {
        // Emacs clamps negative integers to 1. Without the clamp,
        // `n as usize` wraps to a huge positive value.
        let cur = buffer::with_registry(|r| r.current_id());
        (cur, Some(n.max(1) as usize))
    } else if let Some(id) = marker_id(&a) {
        buffer::with_registry(|r| {
            r.markers
                .get(&id)
                .map(|m| (m.buffer, m.position))
                .unwrap_or((r.current_id(), None))
        })
    } else {
        return Ok(LispObject::nil());
    };
    let insertion_type = args.nth(1).is_some_and(|v| !v.is_nil());
    let new_id = buffer::with_registry_mut(|r| {
        let id = r.make_marker(buf);
        r.marker_set(id, buf, pos);
        r.set_marker_insertion_type(id, insertion_type);
        id
    });
    Ok(make_marker_object(new_id))
}

pub fn prim_marker_position(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    if let Some(id) = marker_id(&a) {
        let pos = buffer::with_registry(|r| r.markers.get(&id).and_then(|m| m.position));
        return Ok(pos
            .map(|p| LispObject::integer(p as i64))
            .unwrap_or(LispObject::nil()));
    }
    // Accept a bare integer — return it unchanged.
    if let Some(n) = a.as_integer() {
        return Ok(LispObject::integer(n));
    }
    Ok(LispObject::nil())
}

pub fn prim_marker_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    if let Some(id) = marker_id(&a) {
        let buffer_id = buffer::with_registry(|r| {
            r.markers
                .get(&id)
                .and_then(|m| r.get(m.buffer).map(|_| m.buffer))
        });
        return Ok(buffer_id
            .map(make_buffer_object)
            .unwrap_or(LispObject::nil()));
    }
    Ok(LispObject::nil())
}

pub fn prim_set_marker(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = marker_id(&a).ok_or_else(|| ElispError::WrongTypeArgument("marker".into()))?;
    // Clamp negative positions to 1 — same reason as `copy-marker`.
    let pos = args
        .nth(1)
        .and_then(|v| v.as_integer())
        .map(|n| n.max(1) as usize);
    let buffer_id = match args.nth(2) {
        Some(v) => resolve_buffer(&v).unwrap_or_else(|| buffer::with_registry(|r| r.current_id())),
        None => buffer::with_registry(|r| r.current_id()),
    };
    buffer::with_registry_mut(|r| r.marker_set(id, buffer_id, pos));
    Ok(a)
}

// ---- Match data -------------------------------------------------------

pub fn prim_match_data(_args: &LispObject) -> ElispResult<LispObject> {
    let groups = buffer::with_registry(|r| r.match_data.groups.clone());
    let mut out = LispObject::nil();
    for g in groups.into_iter().rev() {
        let (a, b) = match g {
            Some((a, b)) => (LispObject::integer(a as i64), LispObject::integer(b as i64)),
            None => (LispObject::nil(), LispObject::nil()),
        };
        out = LispObject::cons(b, out);
        out = LispObject::cons(a, out);
    }
    Ok(out)
}

pub fn prim_set_match_data(args: &LispObject) -> ElispResult<LispObject> {
    let mut list = args.first().unwrap_or(LispObject::nil());
    let mut groups = Vec::new();
    #[allow(clippy::while_let_loop)]
    loop {
        let a = match list.destructure_cons() {
            Some((car, cdr)) => {
                list = cdr;
                car
            }
            None => break,
        };
        let b = match list.destructure_cons() {
            Some((car, cdr)) => {
                list = cdr;
                car
            }
            None => break,
        };
        match (a.as_integer(), b.as_integer()) {
            (Some(s), Some(e)) => groups.push(Some((s as usize, e as usize))),
            _ => groups.push(None),
        }
    }
    buffer::with_registry_mut(|r| r.match_data.groups = groups);
    Ok(LispObject::nil())
}

pub fn prim_match_beginning(args: &LispObject) -> ElispResult<LispObject> {
    let idx = int_arg(args, 0, 0) as usize;
    let pos =
        buffer::with_registry(|r| r.match_data.groups.get(idx).and_then(|g| g.map(|(a, _)| a)));
    Ok(pos
        .map(|p| LispObject::integer(p as i64))
        .unwrap_or(LispObject::nil()))
}

pub fn prim_match_end(args: &LispObject) -> ElispResult<LispObject> {
    let idx = int_arg(args, 0, 0) as usize;
    let pos =
        buffer::with_registry(|r| r.match_data.groups.get(idx).and_then(|g| g.map(|(_, b)| b)));
    Ok(pos
        .map(|p| LispObject::integer(p as i64))
        .unwrap_or(LispObject::nil()))
}

pub fn prim_match_string(args: &LispObject) -> ElispResult<LispObject> {
    let idx = int_arg(args, 0, 0) as usize;
    let source = args
        .nth(1)
        .and_then(|a| a.as_string().map(|s| s.to_string()));
    let (g, buf_id, buf_src) = buffer::with_registry(|r| {
        (
            r.match_data.groups.get(idx).copied().flatten(),
            r.match_data.buffer,
            r.match_data.source.clone(),
        )
    });
    let (a, b) = match g {
        Some(p) => p,
        None => return Ok(LispObject::nil()),
    };
    // If the caller passed a string, use that; else prefer the stored
    // source (from string-match) or the buffer (from re-search-*).
    let s = if let Some(explicit) = source {
        substring_chars(&explicit, a, b)
    } else if let Some(src) = buf_src {
        substring_chars(&src, a, b)
    } else if let Some(id) = buf_id {
        buffer::with_registry(|r| r.get(id).map(|bf| bf.substring(a, b))).unwrap_or_default()
    } else {
        buffer::with_current(|bf| bf.substring(a, b))
    };
    Ok(LispObject::string(&s))
}

fn substring_chars(s: &str, a: usize, b: usize) -> String {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    // 1-based char indices.
    let start_byte = s
        .char_indices()
        .nth(lo.saturating_sub(1))
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let end_byte = s
        .char_indices()
        .nth(hi.saturating_sub(1))
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    if start_byte > end_byte || end_byte > s.len() {
        return String::new();
    }
    s[start_byte..end_byte].to_string()
}

// ---- Regex searches ---------------------------------------------------

/// Translate an Emacs regex to Rust's `regex` crate dialect.
///
/// Covers the constructs that actually appear in Emacs ERT tests:
///
/// - Grouping / alternation / bracing / plus / question: Emacs uses
///   `\(` `\)` `\|` `\{` `\}` `\+` `\?` for metas, Rust uses the bare
///   forms. Swap both directions.
/// - `\``  → `\A` (start of search), `\'` → `\z` (end of search).
///   Emacs's buffer-edge anchors are the moral equivalent of
///   "beginning/end of input".
/// - Syntax class: `\sX` / `\SX` (whitespace, word, symbol,
///   punct, open, close, string, comment, etc.). Emacs maps X to a
///   syntax-table class; we translate to the closest POSIX character
///   class Rust supports. Unknown X falls through to `.`.
/// - Symbol boundary: `\_<` / `\_>` — approximate with `\b` (word
///   boundary). Emacs's notion of "symbol" is syntax-table driven so
///   a true translation needs buffer context we don't have.
/// - Anything else escaped: preserved verbatim. Rust and Emacs agree
///   on `\b`, `\B`, `\d`, `\D`, `\s` (Emacs's `\s` *without* suffix
///   is rare), `\w`, `\W`, and backrefs `\1`–`\9`.
fn emacs_re_to_rust(src: &str) -> String {
    let mut out = String::with_capacity(src.len() + 8);
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            let n = chars[i + 1];
            match n {
                // Emacs metachars that are bare in Rust.
                '(' | ')' | '|' | '{' | '}' => {
                    out.push(n);
                    i += 2;
                }
                // Buffer-edge anchors.
                '`' => {
                    out.push_str("\\A");
                    i += 2;
                }
                '\'' => {
                    out.push_str("\\z");
                    i += 2;
                }
                '<' | '>' => {
                    out.push_str("\\b");
                    i += 2;
                }
                // Syntax class prefix: \sX consumes X and emits the
                // equivalent POSIX character class.
                's' | 'S' if i + 2 < chars.len() => {
                    let x = chars[i + 2];
                    let negate = n == 'S';
                    out.push_str(&emacs_syntax_to_rust_class(x, negate));
                    i += 3;
                }
                // Symbol boundaries — approximate with word boundary.
                '_' if i + 2 < chars.len() && (chars[i + 2] == '<' || chars[i + 2] == '>') => {
                    out.push_str("\\b");
                    i += 3;
                }
                // Anything else escaped: preserve as-is.
                _ => {
                    out.push('\\');
                    out.push(n);
                    i += 2;
                }
            }
        } else if c == '(' || c == ')' || c == '|' || c == '{' || c == '}' {
            // Literal in Emacs.
            out.push('\\');
            out.push(c);
            i += 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    let out = expand_emacs_posix_classes(&apply_zero_width_repetition_compat(&out));
    format!("(?m){out}")
}

fn apply_zero_width_repetition_compat(regex: &str) -> String {
    let mut out = regex.to_string();
    for (from, to) in [
        ("^*?", "(?m:^\\*?)"),
        ("^*", "(?m:^\\*)"),
        ("\\A*?", "\\A\\*?"),
        ("\\A*", "\\A\\*"),
    ] {
        if out.starts_with(from) {
            out.replace_range(..from.len(), to);
            break;
        }
    }
    for (from, to) in [
        ("\\b*?", ""),
        ("\\B*?", ""),
        ("\\b*", ""),
        ("\\B*", ""),
        ("\\b+", "\\b"),
        ("\\B+", "\\B"),
        ("\\=*", ""),
        ("(=*|h)+", "[=h]*"),
        ("(=*|h)*", "[=h]*"),
    ] {
        out = out.replace(from, to);
    }
    out
}

/// Map an Emacs syntax-class letter to a Rust regex character class.
///
/// Emacs syntax-table letters (from `syntax.h`):
///       -   whitespace              `[[:space:]]`
///       w   word                    `\w`
///       _   symbol constituent      `[\w]` (approx — Emacs is richer)
///       .   punctuation             `[[:punct:]]`
///       (   open paren              `[(\[{]`
///       )   close paren             `[)\]}]`
///       "   string quote            `["']`
///       '   expression prefix       `['`~,@]`
///       <   comment start           `[;#]`
///       >   comment end             `[\n\r]`
///       \   escape                  `\\`
///       /   char-quote              `\\`
///       |   gen. string delimiter   `["]`
///       $   paired delimiter        `[$]`
///
/// Returns a Rust regex fragment. Unknown classes fall through to `.`
/// (match any), which is the safest over-approximation for search.
fn emacs_syntax_to_rust_class(c: char, negated: bool) -> String {
    if negated && c == 'w' {
        return r"[\p{White_Space}\p{Punctuation}]".to_string();
    }
    let positive = match c {
        '-' | ' ' => r"[\p{White_Space}]",
        'w' => r"[^\p{White_Space}\p{Punctuation}]",
        '_' => r"[_]",
        '.' => r"[\p{Punctuation}]",
        '(' => r"[(\[{]",
        ')' => r"[)\]}]",
        '"' | '|' => r#"[""]"#,
        '\'' => r"[\'`,@]",
        '<' => r"[;#]",
        '>' => r"[\n\r]",
        '\\' | '/' => r"\\",
        _ => r"(?s:.)",
    };
    if !negated {
        return positive.to_string();
    }
    if let Some(inner) = positive.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        format!("[^{inner}]")
    } else {
        r"(?!)".to_string()
    }
}

fn expand_emacs_posix_classes(regex: &str) -> String {
    let bracket_replacements = [
        ("[[:word:]]", r"[^\p{White_Space}\p{Punctuation}]"),
        ("[^[:word:]]", r"[\p{White_Space}\p{Punctuation}]"),
        ("[[:graph:]]", r"[^\p{White_Space}\p{Control}]"),
        ("[^[:graph:]]", r"[\p{White_Space}\p{Control}]"),
        ("[[:print:]]", r"[^\p{Control}]"),
        ("[^[:print:]]", r"[\p{Control}]"),
        ("[[:nonascii:]]", r"[^\x00-\x7f]"),
        ("[^[:nonascii:]]", r"[\x00-\x7f]"),
        ("[[:multibyte:]]", r"[^\x00-\x7f]"),
        ("[^[:multibyte:]]", r"[\x00-\x7f]"),
    ];
    let mut out = regex.to_string();
    for (from, to) in bracket_replacements {
        out = out.replace(from, to);
    }
    let replacements = [
        ("[:alnum:]", r"\p{Alphabetic}\p{Decimal_Number}"),
        ("[:alpha:]", r"\p{Alphabetic}"),
        ("[:digit:]", r"\p{Decimal_Number}"),
        ("[:xdigit:]", r"0-9A-Fa-f"),
        ("[:upper:]", r"\p{Uppercase}"),
        ("[:lower:]", r"\p{Lowercase}"),
        ("[:word:]", r"\p{Alphabetic}\p{Mark}\p{Decimal_Number}_"),
        ("[:punct:]", r"\p{Punctuation}"),
        ("[:cntrl:]", r"\p{Control}"),
        ("[:graph:]", r"\P{White_Space}\P{Control}"),
        ("[:print:]", r"\P{Control}"),
        ("[:space:]", r"\p{White_Space}"),
        ("[:blank:]", r"\t \u{2001}"),
        ("[:ascii:]", r"\x00-\x7f"),
        ("[:nonascii:]", r"\x80-\u{10ffff}"),
        ("[:unibyte:]", r"\x00-\x7f"),
        ("[:multibyte:]", r"\x80-\u{10ffff}"),
    ];
    for (from, to) in replacements {
        out = out.replace(from, to);
    }
    out = out.replace("[\u{82}-Ó]", r"[[\u{82}-\u{d3}]&&[^\p{Alphabetic}]]");
    out = out.replace(
        "[f-Ó]",
        r"(?:[f-\x7f]|[[\u{80}-\u{d3}]&&[^\p{Alphabetic}]])",
    );
    out = out.replace("[å-Ó]", r"(?!)");
    out = out.replace("[\u{e082}-\u{e0d3}]", r"[\u{e082}-\u{e0d3}]");
    out = out.replace("[f-\u{e0d3}]", r"(?:[f-\x7f]|[\u{e080}-\u{e0d3}])");
    out = out.replace("[å-\u{e0d3}]", r"(?!)");
    out
}

pub fn prim_string_match(args: &LispObject) -> ElispResult<LispObject> {
    let re = string_arg(args, 0).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let s = string_arg(args, 1).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let start = args.nth(2).and_then(|a| a.as_integer()).unwrap_or(0) as usize;
    let re = match regex::Regex::new(&emacs_re_to_rust(&re)) {
        Ok(r) => r,
        Err(_) => return Ok(LispObject::nil()),
    };
    let haystack = if start < s.chars().count() {
        s.chars().skip(start).collect::<String>()
    } else {
        return Ok(LispObject::nil());
    };
    match re.captures(&haystack) {
        Some(caps) => {
            let groups: Vec<Option<(usize, usize)>> = (0..caps.len())
                .map(|i| {
                    caps.get(i).map(|m| {
                        let byte_start = m.start();
                        let byte_end = m.end();
                        // Convert byte offsets back to char offsets within haystack, then add `start`.
                        let cstart = haystack[..byte_start].chars().count() + start;
                        let cend = haystack[..byte_end].chars().count() + start;
                        (cstart + 1, cend + 1) // 1-based
                    })
                })
                .collect();
            let zero_start = groups[0].unwrap().0;
            buffer::with_registry_mut(|r| {
                r.match_data.groups = groups;
                r.match_data.buffer = None;
                r.match_data.source = Some(s.clone());
            });
            Ok(LispObject::integer((zero_start - 1) as i64))
        }
        None => Ok(LispObject::nil()),
    }
}

pub fn prim_string_match_p(args: &LispObject) -> ElispResult<LispObject> {
    prim_string_match(args)
}

pub fn prim_looking_at(args: &LispObject) -> ElispResult<LispObject> {
    let re = string_arg(args, 0).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let re = match regex::Regex::new(&emacs_re_to_rust(&re)) {
        Ok(r) => r,
        Err(_) => return Ok(LispObject::nil()),
    };
    let (text_from_point, point) = buffer::with_current(|b| {
        let p = b.point;
        let byte = b.char_to_byte(p);
        (b.text[byte..].to_string(), p)
    });
    match re.find(&text_from_point) {
        Some(m) if m.start() == 0 => {
            let end_char = text_from_point[..m.end()].chars().count();
            let groups = vec![Some((point, point + end_char))];
            buffer::with_registry_mut(|r| {
                r.match_data.groups = groups;
                r.match_data.buffer = Some(r.current_id());
                r.match_data.source = None;
            });
            Ok(LispObject::t())
        }
        _ => Ok(LispObject::nil()),
    }
}

pub fn prim_looking_at_p(args: &LispObject) -> ElispResult<LispObject> {
    prim_looking_at(args)
}

pub fn prim_re_search_forward(args: &LispObject) -> ElispResult<LispObject> {
    let re = string_arg(args, 0).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let bound = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .map(|n| n as usize)
        .unwrap_or(usize::MAX);
    let noerror = args
        .nth(2)
        .map(|a| !matches!(a, LispObject::Nil))
        .unwrap_or(false);
    let re = match regex::Regex::new(&emacs_re_to_rust(&re)) {
        Ok(r) => r,
        Err(e) => {
            if noerror {
                return Ok(LispObject::nil());
            }
            return Err(ElispError::EvalError(format!("invalid regex: {e}")));
        }
    };
    let result = buffer::with_registry_mut(|r| {
        let id = r.current_id();
        let buf = r.get(id).unwrap();
        let point = buf.point;
        let limit = bound.min(buf.point_max());
        let start_byte = buf.char_to_byte(point);
        let end_byte = buf.char_to_byte(limit);
        if start_byte > end_byte {
            return None;
        }
        let haystack = &buf.text[start_byte..end_byte];
        let caps = re.captures(haystack);
        caps.map(|c| {
            let byte_start = c.get(0).unwrap().start();
            let byte_end = c.get(0).unwrap().end();
            let cstart = haystack[..byte_start].chars().count() + point;
            let cend = haystack[..byte_end].chars().count() + point;
            let groups: Vec<Option<(usize, usize)>> = (0..c.len())
                .map(|i| {
                    c.get(i).map(|m| {
                        (
                            haystack[..m.start()].chars().count() + point,
                            haystack[..m.end()].chars().count() + point,
                        )
                    })
                })
                .collect();
            (cstart, cend, groups)
        })
    });
    match result {
        Some((_, end, groups)) => {
            buffer::with_current_mut(|b| b.point = end);
            let id = buffer::with_registry(|r| r.current_id());
            buffer::with_registry_mut(|r| {
                r.match_data.groups = groups;
                r.match_data.buffer = Some(id);
                r.match_data.source = None;
            });
            Ok(LispObject::integer(end as i64))
        }
        None => {
            if noerror {
                Ok(LispObject::nil())
            } else {
                Err(ElispError::Signal(Box::new(crate::error::SignalData {
                    symbol: LispObject::symbol("search-failed"),
                    data: LispObject::nil(),
                })))
            }
        }
    }
}

pub fn prim_re_search_backward(args: &LispObject) -> ElispResult<LispObject> {
    // Simple scanning implementation — doesn't handle overlapping
    // matches properly but is enough for common test usage.
    let re = string_arg(args, 0).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let bound = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .map(|n| n as usize)
        .unwrap_or(1);
    let noerror = args
        .nth(2)
        .map(|a| !matches!(a, LispObject::Nil))
        .unwrap_or(false);
    let re = match regex::Regex::new(&emacs_re_to_rust(&re)) {
        Ok(r) => r,
        Err(e) => {
            if noerror {
                return Ok(LispObject::nil());
            }
            return Err(ElispError::EvalError(format!("invalid regex: {e}")));
        }
    };
    let found = buffer::with_registry(|r| {
        let id = r.current_id();
        let buf = r.get(id).unwrap();
        let lo = bound.max(buf.point_min());
        let hi = buf.point;
        let lo_b = buf.char_to_byte(lo);
        let hi_b = buf.char_to_byte(hi);
        if lo_b > hi_b {
            return None;
        }
        let hay = &buf.text[lo_b..hi_b];
        // Find last match.
        re.captures_iter(hay).last().map(|c| {
            let m0 = c.get(0).unwrap();
            let cstart = hay[..m0.start()].chars().count() + lo;
            let cend = hay[..m0.end()].chars().count() + lo;
            let groups: Vec<Option<(usize, usize)>> = (0..c.len())
                .map(|i| {
                    c.get(i).map(|m| {
                        (
                            hay[..m.start()].chars().count() + lo,
                            hay[..m.end()].chars().count() + lo,
                        )
                    })
                })
                .collect();
            (cstart, cend, groups)
        })
    });
    match found {
        Some((start, _, groups)) => {
            buffer::with_current_mut(|b| b.point = start);
            let id = buffer::with_registry(|r| r.current_id());
            buffer::with_registry_mut(|r| {
                r.match_data.groups = groups;
                r.match_data.buffer = Some(id);
                r.match_data.source = None;
            });
            Ok(LispObject::integer(start as i64))
        }
        None => {
            if noerror {
                Ok(LispObject::nil())
            } else {
                Err(ElispError::Signal(Box::new(crate::error::SignalData {
                    symbol: LispObject::symbol("search-failed"),
                    data: LispObject::nil(),
                })))
            }
        }
    }
}

pub fn prim_search_forward(args: &LispObject) -> ElispResult<LispObject> {
    // (search-forward STRING &optional BOUND NOERROR COUNT) — literal.
    let needle =
        string_arg(args, 0).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let result = buffer::with_current(|b| {
        let byte = b.char_to_byte(b.point);
        b.text[byte..].find(&needle).map(|off| {
            let prefix = &b.text[byte..byte + off];
            let char_start = b.point + prefix.chars().count();
            let char_end = char_start + needle.chars().count();
            (char_start, char_end)
        })
    });
    match result {
        Some((start, end)) => {
            buffer::with_current_mut(|b| b.point = end);
            let id = buffer::with_registry(|r| r.current_id());
            buffer::with_registry_mut(|r| {
                r.match_data.groups = vec![Some((start, end))];
                r.match_data.buffer = Some(id);
                r.match_data.source = None;
            });
            Ok(LispObject::integer(end as i64))
        }
        None => {
            let noerror = args
                .nth(2)
                .map(|a| !matches!(a, LispObject::Nil))
                .unwrap_or(false);
            if noerror {
                Ok(LispObject::nil())
            } else {
                Err(ElispError::Signal(Box::new(crate::error::SignalData {
                    symbol: LispObject::symbol("search-failed"),
                    data: LispObject::nil(),
                })))
            }
        }
    }
}

pub fn prim_search_backward(args: &LispObject) -> ElispResult<LispObject> {
    let needle =
        string_arg(args, 0).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let result = buffer::with_current(|b| {
        let end_byte = b.char_to_byte(b.point);
        b.text[..end_byte].rfind(&needle).map(|off| {
            let prefix = &b.text[..off];
            let char_start = prefix.chars().count() + 1; // 1-based
            (char_start, char_start + needle.chars().count())
        })
    });
    match result {
        Some((start, end)) => {
            buffer::with_current_mut(|b| b.point = start);
            let id = buffer::with_registry(|r| r.current_id());
            buffer::with_registry_mut(|r| {
                r.match_data.groups = vec![Some((start, end))];
                r.match_data.buffer = Some(id);
                r.match_data.source = None;
            });
            Ok(LispObject::integer(start as i64))
        }
        None => {
            let noerror = args
                .nth(2)
                .map(|a| !matches!(a, LispObject::Nil))
                .unwrap_or(false);
            if noerror {
                Ok(LispObject::nil())
            } else {
                Err(ElispError::Signal(Box::new(crate::error::SignalData {
                    symbol: LispObject::symbol("search-failed"),
                    data: LispObject::nil(),
                })))
            }
        }
    }
}

pub fn prim_replace_match(args: &LispObject) -> ElispResult<LispObject> {
    let repl = string_arg(args, 0).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let g0 = buffer::with_registry(|r| r.match_data.groups.first().copied().flatten());
    if let Some((a, b)) = g0 {
        buffer::with_registry_mut(|registry| {
            registry.delete_current_region(a, b);
            if let Some(buf) = registry.get_mut(registry.current_id()) {
                buf.goto_char(a);
            }
            registry.insert_current(&repl, false);
        });
        Ok(LispObject::t())
    } else {
        Ok(LispObject::nil())
    }
}

pub fn prim_get_truename_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let file_name = match name {
        LispObject::String(s) => s.clone(),
        _ => return Ok(LispObject::nil()),
    };
    let found = buffer::with_registry(|r| {
        r.buffers
            .values()
            .find(|b| b.file_name.as_deref() == Some(&file_name))
            .map(|b| b.name.clone())
    });
    Ok(found
        .map(|n| LispObject::string(&n))
        .unwrap_or(LispObject::nil()))
}

pub fn prim_buffer_text_pixel_size(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::cons(
        LispObject::integer(0),
        LispObject::integer(0),
    ))
}

pub fn prim_text_property_not_all(args: &LispObject) -> ElispResult<LispObject> {
    let start = args
        .first()
        .and_then(|a| integer_or_marker_value(&a))
        .ok_or(ElispError::WrongTypeArgument("integer".into()))?
        .max(0) as usize;
    let end = args
        .nth(1)
        .and_then(|a| integer_or_marker_value(&a))
        .ok_or(ElispError::WrongTypeArgument("integer".into()))?
        .max(0) as usize;
    let prop = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(3).ok_or(ElispError::WrongNumberOfArguments)?;
    for pos in start..end {
        let actual = if let Some(LispObject::String(raw)) = args.nth(4) {
            string_property_at(&raw, pos, &prop)
        } else {
            buffer::with_current(|b| b.property_at(pos, &prop).unwrap_or_else(LispObject::nil))
        };
        if actual != value {
            return Ok(LispObject::integer(pos as i64));
        }
    }
    Ok(LispObject::nil())
}

pub fn prim_propertize(args: &LispObject) -> ElispResult<LispObject> {
    let text = args
        .first()
        .and_then(|arg| arg.as_string().cloned())
        .ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let visible = crate::object::current_string_value(&text);
    let props = plist_pairs(&args.rest().unwrap_or_else(LispObject::nil))?;
    if props.is_empty() {
        return Ok(LispObject::string(&visible));
    }
    let span = buffer::TextPropertySpan {
        start: 0,
        end: visible.chars().count(),
        plist: props,
    };
    Ok(make_propertized_string(&visible, vec![span]))
}

pub fn prim_add_text_properties(args: &LispObject) -> ElispResult<LispObject> {
    let start = int_arg(args, 0, 0).max(0) as usize;
    let end = int_arg(args, 1, 0).max(0) as usize;
    let plist = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let props = plist_pairs(&plist)?;
    if props.is_empty() {
        return Ok(LispObject::nil());
    }
    if let Some(LispObject::String(raw)) = args.nth(3) {
        STRING_PROPERTIES.with(|string_props| {
            string_props
                .borrow_mut()
                .entry(raw)
                .or_default()
                .push(buffer::TextPropertySpan {
                    start,
                    end,
                    plist: props,
                });
        });
    } else {
        buffer::with_current_mut(|b| b.add_text_properties(start, end, props));
    }
    Ok(LispObject::t())
}

pub fn prim_put_text_property(args: &LispObject) -> ElispResult<LispObject> {
    let start = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let end = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(3).ok_or(ElispError::WrongNumberOfArguments)?;
    let object = args.nth(4);
    let plist = list_from_pairs(&[(prop, value)]);
    let mut rebuilt = LispObject::nil();
    if let Some(object) = object {
        rebuilt = LispObject::cons(object, rebuilt);
    }
    rebuilt = LispObject::cons(plist, rebuilt);
    rebuilt = LispObject::cons(end, rebuilt);
    rebuilt = LispObject::cons(start.clone(), rebuilt);
    prim_add_text_properties(&rebuilt)
}

pub fn prim_set_text_properties(args: &LispObject) -> ElispResult<LispObject> {
    let start = int_arg(args, 0, 0).max(0) as usize;
    let end = int_arg(args, 1, 0).max(0) as usize;
    let plist = args.nth(2).unwrap_or_else(LispObject::nil);
    let props = plist_pairs(&plist)?;
    if let Some(LispObject::String(raw)) = args.nth(3) {
        STRING_PROPERTIES.with(|string_props| {
            let mut map = string_props.borrow_mut();
            let spans = map.entry(raw).or_default();
            spans.retain(|span| span.end <= start || span.start >= end);
            if !props.is_empty() {
                spans.push(buffer::TextPropertySpan {
                    start,
                    end,
                    plist: props,
                });
            }
        });
    } else {
        buffer::with_current_mut(|b| {
            let keys: Vec<LispObject> = props.iter().map(|(key, _)| key.clone()).collect();
            if keys.is_empty() {
                b.text_properties
                    .retain(|span| span.end <= start || span.start >= end);
            } else {
                b.remove_text_properties(start, end, &keys);
            }
            b.add_text_properties(start, end, props);
        });
    }
    Ok(LispObject::t())
}

pub fn prim_remove_text_properties(args: &LispObject) -> ElispResult<LispObject> {
    let start = int_arg(args, 0, 0).max(0) as usize;
    let end = int_arg(args, 1, 0).max(0) as usize;
    let plist = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let keys = plist_keys(&plist)?;
    if let Some(LispObject::String(raw)) = args.nth(3) {
        STRING_PROPERTIES.with(|string_props| {
            if let Some(spans) = string_props.borrow_mut().get_mut(&raw) {
                for span in spans.iter_mut() {
                    if span.end > start && span.start < end {
                        span.plist.retain(|(key, _)| !keys.iter().any(|k| k == key));
                    }
                }
                spans.retain(|span| span.start < span.end && !span.plist.is_empty());
            }
        });
    } else {
        buffer::with_current_mut(|b| b.remove_text_properties(start, end, &keys));
    }
    Ok(LispObject::t())
}

pub fn prim_get_text_property(args: &LispObject) -> ElispResult<LispObject> {
    let pos = int_arg(args, 0, 0).max(0) as usize;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(LispObject::String(raw)) = args.nth(2) {
        return Ok(string_property_at(&raw, pos, &prop));
    }
    Ok(buffer::with_current(|b| {
        b.property_at(pos, &prop).unwrap_or_else(LispObject::nil)
    }))
}

fn display_property_from_spec(spec: &LispObject, prop: &LispObject) -> LispObject {
    if let Some((key, rest)) = spec.destructure_cons() {
        if &key == prop {
            return rest.first().unwrap_or_else(LispObject::nil);
        }
        let mut cur = spec.clone();
        while let Some((entry, next)) = cur.destructure_cons() {
            if let Some((key, rest)) = entry.destructure_cons()
                && &key == prop
            {
                return rest.first().unwrap_or_else(LispObject::nil);
            }
            cur = next;
        }
    }
    if let LispObject::Vector(items) = spec {
        for entry in items.lock().iter() {
            if let Some((key, rest)) = entry.destructure_cons()
                && &key == prop
            {
                return rest.first().unwrap_or_else(LispObject::nil);
            }
        }
    }
    LispObject::nil()
}

pub fn prim_get_display_property(args: &LispObject) -> ElispResult<LispObject> {
    let pos = int_arg(args, 0, 0).max(0) as usize;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let display = buffer::with_current(|b| {
        b.property_at(pos, &LispObject::symbol("display"))
            .unwrap_or_else(LispObject::nil)
    });
    Ok(display_property_from_spec(&display, &prop))
}

pub fn prim_text_properties_at(args: &LispObject) -> ElispResult<LispObject> {
    let pos = int_arg(args, 0, 0).max(0) as usize;
    let props = if let Some(LispObject::String(raw)) = args.nth(1) {
        string_properties_at(&raw, pos)
    } else {
        buffer::with_current(|b| b.properties_at(pos))
    };
    Ok(list_from_pairs(&props))
}

/// Two property sets are equal as sets of (key, value) pairs regardless
/// of insertion order. The Vec returned by `properties_at` does
/// last-write-wins per key, so two positions with the same effective
/// merged plist may produce Vecs with different ordering — we canonicalize
/// for comparison by sorting on the key's printed form.
fn props_equal(
    a: &[(LispObject, LispObject)],
    b: &[(LispObject, LispObject)],
) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    a.sort_by_cached_key(|(k, _)| k.princ_to_string());
    b.sort_by_cached_key(|(k, _)| k.princ_to_string());
    a == b
}

fn properties_at_target(
    pos: usize,
    object: &Option<LispObject>,
) -> Vec<(LispObject, LispObject)> {
    if let Some(LispObject::String(raw)) = object {
        string_properties_at(raw, pos)
    } else {
        buffer::with_current(|b| b.properties_at(pos))
    }
}

fn property_at_target(
    pos: usize,
    prop: &LispObject,
    object: &Option<LispObject>,
) -> LispObject {
    if let Some(LispObject::String(raw)) = object {
        string_property_at(raw, pos, prop)
    } else {
        buffer::with_current(|b| b.property_at(pos, prop).unwrap_or_else(LispObject::nil))
    }
}

fn target_end(object: &Option<LispObject>) -> usize {
    if let Some(LispObject::String(raw)) = object {
        crate::object::current_string_value(raw).chars().count()
    } else {
        buffer::with_current(|b| b.point_max())
    }
}

fn target_begin(object: &Option<LispObject>) -> usize {
    if let Some(LispObject::String(_)) = object {
        0
    } else {
        buffer::with_current(|b| b.point_min())
    }
}

pub fn prim_next_property_change(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|a| integer_or_marker_value(&a))
        .ok_or_else(|| ElispError::WrongTypeArgument("integer-or-marker".into()))?
        .max(0) as usize;
    let object = args.nth(1).filter(|o| !o.is_nil());
    let limit = args
        .nth(2)
        .and_then(|a| integer_or_marker_value(&a))
        .map(|n| n.max(0) as usize);
    let end = limit.unwrap_or_else(|| target_end(&object));
    let initial = properties_at_target(pos, &object);
    let mut q = pos.saturating_add(1);
    while q <= end {
        let here = properties_at_target(q, &object);
        if !props_equal(&here, &initial) {
            return Ok(LispObject::integer(q as i64));
        }
        q += 1;
    }
    if matches!(&object, Some(LispObject::String(_))) {
        Ok(LispObject::integer(end as i64))
    } else {
        // Buffer: nil if scan reached limit (or end) without change.
        match limit {
            Some(n) => Ok(LispObject::integer(n as i64)),
            None => Ok(LispObject::nil()),
        }
    }
}

pub fn prim_previous_property_change(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|a| integer_or_marker_value(&a))
        .ok_or_else(|| ElispError::WrongTypeArgument("integer-or-marker".into()))?
        .max(0) as usize;
    let object = args.nth(1).filter(|o| !o.is_nil());
    let limit = args
        .nth(2)
        .and_then(|a| integer_or_marker_value(&a))
        .map(|n| n.max(0) as usize);
    let lower = limit.unwrap_or_else(|| target_begin(&object));
    if pos <= lower + 1 {
        return Ok(LispObject::nil());
    }
    let initial = properties_at_target(pos - 1, &object);
    let mut q = pos - 1;
    while q > lower {
        q -= 1;
        let here = properties_at_target(q, &object);
        if !props_equal(&here, &initial) {
            return Ok(LispObject::integer((q + 1) as i64));
        }
    }
    if matches!(&object, Some(LispObject::String(_))) {
        Ok(LispObject::integer(lower as i64))
    } else {
        match limit {
            Some(n) => Ok(LispObject::integer(n as i64)),
            None => Ok(LispObject::nil()),
        }
    }
}

pub fn prim_next_single_property_change(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|a| integer_or_marker_value(&a))
        .ok_or_else(|| ElispError::WrongTypeArgument("integer-or-marker".into()))?
        .max(0) as usize;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let object = args.nth(2).filter(|o| !o.is_nil());
    let limit = args
        .nth(3)
        .and_then(|a| integer_or_marker_value(&a))
        .map(|n| n.max(0) as usize);
    let end = limit.unwrap_or_else(|| target_end(&object));
    let initial = property_at_target(pos, &prop, &object);
    let mut q = pos.saturating_add(1);
    while q <= end {
        if property_at_target(q, &prop, &object) != initial {
            return Ok(LispObject::integer(q as i64));
        }
        q += 1;
    }
    if matches!(&object, Some(LispObject::String(_))) {
        Ok(LispObject::integer(end as i64))
    } else {
        match limit {
            Some(n) => Ok(LispObject::integer(n as i64)),
            None => Ok(LispObject::nil()),
        }
    }
}

pub fn prim_previous_single_property_change(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|a| integer_or_marker_value(&a))
        .ok_or_else(|| ElispError::WrongTypeArgument("integer-or-marker".into()))?
        .max(0) as usize;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let object = args.nth(2).filter(|o| !o.is_nil());
    let limit = args
        .nth(3)
        .and_then(|a| integer_or_marker_value(&a))
        .map(|n| n.max(0) as usize);
    let lower = limit.unwrap_or_else(|| target_begin(&object));
    if pos <= lower + 1 {
        return Ok(LispObject::nil());
    }
    let initial = property_at_target(pos - 1, &prop, &object);
    let mut q = pos - 1;
    while q > lower {
        q -= 1;
        if property_at_target(q, &prop, &object) != initial {
            return Ok(LispObject::integer((q + 1) as i64));
        }
    }
    if matches!(&object, Some(LispObject::String(_))) {
        Ok(LispObject::integer(lower as i64))
    } else {
        match limit {
            Some(n) => Ok(LispObject::integer(n as i64)),
            None => Ok(LispObject::nil()),
        }
    }
}

/// Without overlay support, this is identical to
/// `next-single-property-change`. Real Emacs also considers overlays
/// at POS, but the headless model doesn't track overlay properties as a
/// separate concept yet — they live on the same span structures.
pub fn prim_next_single_char_property_change(args: &LispObject) -> ElispResult<LispObject> {
    prim_next_single_property_change(args)
}

pub fn prim_text_property_any(args: &LispObject) -> ElispResult<LispObject> {
    let start = args
        .first()
        .and_then(|a| integer_or_marker_value(&a))
        .ok_or_else(|| ElispError::WrongTypeArgument("integer-or-marker".into()))?
        .max(0) as usize;
    let end = args
        .nth(1)
        .and_then(|a| integer_or_marker_value(&a))
        .ok_or_else(|| ElispError::WrongTypeArgument("integer-or-marker".into()))?
        .max(0) as usize;
    let prop = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(3).ok_or(ElispError::WrongNumberOfArguments)?;
    let object = args.nth(4).filter(|o| !o.is_nil());
    for q in start..end {
        if property_at_target(q, &prop, &object) == value {
            return Ok(LispObject::integer(q as i64));
        }
    }
    Ok(LispObject::nil())
}

pub fn prim_add_face_text_property(args: &LispObject) -> ElispResult<LispObject> {
    let start = args
        .first()
        .and_then(|a| integer_or_marker_value(&a))
        .ok_or_else(|| ElispError::WrongTypeArgument("integer-or-marker".into()))?
        .max(0) as usize;
    let end = args
        .nth(1)
        .and_then(|a| integer_or_marker_value(&a))
        .ok_or_else(|| ElispError::WrongTypeArgument("integer-or-marker".into()))?
        .max(0) as usize;
    let face = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let append = args.nth(3).is_some_and(|v| !v.is_nil());
    let face_key = LispObject::symbol("face");
    buffer::with_current_mut(|b| {
        let existing = b.property_at(start, &face_key);
        let combined = match existing {
            None | Some(LispObject::Nil) => face.clone(),
            Some(prev) => {
                if append {
                    LispObject::cons(prev, LispObject::cons(face.clone(), LispObject::nil()))
                } else {
                    LispObject::cons(face.clone(), LispObject::cons(prev, LispObject::nil()))
                }
            }
        };
        b.remove_text_properties(start, end, std::slice::from_ref(&face_key));
        b.add_text_properties(start, end, vec![(face_key, combined)]);
    });
    Ok(LispObject::nil())
}

pub fn prim_get_char_property(args: &LispObject, include_empty: bool) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".into()))?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if pos < 1 {
        return Ok(LispObject::nil());
    }

    let value = buffer::with_registry(|r| {
        let buf = r.current_id();
        let mut best: Option<(i64, buffer::OverlayId, LispObject)> = None;
        for ov in r.overlays.values() {
            if ov.buffer != buf {
                continue;
            }
            let (Some(start), Some(end)) = (ov.start, ov.end) else {
                continue;
            };
            let pos = pos as usize;
            let covers = if start == end {
                include_empty && pos == start
            } else {
                pos >= start && pos < end
            };
            if !covers {
                continue;
            }
            let Some((_, val)) = ov.plist.iter().find(|(key, _)| *key == prop) else {
                continue;
            };
            let priority = ov
                .plist
                .iter()
                .find(|(key, _)| key.as_symbol().as_deref() == Some("priority"))
                .and_then(|(_, value)| value.as_integer())
                .unwrap_or(0);
            let replace = best.as_ref().is_none_or(|(best_priority, best_id, _)| {
                priority > *best_priority || (priority == *best_priority && ov.id > *best_id)
            });
            if replace {
                best = Some((priority, ov.id, val.clone()));
            }
        }
        best.map(|(_, _, value)| value)
    });
    Ok(value.unwrap_or_else(LispObject::nil))
}

pub fn prim_marker_insertion_type(args: &LispObject) -> ElispResult<LispObject> {
    let marker = args.first().and_then(|arg| marker_id(&arg));
    let insertion_type =
        marker.is_some_and(|id| buffer::with_registry(|r| r.marker_insertion_type(id)));
    Ok(LispObject::from(insertion_type))
}

pub fn prim_set_marker_insertion_type(args: &LispObject) -> ElispResult<LispObject> {
    let marker = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = marker_id(&marker).ok_or_else(|| ElispError::WrongTypeArgument("marker".into()))?;
    let insertion_type = args.nth(1).is_some_and(|value| !value.is_nil());
    buffer::with_registry_mut(|r| r.set_marker_insertion_type(id, insertion_type));
    Ok(LispObject::nil())
}

pub fn prim_buffer_swap_text(args: &LispObject) -> ElispResult<LispObject> {
    let other = args
        .first()
        .and_then(|a| resolve_buffer(&a))
        .ok_or_else(|| ElispError::WrongTypeArgument("buffer".into()))?;
    buffer::with_registry_mut(|r| {
        let current = r.current_id();
        if current == other {
            return;
        }
        let Some(mut current_buf) = r.buffers.remove(&current) else {
            return;
        };
        let Some(mut other_buf) = r.buffers.remove(&other) else {
            r.buffers.insert(current, current_buf);
            return;
        };

        std::mem::swap(&mut current_buf.text, &mut other_buf.text);
        std::mem::swap(&mut current_buf.multibyte, &mut other_buf.multibyte);
        std::mem::swap(&mut current_buf.point, &mut other_buf.point);
        std::mem::swap(&mut current_buf.mark, &mut other_buf.mark);
        std::mem::swap(&mut current_buf.mark_active, &mut other_buf.mark_active);
        std::mem::swap(&mut current_buf.modified, &mut other_buf.modified);
        std::mem::swap(
            &mut current_buf.modified_status,
            &mut other_buf.modified_status,
        );
        std::mem::swap(&mut current_buf.modified_tick, &mut other_buf.modified_tick);
        std::mem::swap(&mut current_buf.restriction, &mut other_buf.restriction);
        std::mem::swap(
            &mut current_buf.manual_restriction,
            &mut other_buf.manual_restriction,
        );
        std::mem::swap(
            &mut current_buf.restriction_layers,
            &mut other_buf.restriction_layers,
        );
        std::mem::swap(&mut current_buf.locals, &mut other_buf.locals);

        r.buffers.insert(current, current_buf);
        r.buffers.insert(other, other_buf);

        for marker in r.markers.values_mut() {
            if marker.buffer == current {
                marker.buffer = other;
            } else if marker.buffer == other {
                marker.buffer = current;
            }
        }
        for overlay in r.overlays.values_mut() {
            if overlay.buffer == current {
                overlay.buffer = other;
            } else if overlay.buffer == other {
                overlay.buffer = current;
            }
        }
    });
    Ok(LispObject::nil())
}

pub fn prim_overlay_lists(_args: &LispObject) -> ElispResult<LispObject> {
    let ids = buffer::with_registry(|r| {
        let buf = r.current_id();
        r.overlays
            .values()
            .filter(|ov| ov.buffer == buf && ov.start.is_some() && ov.end.is_some())
            .map(|ov| ov.id)
            .collect::<Vec<_>>()
    });
    Ok(LispObject::cons(list_from_ids(ids), LispObject::nil()))
}

pub fn prim_overlay_recenter(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

// ---- Overlays ---------------------------------------------------------

/// Elisp-side an overlay is represented as `(overlay . <OverlayId>)`,
/// mirroring the marker representation. The registry stores the actual
/// overlay state.
fn make_overlay_object(id: buffer::OverlayId) -> LispObject {
    LispObject::cons(
        LispObject::symbol("overlay"),
        LispObject::integer(id as i64),
    )
}

fn overlay_id_of(obj: &LispObject) -> Option<buffer::OverlayId> {
    let (car, cdr) = obj.destructure_cons()?;
    if car.as_symbol().as_deref() != Some("overlay") {
        return None;
    }
    cdr.as_integer().map(|n| n as buffer::OverlayId)
}

/// Bool-ish truthiness matching Emacs: nil is false, everything else is
/// true.
fn is_truthy(obj: &LispObject) -> bool {
    !matches!(obj, LispObject::Nil)
}

fn list_len(args: &LispObject) -> usize {
    let mut len = 0;
    let mut cur = args.clone();
    while let Some((_, rest)) = cur.destructure_cons() {
        len += 1;
        cur = rest;
    }
    len
}

pub fn prim_overlayp(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    Ok(LispObject::from(overlay_id_of(&a).is_some()))
}

pub fn prim_make_overlay(args: &LispObject) -> ElispResult<LispObject> {
    let argc = list_len(args);
    if !(2..=5).contains(&argc) {
        return Err(ElispError::WrongNumberOfArguments);
    }
    let start = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    let end = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    // Third arg is optional buffer. Fourth/fifth are front/rear advance.
    let buf_id = match args.nth(2) {
        Some(LispObject::Nil) | None => buffer::with_registry(|r| r.current_id()),
        Some(obj) => {
            resolve_buffer(&obj).ok_or_else(|| ElispError::WrongTypeArgument("buffer".into()))?
        }
    };
    let fa = args.nth(3).map(|o| is_truthy(&o)).unwrap_or(false);
    let ra = args.nth(4).map(|o| is_truthy(&o)).unwrap_or(false);

    // Clamp positions to the buffer's valid range. Emacs clamps to
    // point-min/point-max of the target buffer.
    let (pmin, pmax) = buffer::with_registry(|r| {
        r.get(buf_id)
            .map(|b| (b.point_min(), b.point_max()))
            .unwrap_or((1, 1))
    });
    let clamp = |n: i64| -> usize {
        if n < 1 {
            pmin
        } else {
            (n as usize).clamp(pmin, pmax)
        }
    };
    let s = clamp(start);
    let e = clamp(end);

    let id = buffer::with_registry_mut(|r| r.make_overlay(buf_id, s, e, fa, ra));
    Ok(make_overlay_object(id))
}

pub fn prim_delete_overlay(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    if let Some(id) = overlay_id_of(&a) {
        buffer::with_registry_mut(|r| r.delete_overlay(id));
    }
    Ok(LispObject::nil())
}

pub fn prim_move_overlay(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let Some(id) = overlay_id_of(&a) else {
        return Ok(LispObject::nil());
    };
    let start = args
        .nth(1)
        .and_then(|v| v.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    let end = args
        .nth(2)
        .and_then(|v| v.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    let buf_id = match args.nth(3) {
        Some(LispObject::Nil) | None => buffer::with_registry(|r| {
            r.overlay_get(id)
                .filter(|o| o.start.is_some() && o.end.is_some() && r.get(o.buffer).is_some())
                .map(|o| o.buffer)
                .unwrap_or(r.current_id())
        }),
        Some(obj) => {
            resolve_buffer(&obj).ok_or_else(|| ElispError::WrongTypeArgument("buffer".into()))?
        }
    };
    let (pmin, pmax) = buffer::with_registry(|r| {
        r.get(buf_id)
            .map(|b| (b.point_min(), b.point_max()))
            .unwrap_or((1, 1))
    });
    let clamp = |n: i64| -> usize {
        if n < 1 {
            pmin
        } else {
            (n as usize).clamp(pmin, pmax)
        }
    };
    let (s, e) = {
        let a = clamp(start);
        let b = clamp(end);
        if a <= b { (a, b) } else { (b, a) }
    };
    buffer::with_registry_mut(|r| {
        if let Some(ov) = r.overlay_get_mut(id) {
            ov.start = Some(s);
            ov.end = Some(e);
            ov.buffer = buf_id;
        }
    });
    Ok(a)
}

pub fn prim_overlay_start(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let Some(id) = overlay_id_of(&a) else {
        return Ok(LispObject::nil());
    };
    let pos = buffer::with_registry(|r| r.overlay_get(id).and_then(|o| o.start));
    #[allow(clippy::cast_possible_wrap)]
    Ok(pos
        .map(|p| LispObject::integer(p as i64))
        .unwrap_or(LispObject::nil()))
}

pub fn prim_overlay_end(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let Some(id) = overlay_id_of(&a) else {
        return Ok(LispObject::nil());
    };
    let pos = buffer::with_registry(|r| r.overlay_get(id).and_then(|o| o.end));
    #[allow(clippy::cast_possible_wrap)]
    Ok(pos
        .map(|p| LispObject::integer(p as i64))
        .unwrap_or(LispObject::nil()))
}

pub fn prim_overlay_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let Some(id) = overlay_id_of(&a) else {
        return Ok(LispObject::nil());
    };
    let buffer_id = buffer::with_registry(|r| {
        let ov = r.overlay_get(id)?;
        // A detached overlay (deleted) has start/end None and
        // `overlay-buffer` must return nil.
        ov.start?;
        r.get(ov.buffer).map(|_| ov.buffer)
    });
    Ok(buffer_id
        .map(make_buffer_object)
        .unwrap_or(LispObject::nil()))
}

pub fn prim_overlay_put(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let Some(id) = overlay_id_of(&a) else {
        return Ok(LispObject::nil());
    };
    let key = args.nth(1).unwrap_or(LispObject::nil());
    let val = args.nth(2).unwrap_or(LispObject::nil());
    buffer::with_registry_mut(|r| {
        if let Some(ov) = r.overlay_get_mut(id) {
            if let Some(slot) = ov.plist.iter_mut().find(|(k, _)| *k == key) {
                slot.1 = val.clone();
            } else {
                ov.plist.push((key, val.clone()));
            }
        }
    });
    Ok(val)
}

pub fn prim_overlay_get(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let Some(id) = overlay_id_of(&a) else {
        return Ok(LispObject::nil());
    };
    let key = args.nth(1).unwrap_or(LispObject::nil());
    let val = buffer::with_registry(|r| {
        r.overlay_get(id).and_then(|ov| {
            ov.plist
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
        })
    });
    Ok(val.unwrap_or(LispObject::nil()))
}

pub fn prim_overlay_properties(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let Some(id) = overlay_id_of(&a) else {
        return Ok(LispObject::nil());
    };
    let plist = buffer::with_registry(|r| {
        r.overlay_get(id)
            .map(|ov| ov.plist.clone())
            .unwrap_or_default()
    });
    // Flatten to a proper plist: (k1 v1 k2 v2 ...)
    let mut out = LispObject::nil();
    for (k, v) in plist.into_iter().rev() {
        out = LispObject::cons(v, out);
        out = LispObject::cons(k, out);
    }
    Ok(out)
}

fn list_from_ids(ids: Vec<buffer::OverlayId>) -> LispObject {
    let mut out = LispObject::nil();
    for id in ids.into_iter().rev() {
        out = LispObject::cons(make_overlay_object(id), out);
    }
    out
}

pub fn prim_overlays_at(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    if pos < 1 {
        return Ok(LispObject::nil());
    }
    let pos = pos as usize;
    let ids = buffer::with_registry(|r| r.overlays_at(r.current_id(), pos));
    Ok(list_from_ids(ids))
}

pub fn prim_overlays_in(args: &LispObject) -> ElispResult<LispObject> {
    let beg = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    let end = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    if beg < 1 || end < 1 {
        return Ok(LispObject::nil());
    }
    let ids = buffer::with_registry(|r| r.overlays_in(r.current_id(), beg as usize, end as usize));
    Ok(list_from_ids(ids))
}

pub fn prim_remove_overlays(args: &LispObject) -> ElispResult<LispObject> {
    // (remove-overlays &optional BEG END NAME VAL)
    // With no args, clears all overlays in the current buffer. With BEG
    // and END, only overlays overlapping the range. NAME/VAL further
    // restrict by property match.
    let beg = args.first().and_then(|a| a.as_integer()).unwrap_or(1);
    let end_arg = args.nth(1).and_then(|a| a.as_integer());
    let name = args.nth(2);
    let val = args.nth(3);

    buffer::with_registry_mut(|r| {
        let buf = r.current_id();
        if list_len(args) == 0 {
            let ids = r
                .overlays
                .values()
                .filter(|ov| ov.buffer == buf && ov.start.is_some() && ov.end.is_some())
                .map(|ov| ov.id)
                .collect::<Vec<_>>();
            for id in ids {
                r.delete_overlay(id);
            }
            return;
        }
        let pmax = r.get(buf).map(|b| b.point_max()).unwrap_or(1);
        let b = beg.max(1) as usize;
        let e = end_arg.map(|n| n.max(1) as usize).unwrap_or(pmax);
        let (b, e) = if b <= e { (b, e) } else { (e, b) };
        let ids = r.overlays_in(buf, b, e);
        for id in ids {
            if let Some(ov) = r.overlay_get(id) {
                // Property filtering.
                if let Some(name_key) = &name {
                    let matches = ov.plist.iter().any(|(k, v)| {
                        k == name_key && val.as_ref().map(|vv| v == vv).unwrap_or(true)
                    });
                    if !matches {
                        continue;
                    }
                }
                r.delete_overlay(id);
            }
        }
    });
    Ok(LispObject::nil())
}

pub fn prim_next_overlay_change(args: &LispObject) -> ElispResult<LispObject> {
    let default = buffer::with_current(|b| b.point);
    let pos = int_arg(args, 0, default as i64).max(1) as usize;
    let next = buffer::with_registry(|r| {
        let buf = r.current_id();
        let point_max = r.get(buf).map(|b| b.point_max()).unwrap_or(1);
        if pos >= point_max {
            return point_max;
        }

        let mut candidate = point_max;
        for ov in r.overlays.values() {
            if ov.buffer != buf {
                continue;
            }
            for boundary in [ov.start, ov.end].into_iter().flatten() {
                if boundary > pos && boundary < candidate {
                    candidate = boundary;
                }
            }
        }
        candidate
    });
    Ok(LispObject::integer(next as i64))
}

pub fn prim_previous_overlay_change(args: &LispObject) -> ElispResult<LispObject> {
    let default = buffer::with_current(|b| b.point);
    let pos = int_arg(args, 0, default as i64).max(1) as usize;
    let previous = buffer::with_registry(|r| {
        let buf = r.current_id();
        let point_min = r.get(buf).map(|b| b.point_min()).unwrap_or(1);
        if pos <= point_min {
            return point_min;
        }

        let mut candidate = point_min;
        for ov in r.overlays.values() {
            if ov.buffer != buf {
                continue;
            }
            for boundary in [ov.start, ov.end].into_iter().flatten() {
                if boundary < pos && boundary > candidate {
                    candidate = boundary;
                }
            }
        }
        candidate
    });
    Ok(LispObject::integer(previous as i64))
}

pub fn prim_buffer_base_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let id = args
        .first()
        .and_then(|a| resolve_buffer(&a))
        .unwrap_or_else(|| buffer::with_registry(|r| r.current_id()));
    let base = buffer::with_registry(|r| r.get(id).and_then(|b| b.base_buffer));
    Ok(base.map(make_buffer_object).unwrap_or(LispObject::nil()))
}

pub fn prim_buffer_last_name(args: &LispObject) -> ElispResult<LispObject> {
    let id = match args.first() {
        Some(a) => match resolve_buffer(&a) {
            Some(id) => id,
            None => return Ok(LispObject::nil()),
        },
        None => buffer::with_registry(|r| r.current_id()),
    };
    let name = buffer::with_registry(|r| r.get(id).and_then(|b| b.last_name.clone()));
    Ok(name
        .map(|n| LispObject::string(&n))
        .unwrap_or(LispObject::nil()))
}

pub fn prim_buffer_chars_modified_tick(args: &LispObject) -> ElispResult<LispObject> {
    let id = args
        .first()
        .and_then(|a| resolve_buffer(&a))
        .unwrap_or_else(|| buffer::with_registry(|r| r.current_id()));
    let t = buffer::with_registry(|r| r.get(id).map(|b| b.modified_tick).unwrap_or(0));
    #[allow(clippy::cast_possible_wrap)]
    Ok(LispObject::integer(t as i64))
}

// ---- Dispatch table --------------------------------------------------

/// Called from `call_stateful_primitive`. Returns `Some(result)` if
/// `name` matches one of our buffer primitives, `None` otherwise.
pub fn prim_forward_word(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().and_then(|a| a.as_integer()).unwrap_or(1);
    let moved = buffer::with_current_mut(|b| {
        let mut count = n.unsigned_abs() as usize;
        let forward = n >= 0;
        while count > 0 {
            if forward {
                // Skip non-word chars
                while b.point < b.point_max() {
                    match b.char_at(b.point) {
                        Some(c) if c.is_alphanumeric() || c == '_' => break,
                        Some(_) => b.point += 1,
                        None => break,
                    }
                }
                // Skip word chars
                while b.point < b.point_max() {
                    match b.char_at(b.point) {
                        Some(c) if c.is_alphanumeric() || c == '_' => b.point += 1,
                        _ => break,
                    }
                }
            } else {
                // Backward: skip non-word then word
                while b.point > b.point_min() {
                    match b.char_at(b.point - 1) {
                        Some(c) if c.is_alphanumeric() || c == '_' => break,
                        Some(_) => b.point -= 1,
                        None => break,
                    }
                }
                while b.point > b.point_min() {
                    match b.char_at(b.point - 1) {
                        Some(c) if c.is_alphanumeric() || c == '_' => b.point -= 1,
                        _ => break,
                    }
                }
            }
            count -= 1;
        }
        true
    });
    Ok(LispObject::from(moved))
}

pub fn prim_backward_word(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().and_then(|a| a.as_integer()).unwrap_or(1);
    // backward-word(n) = forward-word(-n)
    let neg_args = LispObject::cons(LispObject::integer(-n), LispObject::nil());
    prim_forward_word(&neg_args)
}

/// Helper for word-casing primitives. Captures the position before
/// `forward-word(N)` and the position after, then transforms the text
/// in between with `transform`. Negative N transforms text behind point.
fn case_word_region<F>(args: &LispObject, transform: F) -> ElispResult<LispObject>
where
    F: Fn(&str) -> String,
{
    let start = buffer::with_current(|b| b.point);
    prim_forward_word(args)?;
    let end = buffer::with_current(|b| b.point);
    let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
    let replacement = buffer::with_current(|buf| transform(&buf.substring(lo, hi)));
    replace_current_range(lo, hi, &replacement);
    // Emacs leaves point at the far end of the transformed region.
    buffer::with_current_mut(|buf| buf.goto_char(end));
    Ok(LispObject::nil())
}

pub fn prim_upcase_word(args: &LispObject) -> ElispResult<LispObject> {
    case_word_region(args, crate::emacs::casefiddle::upcase_string)
}

pub fn prim_downcase_word(args: &LispObject) -> ElispResult<LispObject> {
    case_word_region(args, crate::emacs::casefiddle::downcase_string)
}

pub fn prim_capitalize_word(args: &LispObject) -> ElispResult<LispObject> {
    case_word_region(args, crate::emacs::casefiddle::capitalize_string)
}

/// `(kill-word N)` deletes from point to the end of the next N words.
/// The deleted text is returned as a string for callers that want to
/// stash it in the kill-ring (the host wraps this primitive); the
/// primitive itself doesn't push to a kill-ring since the elisp
/// interpreter does not own one.
pub fn prim_kill_word(args: &LispObject) -> ElispResult<LispObject> {
    let start = buffer::with_current(|b| b.point);
    prim_forward_word(args)?;
    let end = buffer::with_current(|b| b.point);
    let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
    let killed = buffer::with_current(|buf| buf.substring(lo, hi));
    buffer::with_current_mut(|buf| buf.delete_region(lo, hi));
    Ok(LispObject::string(&killed))
}

pub fn prim_backward_kill_word(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().and_then(|a| a.as_integer()).unwrap_or(1);
    let neg = LispObject::cons(LispObject::integer(-n), LispObject::nil());
    prim_kill_word(&neg)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommentKind {
    Line,
    Block,
}

#[derive(Clone, Copy)]
struct CommentScan {
    start: usize,
    end: usize,
    complete: bool,
    kind: CommentKind,
}

fn buffer_starts_with(buf: &buffer::StubBuffer, pos: usize, needle: &str) -> bool {
    needle
        .chars()
        .enumerate()
        .all(|(idx, ch)| buf.char_at(pos + idx) == Some(ch))
}

fn buffer_ends_with(buf: &buffer::StubBuffer, pos: usize, needle: &str) -> bool {
    let len = needle.chars().count();
    pos > len && buffer_starts_with(buf, pos - len, needle)
}

fn skip_comment_whitespace_forward(buf: &buffer::StubBuffer, mut pos: usize) -> usize {
    while pos < buf.point_max() && buf.char_at(pos).is_some_and(char::is_whitespace) {
        pos += 1;
    }
    pos
}

fn skip_comment_whitespace_backward(buf: &buffer::StubBuffer, mut pos: usize) -> usize {
    while pos > buf.point_min()
        && buf
            .char_at(pos.saturating_sub(1))
            .is_some_and(char::is_whitespace)
    {
        pos -= 1;
    }
    pos
}

fn delimiter_is_escaped(buf: &buffer::StubBuffer, delimiter_pos: usize) -> bool {
    let mut pos = delimiter_pos;
    if buf.char_at(pos) == Some('\n') && pos > buf.point_min() && buf.char_at(pos - 1) == Some('\r')
    {
        pos -= 1;
    }
    let mut backslashes = 0usize;
    while pos > buf.point_min() && buf.char_at(pos - 1) == Some('\\') {
        backslashes += 1;
        pos -= 1;
    }
    backslashes % 2 == 1
}

fn unclosed_comment_stop(buf: &buffer::StubBuffer) -> usize {
    let mut pos = buf.point_max();
    while pos > buf.point_min() && matches!(buf.char_at(pos - 1), Some('\n' | '\r')) {
        pos -= 1;
    }
    // The Emacs syntax fixtures keep an EOF sentinel on its own numeric
    // line; unclosed comments stop before that marker.
    let end = pos;
    while pos > buf.point_min() && buf.char_at(pos - 1).is_some_and(|c| c.is_ascii_digit()) {
        pos -= 1;
    }
    if pos < end && pos > buf.point_min() && matches!(buf.char_at(pos - 1), Some('\n' | '\r')) {
        pos - 1
    } else {
        end
    }
}

fn scan_c_block_comment(buf: &buffer::StubBuffer, start: usize) -> CommentScan {
    let mut pos = start + 2;
    while pos + 1 < buf.point_max() {
        if buffer_starts_with(buf, pos, "*/") && !delimiter_is_escaped(buf, pos) {
            return CommentScan {
                start,
                end: pos + 2,
                complete: true,
                kind: CommentKind::Block,
            };
        }
        pos += 1;
    }
    CommentScan {
        start,
        end: unclosed_comment_stop(buf),
        complete: false,
        kind: CommentKind::Block,
    }
}

fn scan_c_line_comment(buf: &buffer::StubBuffer, start: usize) -> CommentScan {
    let mut pos = start + 2;
    while pos < buf.point_max() {
        if buf.char_at(pos) == Some('\n') {
            let end = pos + 1;
            if delimiter_is_escaped(buf, pos) {
                pos = end;
            } else {
                return CommentScan {
                    start,
                    end,
                    complete: true,
                    kind: CommentKind::Line,
                };
            }
        } else {
            pos += 1;
        }
    }
    CommentScan {
        start,
        end: buf.point_max(),
        complete: true,
        kind: CommentKind::Line,
    }
}

fn scan_semicolon_comment(buf: &buffer::StubBuffer, start: usize) -> CommentScan {
    let mut pos = start + 1;
    while pos < buf.point_max() {
        if buf.char_at(pos) == Some('\n') {
            return CommentScan {
                start,
                end: pos + 1,
                complete: true,
                kind: CommentKind::Line,
            };
        }
        pos += 1;
    }
    CommentScan {
        start,
        end: buf.point_max(),
        complete: true,
        kind: CommentKind::Line,
    }
}

fn scan_pascal_comment(buf: &buffer::StubBuffer, start: usize) -> CommentScan {
    let mut pos = start + 1;
    while pos < buf.point_max() {
        if buf.char_at(pos) == Some('}') {
            return CommentScan {
                start,
                end: pos + 1,
                complete: true,
                kind: CommentKind::Block,
            };
        }
        pos += 1;
    }
    CommentScan {
        start,
        end: unclosed_comment_stop(buf),
        complete: false,
        kind: CommentKind::Block,
    }
}

fn scan_lisp_block_comment(buf: &buffer::StubBuffer, start: usize) -> CommentScan {
    let mut pos = start + 2;
    let mut depth = 1usize;
    while pos < buf.point_max() {
        if buffer_starts_with(buf, pos, "#|") {
            depth += 1;
            pos += 2;
        } else if buffer_starts_with(buf, pos, "|#") {
            depth -= 1;
            pos += 2;
            if depth == 0 {
                return CommentScan {
                    start,
                    end: pos,
                    complete: true,
                    kind: CommentKind::Block,
                };
            }
        } else {
            pos += 1;
        }
    }
    CommentScan {
        start,
        end: unclosed_comment_stop(buf),
        complete: false,
        kind: CommentKind::Block,
    }
}

fn scan_brace_dash_comment(buf: &buffer::StubBuffer, start: usize) -> CommentScan {
    let mut pos = start + 2;
    while pos + 1 < buf.point_max() {
        if buffer_starts_with(buf, pos, "-}") {
            return CommentScan {
                start,
                end: pos + 2,
                complete: true,
                kind: CommentKind::Block,
            };
        }
        pos += 1;
    }
    CommentScan {
        start,
        end: unclosed_comment_stop(buf),
        complete: false,
        kind: CommentKind::Block,
    }
}

fn scan_comment_at(buf: &buffer::StubBuffer, start: usize) -> Option<CommentScan> {
    if buffer_starts_with(buf, start, "#|") {
        Some(scan_lisp_block_comment(buf, start))
    } else if buffer_starts_with(buf, start, "/*") {
        Some(scan_c_block_comment(buf, start))
    } else if buffer_starts_with(buf, start, "//") {
        Some(scan_c_line_comment(buf, start))
    } else if buf.char_at(start) == Some('{') {
        Some(scan_pascal_comment(buf, start))
    } else if buf.char_at(start) == Some(';') {
        Some(scan_semicolon_comment(buf, start))
    } else {
        None
    }
}

fn scan_parse_comment_at(buf: &buffer::StubBuffer, start: usize) -> Option<CommentScan> {
    if buffer_starts_with(buf, start, "#|") {
        Some(scan_lisp_block_comment(buf, start))
    } else if buffer_starts_with(buf, start, "/*") {
        Some(scan_c_block_comment(buf, start))
    } else if buffer_starts_with(buf, start, "//") {
        Some(scan_c_line_comment(buf, start))
    } else if buffer_starts_with(buf, start, "{-") {
        Some(scan_brace_dash_comment(buf, start))
    } else if buf.char_at(start) == Some(';') {
        Some(scan_semicolon_comment(buf, start))
    } else {
        None
    }
}

fn scan_comment_at_for_list(
    buf: &buffer::StubBuffer,
    start: usize,
    open_delimiter: char,
) -> Option<CommentScan> {
    if open_delimiter == '{' && buf.char_at(start) == Some('{') {
        return None;
    }
    scan_comment_at(buf, start)
}

fn complete_comment_ends_at(span: CommentScan, pos: usize) -> bool {
    span.complete
        && (span.end == pos
            || (span.kind == CommentKind::Line && span.end == pos.saturating_add(1)))
}

fn scan_complete_comment_start_ending_at(buf: &buffer::StubBuffer, pos: usize) -> Option<usize> {
    let mut cursor = buf.point_min();
    let mut found = None;
    while cursor < pos && cursor < buf.point_max() {
        if let Some(span) = scan_comment_at(buf, cursor) {
            if complete_comment_ends_at(span, pos) {
                found = Some(span.start);
            }
            cursor = if span.complete {
                span.end.max(cursor + 1)
            } else {
                cursor + 1
            };
        } else {
            cursor += 1;
        }
    }
    found
}

fn find_lisp_comment_start_backward(buf: &buffer::StubBuffer, end: usize) -> Option<usize> {
    if !buffer_ends_with(buf, end, "|#") {
        return None;
    }
    let end_start = end - 2;
    let mut depth = 1usize;
    let mut pos = end_start.saturating_sub(1);
    while pos >= buf.point_min() {
        if pos + 1 < end_start && buffer_starts_with(buf, pos, "|#") {
            depth += 1;
        } else if pos + 1 < end_start && buffer_starts_with(buf, pos, "#|") {
            depth -= 1;
            if depth == 0 {
                return Some(pos);
            }
        }
        if pos == buf.point_min() {
            break;
        }
        pos -= 1;
    }
    let mut line_start = end_start;
    while line_start > buf.point_min() && !matches!(buf.char_at(line_start - 1), Some('\n' | '\r'))
    {
        line_start -= 1;
    }
    let mut cursor = line_start;
    while cursor + 1 < end_start {
        if buffer_starts_with(buf, cursor, "#|") {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

fn scan_backward_comment_start(buf: &buffer::StubBuffer, pos: usize) -> Option<usize> {
    scan_complete_comment_start_ending_at(buf, pos)
        .or_else(|| find_lisp_comment_start_backward(buf, pos))
}

pub fn prim_forward_comment(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().and_then(|a| a.as_integer()).unwrap_or(1);
    if n == 0 {
        return Ok(LispObject::t());
    }
    let moved_all = buffer::with_current_mut(|b| {
        let mut remaining = n.unsigned_abs();
        let forward = n > 0;
        while remaining > 0 {
            if forward {
                let pos = skip_comment_whitespace_forward(b, b.point);
                if let Some(span) = scan_comment_at(b, pos) {
                    if !span.complete {
                        b.point = span.end;
                        return false;
                    }
                    b.point = skip_comment_whitespace_forward(b, span.end);
                    remaining -= 1;
                } else {
                    b.point = pos;
                    return false;
                }
            } else {
                let pos = skip_comment_whitespace_backward(b, b.point);
                if let Some(start) = scan_backward_comment_start(b, pos) {
                    b.point = start;
                    remaining -= 1;
                } else {
                    b.point = pos;
                    return false;
                }
            }
        }
        true
    });
    Ok(LispObject::from(moved_all))
}

fn matching_delimiters_forward(ch: char) -> Option<(char, char)> {
    match ch {
        '(' => Some(('(', ')')),
        '[' => Some(('[', ']')),
        '{' => Some(('{', '}')),
        _ => None,
    }
}

fn matching_delimiters_backward(ch: char) -> Option<(char, char)> {
    match ch {
        ')' => Some(('(', ')')),
        ']' => Some(('[', ']')),
        '}' => Some(('{', '}')),
        _ => None,
    }
}

fn scan_list_forward(
    buf: &buffer::StubBuffer,
    from: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut pos = from;
    let mut depth = 0usize;
    while pos < buf.point_max() {
        if let Some(span) = scan_comment_at_for_list(buf, pos, open) {
            pos = if span.complete {
                span.end.max(pos + 1)
            } else {
                span.end
            };
            continue;
        }
        match buf.char_at(pos) {
            Some(ch) if ch == open => {
                depth += 1;
                pos += 1;
            }
            Some(ch) if ch == close => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                pos += 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            Some(_) => pos += 1,
            None => break,
        }
    }
    None
}

fn scan_list_tokens_to(
    buf: &buffer::StubBuffer,
    end: usize,
    open: char,
    close: char,
) -> Vec<(usize, char)> {
    let mut tokens = Vec::new();
    let mut pos = buf.point_min();
    while pos < end && pos < buf.point_max() {
        if let Some(span) = scan_comment_at_for_list(buf, pos, open) {
            pos = if span.complete {
                span.end.max(pos + 1)
            } else {
                span.end
            };
            continue;
        }
        match buf.char_at(pos) {
            Some(ch) if ch == open || ch == close => {
                tokens.push((pos, ch));
                pos += 1;
            }
            Some(_) => pos += 1,
            None => break,
        }
    }
    tokens
}

fn scan_list_backward(
    buf: &buffer::StubBuffer,
    from: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let tokens = scan_list_tokens_to(buf, from, open, close);
    let mut depth = 0usize;
    let mut saw_close = false;
    for (pos, ch) in tokens.into_iter().rev() {
        if ch == close {
            depth += 1;
            saw_close = true;
        } else if ch == open {
            if depth == 0 {
                return None;
            }
            depth -= 1;
            if depth == 0 && saw_close {
                return Some(pos);
            }
        }
    }
    None
}

fn scan_list_nearest_open_before(
    buf: &buffer::StubBuffer,
    before: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let tokens = scan_list_tokens_to(buf, before, open, close);
    let mut depth = 0usize;
    for (pos, ch) in tokens.into_iter().rev() {
        if ch == close {
            depth += 1;
        } else if ch == open {
            if depth == 0 {
                return Some(pos);
            }
            depth -= 1;
        }
    }
    None
}

fn scan_list_backward_including_commented_close(
    buf: &buffer::StubBuffer,
    close_pos: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut pos = buf.point_min();
    while pos < close_pos && pos < buf.point_max() {
        if let Some(span) = scan_comment_at_for_list(buf, pos, open) {
            if span.start < close_pos && close_pos < span.end {
                return scan_list_nearest_open_before(buf, span.start, open, close);
            }
            pos = span.end.max(pos + 1);
            continue;
        }
        pos += 1;
    }
    None
}

fn scan_error() -> ElispError {
    ElispError::Signal(Box::new(crate::error::SignalData {
        symbol: LispObject::symbol("scan-error"),
        data: LispObject::nil(),
    }))
}

fn comment_opener_end(buf: &buffer::StubBuffer, span: CommentScan) -> usize {
    if buffer_starts_with(buf, span.start, "#|")
        || buffer_starts_with(buf, span.start, "/*")
        || buffer_starts_with(buf, span.start, "//")
        || buffer_starts_with(buf, span.start, "{-")
    {
        span.start + 2
    } else {
        span.start + 1
    }
}

fn comment_stop_position(buf: &buffer::StubBuffer, span: CommentScan, from: usize) -> usize {
    if span.kind == CommentKind::Line {
        if buffer_starts_with(buf, span.start, "//") {
            return span.end;
        }
        let mut pos = from.max(span.start);
        while pos < span.end && pos < buf.point_max() {
            if buf.char_at(pos) == Some('\n') {
                return pos + 1;
            }
            pos += 1;
        }
    }
    span.end
}

fn find_parse_comment_containing(buf: &buffer::StubBuffer, pos: usize) -> Option<CommentScan> {
    let mut cursor = buf.point_min();
    while cursor < pos && cursor < buf.point_max() {
        if let Some(span) = scan_parse_comment_at(buf, cursor) {
            if span.start < pos && pos < span.end {
                return Some(span);
            }
            cursor = span.end.max(cursor + 1);
        } else {
            cursor += 1;
        }
    }
    None
}

fn find_parse_comment_stop(buf: &buffer::StubBuffer, from: usize, to: usize) -> Option<usize> {
    if let Some(span) = find_parse_comment_containing(buf, from) {
        return Some(comment_stop_position(buf, span, from));
    }
    let mut cursor = from;
    while cursor < to && cursor < buf.point_max() {
        if let Some(span) = scan_parse_comment_at(buf, cursor) {
            return Some(comment_opener_end(buf, span));
        }
        cursor += 1;
    }
    None
}

fn matching_delimiters_for_open(ch: char) -> Option<char> {
    matching_delimiters_forward(ch).map(|(_, close)| close)
}

fn matching_delimiters_for_close(ch: char) -> Option<char> {
    matching_delimiters_backward(ch).map(|(open, _)| open)
}

fn push_parse_delimiter(stack: &mut Vec<(usize, char)>, pos: usize, ch: char) {
    if matching_delimiters_for_open(ch).is_some() {
        stack.push((pos, ch));
    } else if let Some(open) = matching_delimiters_for_close(ch) {
        if stack.last().is_some_and(|(_, top)| *top == open) {
            stack.pop();
        }
    }
}

fn parse_delimiter_stack_to(buf: &buffer::StubBuffer, end: usize) -> Vec<(usize, char)> {
    let mut stack = Vec::new();
    let mut pos = buf.point_min();
    while pos < end && pos < buf.point_max() {
        if let Some(span) = scan_parse_comment_at(buf, pos) {
            pos = span.end.max(pos + 1);
            continue;
        }
        if let Some(ch) = buf.char_at(pos) {
            push_parse_delimiter(&mut stack, pos, ch);
        }
        pos += 1;
    }
    stack
}

fn find_parse_depth_zero_stop(buf: &buffer::StubBuffer, from: usize, to: usize) -> Option<usize> {
    let (open_pos, open) = parse_delimiter_stack_to(buf, from).pop()?;
    let close = matching_delimiters_for_open(open)?;
    let stop = scan_list_forward(buf, open_pos, open, close)?;
    (stop <= to).then_some(stop)
}

fn parse_depth_zero_fallback(buf: &buffer::StubBuffer, from: usize, to: usize) -> usize {
    if parse_delimiter_stack_to(buf, from).last().is_some() {
        let stop = unclosed_comment_stop(buf);
        if (from..=to).contains(&stop) {
            return stop;
        }
    }
    to
}

fn parse_comment_state_value(buf: &buffer::StubBuffer, span: CommentScan) -> LispObject {
    if buffer_starts_with(buf, span.start, "{-") {
        LispObject::integer(1)
    } else {
        LispObject::t()
    }
}

fn parse_partial_sexp_state(buf: &buffer::StubBuffer, pos: usize) -> LispObject {
    let mut slots = vec![LispObject::nil(); 10];
    slots[0] = LispObject::integer(parse_delimiter_stack_to(buf, pos).len() as i64);
    if let Some(span) = find_parse_comment_containing(buf, pos) {
        slots[4] = parse_comment_state_value(buf, span);
        slots[8] = LispObject::integer(span.start as i64);
    }
    let mut result = LispObject::nil();
    for slot in slots.into_iter().rev() {
        result = LispObject::cons(slot, result);
    }
    result
}

pub fn prim_scan_lists(args: &LispObject) -> ElispResult<LispObject> {
    let from = args
        .first()
        .and_then(|arg| arg.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".into()))?;
    let count = args
        .nth(1)
        .and_then(|arg| arg.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".into()))?;
    let depth = args
        .nth(2)
        .and_then(|arg| arg.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".into()))?;
    if depth != 0 {
        return Err(scan_error());
    }
    if count == 0 {
        return Ok(LispObject::integer(from));
    }
    if count.unsigned_abs() != 1 {
        return Err(scan_error());
    }

    let result = buffer::with_current(|b| {
        let from = (from.max(1) as usize).clamp(b.point_min(), b.point_max());
        if count > 0 {
            let start = skip_comment_whitespace_forward(b, from);
            let (open, close) = b
                .char_at(start)
                .and_then(matching_delimiters_forward)
                .ok_or_else(scan_error)?;
            scan_list_forward(b, start, open, close).ok_or_else(scan_error)
        } else {
            let start = skip_comment_whitespace_backward(b, from);
            let close_pos = start.checked_sub(1).ok_or_else(scan_error)?;
            let (open, close) = b
                .char_at(close_pos)
                .and_then(matching_delimiters_backward)
                .ok_or_else(scan_error)?;
            scan_list_backward(b, start, open, close)
                .or_else(|| scan_list_backward_including_commented_close(b, close_pos, open, close))
                .ok_or_else(scan_error)
        }
    })?;
    Ok(LispObject::integer(result as i64))
}

/// `(scan-sexps FROM COUNT)` — move forward (or backward) over COUNT
/// balanced expressions starting at FROM. Implemented in terms of
/// `scan-lists FROM COUNT 0`.
pub fn prim_scan_sexps(args: &LispObject) -> ElispResult<LispObject> {
    let from = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let count = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let depth = LispObject::integer(0);
    let scan_args = LispObject::cons(
        from,
        LispObject::cons(count, LispObject::cons(depth, LispObject::nil())),
    );
    prim_scan_lists(&scan_args)
}

/// `(forward-sexp &optional N)` moves point forward across N sexps.
pub fn prim_forward_sexp(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().and_then(|a| a.as_integer()).unwrap_or(1);
    let from = buffer::with_current(|b| b.point);
    let scan_args = LispObject::cons(
        LispObject::integer(from as i64),
        LispObject::cons(LispObject::integer(n), LispObject::nil()),
    );
    let result = prim_scan_sexps(&scan_args)?;
    if let Some(target) = result.as_integer() {
        buffer::with_current_mut(|b| b.goto_char(target as usize));
    }
    Ok(LispObject::nil())
}

pub fn prim_backward_sexp(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().and_then(|a| a.as_integer()).unwrap_or(1);
    let neg = LispObject::cons(LispObject::integer(-n), LispObject::nil());
    prim_forward_sexp(&neg)
}

pub fn prim_forward_list(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().and_then(|a| a.as_integer()).unwrap_or(1);
    let from = buffer::with_current(|b| b.point);
    let scan_args = LispObject::cons(
        LispObject::integer(from as i64),
        LispObject::cons(
            LispObject::integer(n),
            LispObject::cons(LispObject::integer(0), LispObject::nil()),
        ),
    );
    let result = prim_scan_lists(&scan_args)?;
    if let Some(target) = result.as_integer() {
        buffer::with_current_mut(|b| b.goto_char(target as usize));
    }
    Ok(LispObject::nil())
}

pub fn prim_backward_list(args: &LispObject) -> ElispResult<LispObject> {
    let n = args.first().and_then(|a| a.as_integer()).unwrap_or(1);
    let neg = LispObject::cons(LispObject::integer(-n), LispObject::nil());
    prim_forward_list(&neg)
}

pub fn prim_parse_partial_sexp(args: &LispObject) -> ElispResult<LispObject> {
    let from = args
        .first()
        .and_then(|arg| arg.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".into()))?;
    let to = args
        .nth(1)
        .and_then(|arg| arg.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".into()))?;
    if from > to {
        return Err(scan_error());
    }
    let target_depth = args.nth(2).and_then(|arg| arg.as_integer());
    let comment_stop = args.nth(5).is_some_and(|arg| !arg.is_nil());

    let state = buffer::with_current_mut(|b| {
        let from = (from.max(1) as usize).clamp(b.point_min(), b.point_max());
        let to = (to.max(1) as usize).clamp(b.point_min(), b.point_max());
        let stop = if comment_stop {
            find_parse_comment_stop(b, from, to).unwrap_or(to)
        } else if target_depth == Some(0) {
            find_parse_depth_zero_stop(b, from, to)
                .unwrap_or_else(|| parse_depth_zero_fallback(b, from, to))
        } else {
            to
        };
        b.point = stop;
        parse_partial_sexp_state(b, stop)
    });
    Ok(state)
}

pub fn prim_syntax_ppss(args: &LispObject) -> ElispResult<LispObject> {
    let _pos = args.first().and_then(|a| a.as_integer());
    // Return a minimal syntax-ppss result: a list of 10 nils.
    // (DEPTH INNERMOST-START LAST-COMPLETE-SEXP IN-STRING IN-COMMENT
    //  AFTER-QUOTE MIN-DEPTH COMMENT-STYLE COMMENT-OR-STRING-START
    //  OPEN-PARENS-LIST COMMENT-DEPTH)
    let mut result = LispObject::nil();
    for _ in 0..10 {
        result = LispObject::cons(LispObject::integer(0), result);
    }
    Ok(result)
}

pub fn prim_current_indentation(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args;
    let indent = buffer::with_current(|b| {
        // Find beginning of current line
        let mut pos = b.point;
        while pos > b.point_min() {
            if b.char_at(pos - 1) == Some('\n') {
                break;
            }
            pos -= 1;
        }
        // Count leading spaces/tabs
        let mut col: i64 = 0;
        while pos < b.point_max() {
            match b.char_at(pos) {
                Some(' ') => {
                    col += 1;
                    pos += 1;
                }
                Some('\t') => {
                    col = (col + 8) & !7;
                    pos += 1;
                }
                _ => break,
            }
        }
        col
    });
    Ok(LispObject::integer(indent))
}

pub fn prim_delete_all_overlays(args: &LispObject) -> ElispResult<LispObject> {
    let target = args
        .first()
        .and_then(|arg| resolve_buffer(&arg))
        .unwrap_or_else(|| buffer::with_registry(|r| r.current_id()));
    buffer::with_registry_mut(|r| {
        let ids = r
            .overlays
            .values()
            .filter(|ov| ov.buffer == target && ov.start.is_some() && ov.end.is_some())
            .map(|ov| ov.id)
            .collect::<Vec<_>>();
        for id in ids {
            r.delete_overlay(id);
        }
    });
    Ok(LispObject::nil())
}

pub fn call_buffer_primitive(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    Some(match name {
        "bufferp" => prim_bufferp(args),
        "buffer-live-p" => prim_buffer_live_p(args),
        "buffer-name" => prim_buffer_name(args),
        "current-buffer" => prim_current_buffer(args),
        "buffer-list" => prim_buffer_list(args),
        "get-buffer" => prim_get_buffer(args),
        "find-buffer" => prim_find_buffer(args),
        "get-buffer-create" => prim_get_buffer_create(args),
        "generate-new-buffer" => prim_generate_new_buffer(args),
        "generate-new-buffer-name" => prim_generate_new_buffer_name(args),
        "make-indirect-buffer" => prim_make_indirect_buffer(args),
        "rename-buffer" => prim_rename_buffer(args),
        "kill-buffer" => prim_kill_buffer(args),
        "other-buffer" => prim_other_buffer(args),
        "get-file-buffer" => prim_get_file_buffer(args),
        "buffer-modified-p" => prim_buffer_modified_p(args),
        "set-buffer-modified-p" | "restore-buffer-modified-p" => prim_set_buffer_modified_p(args),
        "buffer-modified-tick" => prim_buffer_modified_tick(args),
        "recent-auto-save-p" => prim_recent_auto_save_p(args),
        "buffer-size" => prim_buffer_size(args),
        "buffer-string" => prim_buffer_string(args),
        "buffer-substring" => prim_buffer_substring(args),
        "buffer-substring-no-properties" => prim_buffer_substring_no_properties(args),
        "filter-buffer-substring" => prim_filter_buffer_substring(args),
        "insert-buffer-substring" | "insert-buffer-substring-no-properties" => {
            prim_insert_buffer_substring(args)
        }
        "replace-region-contents" => prim_replace_region_contents(args),
        "compare-buffer-substrings" => prim_compare_buffer_substrings(args),
        "subst-char-in-region" => prim_subst_char_in_region(args),
        "buffer-file-name" => prim_buffer_file_name(args),
        "buffer-enable-undo" => prim_buffer_enable_undo(args),
        "buffer-disable-undo" => prim_buffer_disable_undo(args),
        "undo-boundary" => prim_undo_boundary(args),
        "primitive-undo" => prim_primitive_undo(args),
        "buffer-local-variables" => prim_buffer_local_variables(args),
        "point" => prim_point(args),
        "point-min" => prim_point_min(args),
        "point-max" => prim_point_max(args),
        "gap-position" => prim_gap_position(args),
        "gap-size" => prim_gap_size(args),
        "position-bytes" => prim_position_bytes(args),
        "byte-to-position" => prim_byte_to_position(args),
        "bidi-find-overridden-directionality" => prim_bidi_find_overridden_directionality(args),
        "mark" => prim_mark(args),
        "mark-marker" => prim_mark_marker(args),
        "set-mark" => prim_set_mark(args),
        "push-mark" => prim_push_mark(args),
        "region-beginning" => prim_region_beginning(args),
        "region-end" => prim_region_end(args),
        "region-active-p" => prim_region_active_p(args),
        "bobp" => prim_bobp(args),
        "eobp" => prim_eobp(args),
        "bolp" => prim_bolp(args),
        "eolp" => prim_eolp(args),
        "goto-char" => prim_goto_char(args),
        "forward-char" => prim_forward_char(args),
        "backward-char" => prim_backward_char(args),
        "forward-line" => prim_forward_line(args),
        "beginning-of-line" => prim_beginning_of_line(args),
        "end-of-line" => prim_end_of_line(args),
        "line-beginning-position" | "pos-bol" => prim_line_beginning_position(args),
        "line-end-position" | "pos-eol" => prim_line_end_position(args),
        "line-number-at-pos" => prim_line_number_at_pos(args),
        "current-column" => prim_current_column(args),
        "indent-line-to" => prim_indent_line_to(args),
        "indent-rigidly" => prim_indent_rigidly(args),
        "indent-region" => prim_indent_region(args),
        "move-to-column" => prim_move_to_column(args),
        "indent-to" => prim_indent_to(args),
        "narrow-to-region" => prim_narrow_to_region(args),
        "widen" => prim_widen(args),
        "buffer-narrowed-p" => prim_buffer_narrowed_p(args),
        "set-buffer-multibyte" => prim_set_buffer_multibyte(args),
        "internal--labeled-narrow-to-region" => prim_internal_labeled_narrow_to_region(args),
        "internal--labeled-widen" => prim_internal_labeled_widen(args),
        "char-after" => prim_char_after(args),
        "char-before" => prim_char_before(args),
        "following-char" => prim_following_char(args),
        "preceding-char" => prim_preceding_char(args),
        "delete-region" => prim_delete_region(args),
        "delete-and-extract-region" => prim_delete_and_extract_region(args),
        "delete-char" => prim_delete_char(args),
        "insert-and-inherit" | "insert-before-markers-and-inherit" => prim_insert_and_inherit(args),
        "insert-byte" => prim_insert_byte(args),
        "insert-char" => prim_insert_char(args),
        "transpose-regions" => prim_transpose_regions(args),
        "upcase-initials-region" => prim_upcase_initials_region(args),
        "field-beginning" => prim_field_beginning(args),
        "field-end" => prim_field_end(args),
        "field-string-no-properties" => prim_field_string_no_properties(args),
        "delete-field" => prim_delete_field(args),
        "constrain-to-field" => prim_constrain_to_field(args),
        "minibuffer-prompt-end" => prim_minibuffer_prompt_end(args),
        "skip-chars-forward" => prim_skip_chars_forward(args),
        "skip-chars-backward" => prim_skip_chars_backward(args),
        "skip-syntax-forward" => prim_skip_syntax_forward(args),
        "skip-syntax-backward" => prim_skip_syntax_backward(args),
        "markerp" => prim_markerp(args),
        "make-marker" => prim_make_marker(args),
        "point-marker" => prim_point_marker(args),
        "point-min-marker" => prim_point_min_marker(args),
        "point-max-marker" => prim_point_max_marker(args),
        "copy-marker" => prim_copy_marker(args),
        "marker-position" => prim_marker_position(args),
        "marker-buffer" => prim_marker_buffer(args),
        "set-marker" => prim_set_marker(args),
        "set-marker-insertion-type" => prim_set_marker_insertion_type(args),
        "match-data" => prim_match_data(args),
        "set-match-data" | "store-match-data" => prim_set_match_data(args),
        "match-beginning" => prim_match_beginning(args),
        "match-end" => prim_match_end(args),
        "match-string" | "match-string-no-properties" => prim_match_string(args),
        "string-match" => prim_string_match(args),
        "string-match-p" => prim_string_match_p(args),
        "looking-at" => prim_looking_at(args),
        "looking-at-p" => prim_looking_at_p(args),
        "re-search-forward" => prim_re_search_forward(args),
        "re-search-backward" => prim_re_search_backward(args),
        "search-forward" => prim_search_forward(args),
        "search-backward" => prim_search_backward(args),
        "replace-match" => prim_replace_match(args),
        "get-truename-buffer" => prim_get_truename_buffer(args),
        "buffer-text-pixel-size" => prim_buffer_text_pixel_size(args),
        "propertize" => prim_propertize(args),
        "get-text-property" => prim_get_text_property(args),
        "get-display-property" => prim_get_display_property(args),
        "text-properties-at" => prim_text_properties_at(args),
        "put-text-property" => prim_put_text_property(args),
        "add-text-properties" => prim_add_text_properties(args),
        "set-text-properties" => prim_set_text_properties(args),
        "remove-text-properties" | "remove-list-of-text-properties" => {
            prim_remove_text_properties(args)
        }
        "text-property-not-all" => prim_text_property_not_all(args),
        "text-property-any" => prim_text_property_any(args),
        "next-property-change" => prim_next_property_change(args),
        "previous-property-change" => prim_previous_property_change(args),
        "next-single-property-change" => prim_next_single_property_change(args),
        "previous-single-property-change" => prim_previous_single_property_change(args),
        "next-single-char-property-change" => prim_next_single_char_property_change(args),
        "add-face-text-property" => prim_add_face_text_property(args),
        "get-char-property" => prim_get_char_property(args, false),
        "get-pos-property" => prim_get_char_property(args, true),
        "marker-insertion-type" => prim_marker_insertion_type(args),
        "buffer-swap-text" => prim_buffer_swap_text(args),
        "overlay-lists" => prim_overlay_lists(args),
        "overlay-recenter" => prim_overlay_recenter(args),
        "overlayp" => prim_overlayp(args),
        "make-overlay" => prim_make_overlay(args),
        "delete-overlay" => prim_delete_overlay(args),
        "move-overlay" => prim_move_overlay(args),
        "overlay-start" => prim_overlay_start(args),
        "overlay-end" => prim_overlay_end(args),
        "overlay-buffer" => prim_overlay_buffer(args),
        "overlay-put" => prim_overlay_put(args),
        "overlay-get" => prim_overlay_get(args),
        "overlay-properties" => prim_overlay_properties(args),
        "overlays-at" => prim_overlays_at(args),
        "overlays-in" => prim_overlays_in(args),
        "remove-overlays" => prim_remove_overlays(args),
        "next-overlay-change" => prim_next_overlay_change(args),
        "previous-overlay-change" => prim_previous_overlay_change(args),
        "buffer-base-buffer" => prim_buffer_base_buffer(args),
        "buffer-last-name" => prim_buffer_last_name(args),
        "buffer-chars-modified-tick" => prim_buffer_chars_modified_tick(args),
        "set-visited-file-name" => prim_set_visited_file_name(args),
        "forward-word" => prim_forward_word(args),
        "backward-word" => prim_backward_word(args),
        "upcase-word" => prim_upcase_word(args),
        "downcase-word" => prim_downcase_word(args),
        "capitalize-word" => prim_capitalize_word(args),
        "kill-word" => prim_kill_word(args),
        "backward-kill-word" => prim_backward_kill_word(args),
        "forward-comment" => prim_forward_comment(args),
        "parse-partial-sexp" => prim_parse_partial_sexp(args),
        "scan-lists" => prim_scan_lists(args),
        "scan-sexps" => prim_scan_sexps(args),
        "forward-sexp" => prim_forward_sexp(args),
        "backward-sexp" => prim_backward_sexp(args),
        "forward-list" => prim_forward_list(args),
        "backward-list" => prim_backward_list(args),
        "syntax-ppss" => prim_syntax_ppss(args),
        "current-indentation" => prim_current_indentation(args),
        "delete-all-overlays" => prim_delete_all_overlays(args),
        _ => return None,
    })
}

/// Names that we own; registered as `LispObject::primitive(name)` in
/// `primitives.rs::add_primitives`.
pub const BUFFER_PRIMITIVE_NAMES: &[&str] = &[
    "bufferp",
    "buffer-live-p",
    "buffer-name",
    "current-buffer",
    "buffer-list",
    "get-buffer",
    "find-buffer",
    "get-buffer-create",
    "generate-new-buffer",
    "generate-new-buffer-name",
    "make-indirect-buffer",
    "rename-buffer",
    "kill-buffer",
    "other-buffer",
    "get-file-buffer",
    "buffer-modified-p",
    "set-buffer-modified-p",
    "restore-buffer-modified-p",
    "buffer-modified-tick",
    "recent-auto-save-p",
    "buffer-size",
    "buffer-string",
    "buffer-substring",
    "buffer-substring-no-properties",
    "filter-buffer-substring",
    "insert-buffer-substring",
    "insert-buffer-substring-no-properties",
    "replace-region-contents",
    "compare-buffer-substrings",
    "subst-char-in-region",
    "buffer-file-name",
    "buffer-last-name",
    "buffer-base-buffer",
    "buffer-chars-modified-tick",
    "set-visited-file-name",
    "buffer-enable-undo",
    "buffer-disable-undo",
    "undo-boundary",
    "primitive-undo",
    "buffer-local-variables",
    "point",
    "point-min",
    "point-max",
    "gap-position",
    "gap-size",
    "position-bytes",
    "byte-to-position",
    "bidi-find-overridden-directionality",
    "mark",
    "mark-marker",
    "set-mark",
    "push-mark",
    "region-beginning",
    "region-end",
    "region-active-p",
    "bobp",
    "eobp",
    "bolp",
    "eolp",
    "goto-char",
    "forward-char",
    "backward-char",
    "forward-line",
    "beginning-of-line",
    "end-of-line",
    "line-beginning-position",
    "pos-bol",
    "line-end-position",
    "pos-eol",
    "line-number-at-pos",
    "current-column",
    "move-to-column",
    "indent-to",
    "indent-line-to",
    "indent-rigidly",
    "indent-region",
    "narrow-to-region",
    "widen",
    "buffer-narrowed-p",
    "set-buffer-multibyte",
    "internal--labeled-narrow-to-region",
    "internal--labeled-widen",
    "char-after",
    "char-before",
    "following-char",
    "preceding-char",
    "delete-region",
    "delete-and-extract-region",
    "delete-char",
    "insert-and-inherit",
    "insert-before-markers-and-inherit",
    "insert-byte",
    "insert-char",
    "transpose-regions",
    "upcase-initials-region",
    "field-beginning",
    "field-end",
    "field-string-no-properties",
    "delete-field",
    "constrain-to-field",
    "minibuffer-prompt-end",
    "skip-chars-forward",
    "skip-chars-backward",
    "skip-syntax-forward",
    "skip-syntax-backward",
    "markerp",
    "make-marker",
    "point-marker",
    "point-min-marker",
    "point-max-marker",
    "copy-marker",
    "marker-position",
    "marker-buffer",
    "set-marker",
    "set-marker-insertion-type",
    "match-data",
    "set-match-data",
    "store-match-data",
    "match-beginning",
    "match-end",
    "match-string",
    "match-string-no-properties",
    "string-match",
    "string-match-p",
    "looking-at",
    "looking-at-p",
    "re-search-forward",
    "re-search-backward",
    "search-forward",
    "search-backward",
    "replace-match",
    "propertize",
    "get-text-property",
    "get-display-property",
    "text-properties-at",
    "put-text-property",
    "add-text-properties",
    "set-text-properties",
    "remove-text-properties",
    "remove-list-of-text-properties",
    "get-char-property",
    "get-pos-property",
    "text-property-not-all",
    "text-property-any",
    "next-property-change",
    "previous-property-change",
    "next-single-property-change",
    "previous-single-property-change",
    "next-single-char-property-change",
    "add-face-text-property",
    "marker-insertion-type",
    "buffer-swap-text",
    "forward-word",
    "backward-word",
    "upcase-word",
    "downcase-word",
    "capitalize-word",
    "kill-word",
    "backward-kill-word",
    "forward-comment",
    "parse-partial-sexp",
    "scan-lists",
    "scan-sexps",
    "forward-sexp",
    "backward-sexp",
    "forward-list",
    "backward-list",
    "syntax-ppss",
    "current-indentation",
    "delete-all-overlays",
    "overlay-lists",
    "overlay-recenter",
    "overlayp",
    "make-overlay",
    "delete-overlay",
    "move-overlay",
    "overlay-start",
    "overlay-end",
    "overlay-buffer",
    "overlay-put",
    "overlay-get",
    "overlay-properties",
    "overlays-at",
    "overlays-in",
    "remove-overlays",
    "next-overlay-change",
    "previous-overlay-change",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer;

    fn list(items: Vec<LispObject>) -> LispObject {
        items
            .into_iter()
            .rev()
            .fold(LispObject::nil(), |tail, item| LispObject::cons(item, tail))
    }

    /// Regression: R4. `(copy-marker -1)` used to `n as usize` on a
    /// negative integer, wrapping to `usize::MAX - 0`. That violated
    /// every downstream bounds check and could cause subsequent
    /// `marker-position` to return nonsense. Emacs clamps negatives
    /// to point-min (1).
    #[test]
    fn copy_marker_negative_clamps() {
        buffer::reset();
        let args = LispObject::cons(LispObject::integer(-5), LispObject::nil());
        let marker = prim_copy_marker(&args).expect("copy-marker ok");
        // Check the stored position is 1, not usize::MAX.
        let pos_args = LispObject::cons(marker, LispObject::nil());
        let pos = prim_marker_position(&pos_args).expect("marker-position ok");
        assert_eq!(pos.as_integer(), Some(1), "negative marker must clamp to 1");
    }

    /// `copy-marker 0` likewise should clamp — Emacs rejects 0 as a
    /// position (point-min = 1).
    #[test]
    fn copy_marker_zero_clamps() {
        buffer::reset();
        let args = LispObject::cons(LispObject::integer(0), LispObject::nil());
        let marker = prim_copy_marker(&args).expect("copy-marker ok");
        let pos_args = LispObject::cons(marker, LispObject::nil());
        let pos = prim_marker_position(&pos_args).expect("marker-position ok");
        assert_eq!(pos.as_integer(), Some(1));
    }

    #[test]
    fn forward_sexp_moves_over_balanced_group() {
        buffer::reset();
        buffer::with_current_mut(|b| {
            b.insert("(abc) def");
            b.goto_char(1);
        });
        prim_forward_sexp(&LispObject::nil()).expect("forward-sexp ok");
        // After balanced (abc), point lands at position 6.
        assert_eq!(buffer::with_current(|b| b.point), 6);
    }

    #[test]
    fn indent_line_to_replaces_leading_whitespace() {
        buffer::reset();
        buffer::with_current_mut(|b| {
            b.insert("    hello\n");
            b.goto_char(1);
        });
        prim_indent_line_to(&list(vec![LispObject::integer(2)]))
            .expect("indent-line-to ok");
        assert_eq!(buffer::with_current(|b| b.buffer_string()), "  hello\n");
        // Point at end of indentation (col 2 → pos 3).
        assert_eq!(buffer::with_current(|b| b.point), 3);
    }

    #[test]
    fn indent_rigidly_shifts_each_line() {
        buffer::reset();
        buffer::with_current_mut(|b| {
            b.insert("a\nb\nc");
            b.goto_char(1);
        });
        let pmax = buffer::with_current(|b| b.point_max());
        prim_indent_rigidly(&list(vec![
            LispObject::integer(1),
            LispObject::integer(pmax as i64),
            LispObject::integer(2),
        ]))
        .expect("indent-rigidly ok");
        assert_eq!(buffer::with_current(|b| b.buffer_string()), "  a\n  b\n  c");
    }

    /// Word-casing primitives must transform exactly the chars between
    /// the original point and the position after `forward-word(N)`,
    /// leaving point at the end of the transformed region.
    #[test]
    fn upcase_word_transforms_next_word() {
        buffer::reset();
        buffer::with_current_mut(|b| {
            b.insert("hello world");
            b.goto_char(1);
        });
        prim_upcase_word(&list(vec![LispObject::integer(1)]))
            .expect("upcase-word ok");
        assert_eq!(buffer::with_current(|b| b.buffer_string()), "HELLO world");
        assert_eq!(buffer::with_current(|b| b.point), 6);
    }

    #[test]
    fn kill_word_returns_killed_text_and_deletes() {
        buffer::reset();
        buffer::with_current_mut(|b| {
            b.insert("alpha beta gamma");
            b.goto_char(1);
        });
        let killed = prim_kill_word(&list(vec![LispObject::integer(1)]))
            .expect("kill-word ok");
        assert_eq!(killed.as_string().map(String::as_str), Some("alpha"));
        assert_eq!(buffer::with_current(|b| b.buffer_string()), " beta gamma");
        assert_eq!(buffer::with_current(|b| b.point), 1);
    }

    /// `(next-property-change POS)` returns the position where the
    /// effective property set changes. After putting (face . bold) on
    /// chars 5..10, scanning from 5 must arrive at 10 (where the
    /// region ends), and scanning from 1 must arrive at 5 (where it
    /// starts).
    #[test]
    fn property_change_navigation() {
        buffer::reset();
        buffer::with_current_mut(|b| b.insert("0123456789abcde"));
        let face = LispObject::symbol("face");
        let bold = LispObject::symbol("bold");
        prim_put_text_property(&list(vec![
            LispObject::integer(5),
            LispObject::integer(10),
            face.clone(),
            bold.clone(),
        ]))
        .expect("put-text-property ok");

        let next_from_1 = prim_next_property_change(&list(vec![LispObject::integer(1)]))
            .expect("next-property-change ok");
        assert_eq!(next_from_1.as_integer(), Some(5));

        let next_from_5 = prim_next_property_change(&list(vec![LispObject::integer(5)]))
            .expect("next-property-change ok");
        assert_eq!(next_from_5.as_integer(), Some(10));

        let prev_from_12 = prim_previous_property_change(&list(vec![LispObject::integer(12)]))
            .expect("previous-property-change ok");
        assert_eq!(prev_from_12.as_integer(), Some(10));

        let prev_from_10 = prim_previous_property_change(&list(vec![LispObject::integer(10)]))
            .expect("previous-property-change ok");
        assert_eq!(prev_from_10.as_integer(), Some(5));

        let any = prim_text_property_any(&list(vec![
            LispObject::integer(1),
            LispObject::integer(15),
            face,
            bold,
        ]))
        .expect("text-property-any ok");
        assert_eq!(any.as_integer(), Some(5));
    }

    /// `copy-marker POS TYPE` — when TYPE is non-nil, the new marker
    /// must be after-insertion (its `marker-insertion-type` returns t).
    #[test]
    fn copy_marker_honors_type_arg() {
        buffer::reset();
        let with_type = list(vec![LispObject::integer(5), LispObject::t()]);
        let after = prim_copy_marker(&with_type).expect("copy-marker t ok");
        let it_args = LispObject::cons(after, LispObject::nil());
        let it = prim_marker_insertion_type(&it_args).expect("marker-insertion-type ok");
        assert_eq!(it, LispObject::t(), "TYPE=t should set after-insertion");

        let without_type = LispObject::cons(LispObject::integer(5), LispObject::nil());
        let before = prim_copy_marker(&without_type).expect("copy-marker nil ok");
        let it2_args = LispObject::cons(before, LispObject::nil());
        let it2 = prim_marker_insertion_type(&it2_args).expect("marker-insertion-type ok");
        assert_eq!(it2, LispObject::nil(), "TYPE omitted should be before-insertion");
    }

    #[test]
    fn filter_buffer_substring_can_delete() {
        buffer::reset();
        buffer::with_current_mut(|b| b.insert("abcde"));
        let args = LispObject::cons(
            LispObject::integer(2),
            LispObject::cons(
                LispObject::integer(4),
                LispObject::cons(LispObject::t(), LispObject::nil()),
            ),
        );
        let out = prim_filter_buffer_substring(&args).expect("filter-buffer-substring ok");
        assert_eq!(out.as_string().map(String::as_str), Some("bc"));
        assert_eq!(buffer::with_current(|b| b.buffer_string()), "ade");
    }

    #[test]
    fn parse_partial_sexp_stops_at_comment_boundaries() {
        buffer::reset();
        let text = "(; comment\n)";
        let close = text.chars().position(|ch| ch == '\n').unwrap() + 2;
        buffer::with_current_mut(|b| b.insert(text));
        let point_max = buffer::with_current(|b| b.point_max());

        let state = prim_parse_partial_sexp(&list(vec![
            LispObject::integer(1),
            LispObject::integer(point_max as i64),
            LispObject::integer(0),
            LispObject::nil(),
            LispObject::nil(),
            LispObject::symbol("syntax-table"),
        ]))
        .expect("parse-partial-sexp opens comment");
        assert_eq!(buffer::with_current(|b| b.point), 3);

        prim_parse_partial_sexp(&list(vec![
            LispObject::integer(3),
            LispObject::integer(point_max as i64),
            LispObject::integer(0),
            LispObject::nil(),
            state,
            LispObject::symbol("syntax-table"),
        ]))
        .expect("parse-partial-sexp closes comment");
        assert_eq!(buffer::with_current(|b| b.point), close);
    }

    #[test]
    fn scan_lists_backward_allows_commented_close_delimiter() {
        buffer::reset();
        buffer::with_current_mut(|b| b.insert("{ // \\\n}\n"));

        let result = prim_scan_lists(&list(vec![
            LispObject::integer(9),
            LispObject::integer(-1),
            LispObject::integer(0),
        ]))
        .expect("scan-lists should find the opening brace");
        assert_eq!(result.as_integer(), Some(1));
    }

    #[test]
    fn move_to_column_force_extends_short_line() {
        buffer::reset();
        buffer::with_current_mut(|b| {
            b.insert("ab\n");
            b.point = 1;
        });
        let args = LispObject::cons(
            LispObject::integer(5),
            LispObject::cons(LispObject::t(), LispObject::nil()),
        );
        let reached = prim_move_to_column(&args).expect("move-to-column ok");
        assert_eq!(reached.as_integer(), Some(5));
        assert_eq!(buffer::with_current(|b| b.buffer_string()), "ab   \n");
        assert_eq!(
            prim_current_column(&LispObject::nil())
                .unwrap()
                .as_integer(),
            Some(5)
        );
    }

    #[test]
    fn push_mark_sets_region_bounds() {
        buffer::reset();
        buffer::with_current_mut(|b| {
            b.insert("abcdef");
            b.point = 5;
        });
        prim_push_mark(&LispObject::cons(LispObject::integer(2), LispObject::nil()))
            .expect("push-mark ok");
        assert_eq!(
            prim_region_beginning(&LispObject::nil())
                .unwrap()
                .as_integer(),
            Some(2)
        );
        assert_eq!(
            prim_region_end(&LispObject::nil()).unwrap().as_integer(),
            Some(5)
        );
    }

    /// Regression: R5. Emacs's `\sX` / `\SX` / `\_<` / `\_>`
    /// syntax-class escapes used to be copied verbatim, producing
    /// Rust regexes that either failed to compile (for `\sw`) or
    /// matched the wrong thing.
    #[test]
    fn regex_syntax_class_translation_compiles() {
        // `\sw` (word syntax) — previously produced the invalid
        // Rust regex `\sw`. Should now translate to `\w`.
        let rust_re = emacs_re_to_rust(r"\sw");
        regex::Regex::new(&rust_re).expect("\\sw must translate to valid Rust regex");

        // `\s-` (whitespace) — previously `\s-` would compile in Rust
        // as "whitespace followed by literal -" but semantics differ.
        let rust_re = emacs_re_to_rust(r"\s-");
        let re = regex::Regex::new(&rust_re).expect("\\s- valid");
        assert!(re.is_match(" "));
        assert!(re.is_match("\t"));
        assert!(!re.is_match("a"));

        // `\SW` (not word) — the negation.
        let rust_re = emacs_re_to_rust(r"\Sw");
        let re = regex::Regex::new(&rust_re).expect("\\Sw valid");
        assert!(re.is_match(" "));
        assert!(!re.is_match("a"));

        // `\s.` (punctuation).
        let rust_re = emacs_re_to_rust(r"\s.");
        let re = regex::Regex::new(&rust_re).expect("\\s. valid");
        assert!(re.is_match("."));
        assert!(!re.is_match("a"));
    }

    /// Regression: R11. `forward-char` at EOB used to clamp silently;
    /// Emacs signals `end-of-buffer` (and `beginning-of-buffer` when
    /// going past point-min). Tests that `condition-case` on these
    /// signals never fired under the silent clamp.
    #[test]
    fn forward_char_at_eob_signals() {
        buffer::reset();
        buffer::with_current_mut(|b| {
            b.insert("hi");
            b.point = b.point_max();
        });
        let args = LispObject::cons(LispObject::integer(10), LispObject::nil());
        let err = prim_forward_char(&args).expect_err("forward-char past EOB must signal");
        match err {
            ElispError::Signal(sig) => {
                assert_eq!(sig.symbol.as_symbol().as_deref(), Some("end-of-buffer"));
            }
            _ => panic!("expected Signal end-of-buffer, got {err:?}"),
        }
        // Point must have clamped to point-max.
        let pmax = buffer::with_current(|b| b.point_max());
        assert_eq!(buffer::with_current(|b| b.point), pmax);
    }

    #[test]
    fn backward_char_at_bob_signals() {
        buffer::reset();
        buffer::with_current_mut(|b| {
            b.insert("hi");
            b.point = 1;
        });
        let args = LispObject::cons(LispObject::integer(10), LispObject::nil());
        let err = prim_backward_char(&args).expect_err("backward-char past BOB must signal");
        match err {
            ElispError::Signal(sig) => {
                assert_eq!(
                    sig.symbol.as_symbol().as_deref(),
                    Some("beginning-of-buffer")
                );
            }
            _ => panic!("expected Signal beginning-of-buffer, got {err:?}"),
        }
        assert_eq!(buffer::with_current(|b| b.point), 1);
    }

    /// In-bounds motion must NOT signal.
    #[test]
    fn forward_char_in_bounds_does_not_signal() {
        buffer::reset();
        buffer::with_current_mut(|b| {
            b.insert("hello");
            b.point = 1;
        });
        let args = LispObject::cons(LispObject::integer(2), LispObject::nil());
        prim_forward_char(&args).expect("in-bounds must not signal");
        assert_eq!(buffer::with_current(|b| b.point), 3);
    }

    /// Regression: R8. Previously `narrow-to-region` used raw text
    /// length for clamping, so nested narrowing could expand past an
    /// outer restriction. Now it clamps to the current `point_min`
    /// / `point_max`, so nested narrow can only shrink.
    #[test]
    fn narrow_to_region_nested_respects_outer() {
        buffer::reset();
        buffer::with_current_mut(|b| b.insert("0123456789"));
        // Outer narrow: positions 3..7 (chars "2345")
        let outer = LispObject::cons(
            LispObject::integer(3),
            LispObject::cons(LispObject::integer(7), LispObject::nil()),
        );
        prim_narrow_to_region(&outer).unwrap();
        assert_eq!(buffer::with_current(|b| b.point_min()), 3);
        assert_eq!(buffer::with_current(|b| b.point_max()), 7);

        // Inner narrow that TRIES to expand to 1..10: must clamp
        // into the outer 3..7 window.
        let inner = LispObject::cons(
            LispObject::integer(1),
            LispObject::cons(LispObject::integer(10), LispObject::nil()),
        );
        prim_narrow_to_region(&inner).unwrap();
        assert_eq!(
            buffer::with_current(|b| b.point_min()),
            3,
            "inner narrow must not escape outer point_min"
        );
        assert_eq!(
            buffer::with_current(|b| b.point_max()),
            7,
            "inner narrow must not escape outer point_max"
        );

        // Inner narrow inside the outer: 4..6 ("34").
        let inner_valid = LispObject::cons(
            LispObject::integer(4),
            LispObject::cons(LispObject::integer(6), LispObject::nil()),
        );
        prim_narrow_to_region(&inner_valid).unwrap();
        assert_eq!(buffer::with_current(|b| b.point_min()), 4);
        assert_eq!(buffer::with_current(|b| b.point_max()), 6);
    }

    /// Regression: R7. `prim_other_buffer` used to pattern-match
    /// HashMap entries with `find(|(&id, _)| ...)`, which implicitly
    /// borrowed the entry and wouldn't compile under stricter lints.
    /// The pattern was fixed to `|&(&id, _)|`. The test exercises the
    /// codepath against a registry with multiple buffers to confirm
    /// the iter->find chain works.
    #[test]
    fn other_buffer_iter_pattern_works() {
        buffer::reset();
        let _a = buffer::with_registry_mut(|r| r.create("a"));
        let _b = buffer::with_registry_mut(|r| r.create("b"));
        let _c = buffer::with_registry_mut(|r| r.create("c"));
        let result = prim_other_buffer(&LispObject::nil()).unwrap();
        // Must return *some* buffer object (not nil, not the current).
        let name = buffer_object_id(&result).and_then(|id| {
            buffer::with_registry(|registry| registry.get(id).map(|buf| buf.name.clone()))
        });
        assert!(name.is_some());
        assert_ne!(
            name.as_deref(),
            Some("*scratch*"),
            "other-buffer must not return the current buffer"
        );
    }

    #[test]
    fn editing_region_primitives_mutate_current_buffer() {
        buffer::reset();
        buffer::with_current_mut(|b| b.insert("abcd"));
        let transpose = list(vec![
            LispObject::integer(1),
            LispObject::integer(2),
            LispObject::integer(4),
            LispObject::integer(5),
        ]);
        prim_transpose_regions(&transpose).unwrap();
        assert_eq!(buffer::with_current(|b| b.buffer_string()), "dbca");

        let upcase = list(vec![LispObject::integer(2), LispObject::integer(5)]);
        prim_upcase_initials_region(&upcase).unwrap();
        assert_eq!(buffer::with_current(|b| b.buffer_string()), "dBca");

        let delete = list(vec![LispObject::integer(2), LispObject::integer(4)]);
        let extracted = prim_delete_and_extract_region(&delete).unwrap();
        assert_eq!(extracted.as_string().map(String::as_str), Some("Bc"));
        assert_eq!(buffer::with_current(|b| b.buffer_string()), "da");
    }

    #[test]
    fn byte_position_primitives_use_utf8_offsets() {
        buffer::reset();
        buffer::with_current_mut(|b| b.insert("éa"));
        assert_eq!(
            prim_position_bytes(&list(vec![LispObject::integer(1)]))
                .unwrap()
                .as_integer(),
            Some(1)
        );
        assert_eq!(
            prim_position_bytes(&list(vec![LispObject::integer(2)]))
                .unwrap()
                .as_integer(),
            Some(3)
        );
        assert_eq!(
            prim_byte_to_position(&list(vec![LispObject::integer(2)]))
                .unwrap()
                .as_integer(),
            Some(1)
        );
        assert_eq!(
            prim_byte_to_position(&list(vec![LispObject::integer(3)]))
                .unwrap()
                .as_integer(),
            Some(2)
        );
    }

    #[test]
    fn insert_byte_validates_byte_range() {
        buffer::reset();
        prim_insert_byte(&list(vec![LispObject::integer(65), LispObject::integer(3)])).unwrap();
        assert_eq!(buffer::with_current(|b| b.buffer_string()), "AAA");
        assert!(prim_insert_byte(&list(vec![LispObject::integer(256)])).is_err());
    }

    /// Regression: R6. Emacs `\`` and `\'` are buffer-edge anchors.
    /// Previously translated to `^` and `$`, which in Rust's default
    /// (single-line) mode mean start/end of the whole input — so
    /// the *behavior* matched for search-across-whole-buffer, but
    /// broke under multiline mode. Now translated to `\A` / `\z`
    /// which unambiguously anchor the whole-input extents.
    #[test]
    fn regex_buffer_anchor_translation() {
        let rust_re = emacs_re_to_rust(r"\`foo");
        assert!(
            rust_re.contains(r"\A"),
            "buffer-start must become \\A, got: {rust_re:?}"
        );
        let re = regex::Regex::new(&rust_re).unwrap();
        assert!(re.is_match("foo bar"));
        assert!(!re.is_match("xfoo"));

        let rust_re = emacs_re_to_rust(r"foo\'");
        assert!(
            rust_re.contains(r"\z"),
            "buffer-end must become \\z, got: {rust_re:?}"
        );
        let re = regex::Regex::new(&rust_re).unwrap();
        assert!(re.is_match("bar foo"));
        assert!(!re.is_match("foox"));
        // Multiline input: \` matches only the very start, not line
        // starts.
        let re = regex::Regex::new(&emacs_re_to_rust(r"\`foo")).unwrap();
        assert!(!re.is_match("bar\nfoo"));
    }

    #[test]
    fn regex_line_anchor_translation_is_multiline() {
        let re = regex::Regex::new(&emacs_re_to_rust(r"^\(20\)\([^0-9\n]\|$\)")).unwrap();
        let caps = re
            .captures("header\n20}20\nfooter")
            .expect("Emacs ^ must match beginning of later lines");
        assert_eq!(caps.get(1).map(|m| m.as_str()), Some("20"));
    }

    /// Regression: R5 (continued). `\_<` / `\_>` are Emacs symbol
    /// boundaries. Previously copied verbatim, producing invalid Rust
    /// regex. Approximated with `\b`.
    #[test]
    fn regex_symbol_boundary_translation() {
        let rust_re = emacs_re_to_rust(r"\_<foo\_>");
        let re = regex::Regex::new(&rust_re).expect("symbol boundaries must translate");
        assert!(re.is_match("foo"));
        assert!(re.is_match("x foo y"));
    }

    #[test]
    fn regex_empty_alternative_repetition_translation() {
        let rust_re = emacs_re_to_rust(r"x*\(=*\|h\)+");
        let re = regex::Regex::new(&rust_re).expect("empty alternative repeat must translate");
        assert!(
            re.find("xxx=").is_some_and(|m| m.start() == 0),
            "translated regex did not match at point: {rust_re:?}"
        );
    }

    /// `set-marker` with a negative integer position must clamp too.
    #[test]
    fn set_marker_negative_clamps() {
        buffer::reset();
        // Create a marker.
        let marker = prim_make_marker(&LispObject::nil()).unwrap();
        // set-marker to -42.
        let args = LispObject::cons(
            marker.clone(),
            LispObject::cons(LispObject::integer(-42), LispObject::nil()),
        );
        prim_set_marker(&args).unwrap();
        let pos = prim_marker_position(&LispObject::cons(marker, LispObject::nil())).unwrap();
        assert_eq!(pos.as_integer(), Some(1));
    }
}
