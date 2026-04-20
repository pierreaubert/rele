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

/// Resolve an elisp buffer designator (string name, buffer handle)
/// to a BufferId. A `LispObject::String("<name>")` is treated as a
/// buffer name; a `LispObject::Nil` means "current buffer".
/// Returns None if the named buffer doesn't exist.
fn resolve_buffer(arg: &LispObject) -> Option<BufferId> {
    match arg {
        LispObject::Nil => Some(buffer::with_registry(|r| r.current_id())),
        LispObject::String(name) => buffer::with_registry(|r| r.lookup_by_name(name)),
        // Some call sites pass a symbol (the name). Resolve it.
        LispObject::Symbol(id) => {
            let name = crate::obarray::symbol_name(*id);
            buffer::with_registry(|r| r.lookup_by_name(&name))
        }
        _ => None,
    }
}

fn int_arg(args: &LispObject, n: usize, default: i64) -> i64 {
    args.nth(n).and_then(|v| v.as_integer()).unwrap_or(default)
}

fn string_arg(args: &LispObject, n: usize) -> Option<String> {
    args.nth(n).and_then(|v| v.as_string().map(|s| s.to_string()))
}

// ---- Buffer lifecycle -------------------------------------------------

pub fn prim_bufferp(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    // A "buffer" here is a string that names an existing buffer.
    let is_buf = match a {
        LispObject::String(name) => buffer::with_registry(|r| r.lookup_by_name(&name).is_some()),
        _ => false,
    };
    Ok(LispObject::from(is_buf))
}

pub fn prim_buffer_live_p(args: &LispObject) -> ElispResult<LispObject> {
    let id = args
        .first()
        .and_then(|a| resolve_buffer(&a));
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
    let name = buffer::with_registry(|r| {
        let id = r.current_id();
        r.get(id).map(|b| b.name.clone())
    });
    Ok(LispObject::string(&name.unwrap_or_default()))
}

pub fn prim_buffer_list(_args: &LispObject) -> ElispResult<LispObject> {
    let names = buffer::with_registry(|r| {
        r.list()
            .into_iter()
            .filter_map(|id| r.get(id).map(|b| b.name.clone()))
            .collect::<Vec<_>>()
    });
    let mut out = LispObject::nil();
    for n in names.into_iter().rev() {
        out = LispObject::cons(LispObject::string(&n), out);
    }
    Ok(out)
}

pub fn prim_get_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match resolve_buffer(&a) {
        Some(id) => {
            let name = buffer::with_registry(|r| r.get(id).map(|b| b.name.clone()));
            Ok(name.map(|n| LispObject::string(&n)).unwrap_or(LispObject::nil()))
        }
        None => Ok(LispObject::nil()),
    }
}

pub fn prim_get_buffer_create(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = match &a {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Err(ElispError::WrongTypeArgument("string".into())),
    };
    buffer::with_registry_mut(|r| {
        r.create(&name);
    });
    Ok(LispObject::string(&name))
}

pub fn prim_rename_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let new_name = string_arg(args, 0).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
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
                return Err(ElispError::EvalError("rename-buffer: name collision".into()));
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
    let name = buffer::with_registry(|r| {
        let cur = r.current_id();
        r.buffers
            .iter()
            .find(|&(&id, _)| id != cur)
            .map(|(_, b)| b.name.clone())
            .or_else(|| r.get(cur).map(|b| b.name.clone()))
    });
    Ok(name.map(|n| LispObject::string(&n)).unwrap_or(LispObject::nil()))
}

pub fn prim_get_file_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let path = match args.first() {
        Some(LispObject::String(s)) => s,
        _ => return Ok(LispObject::nil()),
    };
    let found = buffer::with_registry(|r| {
        r.buffers
            .values()
            .find(|b| b.file_name.as_deref() == Some(&path))
            .map(|b| b.name.clone())
    });
    Ok(found.map(|n| LispObject::string(&n)).unwrap_or(LispObject::nil()))
}

pub fn prim_buffer_modified_p(args: &LispObject) -> ElispResult<LispObject> {
    let id = args
        .first()
        .and_then(|a| resolve_buffer(&a))
        .or_else(|| Some(buffer::with_registry(|r| r.current_id())));
    let modified = buffer::with_registry(|r| {
        id.and_then(|id| r.get(id)).map(|b| b.modified).unwrap_or(false)
    });
    Ok(LispObject::from(modified))
}

