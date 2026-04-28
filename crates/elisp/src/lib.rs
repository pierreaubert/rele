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
pub use eval::bootstrap;
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
pub use primitives::primitives_window::{clear_global_keybindings, lookup_global_key};

/// Look up the command symbol bound to `key_str` in the major mode
/// `mode_name`'s keymap (e.g. `dired-mode-map`, `text-mode-map`).
/// Returns the command's symbol name as a `String`, or `None` when:
/// - the mode has no keymap symbol bound,
/// - the keymap doesn't bind that key,
/// - the binding is a non-symbol (closure / number / nil).
///
/// `key_str` is the Emacs `(kbd ...)` form: `"RET"`, `"n"`, `"C-c h"`,
/// etc. We dispatch through the real `(lookup-key (symbol-value
/// MODEMAP) (kbd KEY))` so any keymap structure (sparse-keymap,
/// vector keymap, composed keymap with `:parent`) works the same way
/// it does in Emacs.
///
/// Used by client key handlers (Phase 4 in the dired plan): when the
/// current buffer's major mode is `dired-mode`, RET / n / p / etc.
/// dispatch through the upstream `dired-mode-map` instead of the
/// hard-coded text-mode actions.
pub fn lookup_mode_key(
    interp: &eval::Interpreter,
    mode_name: &str,
    key_str: &str,
) -> Option<String> {
    let escaped = key_str.replace('\\', "\\\\").replace('"', "\\\"");
    let src = format!(
        "(let ((map (intern (concat {mode_name:?} \"-map\")))) \
           (when (boundp map) \
             (let ((binding (lookup-key (symbol-value map) (kbd {escaped:?})))) \
               (and binding (symbolp binding) (symbol-name binding)))))",
        mode_name = mode_name,
        escaped = escaped,
    );
    let form = read(&src).ok()?;
    let result = interp.eval(form).ok()?;
    match result {
        LispObject::String(s) => Some(s),
        _ => None,
    }
}

/// Return the `interactive` spec string of an elisp defun, if any.
///
/// Walks the function cell of `name` looking for an
/// `(interactive "CODE...")` form. Returns the spec string with the
/// leading code letter and prompt (everything after the code).
/// `None` if the cell is unset, holds a primitive, or has no
/// `(interactive ...)` form.
///
/// Used by clients to drive the minibuffer prompt from elisp's
/// canonical interactive declaration instead of the Rust-side
/// `InteractiveSpec` enum.
#[must_use]
pub fn interactive_spec_for(name: &str, state: &eval::InterpreterState) -> Option<String> {
    let sym = obarray::intern(name);
    let cell = state.get_function_cell(sym)?;
    interactive_spec_from_function(&cell)
}

fn interactive_spec_from_function(cell: &LispObject) -> Option<String> {
    // Defuns are `(lambda (params) body...)`; lexical closures are
    // `(closure CAPTURED (params) body...)`. Bytecode functions
    // carry the spec on a dedicated field.
    if let LispObject::BytecodeFn(bf) = cell {
        let interactive = bf.interactive.as_deref()?;
        return interactive_spec_from_form(interactive);
    }
    let head = cell.car()?;
    let rest = cell.cdr()?;
    let body = match head.as_symbol().as_deref() {
        Some("lambda") => rest.cdr()?,         // skip params
        Some("closure") => rest.cdr()?.cdr()?, // skip captured + params
        _ => return None,
    };
    let mut cur = body;
    while let Some((form, next)) = cur.destructure_cons() {
        if let Some(s) = interactive_spec_from_form(&form) {
            return Some(s);
        }
        cur = next;
    }
    None
}

