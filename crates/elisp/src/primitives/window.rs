#![allow(clippy::disallowed_methods)]
//! Window / frame / keymap primitives — batch 2.
//!
//! This runtime is headless (no graphical frames, no actual windows),
//! so these are mostly "reasonable defaults that type-check": there's
//! always exactly one frame and one window, both covering the current
//! buffer. Tests that only *query* windows get real data; tests that
//! try to *partition* the frame (split-window, delete-window) get
//! no-ops that preserve the single-window invariant.
//!
//! See `primitives_buffer.rs` for the dispatch pattern (stateful
//! primitive called from `call_stateful_primitive`).

use std::cell::RefCell;
use std::collections::HashMap;

use crate::buffer::{self, BufferId};
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

// ---- Window-configuration snapshots ---------------------------------

/// Snapshot of (buffer-id, point) for each live buffer, captured by
/// `current-window-configuration`. Restored by `set-window-configuration`.
/// We don't model multi-window layouts; a configuration is essentially
/// (current-buffer + per-buffer point).
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub id: usize,
    pub current_buffer: BufferId,
    pub points: HashMap<BufferId, usize>,
}

thread_local! {
    static WINDOW_CONFIGS: RefCell<HashMap<usize, WindowConfig>> = RefCell::new(HashMap::new());
    static NEXT_WC_ID: std::cell::Cell<usize> = const { std::cell::Cell::new(1) };
    static WINDOW_DEDICATED: RefCell<LispObject> = const { RefCell::new(LispObject::Nil) };
    static WINDOW_START: RefCell<Option<usize>> = const { RefCell::new(None) };
    static WINDOW_PARAMETERS: RefCell<HashMap<String, LispObject>> = RefCell::new(HashMap::new());

    /// Global keymaps registry. Keyed by name (symbol name); the value
    /// is the keymap's Lisp structure (a list). We don't dispatch key
    /// events — this is purely so `define-key`, `use-local-map`,
    /// `current-local-map`, etc. can round-trip their state without
    /// "void function" errors.
    static KEYMAPS: RefCell<HashMap<String, LispObject>> = RefCell::new(HashMap::new());
    static CURRENT_LOCAL_MAP: RefCell<Option<LispObject>> = const { RefCell::new(None) };
    static CURRENT_GLOBAL_MAP: RefCell<Option<LispObject>> = const { RefCell::new(None) };
}

/// Tag used on the cons cell that represents a window-configuration
/// handle (`(window-configuration . ID)`).
fn wc_obj(id: usize) -> LispObject {
    LispObject::cons(
        LispObject::symbol("window-configuration"),
        LispObject::integer(id as i64),
    )
}

fn wc_id(obj: &LispObject) -> Option<usize> {
    let (car, cdr) = obj.destructure_cons()?;
    if car.as_symbol().as_deref() != Some("window-configuration") {
        return None;
    }
    cdr.as_integer().map(|n| n as usize)
}

/// Tag used on a "window" handle. Since there's only ever one window,
/// this is always `(window . 0)`.
fn window_obj() -> LispObject {
    LispObject::cons(LispObject::symbol("window"), LispObject::integer(0))
}

fn is_window(obj: &LispObject) -> bool {
    obj.destructure_cons()
        .map(|(car, _)| car.as_symbol().as_deref() == Some("window"))
        .unwrap_or(false)
}

fn frame_obj() -> LispObject {
    LispObject::cons(LispObject::symbol("frame"), LispObject::integer(0))
}

fn is_frame(obj: &LispObject) -> bool {
    obj.destructure_cons()
        .map(|(car, _)| car.as_symbol().as_deref() == Some("frame"))
        .unwrap_or(false)
}

fn terminal_obj() -> LispObject {
    LispObject::cons(LispObject::symbol("terminal"), LispObject::integer(0))
}

fn is_terminal(obj: &LispObject) -> bool {
    obj.destructure_cons()
        .map(|(car, _)| car.as_symbol().as_deref() == Some("terminal"))
        .unwrap_or(false)
}

const FRAME_COLUMNS: i64 = 80;
const FRAME_LINES: i64 = 24;
const FRAME_CHAR_WIDTH: i64 = 10;
const FRAME_CHAR_HEIGHT: i64 = 20;

fn frame_pixel_width() -> i64 {
    FRAME_COLUMNS * FRAME_CHAR_WIDTH
}

fn frame_pixel_height() -> i64 {
    FRAME_LINES * FRAME_CHAR_HEIGHT
}

fn lisp_list(items: &[LispObject]) -> LispObject {
    let mut out = LispObject::nil();
    for item in items.iter().rev() {
        out = LispObject::cons(item.clone(), out);
    }
    out
}

fn integer_list(items: &[i64]) -> LispObject {
    let mut out = LispObject::nil();
    for item in items.iter().rev() {
        out = LispObject::cons(LispObject::integer(*item), out);
    }
    out
}

fn integer_pair(left: i64, right: i64) -> LispObject {
    LispObject::cons(LispObject::integer(left), LispObject::integer(right))
}

fn clamp_nonnegative(value: i64) -> i64 {
    value.max(0)
}

fn current_window_start_for(point_min: usize) -> usize {
    WINDOW_START.with(|start| start.borrow().unwrap_or(point_min).max(point_min))
}

fn current_window_start() -> usize {
    let point_min = buffer::with_current(|b| b.point_min());
    current_window_start_for(point_min)
}

fn window_parameter_alist() -> LispObject {
    WINDOW_PARAMETERS.with(|params| {
        let params = params.borrow();
        let mut keys: Vec<_> = params.keys().cloned().collect();
        keys.sort();
        let mut out = LispObject::nil();
        for key in keys.into_iter().rev() {
            if let Some(value) = params.get(&key) {
                out = LispObject::cons(
                    LispObject::cons(LispObject::symbol(&key), value.clone()),
                    out,
                );
            }
        }
        out
    })
}

// ---- Window accessors ------------------------------------------------

pub fn prim_selected_window(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(window_obj())
}

pub fn prim_frame_selected_window(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(window_obj())
}

pub fn prim_windowp(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    Ok(LispObject::from(is_window(&a)))
}

pub fn prim_window_live_p(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    Ok(LispObject::from(is_window(&a)))
}

pub fn prim_window_valid_p(args: &LispObject) -> ElispResult<LispObject> {
    prim_window_live_p(args)
}

pub fn prim_window_list(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::cons(window_obj(), LispObject::nil()))
}

pub fn prim_window_buffer(_args: &LispObject) -> ElispResult<LispObject> {
    let name = buffer::with_registry(|r| r.get(r.current_id()).map(|b| b.name.clone()));
    Ok(name
        .map(|n| LispObject::string(&n))
        .unwrap_or(LispObject::nil()))
}

pub fn prim_window_point(_args: &LispObject) -> ElispResult<LispObject> {
    let p = buffer::with_current(|b| b.point);
    Ok(LispObject::integer(p as i64))
}

fn integer_or_marker_position(value: &LispObject) -> Option<usize> {
    value
        .as_integer()
        .or_else(|| {
            crate::primitives_buffer::prim_marker_position(&LispObject::cons(
                value.clone(),
                LispObject::nil(),
            ))
            .ok()
            .and_then(|position| position.as_integer())
        })
        .map(|position| position.max(1) as usize)
}