pub fn prim_set_buffer_modified_p(args: &LispObject) -> ElispResult<LispObject> {
    let flag = args
        .first()
        .map(|a| !matches!(a, LispObject::Nil))
        .unwrap_or(false);
    buffer::with_current_mut(|b| b.modified = flag);
    Ok(LispObject::from(flag))
}

pub fn prim_buffer_modified_tick(_args: &LispObject) -> ElispResult<LispObject> {
    let t = buffer::with_current(|b| b.modified_tick);
    // Tick values fit comfortably in i64.
    #[allow(clippy::cast_possible_wrap)]
    Ok(LispObject::integer(t as i64))
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
    // We don't track undo state — succeed silently.
    Ok(LispObject::nil())
}

pub fn prim_buffer_disable_undo(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_buffer_string(_args: &LispObject) -> ElispResult<LispObject> {
    let s = buffer::with_current(|b| b.buffer_string());
    Ok(LispObject::string(&s))
}

pub fn prim_buffer_file_name(args: &LispObject) -> ElispResult<LispObject> {
    let id = args
        .first()
        .and_then(|a| resolve_buffer(&a))
        .unwrap_or_else(|| buffer::with_registry(|r| r.current_id()));
    let f = buffer::with_registry(|r| r.get(id).and_then(|b| b.file_name.clone()));
    Ok(f.map(|s| LispObject::string(&s)).unwrap_or(LispObject::nil()))
}

// ---- Point / positions ------------------------------------------------

pub fn prim_point(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(buffer::with_current(|b| b.point) as i64))
}

pub fn prim_point_min(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(
        buffer::with_current(|b| b.point_min()) as i64,
    ))
}

pub fn prim_point_max(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(
        buffer::with_current(|b| b.point_max()) as i64,
    ))
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
    let reached = buffer::with_current_mut(|b| {
        let bol = b.line_beginning_position(b.point);
        let eol = b.line_end_position(b.point);
        let target = (bol + col).min(eol);
        b.point = target;
        target - bol
    });
    Ok(LispObject::integer(reached as i64))
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
        buf.restriction = Some((lo, hi));
        if buf.point < lo {
            buf.point = lo;
        } else if buf.point > hi {
            buf.point = hi;
        }
    });
    Ok(LispObject::nil())
}

pub fn prim_widen(_args: &LispObject) -> ElispResult<LispObject> {
    buffer::with_current_mut(|b| b.restriction = None);
    Ok(LispObject::nil())
}

pub fn prim_buffer_narrowed_p(_args: &LispObject) -> ElispResult<LispObject> {
    let narrowed = buffer::with_current(|b| b.restriction.is_some());
    Ok(LispObject::from(narrowed))
}

// ---- Characters and regions -----------------------------------------

pub fn prim_char_after(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|a| a.as_integer())
        .map(|n| n as usize)
        .unwrap_or_else(|| buffer::with_current(|b| b.point));
    let c = buffer::with_current(|b| b.char_at(pos));
    Ok(c.map(|ch| LispObject::integer(ch as i64)).unwrap_or(LispObject::nil()))
}

pub fn prim_char_before(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|a| a.as_integer())
        .map(|n| n as usize)
        .unwrap_or_else(|| buffer::with_current(|b| b.point));
    if pos <= 1 {
        return Ok(LispObject::nil());
    }
    let c = buffer::with_current(|b| b.char_at(pos - 1));
    Ok(c.map(|ch| LispObject::integer(ch as i64)).unwrap_or(LispObject::nil()))
}

pub fn prim_delete_char(args: &LispObject) -> ElispResult<LispObject> {
    let n = int_arg(args, 0, 1);
    buffer::with_current_mut(|b| {
        if n >= 0 {
            let end = (b.point as i64 + n).min(b.point_max() as i64) as usize;
            b.delete_region(b.point, end);
        } else {
            let start = (b.point as i64 + n).max(b.point_min() as i64) as usize;
            b.delete_region(start, b.point);
        }
    });
    Ok(LispObject::nil())
}