fn interactive_spec_from_form(form: &LispObject) -> Option<String> {
    // `(interactive "CODE")` — the spec is the first arg after the
    // `interactive` head. Other Emacs spec shapes (no arg, lisp
    // expression) aren't translated here; they'll fall back to the
    // Rust registry's interactive setting.
    let head = form.car()?;
    if head.as_symbol().as_deref() != Some("interactive") {
        return None;
    }
    let arg = form.cdr()?.car()?;
    match arg {
        LispObject::String(s) => Some(s),
        _ => None,
    }
}

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
pub fn is_user_defined_elisp_function(name: &str, state: &eval::InterpreterState) -> bool {
    let sym = obarray::intern(name);
    matches!(
        state.get_function_cell(sym),
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

    // ---- Kill ring ----
    fn kill_line(&mut self) {}
    /// Kill `n` words forward (negative = backward). Default no-op so
    /// embedders without a kill ring still load the trait cleanly.
    fn kill_word(&mut self, _n: i64) {}
    fn kill_region(&mut self) {}
    fn copy_region_as_kill(&mut self) {}
    fn yank(&mut self) {}
    fn yank_pop(&mut self) {}

    // ---- Rectangles ----
    fn delete_rectangle(&mut self) {}
    fn kill_rectangle(&mut self) {}
    fn yank_rectangle(&mut self) {}
    fn open_rectangle(&mut self) {}
    fn clear_rectangle(&mut self) {}
    fn string_rectangle(&mut self, _text: &str) {}

    // ---- Search / replace ----
    fn search_forward(&mut self, _needle: &str) -> Option<usize> {
        None
    }
    fn search_backward(&mut self, _needle: &str) -> Option<usize> {
        None
    }
    fn re_search_forward(&mut self, _pattern: &str) -> Option<usize> {
        None
    }
    fn re_search_backward(&mut self, _pattern: &str) -> Option<usize> {
        None
    }
    fn replace_match(&mut self, _replacement: &str) -> bool {
        false
    }
    fn replace_string(&mut self, _from: &str, _to: &str) -> usize {
        0
    }
    fn query_replace(&mut self, _from: &str, _to: &str) -> usize {
        0
    }
    fn replace_regexp(&mut self, _pattern: &str, _replacement: &str) -> usize {
        0
    }
    fn query_replace_regexp(&mut self, _pattern: &str, _replacement: &str) -> usize {
        0
    }

    // ---- Case ----
    fn upcase_word(&mut self) {}
    fn downcase_word(&mut self) {}

    // ---- Reorder ----
    fn transpose_chars(&mut self) {}
    fn transpose_words(&mut self) {}

    // ---- Mark ----
    fn set_mark(&mut self) {}
    fn exchange_point_and_mark(&mut self) {}

    // ---- Buffer list ----
    fn next_buffer(&mut self) {}
    fn previous_buffer(&mut self) {}

    // ---- View toggles ----
    fn toggle_preview(&mut self) {}
    fn toggle_line_numbers(&mut self) {}
    fn toggle_preview_line_numbers(&mut self) {}

    // ---- Buffer registry bridge (Phase 1) ----
    //
    // Elisp libraries — dired, ert, custom, every major mode — create
    // and switch between named buffers via `get-buffer-create`,
    // `set-buffer`, `current-buffer`, and friends. With these methods
    // implemented, the elisp interpreter delegates to the embedder's
    // buffer list so any buffer the elisp side creates appears in the
    // editor's UI. Embedders that don't have a buffer list (tests,
    // headless tools) get the trait defaults — `None` from queries,
    // `false` from mutations — and the elisp layer falls back to its
    // own thread-local stub registry (`crate::buffer::Registry`).

    /// Name of the currently selected buffer in the editor, if any.
    fn current_buffer_name(&self) -> Option<String> {
        None
    }
    /// All open buffer names, in display order.
    fn list_buffer_names(&self) -> Vec<String> {
        Vec::new()
    }
    /// Make `name` the current buffer if it exists. Returns true on
    /// success, false if the buffer is unknown.
    fn switch_to_buffer_by_name(&mut self, _name: &str) -> bool {
        false
    }
    /// Get-or-create the buffer named `name`. Returns true on success
    /// (whether the buffer already existed or was just created),
    /// false when the embedder can't create new buffers.
    fn get_or_create_buffer(&mut self, _name: &str) -> bool {
        false
    }
    /// Kill the buffer named `name`. Returns true on success.
    fn kill_buffer_by_name(&mut self, _name: &str) -> bool {
        false
    }
    /// Replace the entire text of the named buffer. Used by dired's
    /// `g` (revert) which rewrites the listing wholesale, and by
    /// `(erase-buffer)` followed by inserts.
    fn set_buffer_text(&mut self, _name: &str, _text: &str) -> bool {
        false
    }
    /// Return the major mode name (e.g. `"dired-mode"`) of the
    /// current buffer, used by Phase 4's mode-map dispatch.
    fn current_buffer_major_mode(&self) -> Option<String> {
        None
    }
    /// Set the current buffer's major mode. Triggered by
    /// `(dired-mode)`, `(text-mode)`, etc. so the editor knows which
    /// keymap and rendering rules apply.
    fn set_current_buffer_major_mode(&mut self, _mode: &str) {}

    /// `(keyboard-quit)` — Emacs's `C-g`. Cancel any in-progress
    /// modal state: prefix arg, isearch, command palette, selection,
    /// pending C-x / M-g chord, find/replace bar. Embedders that
    /// also have spawned cancellable work (LSP requests, async
    /// reads) should signal it here.
    fn keyboard_quit(&mut self) {}
}