pub fn prim_set_window_point(args: &LispObject) -> ElispResult<LispObject> {
    // (set-window-point WINDOW POS)
    let pos = args
        .nth(1)
        .and_then(|a| integer_or_marker_position(&a))
        .unwrap_or(1);
    buffer::with_current_mut(|b| b.goto_char(pos));
    Ok(LispObject::integer(pos as i64))
}

pub fn prim_set_window_buffer(args: &LispObject) -> ElispResult<LispObject> {
    // (set-window-buffer WINDOW BUFFER)
    let buf = args.nth(1).unwrap_or(LispObject::nil());
    let id = match &buf {
        LispObject::String(s) => buffer::with_registry_mut(|r| r.create(s)),
        LispObject::Symbol(id) => {
            let name = crate::obarray::symbol_name(*id);
            buffer::with_registry_mut(|r| r.create(&name))
        }
        _ => return Ok(LispObject::nil()),
    };
    // Replace the current buffer in place — push would leak a frame.
    buffer::with_registry_mut(|r| r.set_current(id));
    Ok(LispObject::nil())
}

pub fn prim_window_start(_args: &LispObject) -> ElispResult<LispObject> {
    let p = current_window_start();
    Ok(LispObject::integer(p as i64))
}

pub fn prim_window_end(_args: &LispObject) -> ElispResult<LispObject> {
    let p = buffer::with_current(|b| b.point_max());
    Ok(LispObject::integer(p as i64))
}

pub fn prim_window_total_height(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(FRAME_LINES))
}

pub fn prim_window_total_width(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(FRAME_COLUMNS))
}

pub fn prim_window_body_height(args: &LispObject) -> ElispResult<LispObject> {
    prim_window_total_height(args)
}

pub fn prim_window_body_width(args: &LispObject) -> ElispResult<LispObject> {
    prim_window_total_width(args)
}

pub fn prim_window_width(args: &LispObject) -> ElispResult<LispObject> {
    prim_window_total_width(args)
}

pub fn prim_window_height(args: &LispObject) -> ElispResult<LispObject> {
    prim_window_total_height(args)
}

pub fn prim_window_parent(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_window_child(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_window_parameter(args: &LispObject) -> ElispResult<LispObject> {
    let name = args.nth(1).and_then(|a| a.as_symbol());
    if let Some(name) = name {
        let value = WINDOW_PARAMETERS.with(|params| params.borrow().get(&name).cloned());
        Ok(value.unwrap_or_else(LispObject::nil))
    } else {
        Ok(LispObject::nil())
    }
}

pub fn prim_window_parameters(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(window_parameter_alist())
}

pub fn prim_set_window_parameter(args: &LispObject) -> ElispResult<LispObject> {
    let name = args.nth(1).and_then(|a| a.as_symbol());
    let value = args.nth(2).unwrap_or_else(LispObject::nil);
    if let Some(name) = name {
        WINDOW_PARAMETERS.with(|params| {
            params.borrow_mut().insert(name, value.clone());
        });
    }
    Ok(value)
}

pub fn prim_window_dedicated_p(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(WINDOW_DEDICATED.with(|dedicated| dedicated.borrow().clone()))
}

pub fn prim_set_window_dedicated_p(args: &LispObject) -> ElispResult<LispObject> {
    let dedicated = args.nth(1).unwrap_or_else(LispObject::nil);
    WINDOW_DEDICATED.with(|state| *state.borrow_mut() = dedicated.clone());
    Ok(dedicated)
}

pub fn prim_window_prev_buffers(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_window_next_buffers(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_window_normal_size(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::float(1.0))
}

pub fn prim_window_resizable(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_window_combination_limit(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_window_new_total(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(FRAME_LINES))
}

pub fn prim_window_new_pixel(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(frame_pixel_height()))
}

pub fn prim_window_new_normal(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::float(1.0))
}

pub fn prim_window_old_point(_args: &LispObject) -> ElispResult<LispObject> {
    let p = buffer::with_current(|b| b.point);
    Ok(LispObject::integer(p as i64))
}

pub fn prim_window_old_pixel_height(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(frame_pixel_height()))
}

pub fn prim_window_text_width(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(FRAME_COLUMNS))
}

pub fn prim_window_text_height(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(FRAME_LINES))
}

pub fn prim_window_pixel_width(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(frame_pixel_width()))
}

pub fn prim_window_pixel_height(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(frame_pixel_height()))
}

pub fn prim_window_total_size(args: &LispObject) -> ElispResult<LispObject> {
    if args.nth(1).is_some_and(|a| !a.is_nil()) {
        Ok(LispObject::integer(FRAME_COLUMNS))
    } else {
        Ok(LispObject::integer(FRAME_LINES))
    }
}

pub fn prim_window_body_size(args: &LispObject) -> ElispResult<LispObject> {
    prim_window_total_size(args)
}

pub fn prim_window_left_column(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(0))
}

pub fn prim_window_top_line(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(0))
}

pub fn prim_window_zero(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(0))
}

pub fn prim_window_fringes(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(lisp_list(&[
        LispObject::integer(0),
        LispObject::integer(0),
        LispObject::nil(),
        LispObject::nil(),
    ]))
}

pub fn prim_window_margins(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::cons(LispObject::nil(), LispObject::nil()))
}

pub fn prim_window_font_height(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(FRAME_CHAR_HEIGHT))
}

pub fn prim_window_font_width(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(FRAME_CHAR_WIDTH))
}

pub fn prim_window_max_chars_per_line(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(FRAME_COLUMNS))
}

pub fn prim_window_screen_lines(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(FRAME_LINES))
}

pub fn prim_window_edges(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(integer_list(&[0, 0, FRAME_COLUMNS, FRAME_LINES]))
}

pub fn prim_window_pixel_edges(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(integer_list(&[
        0,
        0,
        frame_pixel_width(),
        frame_pixel_height(),
    ]))
}

pub fn prim_window_absolute_pixel_position(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(integer_pair(0, 0))
}

pub fn prim_window_text_pixel_size(args: &LispObject) -> ElispResult<LispObject> {
    let from = args.nth(1).and_then(|a| a.as_integer());
    let to = args.nth(2).and_then(|a| a.as_integer());
    let (width, height) = buffer::with_current(|b| {
        let start = from
            .map(|pos| pos.max(1) as usize)
            .unwrap_or_else(|| b.point_min());
        let end = to
            .map(|pos| pos.max(1) as usize)
            .unwrap_or_else(|| b.point_max());
        let text = b.substring(start, end);
        if text.is_empty() {
            return (0, 0);
        }
        let line_count = text.split('\n').count() as i64;
        let max_cols = text
            .split('\n')
            .map(|line| line.chars().count() as i64)
            .max()
            .unwrap_or(0);
        (max_cols * FRAME_CHAR_WIDTH, line_count * FRAME_CHAR_HEIGHT)
    });
    let width = args
        .nth(3)
        .and_then(|a| a.as_integer())
        .map_or(width, |limit| width.min(clamp_nonnegative(limit)));
    let height = args
        .nth(4)
        .and_then(|a| a.as_integer())
        .map_or(height, |limit| height.min(clamp_nonnegative(limit)));
    Ok(integer_pair(width, height))
}

pub fn prim_walk_windows(_args: &LispObject) -> ElispResult<LispObject> {
    // Single-window runtime: walking is a no-op.
    Ok(LispObject::nil())
}

