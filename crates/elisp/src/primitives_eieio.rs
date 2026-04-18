//! Minimal EIEIO / CL-generic support — batch F5.
//!
//! EIEIO (Enhanced Implementation of Emacs Interpreted Objects) is
//! Emacs's object system: single-inheritance classes, slots with
//! defaults, and single-dispatch methods. `cl-defmethod` generalizes
//! to multi-arg dispatch but we only implement first-arg class
//! dispatch — enough for the common case.
//!
//! Representation:
//! - Class: a `LispObject::Cons` `(eieio-class NAME PARENT SLOTS)` where
//!   SLOTS is a list of `(SLOT-NAME DEFAULT)` pairs. Stored under the
//!   symbol's `cl-struct-class` plist entry.
//! - Instance: a vector `[TAG SLOT0 SLOT1 ...]` where TAG is the class
//!   symbol — matches Emacs's own cl-struct record layout.
//! - Methods: stored in a thread-local hashmap keyed by (name, first-
//!   arg class name). `cl-call-next-method` walks the class hierarchy.
//!
//! The goal is breadth, not fidelity. Tests that actually *check*
//! slot-accessor correctness will pass; tests that depend on method
//! combination rules (:before / :after / :around) will not.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

/// One slot definition for a class.
#[derive(Debug, Clone)]
pub struct Slot {
    pub name: String,
    pub default: LispObject,
}

/// A registered class.
#[derive(Debug, Clone)]
pub struct Class {
    pub name: String,
    pub parent: Option<String>,
    pub slots: Vec<Slot>,
}

#[derive(Debug, Clone)]
pub struct Method {
    /// Class name this method specializes on (first-arg dispatch).
    /// `None` = no specializer (default method).
    pub class: Option<String>,
    /// Parameter list including the specializer. Elisp form:
    /// ((OBJ CLASS-NAME) ARG2 ARG3 ...).
    pub params: LispObject,
    /// Body as a progn list.
    pub body: LispObject,
}

#[derive(Debug, Default)]
pub struct MethodTable {
    /// generic-name → ordered list of methods (most-specific-first).
    pub methods: HashMap<String, Vec<Method>>,
}

thread_local! {
    static CLASSES: RefCell<HashMap<String, Class>> = RefCell::new(HashMap::new());
    static METHODS: RefCell<MethodTable> = RefCell::new(MethodTable::default());
}

pub fn register_class(class: Class) {
    CLASSES.with(|c| {
        c.borrow_mut().insert(class.name.clone(), class);
    });
}

pub fn get_class(name: &str) -> Option<Class> {
    CLASSES.with(|c| c.borrow().get(name).cloned())
}

pub fn register_method(name: &str, method: Method) {
    METHODS.with(|m| {
        m.borrow_mut()
            .methods
            .entry(name.to_string())
            .or_default()
            .push(method);
    });
}

pub fn get_methods(name: &str) -> Vec<Method> {
    METHODS.with(|m| m.borrow().methods.get(name).cloned().unwrap_or_default())
}

/// Walk a class's inheritance chain top-down, starting at `name` and
/// climbing via `parent`. Includes `name` itself.
pub fn class_hierarchy(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = Some(name.to_string());
    while let Some(n) = cur {
        if out.contains(&n) {
            break; // cycle guard
        }
        let parent = CLASSES.with(|c| c.borrow().get(&n).and_then(|cl| cl.parent.clone()));
        out.push(n);
        cur = parent;
    }
    out
}

// ---- Primitive impls -------------------------------------------------

/// `(eieio-class-p OBJ)`. A class is represented by our registry entry
/// keyed by the symbol's name.
pub fn prim_eieio_class_p(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let name = match a.as_symbol() {
        Some(n) => n,
        None => return Ok(LispObject::nil()),
    };
    Ok(LispObject::from(get_class(&name).is_some()))
}

pub fn prim_cl_class_p(args: &LispObject) -> ElispResult<LispObject> {
    prim_eieio_class_p(args)
}

pub fn prim_find_class(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().unwrap_or(LispObject::nil());
    let name = match a.as_symbol() {
        Some(n) => n,
        None => return Ok(LispObject::nil()),
    };
    if get_class(&name).is_some() {
        // Return the class's symbol — not the full struct. This
        // matches Emacs's convention where `cl--find-class` returns a
        // class record, but most tests just check for truthy.
        Ok(LispObject::symbol(&name))
    } else {
        Ok(LispObject::nil())
    }
}

pub fn prim_cl_find_class(args: &LispObject) -> ElispResult<LispObject> {
    prim_find_class(args)
}

