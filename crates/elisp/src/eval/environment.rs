use crate::obarray::{self, SymbolCells, SymbolId};
use crate::object::LispObject;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::SyncRefCell;

#[derive(Debug, Clone)]
pub struct Environment {
    bindings: HashMap<SymbolId, LispObject>,
    parent: Option<Arc<Environment>>,
    symbol_cells: Arc<SyncRefCell<SymbolCells>>,
}

impl Environment {
    pub fn new(symbol_cells: Arc<SyncRefCell<SymbolCells>>) -> Self {
        Environment {
            bindings: HashMap::new(),
            parent: None,
            symbol_cells,
        }
    }

    pub fn with_parent(parent: Arc<Environment>) -> Self {
        let symbol_cells = parent.symbol_cells.clone();
        Environment {
            bindings: HashMap::new(),
            parent: Some(parent),
            symbol_cells,
        }
    }

    pub fn get(&self, name: &str) -> Option<LispObject> {
        let id = obarray::intern(name);
        self.get_id(id)
    }

    pub fn get_id(&self, id: SymbolId) -> Option<LispObject> {
        if let Some(val) = self.bindings.get(&id).cloned() {
            return Some(val);
        }
        if let Some(p) = self.parent.as_ref()
            && let Some(val) = p.get_id(id) {
                return Some(val);
            }
        self.symbol_cells.read().get_value_cell(id)
    }

    pub fn get_id_local(&self, id: SymbolId) -> Option<LispObject> {
        if let Some(val) = self.bindings.get(&id).cloned() {
            return Some(val);
        }
        self.parent.as_ref().and_then(|p| p.get_id_local(id))
    }

    pub fn get_function(&self, name: &str) -> Option<LispObject> {
        let id = obarray::intern(name);
        self.get_function_id(id)
    }

    pub fn get_function_id(&self, id: SymbolId) -> Option<LispObject> {
        let mut first_found: Option<LispObject> = None;
        if let Some(val) = self.bindings.get(&id).cloned() {
            if is_callable_value(&val) {
                return Some(val);
            }
            first_found = Some(val);
        }
        let mut parent = self.parent.as_ref();
        while let Some(p) = parent {
            if let Some(val) = p.bindings.get(&id).cloned() {
                if is_callable_value(&val) {
                    return Some(val);
                }
                if first_found.is_none() {
                    first_found = Some(val);
                }
            }
            parent = p.parent.as_ref();
        }
        if let Some(fn_cell) = self.symbol_cells.read().get_function_cell(id) {
            return Some(fn_cell);
        }
        first_found
    }

    pub fn set(&mut self, name: &str, value: LispObject) {
        let id = obarray::intern(name);
        self.bindings.insert(id, value);
    }

    pub fn set_id(&mut self, id: SymbolId, value: LispObject) {
        self.bindings.insert(id, value);
    }

    pub fn unset_id(&mut self, id: SymbolId) {
        self.bindings.remove(&id);
    }

    pub fn define(&mut self, name: &str, value: LispObject) {
        let id = obarray::intern(name);
        self.bindings.insert(id, value);
    }

    pub fn define_id(&mut self, id: SymbolId, value: LispObject) {
        self.bindings.insert(id, value);
    }

    pub fn local_binding_entries(&self) -> Vec<(SymbolId, LispObject)> {
        self.bindings
            .iter()
            .map(|(id, value)| (*id, value.clone()))
            .collect()
    }

    pub fn capture_as_alist(&self) -> LispObject {
        let mut seen: HashSet<SymbolId> = HashSet::new();
        let mut pairs: Vec<(SymbolId, LispObject)> = Vec::new();

        let mut cur: Option<&Environment> = Some(self);
        while let Some(e) = cur {
            if e.parent.is_none() {
                break;
            }
            for (id, val) in &e.bindings {
                if seen.insert(*id) {
                    pairs.push((*id, val.clone()));
                }
            }
            cur = e.parent.as_deref();
        }

        let mut alist = LispObject::nil();
        for (id, val) in pairs.into_iter().rev() {
            let pair = LispObject::cons(LispObject::Symbol(id), val);
            alist = LispObject::cons(pair, alist);
        }
        alist
    }
}

pub fn is_callable_value(obj: &LispObject) -> bool {
    match obj {
        LispObject::Primitive(_) | LispObject::BytecodeFn(_) => true,
        LispObject::Cons(cell) => {
            let b = cell.lock();
            if let LispObject::Symbol(id) = &b.0 {
                let name = crate::obarray::symbol_name(*id);
                name == "lambda" || name == "closure"
            } else {
                false
            }
        }
        _ => false,
    }
}