pub fn prim_split_window(_args: &LispObject) -> ElispResult<LispObject> {
    // Can't split; return the same window. This matches the "no
    // further splitting available" response real Emacs gives when
    // split-size too small.
    Ok(window_obj())
}

pub fn prim_delete_window(_args: &LispObject) -> ElispResult<LispObject> {
    // Can't delete the only window.
    Ok(LispObject::nil())
}

pub fn prim_delete_other_windows(_args: &LispObject) -> ElispResult<LispObject> {
    // Already the only window.
    Ok(LispObject::nil())
}

pub fn prim_delete_other_windows_vertically(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_other_window(_args: &LispObject) -> ElispResult<LispObject> {
    // No other window to switch to.
    Ok(LispObject::nil())
}

pub fn prim_select_window(args: &LispObject) -> ElispResult<LispObject> {
    let window = args.first().unwrap_or_else(window_obj);
    if is_window(&window) {
        Ok(window)
    } else {
        Ok(window_obj())
    }
}

pub fn prim_get_buffer_window(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let id = match &a {
        LispObject::Nil => Some(buffer::with_registry(|r| r.current_id())),
        LispObject::String(n) => buffer::with_registry(|r| r.lookup_by_name(n)),
        _ => None,
    };
    let cur = buffer::with_registry(|r| r.current_id());
    if id == Some(cur) {
        Ok(window_obj())
    } else {
        Ok(LispObject::nil())
    }
}

pub fn prim_display_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    if let Some(id) = match &a {
        LispObject::String(n) => buffer::with_registry(|r| r.lookup_by_name(n)),
        _ => None,
    } {
        buffer::with_registry_mut(|r| r.set_current(id));
    }
    Ok(window_obj())
}

pub fn prim_pop_to_buffer(args: &LispObject) -> ElispResult<LispObject> {
    prim_display_buffer(args)
}

pub fn prim_switch_to_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let id = match &a {
        LispObject::String(n) => Some(buffer::with_registry_mut(|r| r.create(n))),
        LispObject::Symbol(sym) => {
            let n = crate::obarray::symbol_name(*sym);
            Some(buffer::with_registry_mut(|r| r.create(&n)))
        }
        LispObject::Cons(_) => crate::primitives_buffer::buffer_object_id(&a)
            .filter(|id| buffer::with_registry(|r| r.get(*id).is_some())),
        _ => None,
    };
    if let Some(id) = id {
        buffer::with_registry_mut(|r| r.set_current(id));
    }
    let id = buffer::with_registry(|r| r.current_id());
    Ok(crate::primitives_buffer::make_buffer_object(id))
}

pub fn prim_set_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let id = match &a {
        LispObject::String(n) => buffer::with_registry(|r| r.lookup_by_name(n)),
        LispObject::Symbol(sym) => {
            let n = crate::obarray::symbol_name(*sym);
            buffer::with_registry(|r| r.lookup_by_name(&n))
        }
        LispObject::Cons(_) => crate::primitives_buffer::buffer_object_id(&a)
            .filter(|id| buffer::with_registry(|r| r.get(*id).is_some())),
        _ => None,
    };
    if let Some(id) = id {
        buffer::with_registry_mut(|r| r.set_current(id));
        Ok(crate::primitives_buffer::make_buffer_object(id))
    } else {
        Ok(LispObject::nil())
    }
}

// ---- Window configurations -----------------------------------------

pub fn prim_current_window_configuration(_args: &LispObject) -> ElispResult<LispObject> {
    let (cur, points) = buffer::with_registry(|r| {
        let cur = r.current_id();
        let pts: HashMap<BufferId, usize> =
            r.buffers.iter().map(|(&id, b)| (id, b.point)).collect();
        (cur, pts)
    });
    let id = NEXT_WC_ID.with(|c| {
        let n = c.get();
        c.set(n + 1);
        n
    });
    WINDOW_CONFIGS.with(|cfgs| {
        cfgs.borrow_mut().insert(
            id,
            WindowConfig {
                id,
                current_buffer: cur,
                points,
            },
        );
    });
    Ok(wc_obj(id))
}

pub fn prim_set_window_configuration(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let id =
        wc_id(&a).ok_or_else(|| ElispError::WrongTypeArgument("window-configuration".into()))?;
    let cfg = WINDOW_CONFIGS.with(|cfgs| cfgs.borrow().get(&id).cloned());
    if let Some(cfg) = cfg {
        buffer::with_registry_mut(|r| {
            for (bid, pt) in &cfg.points {
                if let Some(buf) = r.get_mut(*bid) {
                    buf.point = *pt;
                }
            }
            // Restore current-buffer by REPLACING the current frame,
            // not pushing — we're restoring state, not wrapping it.
            r.set_current(cfg.current_buffer);
        });
    }
    Ok(LispObject::t())
}

pub fn prim_window_configuration_p(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    Ok(LispObject::from(wc_id(&a).is_some()))
}

pub fn prim_save_window_excursion(_args: &LispObject) -> ElispResult<LispObject> {
    // Implemented as a macro in eval dispatch usually. Returning nil
    // here just avoids "void function" if anything ever funcalls it.
    Ok(LispObject::nil())
}

pub fn prim_set_window_start(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .nth(1)
        .and_then(|a| integer_or_marker_position(&a))
        .unwrap_or_else(|| buffer::with_current(|b| b.point_min()));
    WINDOW_START.with(|start| *start.borrow_mut() = Some(pos));
    Ok(LispObject::integer(pos as i64))
}

pub fn prim_set_window_noop_value(args: &LispObject) -> ElispResult<LispObject> {
    Ok(args.nth(1).unwrap_or_else(LispObject::nil))
}

pub fn prim_pos_visible_in_window_p(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|a| a.as_integer())
        .map(|pos| pos.max(1) as usize)
        .unwrap_or_else(|| buffer::with_current(|b| b.point));
    let visible = buffer::with_current(|b| {
        let start = current_window_start_for(b.point_min());
        pos >= start && pos <= b.point_max()
    });
    Ok(LispObject::from(visible))
}

pub fn prim_coordinates_in_window_p(args: &LispObject) -> ElispResult<LispObject> {
    let coords = args.first().unwrap_or_else(LispObject::nil);
    let Some((x, y_obj)) = coords.destructure_cons() else {
        return Ok(LispObject::nil());
    };
    let Some(x) = x.as_integer() else {
        return Ok(LispObject::nil());
    };
    let Some(y) = y_obj
        .as_integer()
        .or_else(|| y_obj.first().and_then(|item| item.as_integer()))
    else {
        return Ok(LispObject::nil());
    };
    let inside = (0..FRAME_COLUMNS).contains(&x) && (0..FRAME_LINES).contains(&y);
    Ok(LispObject::from(inside))
}

// ---- Frames ----------------------------------------------------------

pub fn prim_framep(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    Ok(LispObject::from(is_frame(&a)))
}

pub fn prim_frame_live_p(args: &LispObject) -> ElispResult<LispObject> {
    let frame = args.first().unwrap_or_else(LispObject::nil);
    Ok(LispObject::from(frame.is_nil() || is_frame(&frame)))
}

pub fn prim_frame_visible_p(args: &LispObject) -> ElispResult<LispObject> {
    prim_frame_live_p(args)
}

pub fn prim_selected_frame(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(frame_obj())
}

pub fn prim_frame_list(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::cons(frame_obj(), LispObject::nil()))
}