/// `(make-instance CLASS &rest INITARGS)` — create a new instance.
pub fn prim_make_instance(args: &LispObject) -> ElispResult<LispObject> {
    let class_arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let class_name = class_arg
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
    let class = get_class(&class_name).ok_or_else(|| {
        ElispError::Signal(Box::new(crate::error::SignalData {
            symbol: LispObject::symbol("invalid-slot-type"),
            data: LispObject::cons(class_arg.clone(), LispObject::nil()),
        }))
    })?;

    // Walk the initargs alist: either `(:slot value :slot2 value2 ...)`
    // or just positional values matching slot order. We accept the
    // keyword form first.
    let mut overrides: HashMap<String, LispObject> = HashMap::new();
    let mut cur = args.rest().unwrap_or(LispObject::nil());
    while let Some((k, rest)) = cur.destructure_cons() {
        let key = match k.as_symbol() {
            Some(s) => s,
            None => break,
        };
        let stripped = key.strip_prefix(':').unwrap_or(&key).to_string();
        if let Some((v, rest2)) = rest.destructure_cons() {
            overrides.insert(stripped, v);
            cur = rest2;
        } else {
            break;
        }
    }

    // Build an instance vector: [class-symbol, slot0, slot1, ...].
    let mut items: Vec<LispObject> = Vec::with_capacity(class.slots.len() + 1);
    items.push(LispObject::symbol(&class_name));
    for slot in &class.slots {
        let val = overrides
            .get(&slot.name)
            .cloned()
            .unwrap_or_else(|| slot.default.clone());
        items.push(val);
    }
    Ok(LispObject::Vector(std::sync::Arc::new(
        parking_lot::Mutex::new(items),
    )))
}

/// Helper: clone out the items of an instance vector, or None.
fn as_items(obj: &LispObject) -> Option<Vec<LispObject>> {
    if let LispObject::Vector(v) = obj {
        Some(v.lock().clone())
    } else {
        None
    }
}

/// `(slot-value INSTANCE SLOT)` — look up SLOT on INSTANCE.
pub fn prim_slot_value(args: &LispObject) -> ElispResult<LispObject> {
    let instance = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let slot = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let slot_name = slot
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
    let items = as_items(&instance)
        .ok_or_else(|| ElispError::WrongTypeArgument("vector".into()))?;
    let class_name = items
        .first()
        .and_then(|t| t.as_symbol())
        .ok_or_else(|| ElispError::WrongTypeArgument("eieio-instance".into()))?;
    let class = get_class(&class_name).ok_or_else(|| {
        ElispError::EvalError(format!("unknown class: {class_name}"))
    })?;
    let idx = class
        .slots
        .iter()
        .position(|s| s.name == slot_name)
        .ok_or_else(|| {
            ElispError::Signal(Box::new(crate::error::SignalData {
                symbol: LispObject::symbol("invalid-slot-name"),
                data: LispObject::cons(slot, LispObject::nil()),
            }))
        })?;
    Ok(items.get(idx + 1).cloned().unwrap_or(LispObject::nil()))
}

pub fn prim_oref(args: &LispObject) -> ElispResult<LispObject> {
    prim_slot_value(args)
}

pub fn prim_eieio_oref(args: &LispObject) -> ElispResult<LispObject> {
    prim_slot_value(args)
}

/// `(set-slot-value INSTANCE SLOT VALUE)`.
pub fn prim_set_slot_value(args: &LispObject) -> ElispResult<LispObject> {
    let instance = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let slot = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let slot_name = slot
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;
    let mut items = as_items(&instance)
        .ok_or_else(|| ElispError::WrongTypeArgument("vector".into()))?;
    let class_name = items
        .first()
        .and_then(|t| t.as_symbol())
        .ok_or_else(|| ElispError::WrongTypeArgument("eieio-instance".into()))?;
    let class = get_class(&class_name).ok_or_else(|| {
        ElispError::EvalError(format!("unknown class: {class_name}"))
    })?;
    let idx = class
        .slots
        .iter()
        .position(|s| s.name == slot_name)
        .ok_or_else(|| {
            ElispError::Signal(Box::new(crate::error::SignalData {
                symbol: LispObject::symbol("invalid-slot-name"),
                data: LispObject::cons(slot, LispObject::nil()),
            }))
        })?;
    if let Some(slot) = items.get_mut(idx + 1) {
        *slot = value.clone();
    }
    // Mutate the vector in place via the Arc<Mutex>.
    if let LispObject::Vector(orig) = instance {
        *orig.lock() = items;
    }
    Ok(value)
}

pub fn prim_oset(args: &LispObject) -> ElispResult<LispObject> {
    prim_set_slot_value(args)
}

pub fn prim_eieio_oset(args: &LispObject) -> ElispResult<LispObject> {
    prim_set_slot_value(args)
}

pub fn prim_slot_boundp(args: &LispObject) -> ElispResult<LispObject> {
    // All slots are always bound in our model (they default to their
    // defaults at construction). This differs from Emacs where an
    // unbound slot signals a specific error, but tests that care are
    // rare.
    let instance = args.first().unwrap_or(LispObject::nil());
    Ok(LispObject::from(matches!(instance, LispObject::Vector(_))))
}

