use crate::object::LispObject;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Index into the global symbol table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

/// The global symbol-name registry (obarray).
///
/// This table owns only the name↔SymbolId mapping. It is append-only
/// and safe to share across every interpreter in the process. All
/// mutable per-symbol data (value cells, function cells, plists,
/// flags) lives in [`SymbolCells`], which each interpreter owns
/// independently.
pub struct SymbolTable {
    names: Vec<String>,
    name_to_id: HashMap<String, SymbolId>,
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolTable {
    pub fn new() -> Self {
        let mut table = SymbolTable {
            names: Vec::new(),
            name_to_id: HashMap::new(),
        };
        // Pre-intern common symbols
        table.intern("nil");
        table.intern("t");
        table
    }

    pub fn intern(&mut self, name: &str) -> SymbolId {
        if let Some(&id) = self.name_to_id.get(name) {
            return id;
        }
        let id = SymbolId(self.names.len() as u32);
        self.names.push(name.to_string());
        self.name_to_id.insert(name.to_string(), id);
        id
    }

    pub fn name(&self, id: SymbolId) -> &str {
        &self.names[id.0 as usize]
    }

    /// Total number of interned symbols.
    pub fn symbol_count(&self) -> usize {
        self.names.len()
    }

    pub fn find(&self, name: &str) -> Option<SymbolId> {
        self.name_to_id.get(name).copied()
    }
}

/// The global obarray shared by all interpreter instances.
///
/// Contains only name↔SymbolId mappings. Mutable per-symbol data
/// lives in per-interpreter [`SymbolCells`].
pub static GLOBAL_OBARRAY: LazyLock<RwLock<SymbolTable>> =
    LazyLock::new(|| RwLock::new(SymbolTable::new()));

/// Intern a symbol name in the global obarray, returning its ID.
///
/// Read-lock fast-path: most interns hit an already-present symbol,
/// so we try a read first and only upgrade to a write-lock if needed.
pub fn intern(name: &str) -> SymbolId {
    if let Some(id) = GLOBAL_OBARRAY.read().find(name) {
        return id;
    }
    GLOBAL_OBARRAY.write().intern(name)
}

/// Look up the name for a symbol ID.
pub fn symbol_name(id: SymbolId) -> String {
    GLOBAL_OBARRAY.read().name(id).to_string()
}

// ---------------------------------------------------------------------------
// Per-interpreter mutable symbol data
// ---------------------------------------------------------------------------

/// Flags associated with a symbol.
#[derive(Debug, Clone, Copy, Default)]
pub struct SymbolFlags {
    pub special: bool,
    pub constant: bool,
}

/// Mutable data for a single symbol, owned per-interpreter.
#[derive(Clone, Default)]
struct CellData {
    value: Option<LispObject>,
    function: Option<LispObject>,
    plist: Vec<(SymbolId, LispObject)>,
    flags: SymbolFlags,
    def_version: u64,
}

/// Per-interpreter mutable symbol data — value cells, function cells,
/// plists, flags and def-version counters. Indexed by [`SymbolId`]
/// (which comes from the global name registry).
///
/// Each [`InterpreterState`](crate::eval::InterpreterState) owns one
/// of these behind `Arc<SyncRefCell<SymbolCells>>`, giving every
/// interpreter an isolated namespace for symbol state.
pub struct SymbolCells {
    cells: Vec<CellData>,
}

impl std::fmt::Debug for SymbolCells {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SymbolCells({} entries)", self.cells.len())
    }
}

impl Default for SymbolCells {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolCells {
    pub fn new() -> Self {
        SymbolCells { cells: Vec::new() }
    }

    /// Grow the backing vec so `id` is in-bounds.
    fn ensure(&mut self, id: SymbolId) {
        let idx = id.0 as usize;
        if idx >= self.cells.len() {
            self.cells.resize_with(idx + 1, CellData::default);
        }
    }

    // -- value cell ---------------------------------------------------------

    pub fn get_value_cell(&self, sym: SymbolId) -> Option<LispObject> {
        self.cells.get(sym.0 as usize)?.value.clone()
    }