pub fn prim_frame_parameter(args: &LispObject) -> ElispResult<LispObject> {
    let name = args.nth(1).and_then(|a| a.as_symbol());
    match name.as_deref() {
        Some("name") => Ok(LispObject::string("rele")),
        Some("height") => Ok(LispObject::integer(FRAME_LINES)),
        Some("width") => Ok(LispObject::integer(FRAME_COLUMNS)),
        Some("window-system") => Ok(LispObject::nil()),
        _ => Ok(LispObject::nil()),
    }
}

pub fn prim_frame_parameters(_args: &LispObject) -> ElispResult<LispObject> {
    // (cons 'height (cons 24 ...)) style alist.
    let list = LispObject::cons(
        LispObject::cons(LispObject::symbol("name"), LispObject::string("rele")),
        LispObject::cons(
            LispObject::cons(
                LispObject::symbol("height"),
                LispObject::integer(FRAME_LINES),
            ),
            LispObject::cons(
                LispObject::cons(
                    LispObject::symbol("width"),
                    LispObject::integer(FRAME_COLUMNS),
                ),
                LispObject::nil(),
            ),
        ),
    );
    Ok(list)
}

pub fn prim_make_frame(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(frame_obj())
}

pub fn prim_delete_frame(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_select_frame(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(frame_obj())
}

pub fn prim_frame_visibility_noop(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_window_system(_args: &LispObject) -> ElispResult<LispObject> {
    // Headless — return nil ("no windowing system").
    Ok(LispObject::nil())
}

pub fn prim_window_frame(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(frame_obj())
}

pub fn prim_display_predicate(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_redisplay(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_frame_or_buffer_changed_p(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::t())
}

pub fn prim_frame_pixel_width(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(frame_pixel_width()))
}

pub fn prim_frame_pixel_height(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(frame_pixel_height()))
}

pub fn prim_frame_width(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(FRAME_COLUMNS))
}

pub fn prim_frame_height(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(FRAME_LINES))
}

pub fn prim_frame_char_width(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(FRAME_CHAR_WIDTH))
}

pub fn prim_frame_char_height(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(FRAME_CHAR_HEIGHT))
}

pub fn prim_frame_zero(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(0))
}

pub fn prim_frame_position(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(integer_pair(0, 0))
}

pub fn prim_frame_root_window(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(window_obj())
}

pub fn prim_frame_focus(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_frame_edges(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(integer_list(&[0, 0, FRAME_COLUMNS, FRAME_LINES]))
}

pub fn prim_frame_font(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::string("rele-headless"))
}

pub fn prim_frame_terminal(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(terminal_obj())
}

pub fn prim_frame_set_noop(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_x_display_pixel_width(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(frame_pixel_width()))
}

pub fn prim_x_display_pixel_height(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(frame_pixel_height()))
}

pub fn prim_x_display_zero(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(0))
}