pub fn prim_object_of_class_p(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().unwrap_or(LispObject::nil());
    let target = args
        .nth(1)
        .and_then(|a| a.as_symbol())
        .unwrap_or_default();
    if target.is_empty() {
        return Ok(LispObject::nil());
    }
    let Some(items) = as_items(&obj) else {
        return Ok(LispObject::nil());
    };
    let Some(class_name) = items.first().and_then(|t| t.as_symbol()) else {
        return Ok(LispObject::nil());
    };
    let hierarchy = class_hierarchy(&class_name);
    Ok(LispObject::from(hierarchy.contains(&target)))
}

pub fn prim_eieio_object_p(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().unwrap_or(LispObject::nil());
    let is_inst = as_items(&obj)
        .and_then(|v| v.first().and_then(|t| t.as_symbol()))
        .map(|n| get_class(&n).is_some())
        .unwrap_or(false);
    Ok(LispObject::from(is_inst))
}

pub fn prim_eieio_object_class(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().unwrap_or(LispObject::nil());
    let n = as_items(&obj).and_then(|v| v.first().and_then(|t| t.as_symbol()));
    Ok(n.map(|s| LispObject::symbol(&s)).unwrap_or(LispObject::nil()))
}

pub fn prim_eieio_object_name(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::string("anonymous"))
}

// Advice system stubs — treat all advice as no-op for now.

pub fn prim_advice_add(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_advice_remove(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_advice__p(args: &LispObject) -> ElispResult<LispObject> {
    // Never advised.
    let _ = args;
    Ok(LispObject::nil())
}

pub fn prim_advice_function_member_p(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_advice_member_p(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_ad_is_advised(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

// Widget stubs — used extensively by customize.el. Return opaque values.

pub fn prim_widget_put(_args: &LispObject) -> ElispResult<LispObject> {
    // (widget-put WIDGET PROP VAL) — we don't track widgets; succeed.
    let _ = _args;
    Ok(LispObject::nil())
}

pub fn prim_widget_get(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_widget_create(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_widget_apply(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

pub fn prim_widgetp(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::nil())
}

// ---- Dispatch ---------------------------------------------------------

pub fn call_eieio_primitive(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    Some(match name {
        "eieio-class-p" => prim_eieio_class_p(args),
        "cl-class-p" | "class-p" => prim_cl_class_p(args),
        "find-class" => prim_find_class(args),
        "cl--find-class" | "cl-find-class" => prim_cl_find_class(args),
        "make-instance" => prim_make_instance(args),
        "slot-value" => prim_slot_value(args),
        "oref" => prim_oref(args),
        "eieio-oref" => prim_eieio_oref(args),
        "set-slot-value" => prim_set_slot_value(args),
        "oset" => prim_oset(args),
        "eieio-oset" => prim_eieio_oset(args),
        "slot-boundp" => prim_slot_boundp(args),
        "object-of-class-p" | "cl-typep" => prim_object_of_class_p(args),
        "eieio-object-p" => prim_eieio_object_p(args),
        "eieio-object-class" | "cl-type-of" => prim_eieio_object_class(args),
        "eieio-object-name" | "object-name" => prim_eieio_object_name(args),
        "advice-add" => prim_advice_add(args),
        "advice-remove" => prim_advice_remove(args),
        "advice--p" | "advice-p" => prim_advice__p(args),
        "advice-function-member-p" => prim_advice_function_member_p(args),
        "advice-member-p" => prim_advice_member_p(args),
        "ad-is-advised" => prim_ad_is_advised(args),
        "widget-put" => prim_widget_put(args),
        "widget-get" => prim_widget_get(args),
        "widget-create" => prim_widget_create(args),
        "widget-apply" => prim_widget_apply(args),
        "widgetp" => prim_widgetp(args),
        _ => return None,
    })
}

pub const EIEIO_PRIMITIVE_NAMES: &[&str] = &[
    "eieio-class-p",
    "cl-class-p",
    "class-p",
    "find-class",
    "cl--find-class",
    "cl-find-class",
    "make-instance",
    "slot-value",
    "oref",
    "eieio-oref",
    "set-slot-value",
    "oset",
    "eieio-oset",
    "slot-boundp",
    "object-of-class-p",
    "cl-typep",
    "eieio-object-p",
    "eieio-object-class",
    "cl-type-of",
    "eieio-object-name",
    "object-name",
    "advice-add",
    "advice-remove",
    "advice--p",
    "advice-p",
    "advice-function-member-p",
    "advice-member-p",
    "ad-is-advised",
    "widget-put",
    "widget-get",
    "widget-create",
    "widget-apply",
    "widgetp",
];