pub fn prim_insert_char(args: &LispObject) -> ElispResult<LispObject> {
    let ch = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("character".into()))?;
    let count = int_arg(args, 1, 1) as usize;
    if let Some(c) = char::from_u32(ch as u32) {
        let s: String = std::iter::repeat(c).take(count).collect();
        buffer::with_current_mut(|b| b.insert(&s));
    }
    Ok(LispObject::nil())
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
    let new_id = buffer::with_registry_mut(|r| {
        let id = r.make_marker(buf);
        r.marker_set(id, buf, pos);
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
        let name = buffer::with_registry(|r| {
            r.markers
                .get(&id)
                .and_then(|m| r.get(m.buffer).map(|b| b.name.clone()))
        });
        return Ok(name.map(|n| LispObject::string(&n)).unwrap_or(LispObject::nil()));
    }
    Ok(LispObject::nil())
}

pub fn prim_set_marker(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = marker_id(&a).ok_or_else(|| ElispError::WrongTypeArgument("marker".into()))?;
    // Clamp negative positions to 1 — same reason as `copy-marker`.
    let pos = args.nth(1).and_then(|v| v.as_integer()).map(|n| n.max(1) as usize);
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
            Some((a, b)) => (
                LispObject::integer(a as i64),
                LispObject::integer(b as i64),
            ),
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
    let pos = buffer::with_registry(|r| {
        r.match_data
            .groups
            .get(idx)
            .and_then(|g| g.map(|(a, _)| a))
    });
    Ok(pos.map(|p| LispObject::integer(p as i64)).unwrap_or(LispObject::nil()))
}

pub fn prim_match_end(args: &LispObject) -> ElispResult<LispObject> {
    let idx = int_arg(args, 0, 0) as usize;
    let pos = buffer::with_registry(|r| {
        r.match_data
            .groups
            .get(idx)
            .and_then(|g| g.map(|(_, b)| b))
    });
    Ok(pos.map(|p| LispObject::integer(p as i64)).unwrap_or(LispObject::nil()))
}