pub fn prim_x_display_visual_class(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_x_display_list(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_x_display_name(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

// ---- Keymaps ---------------------------------------------------------

fn keymap_cons(parent: Option<LispObject>) -> LispObject {
    // (keymap [PARENT]) — empty keymap.
    match parent {
        Some(p) => LispObject::cons(
            LispObject::symbol("keymap"),
            LispObject::cons(p, LispObject::nil()),
        ),
        None => LispObject::cons(LispObject::symbol("keymap"), LispObject::nil()),
    }
}

fn is_keymap(obj: &LispObject) -> bool {
    obj.destructure_cons()
        .map(|(car, _)| car.as_symbol().as_deref() == Some("keymap"))
        .unwrap_or(false)
}

pub fn prim_keymapp(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    Ok(LispObject::from(is_keymap(&a)))
}

pub fn prim_make_keymap(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(keymap_cons(None))
}

pub fn prim_make_sparse_keymap(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(keymap_cons(None))
}

pub fn prim_copy_keymap(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    // Shallow copy — good enough since we don't dispatch keys.
    Ok(a)
}

pub fn prim_define_key(args: &LispObject) -> ElispResult<LispObject> {
    // (define-key KEYMAP KEY DEF) — We accept but don't wire into
    // input dispatch. Returning DEF keeps chained define-keys working.
    Ok(args.nth(2).unwrap_or(LispObject::nil()))
}

pub fn prim_global_set_key(args: &LispObject) -> ElispResult<LispObject> {
    // Two paths reach `(global-set-key)`: the stateful core
    // `call_primitive` dispatcher (handled by
    // `core::keymaps::prim_set_current_map_key`, which writes to the
    // table below) and this one for direct callers. Mirror the same
    // table write so either entry point honours user bindings.
    let key = args.first().unwrap_or(LispObject::nil());
    let cmd = args.nth(1).unwrap_or(LispObject::nil());
    if let (LispObject::String(key_str), Some(cmd_name)) = (&key, cmd.as_symbol()) {
        record_global_keybinding(key_str.clone(), cmd_name);
    }
    Ok(cmd)
}

/// Insert `(key, command-name)` into the global keybinding table.
/// Both `prim_global_set_key` paths funnel through this so the
/// table stays the single source of truth.
pub fn record_global_keybinding(key: String, command: String) {
    if let Ok(mut t) = GLOBAL_KEYBINDINGS.lock() {
        t.insert(key, command);
    }
}

/// Look up the command name bound to `key` by `(global-set-key
/// KEY 'command)`. Used by client key handlers (TUI / GPUI) to
/// honour user bindings before falling back to the hard-coded
/// dispatch table.
#[must_use]
pub fn lookup_global_key(key: &str) -> Option<String> {
    GLOBAL_KEYBINDINGS.lock().ok()?.get(key).cloned()
}

/// Clear all `global-set-key` bindings. Tests use this so they
/// don't leak state into other tests sharing the same process.
pub fn clear_global_keybindings() {
    if let Ok(mut t) = GLOBAL_KEYBINDINGS.lock() {
        t.clear();
    }
}

static GLOBAL_KEYBINDINGS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, String>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

pub fn prim_local_set_key(args: &LispObject) -> ElispResult<LispObject> {
    Ok(args.nth(1).unwrap_or(LispObject::nil()))
}

pub fn prim_use_local_map(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    CURRENT_LOCAL_MAP.with(|m| *m.borrow_mut() = Some(a));
    Ok(LispObject::nil())
}

pub fn prim_use_global_map(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    CURRENT_GLOBAL_MAP.with(|m| *m.borrow_mut() = Some(a));
    Ok(LispObject::nil())
}

pub fn prim_current_local_map(_args: &LispObject) -> ElispResult<LispObject> {
    let m = CURRENT_LOCAL_MAP.with(|m| m.borrow().clone());
    Ok(m.unwrap_or(LispObject::nil()))
}

pub fn prim_current_global_map(_args: &LispObject) -> ElispResult<LispObject> {
    let m = CURRENT_GLOBAL_MAP.with(|m| m.borrow().clone());
    Ok(m.unwrap_or_else(|| keymap_cons(None)))
}

pub fn prim_lookup_key(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_where_is_internal(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_define_keymap(args: &LispObject) -> ElispResult<LispObject> {
    // Emacs 29+ `define-keymap`. Just return an empty keymap.
    let _ = args;
    Ok(keymap_cons(None))
}

pub fn prim_defvar_keymap(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args;
    Ok(keymap_cons(None))
}

pub fn prim_set_keymap_parent(args: &LispObject) -> ElispResult<LispObject> {
    // (set-keymap-parent MAP PARENT) — ignore.
    Ok(args.nth(1).unwrap_or(LispObject::nil()))
}

pub fn prim_keymap_parent(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_keymap_set(args: &LispObject) -> ElispResult<LispObject> {
    Ok(args.nth(2).unwrap_or(LispObject::nil()))
}

pub fn prim_keymap_lookup(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_key_binding(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_minor_mode_key_binding(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

// ---- P4 window query primitives -----------------------------------------------

pub fn prim_window_minibuffer_p(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_minibuffer_window(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(window_obj())
}

pub fn prim_active_minibuffer_window(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_last_nonminibuffer_frame(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(frame_obj())
}

pub fn prim_frame_initial_p(args: &LispObject) -> ElispResult<LispObject> {
    let Some(a) = args.first() else {
        return Ok(LispObject::t());
    };
    Ok(LispObject::from(is_frame(&a) || is_terminal(&a)))
}

pub fn prim_frame_internal_border_width(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(0))
}

pub fn prim_set_frame_width(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_terminal_coding_system(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::symbol("utf-8"))
}

pub fn prim_terminal_list(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::cons(terminal_obj(), LispObject::nil()))
}

pub fn prim_terminal_live_p(args: &LispObject) -> ElispResult<LispObject> {
    let Some(terminal) = args.first() else {
        return Ok(LispObject::t());
    };
    Ok(LispObject::from(
        terminal.is_nil() || is_terminal(&terminal) || is_frame(&terminal),
    ))
}

pub fn prim_terminal_name(args: &LispObject) -> ElispResult<LispObject> {
    let Some(terminal) = args.first() else {
        return Ok(LispObject::string("initial_terminal"));
    };
    if is_terminal(&terminal) || is_frame(&terminal) {
        Ok(LispObject::string("initial_terminal"))
    } else {
        Ok(LispObject::nil())
    }
}

pub fn prim_tty_top_frame(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(frame_obj())
}

pub fn prim_accessible_keymaps(args: &LispObject) -> ElispResult<LispObject> {
    let keymap = match args.first() {
        Some(k) => k.clone(),
        None => keymap_cons(None),
    };
    let entry = LispObject::cons(LispObject::string(""), keymap);
    Ok(LispObject::cons(entry, LispObject::nil()))
}

pub fn prim_color_values_from_color_spec(args: &LispObject) -> ElispResult<LispObject> {
    let spec = args.first().unwrap_or(LispObject::nil());
    match spec {
        LispObject::String(s) => {
            if !s.starts_with('#') || s.len() != 7 {
                return Ok(LispObject::nil());
            }
            match u32::from_str_radix(&s[1..], 16) {
                Ok(rgb) => {
                    let r = (((rgb >> 16) & 0xFF) as i64) * 257;
                    let g = (((rgb >> 8) & 0xFF) as i64) * 257;
                    let b = ((rgb & 0xFF) as i64) * 257;
                    Ok(LispObject::cons(
                        LispObject::integer(r),
                        LispObject::cons(
                            LispObject::integer(g),
                            LispObject::cons(LispObject::integer(b), LispObject::nil()),
                        ),
                    ))
                }
                Err(_) => Ok(LispObject::nil()),
            }
        }
        _ => Ok(LispObject::nil()),
    }
}

pub fn prim_color_blend(args: &LispObject) -> ElispResult<LispObject> {
    let c1 = args
        .nth(0)
        .and_then(|a| a.as_string().cloned())
        .unwrap_or_default();
    let c2 = args
        .nth(1)
        .and_then(|a| a.as_string().cloned())
        .unwrap_or_default();
    let alpha = match args.nth(2).and_then(|a| a.as_float()) {
        Some(f) => f.clamp(0.0, 1.0),
        None => 0.5,
    };
    let parse = |s: &str| -> Option<(u8, u8, u8)> {
        if !s.starts_with('#') || s.len() != 7 {
            return None;
        }
        u32::from_str_radix(&s[1..], 16).ok().map(|rgb| {
            (
                ((rgb >> 16) & 0xFF) as u8,
                ((rgb >> 8) & 0xFF) as u8,
                (rgb & 0xFF) as u8,
            )
        })
    };
    match (parse(&c1), parse(&c2)) {
        (Some((r1, g1, b1)), Some((r2, g2, b2))) => {
            let ia = 1.0 - alpha;
            let r = ((r1 as f64 * ia + r2 as f64 * alpha) as u32) & 0xFF;
            let g = ((g1 as f64 * ia + g2 as f64 * alpha) as u32) & 0xFF;
            let b = ((b1 as f64 * ia + b2 as f64 * alpha) as u32) & 0xFF;
            Ok(LispObject::string(&format!(
                "#{:06x}",
                (r << 16) | (g << 8) | b
            )))
        }
        _ => Ok(LispObject::nil()),
    }
}

pub fn prim_color_name_to_rgb(args: &LispObject) -> ElispResult<LispObject> {
    match args.first().and_then(|a| a.as_string().cloned()) {
        Some(name)
            if name.starts_with('#')
                && name.len() == 7
                && u32::from_str_radix(&name[1..], 16).is_ok() =>
        {
            Ok(LispObject::string(&name))
        }
        _ => Ok(LispObject::nil()),
    }
}

pub fn prim_color_distance(args: &LispObject) -> ElispResult<LispObject> {
    let c1 = args
        .nth(0)
        .and_then(|a| a.as_string().cloned())
        .unwrap_or_default();
    let c2 = args
        .nth(1)
        .and_then(|a| a.as_string().cloned())
        .unwrap_or_default();
    let parse = |s: &str| -> Option<(f64, f64, f64)> {
        if !s.starts_with('#') || s.len() != 7 {
            return None;
        }
        u32::from_str_radix(&s[1..], 16).ok().map(|rgb| {
            (
                ((rgb >> 16) & 0xFF) as f64,
                ((rgb >> 8) & 0xFF) as f64,
                (rgb & 0xFF) as f64,
            )
        })
    };
    match (parse(&c1), parse(&c2)) {
        (Some((r1, g1, b1)), Some((r2, g2, b2))) => {
            let dist = ((r1 - r2).powi(2) + (g1 - g2).powi(2) + (b1 - b2).powi(2)).sqrt();
            Ok(LispObject::float(dist))
        }
        _ => Ok(LispObject::nil()),
    }
}

fn plist_get(plist: &LispObject, key: &str) -> LispObject {
    let mut cur = plist.clone();
    while let Some((car, rest)) = cur.destructure_cons() {
        if car.as_symbol().as_deref() == Some(key) {
            return rest.first().unwrap_or_else(LispObject::nil);
        }
        cur = rest.cdr().unwrap_or_else(LispObject::nil);
    }
    LispObject::nil()
}

fn plist_from_pairs(pairs: Vec<(&str, LispObject)>) -> LispObject {
    let mut out = LispObject::nil();
    for (key, value) in pairs.into_iter().rev() {
        out = LispObject::cons(LispObject::symbol(key), LispObject::cons(value, out));
    }
    out
}

fn symbol_or_nil(name: Option<String>) -> LispObject {
    name.map(|name| LispObject::symbol(&name))
        .unwrap_or_else(LispObject::nil)
}

fn parse_number_token(token: &str) -> Option<f64> {
    (!token.is_empty() && token.chars().all(|c| c.is_ascii_digit()))
        .then(|| token.parse::<f64>().ok())
        .flatten()
}

fn split_hyphen_size(name: &str) -> Option<(String, f64)> {
    let (family, size) = name.rsplit_once('-')?;
    parse_number_token(size).map(|size| (family.to_string(), size))
}

#[derive(Default)]
struct FontParts {
    family: Option<String>,
    foundry: Option<String>,
    size: Option<f64>,
    weight: Option<String>,
    slant: Option<String>,
    spacing: Option<i64>,
}

fn apply_font_style(parts: &mut FontParts, token: &str, overwrite_weight: bool) -> bool {
    let lower = token.to_ascii_lowercase();
    match lower.as_str() {
        "thin" | "ultra-light" | "light" | "book" | "medium" | "demibold" | "demi-bold"
        | "semi-bold" | "bold" | "black" | "normal" => {
            if overwrite_weight || parts.weight.is_none() {
                parts.weight = Some(lower);
            }
            true
        }
        "italic" | "oblique" | "roman" => {
            parts.slant = Some(lower);
            true
        }
        "mono" => {
            parts.spacing = Some(100);
            true
        }
        "proportional" => {
            parts.spacing = Some(0);
            true
        }
        "condensed" | "semi-condensed" | "expanded" => true,
        _ => false,
    }
}

fn parse_fontconfig_name(name: &str) -> FontParts {
    let (base, rest) = name.split_once(':').unwrap_or((name, ""));
    let mut parts = FontParts::default();
    if let Some((family, size)) = split_hyphen_size(base) {
        if !family.is_empty() {
            parts.family = Some(family);
        }
        parts.size = Some(size);
    } else if let Some(size) = parse_number_token(base) {
        parts.size = Some(size);
    } else if !base.is_empty() {
        parts.family = Some(base.to_string());
    }

    for token in rest.split(':').filter(|token| !token.is_empty()) {
        if let Some((key, value)) = token.split_once('=') {
            match key {
                "weight" => parts.weight = Some(value.to_ascii_lowercase()),
                "slant" => parts.slant = Some(value.to_ascii_lowercase()),
                "spacing" => {
                    if let Ok(value) = value.parse::<i64>() {
                        parts.spacing = Some(value);
                    }
                }
                _ => {}
            }
        } else {
            apply_font_style(&mut parts, token, true);
        }
    }
    parts
}

fn parse_gtk_name(name: &str) -> FontParts {
    let mut parts = FontParts::default();
    let mut tokens: Vec<&str> = name.split_whitespace().collect();
    if tokens.is_empty() {
        parts.family = Some(name.to_string());
        return parts;
    }
    if let Some(size) = tokens.last().and_then(|token| parse_number_token(token)) {
        parts.size = Some(size);
        tokens.pop();
    }
    while let Some(token) = tokens.last().copied() {
        if apply_font_style(&mut parts, token, false) {
            tokens.pop();
        } else {
            break;
        }
    }
    if !tokens.is_empty() {
        parts.family = Some(tokens.join(" "));
    }
    parts
}

fn parse_xlfd_name(name: &str) -> FontParts {
    let mut parts = FontParts::default();
    let fields: Vec<&str> = name.split('-').collect();
    if fields.len() < 15 {
        return parts;
    }
    parts.foundry = fields.get(1).map(|value| (*value).to_string());
    let first_trailing = fields.len().saturating_sub(12);
    if first_trailing > 2 {
        parts.family = Some(fields[2..first_trailing].join("-"));
    }
    parts.weight = fields
        .get(first_trailing)
        .map(|value| value.to_ascii_lowercase());
    parts.slant = fields
        .get(first_trailing + 1)
        .map(|value| value.to_ascii_lowercase());
    parts
}

fn parse_font_name(name: &str) -> FontParts {
    if name.starts_with('-') {
        return parse_xlfd_name(name);
    }
    if name.contains(':') {
        return parse_fontconfig_name(name);
    }
    if name.trim() != name {
        return FontParts {
            family: Some(name.to_string()),
            ..FontParts::default()
        };
    }
    if let Some((family, size)) = split_hyphen_size(name) {
        return FontParts {
            family: (!family.is_empty()).then(|| family.to_string()),
            size: Some(size),
            ..FontParts::default()
        };
    }
    parse_gtk_name(name)
}

fn font_parts_to_plist(name: Option<String>, parts: FontParts) -> LispObject {
    let mut pairs = vec![
        (":family", symbol_or_nil(parts.family)),
        (
            ":size",
            parts
                .size
                .map(LispObject::float)
                .unwrap_or_else(LispObject::nil),
        ),
        (":weight", symbol_or_nil(parts.weight)),
        (":slant", symbol_or_nil(parts.slant)),
        (
            ":spacing",
            parts
                .spacing
                .map(LispObject::integer)
                .unwrap_or_else(LispObject::nil),
        ),
        (":foundry", symbol_or_nil(parts.foundry)),
    ];
    if let Some(name) = name {
        pairs.push((":name", LispObject::string(&name)));
    }
    plist_from_pairs(pairs)
}

pub fn prim_font_spec(args: &LispObject) -> ElispResult<LispObject> {
    let mut name = None;
    let mut cur = args.clone();
    while let Some((key, rest)) = cur.destructure_cons() {
        if key.as_symbol().as_deref() == Some(":name") {
            name = rest.first().and_then(|value| value.as_string().cloned());
        }
        cur = rest.cdr().unwrap_or_else(LispObject::nil);
    }
    let parts = name
        .as_deref()
        .map(parse_font_name)
        .unwrap_or_else(FontParts::default);
    Ok(font_parts_to_plist(name, parts))
}

pub fn prim_font_get(args: &LispObject) -> ElispResult<LispObject> {
    let font = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args
        .nth(1)
        .and_then(|value| value.as_symbol())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
    Ok(plist_get(&font, &prop))
}

// ---- Dispatch ---------------------------------------------------------

pub fn call_window_primitive(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    Some(match name {
        "selected-window" => prim_selected_window(args),
        "frame-selected-window" => prim_frame_selected_window(args),
        "windowp" => prim_windowp(args),
        "window-live-p" => prim_window_live_p(args),
        "window-valid-p" => prim_window_valid_p(args),
        "window-list" => prim_window_list(args),
        "window-buffer" => prim_window_buffer(args),
        "window-point" => prim_window_point(args),
        "set-window-point" => prim_set_window_point(args),
        "set-window-buffer" => prim_set_window_buffer(args),
        "window-start" => prim_window_start(args),
        "window-end" => prim_window_end(args),
        "window-total-height" | "window-height" => prim_window_total_height(args),
        "window-total-width" | "window-width" => prim_window_total_width(args),
        "window-body-height" => prim_window_body_height(args),
        "window-body-width" => prim_window_body_width(args),
        "window-parent" => prim_window_parent(args),
        "window-child" | "window-left-child" | "window-right-child" | "window-top-child" => {
            prim_window_child(args)
        }
        "window-prev-sibling" | "window-next-sibling" => prim_window_child(args),
        "window-parameter" => prim_window_parameter(args),
        "window-parameters" => prim_window_parameters(args),
        "set-window-parameter" => prim_set_window_parameter(args),
        "window-dedicated-p" => prim_window_dedicated_p(args),
        "set-window-dedicated-p" => prim_set_window_dedicated_p(args),
        "window-prev-buffers" => prim_window_prev_buffers(args),
        "window-next-buffers" => prim_window_next_buffers(args),
        "window-normal-size" => prim_window_normal_size(args),
        "window-resizable" | "window-resizable-p" => prim_window_resizable(args),
        "window-combination-limit" => prim_window_combination_limit(args),
        "window-new-total" => prim_window_new_total(args),
        "window-new-pixel" => prim_window_new_pixel(args),
        "window-new-normal" => prim_window_new_normal(args),
        "window-old-point" => prim_window_old_point(args),
        "window-old-pixel-height" => prim_window_old_pixel_height(args),
        "window-text-width" => prim_window_text_width(args),
        "window-text-height" => prim_window_text_height(args),
        "window-pixel-width" => prim_window_pixel_width(args),
        "window-pixel-height" => prim_window_pixel_height(args),
        "window-total-size" => prim_window_total_size(args),
        "window-body-size" => prim_window_body_size(args),
        "window-left-column" => prim_window_left_column(args),
        "window-top-line" => prim_window_top_line(args),
        "window-scroll-bar-height"
        | "window-scroll-bar-width"
        | "window-hscroll"
        | "window-vscroll" => prim_window_zero(args),
        "window-fringes" => prim_window_fringes(args),
        "window-margins" => prim_window_margins(args),
        "window-line-height" => prim_window_zero(args),
        "window-font-height" => prim_window_font_height(args),
        "window-font-width" => prim_window_font_width(args),
        "window-max-chars-per-line" => prim_window_max_chars_per_line(args),
        "window-screen-lines" => prim_window_screen_lines(args),
        "window-edges" | "window-inside-edges" => prim_window_edges(args),
        "window-pixel-edges" | "window-inside-pixel-edges" | "window-absolute-pixel-edges" => {
            prim_window_pixel_edges(args)
        }
        "window-absolute-pixel-position" => prim_window_absolute_pixel_position(args),
        "window-text-pixel-size" => prim_window_text_pixel_size(args),
        "window-prompt" => prim_window_child(args),
        "minibuffer-window" => prim_minibuffer_window(args),
        "active-minibuffer-window" => prim_active_minibuffer_window(args),
        "walk-windows" => prim_walk_windows(args),
        "split-window"
        | "split-window-below"
        | "split-window-right"
        | "split-window-horizontally"
        | "split-window-vertically" => prim_split_window(args),
        "delete-window" => prim_delete_window(args),
        "delete-other-windows" => prim_delete_other_windows(args),
        "delete-other-windows-vertically" => prim_delete_other_windows_vertically(args),
        "other-window" => prim_other_window(args),
        "select-window" => prim_select_window(args),
        "get-buffer-window" | "get-buffer-window-list" => prim_get_buffer_window(args),
        "display-buffer" => prim_display_buffer(args),
        "pop-to-buffer" | "pop-to-buffer-same-window" => prim_pop_to_buffer(args),
        "switch-to-buffer" | "switch-to-buffer-other-window" | "switch-to-buffer-other-frame" => {
            prim_switch_to_buffer(args)
        }
        "set-buffer" => prim_set_buffer(args),
        "current-window-configuration" => prim_current_window_configuration(args),
        "set-window-configuration" => prim_set_window_configuration(args),
        "window-configuration-p" => prim_window_configuration_p(args),
        "save-window-excursion" => prim_save_window_excursion(args),
        "set-window-start" => prim_set_window_start(args),
        "set-window-hscroll"
        | "set-window-vscroll"
        | "set-window-fringes"
        | "set-window-margins"
        | "set-window-scroll-bars"
        | "set-window-display-table" => prim_set_window_noop_value(args),
        "pos-visible-in-window-p" => prim_pos_visible_in_window_p(args),
        "coordinates-in-window-p" => prim_coordinates_in_window_p(args),
        "framep" => prim_framep(args),
        "frame-live-p" => prim_frame_live_p(args),
        "frame-visible-p" => prim_frame_visible_p(args),
        "selected-frame" => prim_selected_frame(args),
        "frame-list" | "visible-frame-list" => prim_frame_list(args),
        "frame-parameter" => prim_frame_parameter(args),
        "frame-parameters" | "frame-default-alist" => prim_frame_parameters(args),
        "make-frame" => prim_make_frame(args),
        "delete-frame" => prim_delete_frame(args),
        "select-frame" | "select-frame-set-input-focus" => prim_select_frame(args),
        "make-frame-visible"
        | "make-frame-invisible"
        | "iconify-frame"
        | "raise-frame"
        | "lower-frame" => prim_frame_visibility_noop(args),
        "window-system" => prim_window_system(args),
        "terminal-name" => prim_terminal_name(args),
        "window-frame" => prim_window_frame(args),
        "display-graphic-p"
        | "display-multi-frame-p"
        | "display-color-p"
        | "display-mouse-p"
        | "display-images-p"
        | "display-popup-menus-p" => prim_display_predicate(args),
        "redisplay" | "force-mode-line-update" => prim_redisplay(args),
        "frame-or-buffer-changed-p" => prim_frame_or_buffer_changed_p(args),
        "frame-pixel-width" | "frame-native-width" => prim_frame_pixel_width(args),
        "frame-pixel-height" | "frame-native-height" => prim_frame_pixel_height(args),
        "frame-width" | "frame-text-cols" => prim_frame_width(args),
        "frame-height" | "frame-total-lines" | "frame-text-lines" => prim_frame_height(args),
        "frame-char-width" => prim_frame_char_width(args),
        "frame-char-height" => prim_frame_char_height(args),
        "frame-scroll-bar-width"
        | "frame-scroll-bar-height"
        | "frame-fringe-width"
        | "frame-border-width"
        | "frame-internal-border-height"
        | "frame-internal-border"
        | "frame-tool-bar-lines"
        | "frame-menu-bar-lines" => prim_frame_zero(args),
        "frame-font" => prim_frame_font(args),
        "frame-position" => prim_frame_position(args),
        "frame-root-window" | "frame-first-window" => prim_frame_root_window(args),
        "frame-focus" => prim_frame_focus(args),
        "frame-edges" => prim_frame_edges(args),
        "frame-terminal" => prim_frame_terminal(args),
        "terminal-list" => prim_terminal_list(args),
        "terminal-live-p" => prim_terminal_live_p(args),
        "redirect-frame-focus"
        | "set-frame-size"
        | "set-frame-position"
        | "set-frame-height"
        | "set-frame-parameter"
        | "modify-frame-parameters" => prim_frame_set_noop(args),
        "x-display-pixel-width" => prim_x_display_pixel_width(args),
        "x-display-pixel-height" => prim_x_display_pixel_height(args),
        "x-display-mm-width"
        | "x-display-mm-height"
        | "x-display-color-cells"
        | "x-display-planes"
        | "x-display-screens" => prim_x_display_zero(args),
        "x-display-visual-class" => prim_x_display_visual_class(args),
        "x-display-save-under" | "x-display-backing-store" => prim_display_predicate(args),
        "x-display-list" => prim_x_display_list(args),
        "x-display-name" => prim_x_display_name(args),
        "keymapp" => prim_keymapp(args),
        "make-keymap" => prim_make_keymap(args),
        "make-sparse-keymap" => prim_make_sparse_keymap(args),
        "copy-keymap" => prim_copy_keymap(args),
        "define-key" | "keymap-set" => {
            if name == "keymap-set" {
                prim_keymap_set(args)
            } else {
                prim_define_key(args)
            }
        }
        "global-set-key" => prim_global_set_key(args),
        "local-set-key" => prim_local_set_key(args),
        "use-local-map" => prim_use_local_map(args),
        "use-global-map" => prim_use_global_map(args),
        "current-local-map" => prim_current_local_map(args),
        "current-global-map" => prim_current_global_map(args),
        "lookup-key" => prim_lookup_key(args),
        "where-is-internal" => prim_where_is_internal(args),
        "define-keymap" => prim_define_keymap(args),
        "defvar-keymap" => prim_defvar_keymap(args),
        "set-keymap-parent" => prim_set_keymap_parent(args),
        "keymap-parent" => prim_keymap_parent(args),
        "keymap-lookup" => prim_keymap_lookup(args),
        "key-binding" => prim_key_binding(args),
        "minor-mode-key-binding" => prim_minor_mode_key_binding(args),
        "window-minibuffer-p" => prim_window_minibuffer_p(args),
        "frame-internal-border-width" => prim_frame_internal_border_width(args),
        "last-nonminibuffer-frame" => prim_last_nonminibuffer_frame(args),
        "frame-initial-p" => prim_frame_initial_p(args),
        "set-frame-width" => prim_set_frame_width(args),
        "terminal-coding-system" => prim_terminal_coding_system(args),
        "tty-top-frame" => prim_tty_top_frame(args),
        "accessible-keymaps" => prim_accessible_keymaps(args),
        "color-values-from-color-spec" => prim_color_values_from_color_spec(args),
        "color-blend" => prim_color_blend(args),
        "color-name-to-rgb" => prim_color_name_to_rgb(args),
        "color-distance" => prim_color_distance(args),
        "font-spec" => prim_font_spec(args),
        "font-get" => prim_font_get(args),
        _ => return None,
    })
}

pub const WINDOW_PRIMITIVE_NAMES: &[&str] = &[
    "selected-window",
    "frame-selected-window",
    "windowp",
    "window-live-p",
    "window-valid-p",
    "window-list",
    "window-buffer",
    "window-point",
    "set-window-point",
    "set-window-buffer",
    "window-start",
    "window-end",
    "window-total-height",
    "window-height",
    "window-total-width",
    "window-width",
    "window-body-height",
    "window-body-width",
    "window-parent",
    "window-child",
    "window-left-child",
    "window-right-child",
    "window-top-child",
    "window-prev-sibling",
    "window-next-sibling",
    "window-parameter",
    "window-parameters",
    "set-window-parameter",
    "window-dedicated-p",
    "set-window-dedicated-p",
    "window-prev-buffers",
    "window-next-buffers",
    "window-normal-size",
    "window-resizable",
    "window-resizable-p",
    "window-combination-limit",
    "window-new-total",
    "window-new-pixel",
    "window-new-normal",
    "window-old-point",
    "window-old-pixel-height",
    "window-text-width",
    "window-text-height",
    "window-pixel-width",
    "window-pixel-height",
    "window-total-size",
    "window-body-size",
    "window-left-column",
    "window-top-line",
    "window-scroll-bar-height",
    "window-scroll-bar-width",
    "window-fringes",
    "window-margins",
    "window-hscroll",
    "window-vscroll",
    "window-line-height",
    "window-font-height",
    "window-font-width",
    "window-max-chars-per-line",
    "window-screen-lines",
    "window-pixel-edges",
    "window-edges",
    "window-inside-edges",
    "window-inside-pixel-edges",
    "window-absolute-pixel-edges",
    "window-absolute-pixel-position",
    "window-text-pixel-size",
    "window-prompt",
    "minibuffer-window",
    "active-minibuffer-window",
    "walk-windows",
    "split-window",
    "split-window-below",
    "split-window-right",
    "split-window-horizontally",
    "split-window-vertically",
    "delete-window",
    "delete-other-windows",
    "delete-other-windows-vertically",
    "other-window",
    "select-window",
    "get-buffer-window",
    "get-buffer-window-list",
    "display-buffer",
    "pop-to-buffer",
    "pop-to-buffer-same-window",
    "switch-to-buffer",
    "switch-to-buffer-other-window",
    "switch-to-buffer-other-frame",
    "set-buffer",
    "current-window-configuration",
    "set-window-configuration",
    "window-configuration-p",
    "save-window-excursion",
    "set-window-start",
    "set-window-hscroll",
    "set-window-vscroll",
    "set-window-fringes",
    "set-window-margins",
    "set-window-scroll-bars",
    "set-window-display-table",
    "pos-visible-in-window-p",
    "coordinates-in-window-p",
    "framep",
    "frame-live-p",
    "frame-visible-p",
    "selected-frame",
    "frame-list",
    "visible-frame-list",
    "frame-parameter",
    "frame-parameters",
    "frame-default-alist",
    "make-frame",
    "delete-frame",
    "select-frame",
    "select-frame-set-input-focus",
    "make-frame-visible",
    "make-frame-invisible",
    "iconify-frame",
    "raise-frame",
    "lower-frame",
    "window-system",
    "terminal-name",
    "window-frame",
    "display-graphic-p",
    "display-multi-frame-p",
    "display-color-p",
    "display-mouse-p",
    "display-images-p",
    "display-popup-menus-p",
    "redisplay",
    "force-mode-line-update",
    "frame-or-buffer-changed-p",
    "frame-pixel-width",
    "frame-pixel-height",
    "frame-width",
    "frame-height",
    "frame-total-lines",
    "frame-native-width",
    "frame-native-height",
    "frame-char-width",
    "frame-char-height",
    "frame-text-cols",
    "frame-text-lines",
    "frame-scroll-bar-width",
    "frame-scroll-bar-height",
    "frame-fringe-width",
    "frame-font",
    "frame-position",
    "frame-root-window",
    "frame-first-window",
    "frame-focus",
    "frame-edges",
    "frame-border-width",
    "frame-internal-border-height",
    "frame-internal-border",
    "frame-terminal",
    "terminal-list",
    "terminal-live-p",
    "frame-tool-bar-lines",
    "frame-menu-bar-lines",
    "redirect-frame-focus",
    "set-frame-size",
    "set-frame-position",
    "set-frame-height",
    "set-frame-parameter",
    "modify-frame-parameters",
    "x-display-pixel-width",
    "x-display-pixel-height",
    "x-display-mm-width",
    "x-display-mm-height",
    "x-display-color-cells",
    "x-display-planes",
    "x-display-visual-class",
    "x-display-screens",
    "x-display-save-under",
    "x-display-backing-store",
    "x-display-list",
    "x-display-name",
    "keymapp",
    "make-keymap",
    "make-sparse-keymap",
    "copy-keymap",
    "define-key",
    "keymap-set",
    "global-set-key",
    "local-set-key",
    "use-local-map",
    "use-global-map",
    "current-local-map",
    "current-global-map",
    "lookup-key",
    "where-is-internal",
    "define-keymap",
    "defvar-keymap",
    "set-keymap-parent",
    "keymap-parent",
    "keymap-lookup",
    "key-binding",
    "minor-mode-key-binding",
    "window-minibuffer-p",
    "frame-internal-border-width",
    "last-nonminibuffer-frame",
    "frame-initial-p",
    "set-frame-width",
    "terminal-coding-system",
    "tty-top-frame",
    "accessible-keymaps",
    "color-values-from-color-spec",
    "color-blend",
    "color-name-to-rgb",
    "color-distance",
    "font-spec",
    "font-get",
];
