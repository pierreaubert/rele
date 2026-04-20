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

    /// Global keymaps registry. Keyed by name (symbol name); the value
    /// is the keymap's Lisp structure (a list). We don't dispatch key
    /// events — this is purely so `define-key`, `use-local-map`,
    /// `current-local-map`, etc. can round-trip their state without
    /// "void function" errors.
    static KEYMAPS: RefCell<HashMap<String, LispObject>> = RefCell::new(HashMap::new());
    static CURRENT_LOCAL_MAP: RefCell<Option<LispObject>> = RefCell::new(None);
    static CURRENT_GLOBAL_MAP: RefCell<Option<LispObject>> = RefCell::new(None);
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
    Ok(name.map(|n| LispObject::string(&n)).unwrap_or(LispObject::nil()))
}

pub fn prim_window_point(_args: &LispObject) -> ElispResult<LispObject> {
    let p = buffer::with_current(|b| b.point);
    Ok(LispObject::integer(p as i64))
}

pub fn prim_set_window_point(args: &LispObject) -> ElispResult<LispObject> {
    // (set-window-point WINDOW POS)
    let pos = args.nth(1).and_then(|a| a.as_integer()).unwrap_or(1) as usize;
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
    let p = buffer::with_current(|b| b.point_min());
    Ok(LispObject::integer(p as i64))
}

pub fn prim_window_end(_args: &LispObject) -> ElispResult<LispObject> {
    let p = buffer::with_current(|b| b.point_max());
    Ok(LispObject::integer(p as i64))
}

pub fn prim_window_total_height(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(24))
}

pub fn prim_window_total_width(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(80))
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

pub fn prim_other_window(_args: &LispObject) -> ElispResult<LispObject> {
    // No other window to switch to.
    Ok(LispObject::nil())
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
        _ => None,
    };
    if let Some(id) = id {
        buffer::with_registry_mut(|r| r.set_current(id));
    }
    let name = buffer::with_registry(|r| r.get(r.current_id()).map(|b| b.name.clone()));
    Ok(name.map(|n| LispObject::string(&n)).unwrap_or(LispObject::nil()))
}

pub fn prim_set_buffer(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let id = match &a {
        LispObject::String(n) => buffer::with_registry(|r| r.lookup_by_name(n)),
        LispObject::Symbol(sym) => {
            let n = crate::obarray::symbol_name(*sym);
            buffer::with_registry(|r| r.lookup_by_name(&n))
        }
        _ => None,
    };
    if let Some(id) = id {
        buffer::with_registry_mut(|r| r.set_current(id));
        Ok(a)
    } else {
        Ok(LispObject::nil())
    }
}

// ---- Window configurations -----------------------------------------

