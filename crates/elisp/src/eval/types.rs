//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use super::environment::Environment;
use super::macro_table::MacroTable;
use super::sync_cell::SyncRefCell;
use super::thread_locals::FeatureList;
use super::{AutoloadTable, RwLock, SpecialVars, Specpdl};
use super::{eval, is_callable_value};
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::obarray::{self, SymbolId};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Shared interpreter state accessible during evaluation.
pub struct InterpreterState {
    pub features: FeatureList,
    pub profiler: Arc<RwLock<crate::jit::Profiler>>,
    #[cfg(feature = "jit")]
    pub jit: Arc<RwLock<crate::jit::JitCompiler>>,
    /// Variables declared special (dynamically bound) via `defvar`/`defconst`.
    pub special_vars: SpecialVars,
    /// Dynamic binding stack — saves/restores old values of special variables.
    pub specpdl: Specpdl,
    /// The root (global) environment. Special variables are always read/written here.
    pub global_env: Arc<RwLock<Environment>>,
    /// Per-interpreter mutable symbol data (value cells, function cells,
    /// plists, flags, def-version counters). Isolates each interpreter
    /// from all others so concurrent tests cannot pollute each other.
    pub symbol_cells: Arc<SyncRefCell<obarray::SymbolCells>>,
    /// Garbage-collected heap for cons cell allocation.
    pub heap: Arc<SyncRefCell<crate::gc::Heap>>,
    /// Counter for total cons cell allocations (monotonically increasing).
    pub cons_count: Arc<std::sync::atomic::AtomicU64>,
    /// Autoload mappings: function-name -> file-to-load.
    pub autoloads: AutoloadTable,
    /// Per-eval operation counter. Incremented on every eval call.
    /// When `eval_ops_limit` is > 0 and ops exceeds it, eval returns an error.
    pub eval_ops: Arc<std::sync::atomic::AtomicU64>,
    /// Maximum number of eval operations before aborting (0 = unlimited).
    pub eval_ops_limit: Arc<std::sync::atomic::AtomicU64>,
    /// Wall-clock deadline. When set, `eval()` checks `Instant::now()`
    /// every 1024 ops and errors if past the deadline. Replaces the
    /// watchdog thread approach — no threads needed.
    pub deadline: std::cell::Cell<Option<std::time::Instant>>,
}
impl InterpreterState {
    /// Charge `n` eval operations against this interpreter's budget.
    /// Returns `Err(EvalError)` if the operation count would exceed
    /// `eval_ops_limit` (0 = unlimited).
    ///
    /// Use this at the top of any Rust loop that walks user data or
    /// performs an unknown amount of work — it's how we prevent a
    /// rogue input from sending the interpreter into an unbounded
    /// Rust-level loop where the existing per-eval `eval_ops` bump
    /// never gets to run.
    pub fn charge(&self, n: u64) -> ElispResult<()> {
        use std::sync::atomic::Ordering;
        let new_ops = self.eval_ops.fetch_add(n, Ordering::Relaxed) + n;
        let limit = self.eval_ops_limit.load(Ordering::Relaxed);
        if limit > 0 && new_ops > limit {
            return Err(ElispError::EvalError(
                "eval operation limit exceeded".to_string(),
            ));
        }
        if new_ops & 0x3FF == 0 {
            if let Some(dl) = self.deadline.get() {
                if std::time::Instant::now() >= dl {
                    return Err(ElispError::EvalError("hard eval limit".into()));
                }
            }
        }
        Ok(())
    }
    pub fn get_value_cell(&self, sym: obarray::SymbolId) -> Option<LispObject> {
        self.symbol_cells.read().get_value_cell(sym)
    }
    pub fn set_value_cell(&self, sym: obarray::SymbolId, val: LispObject) {
        self.symbol_cells.write().set_value_cell(sym, val);
    }
    pub fn get_function_cell(&self, sym: obarray::SymbolId) -> Option<LispObject> {
        self.symbol_cells.read().get_function_cell(sym)
    }
    pub fn set_function_cell(&self, sym: obarray::SymbolId, val: LispObject) {
        self.symbol_cells.write().set_function_cell(sym, val);
    }
    pub fn clear_value_cell(&self, sym: obarray::SymbolId) {
        self.symbol_cells.write().clear_value_cell(sym);
    }
    pub fn clear_function_cell(&self, sym: obarray::SymbolId) {
        self.symbol_cells.write().clear_function_cell(sym);
    }
    pub fn get_plist(&self, sym: obarray::SymbolId, prop: obarray::SymbolId) -> LispObject {
        self.symbol_cells.read().get_plist(sym, prop)
    }
    pub fn put_plist(&self, sym: obarray::SymbolId, prop: obarray::SymbolId, value: LispObject) {
        self.symbol_cells.write().put_plist(sym, prop, value);
    }
    pub fn full_plist(&self, sym: obarray::SymbolId) -> LispObject {
        self.symbol_cells.read().full_plist(sym)
    }
    pub fn replace_plist(&self, sym: obarray::SymbolId, plist: LispObject) {
        self.symbol_cells.write().replace_plist(sym, plist);
    }
    pub fn def_version(&self, sym: obarray::SymbolId) -> u64 {
        self.symbol_cells.read().def_version(sym)
    }
    /// Allocate a cons cell on the interpreter's real GC heap and return
    /// it as a Value (TAG_HEAP_PTR). This is the chokepoint for every
    /// future `LispObject::cons` → heap migration: callers that need a
    /// traceable cons go through this method rather than constructing a
    /// `LispObject::Cons(Arc<Mutex<_>>)`.
    ///
    /// The returned Value is safe to use between safepoints. The
    /// interpreter runs the heap in `GcMode::Manual`, so no allocation
    /// implicitly sweeps — only the `(garbage-collect)` primitive does.
    /// If you need the Value to survive an explicit collection, push it
    /// onto the heap's root stack via `with_heap(|h| h.root_value(v))`
    /// before the collection and pop afterwards.
    pub fn heap_cons(
        &self,
        car: crate::value::Value,
        cdr: crate::value::Value,
    ) -> crate::value::Value {
        self.heap.lock().cons_value(car.raw(), cdr.raw())
    }
    /// Run `f` with exclusive access to the heap. Use this when a flow
    /// allocates several cons cells and wants to hold the heap lock
    /// once, rather than re-locking for every cell. Multi-step rooting
    /// (push several roots, allocate, pop) also goes through here.
    pub fn with_heap<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut crate::gc::Heap) -> R,
    {
        f(&mut self.heap.lock())
    }
    /// Build a proper Lisp list on the real GC heap from `items`, in
    /// natural order: `[a, b, c]` becomes `(a b c)`.
    ///
    /// All cons cells are allocated under one `with_heap` closure so
    /// the heap lock is taken only once. Each item is routed through
    /// the existing `obj_to_value` bridge — immediate types stay
    /// immediate, heap-typed `LispObject`s land in the thread-local
    /// side-table. Only the *spine* of the list (the cons cells) lives
    /// on the real GC heap; the items themselves still use whatever
    /// representation `obj_to_value` picks.
    ///
    /// The returned Value carries `TAG_HEAP_PTR` and is safe to use
    /// until the next explicit `(garbage-collect)`. `value_to_obj`
    /// decodes the chain back into `LispObject::Cons` at the eval
    /// boundary.
    pub fn list_from_objects<I>(&self, items: I) -> Value
    where
        I: IntoIterator<Item = LispObject>,
        I::IntoIter: DoubleEndedIterator,
    {
        let converted: Vec<Value> = items.into_iter().map(obj_to_value).collect();
        self.with_heap(|heap| {
            let mut result = Value::nil();
            for v in converted.into_iter().rev() {
                result = heap.cons_value(v.raw(), result.raw());
            }
            result
        })
    }
    /// Build a Lisp list on the real GC heap from `items`, with each
    /// item prepended in iteration order: `[a, b, c]` becomes
    /// `(c b a)`. This is the "destructive reverse" shape used by
    /// `nreverse` and similar primitives where the caller collected
    /// items from a source list and wants the output reversed.
    ///
    /// Contrast with [`Self::list_from_objects`], which produces the
    /// items in natural order. Same rooting/GC semantics — all cons
    /// cells are allocated under one `with_heap` closure.
    pub fn list_from_objects_reversed<I>(&self, items: I) -> Value
    where
        I: IntoIterator<Item = LispObject>,
    {
        let converted: Vec<Value> = items.into_iter().map(obj_to_value).collect();
        self.with_heap(|heap| {
            let mut result = Value::nil();
            for v in converted {
                result = heap.cons_value(v.raw(), result.raw());
            }
            result
        })
    }
    /// Allocate a string on the real GC heap and return a Value. This
    /// is the chokepoint for migrating `LispObject::String(...)` sites
    /// away from the `HEAP_OBJECTS` side-table.
    ///
    /// The returned Value is safe to use until the next explicit
    /// `(garbage-collect)`; it carries `TAG_HEAP_PTR` and decodes back
    /// to `LispObject::String` via `value_to_obj` at the eval boundary.
    pub fn heap_string(&self, s: &str) -> Value {
        self.heap.lock().string_value(s)
    }
    /// Allocate a vector on the real GC heap from an iterator of
    /// `Value`s. Phase 2n: the resulting heap object wraps a fresh
    /// `SharedVec` (Arc<Mutex<Vec<LispObject>>>) so identity is
    /// preserved across `value_to_obj` round-trips.
    pub fn heap_vector<I>(&self, elements: I) -> Value
    where
        I: IntoIterator<Item = Value>,
    {
        let items: Vec<LispObject> = elements.into_iter().map(value_to_obj).collect();
        let arc: crate::object::SharedVec =
            std::sync::Arc::new(crate::eval::SyncRefCell::new(items));
        self.heap.lock().vector_value(arc)
    }
    /// Allocate a vector on the real GC heap from a slice of
    /// `LispObject`s. Phase 2n: wraps a fresh `SharedVec`.
    pub fn heap_vector_from_objects(&self, items: &[LispObject]) -> Value {
        let arc: crate::object::SharedVec =
            std::sync::Arc::new(crate::eval::SyncRefCell::new(items.to_vec()));
        self.heap.lock().vector_value(arc)
    }
    /// Allocate a hash table on the real GC heap wrapping the given
    /// `LispHashTable`. Phase 2n: wraps a fresh `SharedHashTable`.
    pub fn heap_hashtable(&self, table: crate::object::LispHashTable) -> Value {
        let arc: crate::object::SharedHashTable =
            std::sync::Arc::new(crate::eval::SyncRefCell::new(table));
        self.heap.lock().hashtable_value(arc)
    }
    /// Build a proper Lisp list on the real GC heap from already-valued
    /// items in natural order: `[a, b, c]` becomes `(a b c)`. Mirror of
    /// `list_from_objects` but takes `Value`s directly, so call sites
    /// that already produce heap-allocated Values (e.g. via
    /// `heap_string`) don't round-trip through `LispObject`.
    pub fn list_from_values<I>(&self, values: I) -> Value
    where
        I: IntoIterator<Item = Value>,
        I::IntoIter: DoubleEndedIterator,
    {
        self.with_heap(|heap| {
            let mut result = Value::nil();
            for v in values.into_iter().rev() {
                result = heap.cons_value(v.raw(), result.raw());
            }
            result
        })
    }
}
/// `Clone` is cheap — every field is `Arc<RwLock<...>>` (or already
/// Clone). Cloning shares the underlying state, so two clones see each
/// other's mutations. The harness uses this to bootstrap-once-and-share
/// across per-file workers (we already accept obarray pollution between
/// test files; sharing env/macros is no different).
#[derive(Clone)]
pub struct Interpreter {
    pub(super) env: Arc<RwLock<Environment>>,
    pub(super) editor: Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    pub(super) macros: MacroTable,
    pub state: InterpreterState,
}
impl Interpreter {
    pub fn new() -> Self {
        let symbol_cells = Arc::new(SyncRefCell::new(obarray::SymbolCells::new()));
        let mut env = Environment::new(symbol_cells.clone());
        env.define("nil", LispObject::nil());
        env.define("t", LispObject::t());
        let special_vars: HashSet<SymbolId> = [
            "load-path",
            "features",
            "standard-output",
            "standard-input",
            "print-escape-newlines",
            "print-length",
            "print-level",
            "debug-on-error",
            "inhibit-quit",
            "case-fold-search",
            "default-directory",
            "buffer-file-name",
            "last-command",
            "this-command",
        ]
        .iter()
        .map(|s| {
            let id = obarray::intern(s);
            symbol_cells.write().mark_special(id);
            id
        })
        .collect();
        let env = Arc::new(RwLock::new(env));
        Interpreter {
            env: env.clone(),
            editor: Arc::new(RwLock::new(None)),
            macros: Arc::new(RwLock::new(HashMap::new())),
            state: InterpreterState {
                features: Arc::new(RwLock::new(Vec::new())),
                profiler: Arc::new(RwLock::new(crate::jit::Profiler::new(1000))),
                #[cfg(feature = "jit")]
                jit: Arc::new(RwLock::new(crate::jit::JitCompiler::new())),
                special_vars: Arc::new(RwLock::new(special_vars)),
                specpdl: Arc::new(RwLock::new(Vec::new())),
                global_env: env,
                symbol_cells,
                heap: Arc::new(SyncRefCell::new({
                    let mut h = crate::gc::Heap::new();
                    h.set_gc_mode(crate::gc::GcMode::Manual);
                    h
                })),
                cons_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                autoloads: Arc::new(RwLock::new(HashMap::new())),
                eval_ops: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                eval_ops_limit: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                deadline: std::cell::Cell::new(None),
            },
        }
    }
    /// Public API: evaluate a LispObject expression, returning a LispObject.
    /// Converts at the boundary to/from the internal Value representation.
    pub fn eval(&self, expr: LispObject) -> ElispResult<LispObject> {
        let _scope = crate::value::HeapScope::enter(self.state.heap.clone());
        let val = obj_to_value(expr);
        let result = eval(val, &self.env, &self.editor, &self.macros, &self.state)?;
        let obj_result = value_to_obj(result);
        {
            let mut heap = self.state.heap.lock();
            if heap.should_gc() {
                heap.collect();
            }
        }
        Ok(obj_result)
    }
    pub fn define(&self, name: &str, value: LispObject) {
        let id = obarray::intern(name);
        if name == "nil" || name == "t" {
            self.env.write().define_id(id, value);
            return;
        }
        if is_callable_value(&value) {
            self.state.set_function_cell(id, value);
        } else {
            self.state.set_value_cell(id, value);
        }
    }
    pub fn set_editor(&self, editor: Box<dyn EditorCallbacks>) {
        let mut e = self.editor.write();
        *e = Some(editor);
    }
    /// Set a maximum number of eval operations. 0 means unlimited.
    /// When the limit is reached, eval returns an error.
    pub fn set_eval_ops_limit(&self, limit: u64) {
        self.state
            .eval_ops_limit
            .store(limit, std::sync::atomic::Ordering::Relaxed);
    }
    /// Reset the eval operation counter to zero.
    pub fn reset_eval_ops(&self) {
        self.state
            .eval_ops
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }
    /// Force a full GC cycle on the interpreter's heap. Normally GC is
    /// Manual (only `(garbage-collect)` triggers it) so the heap can
    /// grow unbounded during bulk loads. This helper lets the bootstrap
    /// reclaim after every N forms, preventing OOM on large files like
    /// `cl-macs.el`.
    pub fn gc(&self) {
        self.state.with_heap(|heap| heap.collect());
    }
    /// Set a wall-clock deadline. `eval()` will check and error when past it.
    pub fn set_deadline(&self, deadline: std::time::Instant) {
        self.state.deadline.set(Some(deadline));
    }
    /// Clear the wall-clock deadline.
    pub fn clear_deadline(&self) {
        self.state.deadline.set(None);
    }
    /// Evaluate all forms in a source string. Returns the result of the last form,
    /// or the first error encountered (with the count of successful forms).
    pub fn eval_source(&self, source: &str) -> Result<LispObject, (usize, ElispError)> {
        let forms = crate::read_all(source).map_err(|e| (0, e))?;
        let mut result = LispObject::nil();
        for (i, form) in forms.into_iter().enumerate() {
            result = self.eval(form).map_err(|e| (i, e))?;
        }
        Ok(result)
    }
    /// Evaluate a Value expression directly (internal Value representation).
    pub fn eval_value(&self, expr: Value) -> ElispResult<Value> {
        let _scope = crate::value::HeapScope::enter(self.state.heap.clone());
        eval(expr, &self.env, &self.editor, &self.macros, &self.state)
    }
    /// Evaluate all forms in a source string and return a Value.
    pub fn eval_source_value(&self, source: &str) -> Result<Value, (usize, ElispError)> {
        let forms = crate::read_all(source).map_err(|e| (0, e))?;
        let _scope = crate::value::HeapScope::enter(self.state.heap.clone());
        let mut result = Value::nil();
        for (i, form) in forms.into_iter().enumerate() {
            let val = obj_to_value(form);
            result = eval(val, &self.env, &self.editor, &self.macros, &self.state)
                .map_err(|e| (i, e))?;
        }
        Ok(result)
    }
    /// Get a variable's value, or None if unbound.
    pub fn get(&self, name: &str) -> Option<LispObject> {
        self.env.read().get(name)
    }
    /// Returns `(total_calls, hot_functions_count)` from the JIT profiler.
    pub fn profiler_stats(&self) -> (u64, u64) {
        let profiler = self.state.profiler.read();
        (profiler.total_calls(), profiler.hot_function_count())
    }
    /// Return the current execution tier of `name`. A function that
    /// has never been called, or has been invalidated via redefinition,
    /// reports `Tier::Interp`.
    ///
    /// Implementation: a function is in `Tier::Compiled` when the
    /// profiler counts it as hot AND its current `def_version` matches
    /// the version we'd have compiled against. Without the `jit`
    /// feature, always `Tier::Interp`.
    pub fn jit_tier(&self, name: &str) -> crate::jit::Tier {
        #[cfg(feature = "jit")]
        {
            let sym = crate::obarray::intern(name);
            let Some(cell) = self.state.get_function_cell(sym) else {
                return crate::jit::Tier::Interp;
            };
            let crate::object::LispObject::BytecodeFn(ref bc) = cell else {
                return crate::jit::Tier::Interp;
            };
            let func_id = bc as *const _ as usize;
            let profiler = self.state.profiler.read();
            if profiler.should_compile(func_id) {
                crate::jit::Tier::Compiled
            } else {
                crate::jit::Tier::Interp
            }
        }
        #[cfg(not(feature = "jit"))]
        {
            let _ = name;
            crate::jit::Tier::Interp
        }
    }
    /// Eagerly compile the named function to native code, bypassing
    /// the profiler's hot-call threshold. Useful for tests, ahead-
    /// of-time tooling, and warm-up paths.
    ///
    /// Returns `Err(JitError::UnknownFunction)` when the name has no
    /// function cell, `Err(JitError::NotBytecode)` when the cell
    /// holds something other than bytecode (lambda, primitive,
    /// autoload), `Err(JitError::UnsupportedOpcode)` when the
    /// bytecode uses an opcode the compiler doesn't yet emit, and
    /// `Err(JitError::JitDisabled)` on a build without the `jit`
    /// feature.
    ///
    /// On success, subsequent calls to the named function go through
    /// the JIT until the next `set_function_cell` (which bumps
    /// `def_version` and orphans the compiled entry — see Phase D+).
    pub fn jit_compile(&self, name: &str) -> Result<(), crate::jit::JitError> {
        #[cfg(feature = "jit")]
        {
            let sym = crate::obarray::intern(name);
            let cell = self
                .state
                .get_function_cell(sym)
                .ok_or_else(|| crate::jit::JitError::UnknownFunction(name.to_string()))?;
            let crate::object::LispObject::BytecodeFn(ref bc) = cell else {
                return Err(crate::jit::JitError::NotBytecode(name.to_string()));
            };
            let func_id = bc as *const _ as usize;
            let version = self.state.def_version(sym);
            let mut jit = self.state.jit.write();
            jit.compile_with_version(func_id, bc, version)
                .map(|_| ())
                .ok_or(crate::jit::JitError::UnsupportedOpcode)
        }
        #[cfg(not(feature = "jit"))]
        {
            let _ = name;
            Err(crate::jit::JitError::JitDisabled)
        }
    }
    /// Snapshot of cumulative JIT counters and per-function version
    /// map. Intended for tests + diagnostic tooling.
    pub fn jit_stats(&self) -> crate::jit::JitStats {
        let profiler = self.state.profiler.read();
        crate::jit::JitStats {
            total_calls: profiler.total_calls(),
            hot_count: profiler.hot_function_count(),
            compiled_count: {
                #[cfg(feature = "jit")]
                {
                    profiler.hot_function_count()
                }
                #[cfg(not(feature = "jit"))]
                {
                    0
                }
            },
        }
    }
}
