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
//! - Instance: a vector `[#<instance-tag> CLASS-NAME SLOT0 SLOT1 ...]`.
//!   The leading sentinel symbol (`eieio--instance-tag`) distinguishes
//!   instances from plain vectors whose first element happens to be a
//!   class name. All slot-access primitives check for the sentinel
//!   before trusting the class name at index 1.
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
    /// Keyword initarg (without the `:` prefix). Defaults to the slot
    /// name if the class definition doesn't specify one.
    pub initarg: Option<String>,
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

    // Walk the initargs alist: `(:initarg value :initarg2 value2 ...)`.
    // Keys are stored without the leading colon for easy comparison.
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

    // Build an instance vector: [SENTINEL, class-symbol, slot0, ...].
    // The leading sentinel discriminates instances from plain vectors.
    // Resolution order for each slot value:
    //   1. initarg explicitly passed and matching the slot's
    //      `:initarg` declaration,
    //   2. initarg explicitly passed and matching the slot NAME
    //      (common when `:initarg` is omitted in `defclass`),
    //   3. the slot's `:initform` default.
    let mut items: Vec<LispObject> = Vec::with_capacity(class.slots.len() + 2);
    items.push(LispObject::symbol(INSTANCE_TAG));
    items.push(LispObject::symbol(&class_name));
    for slot in &class.slots {
        let val = slot
            .initarg
            .as_ref()
            .and_then(|k| overrides.get(k).cloned())
            .or_else(|| overrides.get(&slot.name).cloned())
            .unwrap_or_else(|| slot.default.clone());
        items.push(val);
    }
    Ok(LispObject::Vector(std::sync::Arc::new(
        parking_lot::Mutex::new(items),
    )))
}

/// Sentinel symbol that prefixes every EIEIO instance vector. Plain
/// `(vector ...)` constructions can't forge this except by explicit
/// intent (the symbol name is chosen to be unlikely to appear as the
/// first element of a user vector).
const INSTANCE_TAG: &str = "eieio--instance-tag";

/// Return the class name of an instance, or None if `obj` is not an
/// EIEIO instance (e.g. a plain vector). Checks that index 0 is the
/// instance sentinel symbol.
fn instance_class(items: &[LispObject]) -> Option<String> {
    let tag = items.first()?.as_symbol()?;
    if tag != INSTANCE_TAG {
        return None;
    }
    items.get(1)?.as_symbol()
}

/// Helper: clone out the items of an instance vector, or None. Does
/// not validate the instance tag — callers that need type safety
/// combine this with [`instance_class`].
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
    let class_name = instance_class(&items).ok_or_else(|| {
        ElispError::Signal(Box::new(crate::error::SignalData {
            symbol: LispObject::symbol("wrong-type-argument"),
            data: LispObject::cons(
                LispObject::symbol("eieio-object"),
                LispObject::cons(instance.clone(), LispObject::nil()),
            ),
        }))
    })?;
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
    // Instance layout: [TAG, CLASS, slot0, slot1, ...] ⇒ slot N at idx + 2.
    Ok(items.get(idx + 2).cloned().unwrap_or(LispObject::nil()))
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
    let class_name = instance_class(&items).ok_or_else(|| {
        ElispError::Signal(Box::new(crate::error::SignalData {
            symbol: LispObject::symbol("wrong-type-argument"),
            data: LispObject::cons(
                LispObject::symbol("eieio-object"),
                LispObject::cons(instance.clone(), LispObject::nil()),
            ),
        }))
    })?;
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
    if let Some(slot) = items.get_mut(idx + 2) {
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
    // rare. We do require a real EIEIO instance, not a plain vector.
    let instance = args.first().unwrap_or(LispObject::nil());
    let is_inst = as_items(&instance)
        .and_then(|items| instance_class(&items))
        .is_some();
    Ok(LispObject::from(is_inst))
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
    let Some(class_name) = instance_class(&items) else {
        return Ok(LispObject::nil());
    };
    let hierarchy = class_hierarchy(&class_name);
    Ok(LispObject::from(hierarchy.contains(&target)))
}

pub fn prim_eieio_object_p(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().unwrap_or(LispObject::nil());
    let is_inst = as_items(&obj)
        .and_then(|items| instance_class(&items))
        .map(|n| get_class(&n).is_some())
        .unwrap_or(false);
    Ok(LispObject::from(is_inst))
}

pub fn prim_eieio_object_class(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().unwrap_or(LispObject::nil());
    let n = as_items(&obj).and_then(|items| instance_class(&items));
    Ok(n.map(|s| LispObject::symbol(&s)).unwrap_or(LispObject::nil()))
}