    pub fn set_value_cell(&mut self, sym: SymbolId, val: LispObject) {
        self.ensure(sym);
        self.cells[sym.0 as usize].value = Some(val);
    }

    pub fn clear_value_cell(&mut self, sym: SymbolId) {
        if let Some(data) = self.cells.get_mut(sym.0 as usize) {
            data.value = None;
        }
    }

    // -- function cell ------------------------------------------------------

    pub fn get_function_cell(&self, sym: SymbolId) -> Option<LispObject> {
        self.cells.get(sym.0 as usize)?.function.clone()
    }

    /// Write the symbol's function cell.
    ///
    /// Side effect: bumps the symbol's `def_version` counter.
    /// JIT-compiled code records the version it was compiled against;
    /// when the cached version no longer matches, the compiled code
    /// is stale and must be deoptimized.
    pub fn set_function_cell(&mut self, sym: SymbolId, val: LispObject) {
        self.ensure(sym);
        self.cells[sym.0 as usize].function = Some(val);
        self.cells[sym.0 as usize].def_version += 1;
    }

    pub fn clear_function_cell(&mut self, sym: SymbolId) {
        if let Some(data) = self.cells.get_mut(sym.0 as usize) {
            data.function = None;
            data.def_version += 1;
        }
    }

    // -- plist --------------------------------------------------------------

    pub fn get_plist(&self, sym: SymbolId, prop: SymbolId) -> LispObject {
        let Some(data) = self.cells.get(sym.0 as usize) else {
            return LispObject::Nil;
        };
        for (k, v) in &data.plist {
            if *k == prop {
                return v.clone();
            }
        }
        LispObject::Nil
    }

    pub fn put_plist(&mut self, sym: SymbolId, prop: SymbolId, value: LispObject) {
        self.ensure(sym);
        let data = &mut self.cells[sym.0 as usize];
        for entry in data.plist.iter_mut() {
            if entry.0 == prop {
                entry.1 = value;
                return;
            }
        }
        data.plist.push((prop, value));
    }

    /// Return the plist as a freshly-built cons list `(prop val prop val ...)`.
    pub fn full_plist(&self, sym: SymbolId) -> LispObject {
        let Some(data) = self.cells.get(sym.0 as usize) else {
            return LispObject::Nil;
        };
        let mut result = LispObject::Nil;
        for (k, v) in data.plist.iter().rev() {
            result = LispObject::cons(v.clone(), result);
            result = LispObject::cons(LispObject::Symbol(*k), result);
        }
        result
    }

    /// Replace a symbol's entire plist from a Lisp cons list `(prop val ...)`.
    pub fn replace_plist(&mut self, sym: SymbolId, plist: LispObject) {
        self.ensure(sym);
        self.cells[sym.0 as usize].plist.clear();
        let mut cur = plist;
        while let Some((prop, rest)) = cur.destructure_cons() {
            if let Some((val, rest2)) = rest.destructure_cons() {
                if let LispObject::Symbol(prop_id) = &prop {
                    self.cells[sym.0 as usize].plist.push((*prop_id, val));
                }
                cur = rest2;
            } else {
                break;
            }
        }
    }

    // -- flags --------------------------------------------------------------

    pub fn get_flags(&self, sym: SymbolId) -> SymbolFlags {
        self.cells
            .get(sym.0 as usize)
            .map_or(SymbolFlags::default(), |c| c.flags)
    }

    pub fn mark_special(&mut self, sym: SymbolId) {
        self.ensure(sym);
        self.cells[sym.0 as usize].flags.special = true;
    }

    // -- def_version --------------------------------------------------------

    /// Return `sym`'s current def_version (0 if never defined).
    pub fn def_version(&self, sym: SymbolId) -> u64 {
        self.cells.get(sym.0 as usize).map_or(0, |c| c.def_version)
    }

    /// Increment `sym`'s def_version counter.
    pub fn bump_def_version(&mut self, sym: SymbolId) {
        self.ensure(sym);
        self.cells[sym.0 as usize].def_version += 1;
    }
}