pub fn prim_match_string(args: &LispObject) -> ElispResult<LispObject> {
    let idx = int_arg(args, 0, 0) as usize;
    let source = args.nth(1).and_then(|a| a.as_string().map(|s| s.to_string()));
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
        buffer::with_registry(|r| r.get(id).map(|bf| bf.substring(a, b)))
            .unwrap_or_default()
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
                '(' | ')' | '|' | '{' | '}' | '?' | '+' => {
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
                // Syntax class prefix: \sX consumes X and emits the
                // equivalent POSIX character class.
                's' | 'S' if i + 2 < chars.len() => {
                    let x = chars[i + 2];
                    let negate = n == 'S';
                    let class = emacs_syntax_to_rust_class(x);
                    if negate {
                        // `\SX` = "not of syntax class X". Build an
                        // inverted character-class set.
                        out.push_str(&negate_posix_class(class));
                    } else {
                        out.push_str(class);
                    }
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
        } else if c == '(' || c == ')' || c == '|' || c == '{' || c == '}' || c == '?' || c == '+' {
            // Literal in Emacs.
            out.push('\\');
            out.push(c);
            i += 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Map an Emacs syntax-class letter to a Rust regex character class.
///
/// Emacs syntax-table letters (from `syntax.h`):
///   -   whitespace              `[[:space:]]`
///   w   word                    `\w`
///   _   symbol constituent      `[\w]` (approx — Emacs is richer)
///   .   punctuation             `[[:punct:]]`
///   (   open paren              `[(\[{]`
///   )   close paren             `[)\]}]`
///   "   string quote            `["']`
///   '   expression prefix       `['`~,@]`
///   <   comment start           `[;#]`
///   >   comment end             `[\n\r]`
///   \\  escape                  `\\`
///   /   char-quote              `\\`
///   |   gen. string delimiter   `["]`
///   $   paired delimiter        `[$]`
///
/// Returns a Rust regex fragment. Unknown classes fall through to `.`
/// (match any), which is the safest over-approximation for search.
fn emacs_syntax_to_rust_class(c: char) -> &'static str {
    match c {
        '-' | ' ' => r"[[:space:]]",
        'w' => r"\w",
        '_' => r"\w",
        '.' => r"[[:punct:]]",
        '(' => r"[(\[{]",
        ')' => r"[)\]}]",
        '"' | '|' => r#"[""]"#,
        '\'' => r"[\'`,@]",
        '<' => r"[;#]",
        '>' => r"[\n\r]",
        '\\' | '/' => r"\\",
        _ => r".",
    }
}

/// Turn a POSIX-ish character class produced by
/// `emacs_syntax_to_rust_class` into its negation. Only handles the
/// shapes we actually emit.
fn negate_posix_class(cls: &str) -> String {
    if let Some(inner) = cls.strip_prefix("[[:").and_then(|s| s.strip_suffix(":]]")) {
        format!("[^[:{inner}:]]")
    } else if let Some(inner) = cls.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        format!("[^{inner}]")
    } else if cls == r"\w" {
        r"\W".into()
    } else if cls == r"\\" {
        r"[^\\]".into()
    } else {
        // Fallback — match any single char (same as the positive
        // class's permissive default of `.`).
        r".".into()
    }
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
    let noerror = args.nth(2).map(|a| !matches!(a, LispObject::Nil)).unwrap_or(false);
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
    let bound = args.nth(1).and_then(|a| a.as_integer()).map(|n| n as usize).unwrap_or(1);
    let noerror = args.nth(2).map(|a| !matches!(a, LispObject::Nil)).unwrap_or(false);
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
    let needle = string_arg(args, 0).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
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
        Some((_, end)) => {
            buffer::with_current_mut(|b| b.point = end);
            Ok(LispObject::integer(end as i64))
        }
        None => {
            let noerror = args.nth(2).map(|a| !matches!(a, LispObject::Nil)).unwrap_or(false);
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
    let needle = string_arg(args, 0).ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let result = buffer::with_current(|b| {
        let end_byte = b.char_to_byte(b.point);
        b.text[..end_byte].rfind(&needle).map(|off| {
            let prefix = &b.text[..off];
            let char_start = prefix.chars().count() + 1; // 1-based
            (char_start, char_start + needle.chars().count())
        })
    });
    match result {
        Some((start, _)) => {
            buffer::with_current_mut(|b| b.point = start);
            Ok(LispObject::integer(start as i64))
        }
        None => {
            let noerror = args.nth(2).map(|a| !matches!(a, LispObject::Nil)).unwrap_or(false);
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
        buffer::with_current_mut(|buf| {
            buf.delete_region(a, b);
            buf.goto_char(a);
            buf.insert(&repl);
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
    Ok(found.map(|n| LispObject::string(&n)).unwrap_or(LispObject::nil()))
}

pub fn prim_buffer_text_pixel_size(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::cons(LispObject::integer(0), LispObject::integer(0)))
}

pub fn prim_text_property_not_all(args: &LispObject) -> ElispResult<LispObject> {
    let start = args.first().and_then(|a| a.as_integer()).ok_or(ElispError::WrongTypeArgument("integer".into()))? as usize;
    let end = args.nth(1).and_then(|a| a.as_integer()).ok_or(ElispError::WrongTypeArgument("integer".into()))? as usize;
    let _prop = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let _value = args.nth(3).ok_or(ElispError::WrongNumberOfArguments)?;
    if start < end { Ok(LispObject::integer(start as i64)) } else { Ok(LispObject::nil()) }
}

pub fn prim_marker_insertion_type(args: &LispObject) -> ElispResult<LispObject> {
    let _marker = args.first().unwrap_or(LispObject::nil());
    Ok(LispObject::nil())
}

pub fn prim_buffer_swap_text(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args;
    Err(ElispError::Signal(Box::new(crate::error::SignalData {
        symbol: LispObject::symbol("error"),
        data: LispObject::cons(LispObject::string("buffer-swap-text not supported"), LispObject::nil()),
    })))
}

pub fn prim_overlay_lists(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::cons(LispObject::nil(), LispObject::nil()))
}

pub fn prim_overlay_recenter(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_buffer_base_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let id = args.first().and_then(|a| resolve_buffer(&a)).unwrap_or_else(|| buffer::with_registry(|r| r.current_id()));
    let name = buffer::with_registry(|r| r.get(id).map(|b| b.name.clone()));
    Ok(name.map(|n| LispObject::string(&n)).unwrap_or(LispObject::nil()))
}

pub fn prim_buffer_last_name(args: &LispObject) -> ElispResult<LispObject> {
    prim_buffer_name(args)
}

pub fn prim_buffer_chars_modified_tick(args: &LispObject) -> ElispResult<LispObject> {
    let id = args.first().and_then(|a| resolve_buffer(&a)).unwrap_or_else(|| buffer::with_registry(|r| r.current_id()));
    let t = buffer::with_registry(|r| r.get(id).map(|b| b.modified_tick).unwrap_or(0));
    #[allow(clippy::cast_possible_wrap)]
    Ok(LispObject::integer(t as i64))
}

// ---- Dispatch table --------------------------------------------------

/// Called from `call_stateful_primitive`. Returns `Some(result)` if
/// `name` matches one of our buffer primitives, `None` otherwise.
pub fn call_buffer_primitive(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    Some(match name {
        "bufferp" => prim_bufferp(args),
        "buffer-live-p" => prim_buffer_live_p(args),
        "buffer-name" => prim_buffer_name(args),
        "current-buffer" => prim_current_buffer(args),
        "buffer-list" => prim_buffer_list(args),
        "get-buffer" => prim_get_buffer(args),
        "get-buffer-create" => prim_get_buffer_create(args),
        "rename-buffer" => prim_rename_buffer(args),
        "kill-buffer" => prim_kill_buffer(args),
        "other-buffer" => prim_other_buffer(args),
        "get-file-buffer" => prim_get_file_buffer(args),
        "buffer-modified-p" => prim_buffer_modified_p(args),
        "set-buffer-modified-p" | "restore-buffer-modified-p" => prim_set_buffer_modified_p(args),
        "buffer-modified-tick" => prim_buffer_modified_tick(args),
        "buffer-size" => prim_buffer_size(args),
        "buffer-string" => prim_buffer_string(args),
        "buffer-file-name" => prim_buffer_file_name(args),
        "buffer-enable-undo" => prim_buffer_enable_undo(args),
        "buffer-disable-undo" => prim_buffer_disable_undo(args),
        "point" => prim_point(args),
        "point-min" => prim_point_min(args),
        "point-max" => prim_point_max(args),
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
        "move-to-column" => prim_move_to_column(args),
        "narrow-to-region" => prim_narrow_to_region(args),
        "widen" => prim_widen(args),
        "buffer-narrowed-p" => prim_buffer_narrowed_p(args),
        "char-after" => prim_char_after(args),
        "char-before" => prim_char_before(args),
        "delete-char" => prim_delete_char(args),
        "insert-char" => prim_insert_char(args),
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
        "text-property-not-all" => prim_text_property_not_all(args),
        "marker-insertion-type" => prim_marker_insertion_type(args),
        "buffer-swap-text" => prim_buffer_swap_text(args),
        "overlay-lists" => prim_overlay_lists(args),
        "overlay-recenter" => prim_overlay_recenter(args),
        "buffer-base-buffer" => prim_buffer_base_buffer(args),
        "buffer-last-name" => prim_buffer_last_name(args),
        "buffer-chars-modified-tick" => prim_buffer_chars_modified_tick(args),
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
    "get-buffer-create",
    "rename-buffer",
    "kill-buffer",
    "other-buffer",
    "get-file-buffer",
    "buffer-modified-p",
    "set-buffer-modified-p",
    "restore-buffer-modified-p",
    "buffer-modified-tick",
    "buffer-size",
    "buffer-string",
    "buffer-file-name",
    "buffer-enable-undo",
    "buffer-disable-undo",
    "point",
    "point-min",
    "point-max",
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
    "narrow-to-region",
    "widen",
    "buffer-narrowed-p",
    "char-after",
    "char-before",
    "delete-char",
    "insert-char",
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
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer;

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
                assert_eq!(
                    sig.symbol.as_symbol().as_deref(),
                    Some("end-of-buffer")
                );
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
        // Must return *some* buffer name (not nil, not the current).
        let name = result.as_string().map(|s| s.to_string());
        assert!(name.is_some());
        assert_ne!(
            name.as_deref(),
            Some("*scratch*"),
            "other-buffer must not return the current buffer"
        );
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