pub fn prim_current_window_configuration(_args: &LispObject) -> ElispResult<LispObject> {
    let (cur, points) = buffer::with_registry(|r| {
        let cur = r.current_id();
        let pts: HashMap<BufferId, usize> = r
            .buffers
            .iter()
            .map(|(&id, b)| (id, b.point))
            .collect();
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
    let id = wc_id(&a).ok_or_else(|| ElispError::WrongTypeArgument("window-configuration".into()))?;
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

// ---- Frames ----------------------------------------------------------

pub fn prim_framep(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    Ok(LispObject::from(is_frame(&a)))
}

pub fn prim_frame_live_p(args: &LispObject) -> ElispResult<LispObject> {
    prim_framep(args)
}

pub fn prim_frame_visible_p(args: &LispObject) -> ElispResult<LispObject> {
    prim_framep(args)
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
        Some("height") => Ok(LispObject::integer(24)),
        Some("width") => Ok(LispObject::integer(80)),
        _ => Ok(LispObject::nil()),
    }
}

pub fn prim_frame_parameters(_args: &LispObject) -> ElispResult<LispObject> {
    // (cons 'height (cons 24 ...)) style alist.
    let list = LispObject::cons(
        LispObject::cons(LispObject::symbol("name"), LispObject::string("rele")),
        LispObject::cons(
            LispObject::cons(LispObject::symbol("height"), LispObject::integer(24)),
            LispObject::cons(
                LispObject::cons(LispObject::symbol("width"), LispObject::integer(80)),
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

pub fn prim_window_system(_args: &LispObject) -> ElispResult<LispObject> {
    // Headless — return nil ("no windowing system").
    Ok(LispObject::nil())
}

pub fn prim_window_frame(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(frame_obj())
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
    Ok(args.nth(1).unwrap_or(LispObject::nil()))
}

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

pub fn prim_last_nonminibuffer_frame(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(frame_obj())
}

pub fn prim_frame_initial_p(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    Ok(LispObject::from(is_frame(&a)))
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
    let c1 = args.nth(0).and_then(|a| a.as_string()).map(|s| s.clone()).unwrap_or_default();
    let c2 = args.nth(1).and_then(|a| a.as_string()).map(|s| s.clone()).unwrap_or_default();
    let alpha = match args.nth(2).and_then(|a| a.as_float()) {
        Some(&f) => f.max(0.0).min(1.0),
        None => 0.5,
    };
    let parse = |s: &str| -> Option<(u8, u8, u8)> {
        if !s.starts_with('#') || s.len() != 7 { return None; }
        u32::from_str_radix(&s[1..], 16).ok().map(|rgb| (
            ((rgb >> 16) & 0xFF) as u8,
            ((rgb >> 8) & 0xFF) as u8,
            (rgb & 0xFF) as u8,
        ))
    };
    match (parse(&c1), parse(&c2)) {
        (Some((r1, g1, b1)), Some((r2, g2, b2))) => {
            let ia = 1.0 - alpha;
            let r = ((r1 as f64 * ia + r2 as f64 * alpha) as u32) & 0xFF;
            let g = ((g1 as f64 * ia + g2 as f64 * alpha) as u32) & 0xFF;
            let b = ((b1 as f64 * ia + b2 as f64 * alpha) as u32) & 0xFF;
            Ok(LispObject::string(&format!("#{:06x}", (r << 16) | (g << 8) | b)))
        }
        _ => Ok(LispObject::nil()),
    }
}

pub fn prim_color_name_to_rgb(args: &LispObject) -> ElispResult<LispObject> {
    match args.first().and_then(|a| a.as_string()) {
        Some(name) if name.starts_with('#') && name.len() == 7 && u32::from_str_radix(&name[1..], 16).is_ok() => {
            Ok(LispObject::string(name))
        }
        _ => Ok(LispObject::nil()),
    }
}

pub fn prim_color_distance(args: &LispObject) -> ElispResult<LispObject> {
    let c1 = args.nth(0).and_then(|a| a.as_string()).map(|s| s.clone()).unwrap_or_default();
    let c2 = args.nth(1).and_then(|a| a.as_string()).map(|s| s.clone()).unwrap_or_default();
    let parse = |s: &str| -> Option<(f64, f64, f64)> {
        if !s.starts_with('#') || s.len() != 7 { return None; }
        u32::from_str_radix(&s[1..], 16).ok().map(|rgb| (
            ((rgb >> 16) & 0xFF) as f64,
            ((rgb >> 8) & 0xFF) as f64,
            (rgb & 0xFF) as f64,
        ))
    };
    match (parse(&c1), parse(&c2)) {
        (Some((r1, g1, b1)), Some((r2, g2, b2))) => {
            let dist = ((r1 - r2).powi(2) + (g1 - g2).powi(2) + (b1 - b2).powi(2)).sqrt();
            Ok(LispObject::float(dist))
        }
        _ => Ok(LispObject::nil()),
    }
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
        "window-child" => prim_window_child(args),
        "walk-windows" => prim_walk_windows(args),
        "split-window" | "split-window-below" | "split-window-right"
        | "split-window-horizontally" | "split-window-vertically" => prim_split_window(args),
        "delete-window" => prim_delete_window(args),
        "delete-other-windows" => prim_delete_other_windows(args),
        "other-window" => prim_other_window(args),
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
        "framep" => prim_framep(args),
        "frame-live-p" => prim_frame_live_p(args),
        "frame-visible-p" => prim_frame_visible_p(args),
        "selected-frame" => prim_selected_frame(args),
        "frame-list" | "visible-frame-list" => prim_frame_list(args),
        "frame-parameter" => prim_frame_parameter(args),
        "frame-parameters" | "frame-default-alist" => prim_frame_parameters(args),
        "make-frame" => prim_make_frame(args),
        "delete-frame" => prim_delete_frame(args),
        "window-system" | "terminal-name" => prim_window_system(args),
        "window-frame" => prim_window_frame(args),
        "keymapp" => prim_keymapp(args),
        "make-keymap" => prim_make_keymap(args),
        "make-sparse-keymap" => prim_make_sparse_keymap(args),
        "copy-keymap" => prim_copy_keymap(args),
        "define-key" | "keymap-set" => {
            if name == "keymap-set" { prim_keymap_set(args) } else { prim_define_key(args) }
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
    "walk-windows",
    "split-window",
    "split-window-below",
    "split-window-right",
    "split-window-horizontally",
    "split-window-vertically",
    "delete-window",
    "delete-other-windows",
    "other-window",
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
    "window-system",
    "terminal-name",
    "window-frame",
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
];
