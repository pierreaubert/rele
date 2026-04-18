# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Performance — required reading before touching editor paths

See [`PERFORMANCE.md`](./PERFORMANCE.md) for the 10 durable rules this
project follows to stay fast. In short:

1. Never block the UI thread.
2. Don't clone the whole buffer (avoid `document.text()` outside save/export).
3. Cache buffer-derived data, key it on `DocumentBuffer::version()`.
4. Respect viewport virtualization — do per-line work only for visible lines.
5. Cap pathological inputs at render time (long lines).
6. Edit paths must be O(log n); no whole-buffer scans per keystroke.
7. Long-running work must be cancellable via the shared cancellation flag.
8. Performance-sensitive changes need a bench before/after.
9. Parse once, query many (cache parse trees, not just text).
10. Views observe minimally — prefer dirty flags / narrow entities over
    god-state observers.

Clippy lints in `clippy.toml` enforce rule 1.

## Project Overview

**rele** (Rust Emacs-Lisp Editor) — a multi-client markdown editor with Emacs semantics. The workspace has three published crates plus a vendored toolkit:

| Crate | Package name | Purpose |
|-------|-------------|---------|
| `crates/elisp` | `rele-elisp` | Emacs Lisp interpreter (reader, evaluator, bytecode VM, GC, optional Cranelift JIT) |
| `crates/server` | `rele-server` | Shared editor core: document model, commands, macros, markdown parsing, import/export |
| `crates/app-gpui` | `rele-gpui` | GPUI native UI client (views, keybindings, markdown rendering) |
| `crates/app-tui` | `rele-tui` | Terminal UI client (TUI components, input handling) |
| `crates/gpui-toolkit/*` | `gpui-ui-kit`, `gpui-builder`, `gpui-design`, `gpui-keybinding`, `gpui-pretext` | UI toolkit (vendored as git deps from sotf project) |

## Build & Test Commands

```bash
# Build everything
cargo build

# Run the editor
cargo run -p gpui-md --bin gpui-md-editor

# Test all workspace crates
cargo test

# Test a single crate
cargo test -p gpui-md
cargo test -p gpui-elisp

# Run a single test by name
cargo test -p gpui-md test_cursor
cargo test -p gpui-elisp -- eval::tests::test_arithmetic

# Check with optional features
cargo check -p gpui-md --features pdf-export
cargo check -p gpui-elisp --features jit

# Lint (clippy pedantic is enabled workspace-wide)
cargo clippy --workspace
```

## Lint Configuration

The workspace enables aggressive Clippy lints: `pedantic`, `complexity`, `correctness`, `perf`, `style`, `suspicious`, and `cargo` groups are all `warn`. Many `restriction` lints are also enabled (see `[workspace.lints.clippy]` in root `Cargo.toml`). New code must pass all of these.

## Architecture

### Multi-Client Design

The shared **server** crate (`rele-server`) contains all non-UI logic, allowing both TUI and GPUI clients to reuse:
- Document model and editing operations
- Command metadata and keybinding infrastructure
- Keyboard macro recording/playback
- Markdown parsing (GFM via comrak)
- Import/export (docx, pdf)

Each client imports from `rele-server` and implements its own:
- Command registry and handlers (bound to client-specific state)
- UI/rendering (GPUI views vs TUI components)
- Keybinding presentation (gpui-keybinding vs TUI event loop)

### rele-elisp crate

A standalone Emacs Lisp interpreter, no UI dependency:
- **reader.rs** — S-expression parser (`read`, `read_all`, `detect_lexical_binding`)
- **eval/** — Tree-walking evaluator (`Interpreter`), special forms, builtins, dynamic scoping, editor integration
- **vm.rs** — Stack-based bytecode VM (Emacs 30.x opcode set), NaN-boxed values
- **gc.rs** — Garbage collector with `HeapScope` for VM/interpreter integration
- **jit/** — Optional Cranelift-based JIT compiler (behind `jit` feature flag)
- **`EditorCallbacks` trait** — bridge between elisp and the editor; clients implement this to let Lisp code manipulate buffers

### rele-server crate

Shared editor core consumed by all clients:
- **document/** — Core model: `DocumentBuffer` (ropey-backed), `EditorCursor`, `EditHistory` (undo/redo), `KillRing`, `BufferList`
- **commands.rs** — `CommandArgs`, `CommandCategory`, `InteractiveSpec` — metadata types for command definition
- **macros.rs** — `RecordedAction`, `KeyboardMacro`, `MacroState` — keyboard macro recording (generic, not tied to UI state)
- **markdown/parser.rs** — GFM parsing via comrak (shared between clients)
- **export/** — `docx.rs`, `pdf.rs` — export format handlers
- **import/** — `docx.rs` — import format handlers

### rele-gpui crate (GPUI client)

GPUI-specific application built on `rele-server`:
- **document/** — *(re-exported from rele-server)* — document model types
- **markdown/** — Mix of shared and GPUI-specific:
  - `parser.rs` — *(re-exported from rele-server)* — GFM parsing
  - `renderer.rs` — GPUI element rendering from AST
  - `source_map.rs` — Click-to-locate (preview position → editor line)
  - `text_layout.rs`, `syntax_highlight.rs`, `theme_colors.rs` — GPUI-specific rendering
- **views/** — GPUI views: `MainView`, `EditorPane`, `PreviewPane`, `ToolbarView`, `FindBar`, `CommandPalette`, `MinibufferView`
- **commands.rs** — `CommandRegistry<S=MdAppState>` and `register_builtin_commands()` — app-specific command implementations
- **state.rs** — `MdAppState`: owns document, cursor, elisp interpreter, command registry, views. Implements `EditorCallbacks` for elisp integration
- **keybindings.rs** — `MdKeybindingProvider` for Vim/Emacs/VSCode presets via `gpui-keybinding`

### rele-tui crate (TUI client)

Terminal UI client (in progress):
- Imports from `rele-server` for document model, commands, macros, parsing, import/export
- Implements its own TUI views, keybinding handler, and command registry

### Key Integration Points

1. **Elisp <-> Editor**: Client state implements `EditorCallbacks` trait so Lisp code can call `(insert ...)`, `(goto-char ...)`, etc. on the live buffer
2. **Commands**: Client creates `CommandRegistry` and registers handlers via `register_fn()` or `register()`. Handlers take `&mut ClientState` + `CommandArgs`
3. **Keybindings**: Keybinding metadata (name, category, description) is defined in keybindings module. Client keybinding handler looks up command by name in registry and executes it

## Feature Flags

- `gpui-md`: `pdf-export` (genpdf), `google-docs` (reqwest)
- `gpui-elisp`: `jit` (Cranelift JIT compilation)

## Testing Notes

- Tests for `gpui-md` are in `crates/app-gpui/tests/` (integration-style, one file per area)
- Tests for `gpui-elisp` are inline in `crates/elisp/src/eval/tests.rs`
- GPUI tests use `gpui`'s `test-support` feature (dev-dependency)
- Toolchain is stable Rust (see `rust-toolchain.toml`)
