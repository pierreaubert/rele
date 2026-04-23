//! Fast-path dispatch cache for hot special-form names.
//!
//! `eval_inner` (crates/elisp/src/eval/mod.rs) dispatches a
//! cons-headed form by calling `obarray::symbol_name(id)` and
//! running a 200+-arm `match` on the resulting `&str`. The cost we
//! target here is the per-call `String` allocation inside
//! `symbol_name`: every evaluated cons pays for a fresh heap-
//! allocated copy of the head symbol's name.
//!
//! This module skips that allocation for the ~20 most frequent
//! special forms. It caches each form's `SymbolId` in a `OnceLock`
//! on first use and maps a matching `SymbolId` back to a
//! `&'static str` the caller can feed directly into the existing
//! `match sym_name.as_str() { ... }`. On a miss we fall through to
//! the full `symbol_name` call — so the slow path is unchanged and
//! behaviour is preserved.
//!
//! The global obarray is process-global and never reset, so the
//! `OnceLock<SymbolId>` cache is safe to hold for the lifetime of
//! the process.

use crate::obarray::{self, SymbolId};
use std::sync::OnceLock;

/// Map a SymbolId to a static name string if it matches one of the
/// hot special-form names, else None. Lookup cost is a sequence of
/// `u32` equality tests; the hit case costs no allocation.
#[inline]
pub fn hot_form_name(id: SymbolId) -> Option<&'static str> {
    macro_rules! probe {
        ($slot:ident, $name:literal) => {{
            static SLOT: OnceLock<SymbolId> = OnceLock::new();
            if id == *SLOT.get_or_init(|| obarray::intern($name)) {
                return Some($name);
            }
        }};
    }
    probe!(quote, "quote");
    probe!(if_, "if");
    probe!(progn, "progn");
    probe!(setq, "setq");
    probe!(let_, "let");
    probe!(let_star, "let*");
    probe!(lambda, "lambda");
    probe!(function, "function");
    probe!(cond, "cond");
    probe!(and_, "and");
    probe!(or_, "or");
    probe!(when, "when");
    probe!(unless, "unless");
    probe!(while_, "while");
    probe!(prog1, "prog1");
    probe!(prog2, "prog2");
    probe!(funcall, "funcall");
    probe!(apply, "apply");
    probe!(save_excursion, "save-excursion");
    probe!(save_restriction, "save-restriction");
    probe!(backquote, "`");
    probe!(defun, "defun");
    probe!(defvar, "defvar");
    probe!(defmacro_, "defmacro");
    probe!(defconst, "defconst");
    probe!(condition_case, "condition-case");
    probe!(unwind_protect, "unwind-protect");
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_returns_same_static_str() {
        let id = obarray::intern("quote");
        let s = hot_form_name(id).expect("quote is hot");
        assert_eq!(s, "quote");
    }

    #[test]
    fn every_hot_name_resolves() {
        for name in [
            "quote",
            "if",
            "progn",
            "setq",
            "let",
            "let*",
            "lambda",
            "function",
            "cond",
            "and",
            "or",
            "when",
            "unless",
            "while",
            "prog1",
            "prog2",
            "funcall",
            "apply",
            "save-excursion",
            "save-restriction",
            "`",
            "defun",
            "defvar",
            "defmacro",
            "defconst",
            "condition-case",
            "unwind-protect",
        ] {
            let id = obarray::intern(name);
            assert_eq!(hot_form_name(id), Some(name), "probe for {name}");
        }
    }

    #[test]
    fn unknown_symbol_returns_none() {
        let id = obarray::intern("user-function-that-is-not-a-form");
        assert_eq!(hot_form_name(id), None);
    }
}
