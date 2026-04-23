pub mod buffer;
pub mod emacs;
mod error;
pub mod eval;
pub mod gc;
pub mod jit;
pub mod obarray;
mod object;
pub mod primitives;
mod reader;
pub mod value;
pub mod vm;

pub use error::{ElispError, ElispResult};
pub use eval::Interpreter;
pub use object::{BytecodeFunction, LispObject, global_cons_count};
pub use primitives::add_primitives;
pub use primitives::core::call_primitive;
pub use reader::{detect_lexical_binding, read, read_all};

// Re-export primitives submodules at crate root for compatibility
pub use primitives::primitives_buffer;
pub use primitives::primitives_cl;
pub use primitives::primitives_eieio;
pub use primitives::primitives_file;
pub use primitives::primitives_modules;
pub use primitives::primitives_value;
pub use primitives::primitives_window;

/// Does `name` resolve to a user-authored callable (defun / defmacro
/// / bytecode) rather than a built-in primitive?
///
/// Clients use this to decide whether to dispatch a command through
/// the elisp interpreter or fall back to their Rust registry.
/// `add_primitives` registers every built-in primitive (e.g.
/// `forward-char`, `point`, `buffer-string`) in the obarray as
/// `LispObject::Primitive(name)` — a function cell being populated
/// is therefore not enough to decide that user code owns the command.
/// A `Primitive` value always means "the Rust handler should run";
/// anything else (a lambda produced by `(defun ...)`, a bytecode
/// closure, a macro) is user-authored.
#[must_use]
pub fn is_user_defined_elisp_function(name: &str) -> bool {
    let sym = obarray::intern(name);
    matches!(
        obarray::get_function_cell(sym),
        Some(cell) if !matches!(cell, LispObject::Primitive(_))
    )
}

/// Bridge between the elisp interpreter and whatever client owns the
/// buffer/cursor (the TUI's `TuiAppState`, the GPUI client's
/// `MdAppState`, …).
///
/// Methods come in three groups:
/// - **Queries** (`point`, `buffer_string`, …): pure reads, no side
///   effects.
/// - **Mutations** (`insert`, `delete_char`, …): modify the buffer.
///   These must keep the document's change journal consistent so LSP
///   and tree-sitter see the edits.
/// - **Navigation** (`forward_char`, `forward_line`,
///   `move_beginning_of_line`, …): move the cursor. Counts are
///   signed — negative goes backward — so elisp can map both
///   `forward-char` and `backward-char` onto the same primitive.
///
/// Default implementations exist for the post-initial-landing methods
/// so embedders can migrate at their own pace without the trait
/// breaking compilation.
pub trait EditorCallbacks: Send + Sync {
    // ---- Queries ----
    fn buffer_string(&self) -> String;
    fn buffer_size(&self) -> usize;
    fn point(&self) -> usize;
    /// The smallest valid position in the buffer. Emacs convention is
    /// 1; we currently use 0 — conversions live in the elisp layer.
    fn point_min(&self) -> usize {
        0
    }
    /// One past the last valid position. Equivalent to `buffer_size`.
    fn point_max(&self) -> usize {
        self.buffer_size()
    }

    // ---- Mutations ----
    fn insert(&mut self, text: &str);
    fn delete_char(&mut self, n: i64);
    fn find_file(&mut self, path: &str) -> bool;
    fn save_buffer(&mut self) -> bool;

    // ---- Navigation ----
    fn goto_char(&mut self, pos: usize);
    fn forward_char(&mut self, n: i64);
    /// Move forward `n` lines (negative = backward), preserving the
    /// preferred column for vertical movement.
    fn forward_line(&mut self, _n: i64) {
        // Default: no-op. Real clients override.
    }
    /// Move to the start of the current line.
    fn move_beginning_of_line(&mut self) {}
    /// Move to the end of the current line (before the newline).
    fn move_end_of_line(&mut self) {}
    /// Move to the start of the buffer.
    fn move_beginning_of_buffer(&mut self) {
        self.goto_char(self.point_min());
    }
    /// Move to the end of the buffer.
    fn move_end_of_buffer(&mut self) {
        self.goto_char(self.point_max());
    }

    // ---- Edit history ----
    fn undo(&mut self) {}
    fn redo(&mut self) {}
}