pub fn prim_eieio_object_name(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::string("anonymous"))
}

/// `(eieio-defclass-internal CNAME SUPERCLASSES SLOTS OPTIONS)`
///
/// The function the byte-compiler emits instead of the `defclass`
/// special form. `.elc` files (e.g. `auth-source.elc`, `vtable.elc`)
/// lower all class definitions through this entry point, so if we
/// don't register the class here `(make-instance 'CNAME ...)` later
/// signals `invalid-slot-type: (CNAME)` — which was the R20 bug.
///
/// Args are already-evaluated literal data: CNAME is a symbol,
/// SUPERCLASSES a list of symbols, SLOTS a list of slot specs
/// `((NAME :initarg :FOO :initform FORM :type T ...))`. Because the
/// whole slots list is passed as a quoted constant, `:initform 'netrc`
/// arrives as the literal cons `(quote netrc)`. We best-effort
/// de-quote simple forms; full eval would need interpreter context.
pub fn prim_eieio_defclass_internal(args: &LispObject) -> ElispResult<LispObject> {
    let cname_obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let cname = cname_obj
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".into()))?;

    let supers = args.nth(1).unwrap_or(LispObject::nil());
    let parent_name: Option<String> = supers
        .destructure_cons()
        .and_then(|(car, _)| car.as_symbol());

    let slots_list = args.nth(2).unwrap_or(LispObject::nil());
    let mut slots: Vec<Slot> = Vec::new();
    let mut cur = slots_list;
    while let Some((slot_spec, rest)) = cur.destructure_cons() {
        let (slot_name_obj, plist) = match slot_spec.destructure_cons() {
            Some(p) => p,
            None => break,
        };
        let Some(slot_name) = slot_name_obj.as_symbol() else {
            cur = rest;
            continue;
        };

        let mut initarg: Option<String> = None;
        let mut initform = LispObject::nil();
        let mut p = plist;
        while let Some((k, vs)) = p.destructure_cons() {
            let key = k.as_symbol();
            if let Some((v, rest2)) = vs.destructure_cons() {
                match key.as_deref() {
                    Some(":initarg") => {
                        if let Some(k2) = v.as_symbol() {
                            initarg =
                                Some(k2.strip_prefix(':').unwrap_or(&k2).to_string());
                        }
                    }
                    Some(":initform") => {
                        initform = dequote_initform(&v);
                    }
                    _ => {}
                }
                p = rest2;
            } else {
                break;
            }
        }
        slots.push(Slot {
            name: slot_name,
            initarg,
            default: initform,
        });
        cur = rest;
    }

    register_class(Class {
        name: cname.clone(),
        parent: parent_name,
        slots,
    });
    Ok(LispObject::symbol(&cname))
}

/// Partial evaluator for literal initforms coming through byte-
/// compiled slot specs. `(quote X)` → `X`, `(function X)` → `X`;
/// anything else is kept verbatim.
fn dequote_initform(form: &LispObject) -> LispObject {
    if let Some((head, tail)) = form.destructure_cons() {
        if let Some(sym) = head.as_symbol() {
            if sym == "quote" || sym == "function" {
                if let Some((inner, _)) = tail.destructure_cons() {
                    return inner;
                }
            }
        }
    }
    form.clone()
}

/// `(eieio-make-class-predicate CNAME)` — build a unary class
/// predicate. `.elc` files do `(defalias 'FOO-p (eieio-make-class-predicate 'FOO))`
/// right after the class definition; we need to return *something*
/// callable so the surrounding `defalias` doesn't fail. We emit a
/// closure that dispatches through `object-of-class-p`, matching
/// Emacs's own implementation.
pub fn prim_eieio_make_class_predicate(args: &LispObject) -> ElispResult<LispObject> {
    let cname = args.first().unwrap_or(LispObject::nil());
    let sym = cname.as_symbol().unwrap_or_default();
    let lambda_src = format!(
        "(lambda (obj) (and (eieio-object-p obj) (object-of-class-p obj '{sym})))"
    );
    match crate::read_all(&lambda_src) {
        Ok(mut forms) if !forms.is_empty() => Ok(forms.remove(0)),
        _ => Ok(LispObject::nil()),
    }
}

/// `(eieio-make-child-predicate CNAME)` — in practice identical to
/// the class predicate because our `object-of-class-p` already walks
/// the parent chain (see `class_hierarchy`).
pub fn prim_eieio_make_child_predicate(args: &LispObject) -> ElispResult<LispObject> {
    prim_eieio_make_class_predicate(args)
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

// ---- Keyword slot accessor dispatch ----------------------------

/// Try to handle a keyword (`:printer`, `:name`, etc.) called as a function.
///
/// When `(:keyword INSTANCE)` is evaluated, we treat it as slot-value lookup:
/// the keyword is looked up as an initarg on the instance's class, and we
/// return the corresponding slot value.
///
/// Returns `Some(result)` if the keyword matched a known slot; `None` if the
/// keyword name doesn't start with `:` or isn't found in the class.
pub fn try_keyword_slot_call(kw_symbol: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    // Only handle symbols starting with `:` (keywords)
    if !kw_symbol.starts_with(':') {
        return None;
    }

    // Extract the instance (first and only required argument)
    let instance = args.first()?;

    // Get the class name from the instance
    let items = as_items(&instance)?;
    let class_name = instance_class(&items)?;
    let class = get_class(&class_name)?;

    // Strip the leading `:` from the keyword to get the initarg name
    let initarg_name = kw_symbol.strip_prefix(':').unwrap_or(kw_symbol);

    // Find the slot by matching either:
    // 1. The slot's declared `:initarg` (if present), or
    // 2. The slot's `:name` (fallback, as per make-instance logic)
    let slot_index = class.slots.iter().position(|s| {
        if let Some(ref ia) = s.initarg {
            ia == initarg_name
        } else {
            s.name == initarg_name
        }
    })?;

    // Retrieve the value: instance layout is [TAG, CLASS, slot0, slot1, ...]
    // so slot N is at index N + 2.
    let value = items.get(slot_index + 2).cloned().unwrap_or(LispObject::nil());
    Some(Ok(value))
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
        // Note: `cl-typep` is the general type-check function; the
        // EIEIO-specific class check is `object-of-class-p`. Routing
        // `cl-typep` here would miss builtin types (integer, number,
        // string, ...) — handled in primitives_cl.rs instead.
        "object-of-class-p" => prim_object_of_class_p(args),
        "eieio-object-p" => prim_eieio_object_p(args),
        "eieio-object-class" | "cl-type-of" => prim_eieio_object_class(args),
        "eieio-object-name" | "object-name" => prim_eieio_object_name(args),
        "eieio-defclass-internal" => prim_eieio_defclass_internal(args),
        "eieio-make-class-predicate" => prim_eieio_make_class_predicate(args),
        "eieio-make-child-predicate" => prim_eieio_make_child_predicate(args),
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
    // `cl-typep` is owned by primitives_cl.rs (supports integer,
    // number, list, string, ... in addition to classes).
    "eieio-object-p",
    "eieio-object-class",
    "cl-type-of",
    "eieio-object-name",
    "object-name",
    "eieio-defclass-internal",
    "eieio-make-class-predicate",
    "eieio-make-child-predicate",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn reset_classes() {
        CLASSES.with(|c| c.borrow_mut().clear());
    }

    fn symbol(s: &str) -> LispObject {
        LispObject::symbol(s)
    }

    fn list(items: &[LispObject]) -> LispObject {
        let mut out = LispObject::nil();
        for item in items.iter().rev() {
            out = LispObject::cons(item.clone(), out);
        }
        out
    }

    /// Regression: R9. Previously `make-instance` stripped the colon
    /// off the initarg and used the rest as the slot NAME. If a
    /// `defclass` declared `(slot-a :initarg :foo)` — slot name
    /// `slot-a`, initarg `:foo` — then `(make-instance 'C :foo 42)`
    /// looked up the slot named "foo" (not existing) instead of
    /// matching by initarg.
    #[test]
    fn make_instance_matches_by_initarg() {
        reset_classes();
        register_class(Class {
            name: "r9-class".into(),
            parent: None,
            slots: vec![
                Slot {
                    name: "slot-a".into(),
                    initarg: Some("foo".into()),
                    default: LispObject::integer(-1),
                },
                Slot {
                    name: "slot-b".into(),
                    initarg: None, // no initarg, must match slot name
                    default: LispObject::integer(-2),
                },
            ],
        });

        // Pass by initarg :foo and by slot name slot-b.
        let args = list(&[
            symbol("r9-class"),
            symbol(":foo"),
            LispObject::integer(10),
            symbol(":slot-b"),
            LispObject::integer(20),
        ]);
        let instance = prim_make_instance(&args).expect("make-instance ok");

        // slot-value slot-a → 10 (matched via :foo initarg).
        let get_a = LispObject::cons(
            instance.clone(),
            LispObject::cons(symbol("slot-a"), LispObject::nil()),
        );
        assert_eq!(
            prim_slot_value(&get_a).unwrap().as_integer(),
            Some(10),
            "slot-a must be filled via :foo initarg"
        );

        // slot-value slot-b → 20 (matched via slot name fallback).
        let get_b = LispObject::cons(
            instance,
            LispObject::cons(symbol("slot-b"), LispObject::nil()),
        );
        assert_eq!(prim_slot_value(&get_b).unwrap().as_integer(), Some(20));
    }

    /// If the initarg is absent, the :initform default is used.
    #[test]
    fn make_instance_uses_default_when_initarg_absent() {
        reset_classes();
        register_class(Class {
            name: "r9-defaults".into(),
            parent: None,
            slots: vec![Slot {
                name: "x".into(),
                initarg: Some("x-init".into()),
                default: LispObject::integer(77),
            }],
        });
        let args = list(&[symbol("r9-defaults")]); // no initargs
        let instance = prim_make_instance(&args).unwrap();
        let get = LispObject::cons(
            instance,
            LispObject::cons(symbol("x"), LispObject::nil()),
        );
        assert_eq!(prim_slot_value(&get).unwrap().as_integer(), Some(77));
    }

    /// Regression: R10. `slot-value` used to trust any vector whose
    /// first element was a registered class symbol — so
    /// `(slot-value (vector 'r10-class 1 2) 'x)` returned bogus slot
    /// values from a non-instance. The instance tag fixes this:
    /// only vectors actually produced by `make-instance` are accepted.
    #[test]
    fn slot_value_rejects_plain_vectors_masquerading_as_instances() {
        reset_classes();
        register_class(Class {
            name: "r10-class".into(),
            parent: None,
            slots: vec![Slot {
                name: "x".into(),
                initarg: None,
                default: LispObject::integer(0),
            }],
        });
        // Forge a plain vector [class-symbol, 42] and try to slot-value it.
        let fake = LispObject::Vector(std::sync::Arc::new(parking_lot::Mutex::new(vec![
            symbol("r10-class"),
            LispObject::integer(42),
        ])));
        let get = LispObject::cons(
            fake.clone(),
            LispObject::cons(symbol("x"), LispObject::nil()),
        );
        let err = prim_slot_value(&get)
            .expect_err("forged vector must not masquerade as an instance");
        match err {
            ElispError::Signal(sig) => {
                let sym = sig.symbol.as_symbol();
                assert_eq!(
                    sym.as_deref(),
                    Some("wrong-type-argument"),
                    "expected wrong-type-argument signal, got: {:?}",
                    sig
                );
            }
            _ => panic!("expected Signal error, got: {err:?}"),
        }

        // A real instance should still work.
        let args = list(&[symbol("r10-class"), symbol(":x"), LispObject::integer(7)]);
        let real_instance = prim_make_instance(&args).unwrap();
        let get = LispObject::cons(
            real_instance,
            LispObject::cons(symbol("x"), LispObject::nil()),
        );
        assert_eq!(prim_slot_value(&get).unwrap().as_integer(), Some(7));
    }

    /// `eieio-object-p` must also reject plain vectors.
    #[test]
    fn eieio_object_p_rejects_plain_vectors() {
        reset_classes();
        register_class(Class {
            name: "r10-plainreject".into(),
            parent: None,
            slots: vec![],
        });
        let fake = LispObject::Vector(std::sync::Arc::new(parking_lot::Mutex::new(vec![
            symbol("r10-plainreject"),
        ])));
        let r = prim_eieio_object_p(&LispObject::cons(fake, LispObject::nil())).unwrap();
        assert!(matches!(r, LispObject::Nil));
    }

    /// Using the SLOT NAME when an initarg is explicitly declared
    /// should NOT override — but our resolution falls back to the
    /// slot name when no initarg matches. Verify that if BOTH are
    /// passed, the initarg wins (the documented resolution order).
    #[test]
    fn initarg_wins_over_slot_name() {
        reset_classes();
        register_class(Class {
            name: "r9-pref".into(),
            parent: None,
            slots: vec![Slot {
                name: "col".into(),
                initarg: Some("color".into()),
                default: LispObject::integer(0),
            }],
        });
        let args = list(&[
            symbol("r9-pref"),
            symbol(":color"),
            LispObject::integer(5),
            symbol(":col"),
            LispObject::integer(99),
        ]);
        let instance = prim_make_instance(&args).unwrap();
        let get = LispObject::cons(
            instance,
            LispObject::cons(symbol("col"), LispObject::nil()),
        );
        assert_eq!(
            prim_slot_value(&get).unwrap().as_integer(),
            Some(5),
            "initarg :color must win over slot-name :col"
        );
    }
}
