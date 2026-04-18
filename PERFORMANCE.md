# Rele Performance Rules

These rules exist so rele stays responsive as the codebase grows. Every rule
has a **reason**, a **how to apply**, and, where relevant, an **enforcement**
mechanism (clippy config, bench threshold, runtime assert).

**Targets:**

- p99 input-to-paint latency ≤ 16 ms (one frame at 60 Hz) on files up to 5 MB.
- No single frame > 50 ms during any interactive operation.
- No stop-the-world pause > 5 ms during typing.
- Benchmarks in `crates/server/benches/` must stay within 10 % of the
  recorded baseline (`docs/perf-baseline.md`).

---

## Rule 1: Never block the UI thread

**Reason.** This is the number one reason Emacs feels slow. Once the UI
thread stalls, nothing the user does takes effect until it unblocks.

**How to apply.**

- On the GPUI main thread and inside any `state.update(cx, |s, _cx| ...)`
  closure, do not call:
  - `std::fs::{read_to_string, read, write, metadata, canonicalize}`
  - `std::process::Command::{output, status, spawn + wait}`
  - `std::thread::sleep`
  - blocking `reqwest`, any synchronous HTTP client
  - `tokio::runtime::Runtime::block_on`
  - any LSP request that awaits a response
- For these operations, use `cx.spawn(async move |cx| ...)` (GPUI),
  `registry.runtime_handle().spawn(...)` (tokio runtime on
  `LspRegistry`), or a channel back from a worker thread.
- Results come back via `state.update` inside the async task, or via an
  `mpsc` receiver that the UI drains.
- For the TUI, the same rule applies: long work spawns on the tokio
  runtime and reports via channel; the event loop polls with
  `try_recv()`.

**Enforcement.** Clippy `disallowed_methods` in the workspace
`clippy.toml` lists the blocking APIs. Violations are compile-time warnings.
If you genuinely need one on a background path, suppress with
`#[allow(clippy::disallowed_methods)]` and a comment explaining why.

---

## Rule 2: Don't clone the whole buffer

**Reason.** `DocumentBuffer::text()` allocates a `String` of the entire
buffer. For a 5 MB buffer that's a 5 MB allocation and copy every call.
Emacs's `(buffer-string)` had the same footgun.

**How to apply.**

- Operate on `rope.slice(range).chunks()` or `rope.line(n)` instead.
- `document.text()` is allowed only in **save/export paths** — places that
  genuinely must produce a contiguous `String` for an external API
  (`std::fs::write`, `comrak::markdown_to_html`, docx/pdf export, LSP
  `didOpen`/`didSave`). Every other call site is a bug.
- For LSP `didChange`, use the change journal
  (`DocumentBuffer::drain_changes()`) — it already records incremental
  edits.

**Enforcement.** Grep for `document.text()` during review. The set of
permitted call sites is small and stable; new ones should be justified in
the commit message.

---

## Rule 3: Cache buffer-derived data, key it on `version()`

**Reason.** `MainView` used to recompute word count on every frame
(O(n) per render on whole-buffer text). That's the kind of thing that
makes a 50k-line file feel sluggish for no visible reason.

**How to apply.**

- `DocumentBuffer::version()` returns a monotonic counter that bumps on
  every mutation.
- If your view computes something from the buffer that is O(n) or worse,
  cache it on `DocumentBuffer` (or in a view-local `Cell`) keyed on the
  version at compute time; recompute only when the version changes.
- Prefer adding the cache on `DocumentBuffer` so multiple views share it.

---

## Rule 4: Respect viewport virtualization

**Reason.** `EditorPane` only emits lines in a viewport window to GPUI.
Work done for off-screen lines is wasted.

**How to apply.**

- Syntax highlighting, diagnostics rendering, click targets, and any
  per-line decoration run only over the visible range
  (`render_start..render_end` in `editor_pane.rs`).
- Preview pane does the same via GPUI's `uniform_list` / scroll item
  bounds.
- If you need a buffer-wide structure (e.g. tree-sitter parse tree),
  build and cache it once, then **query** the visible range — don't
  traverse the whole tree on render.

---

## Rule 5: Cap pathological inputs at render time

**Reason.** One 10 MB line (minified JSON, compiled CSS) must not freeze
the editor. The display path is the right place to enforce this — the
underlying `DocumentBuffer` stays untouched.

**How to apply.**

- `build_line_text_runs` in `editor_pane.rs` truncates lines to at most
  `MAX_LINE_DISPLAY_CHARS` (default: 10 000), appending a visible
  ellipsis marker.
- No TUI analogue yet, but follow the same pattern if one is needed.

---

## Rule 6: Edit paths must be rope-friendly and bounded

**Reason.** A single keystroke should be O(log n) end-to-end. Anything
quadratic in buffer size compounds fast.

**How to apply.**

- Never iterate `document.chars()` without bounding the range.
- Use `rope.char_to_line` / `rope.line_to_char` for navigation — they're
  O(log n). Don't write your own full scan.
- `DocumentBuffer::insert`, `remove`, `drain_changes` already satisfy
  this — keep them that way.

---

## Rule 7: Long-running work must be cancellable

**Reason.** Users press `C-g`. If the work ignores that, we've
reimplemented Emacs's "frozen for an unknowable reason" problem.

**How to apply.**

- Any command that can take more than one frame must accept a
  `CancellationFlag` (Arc<AtomicBool>) and check it at iteration
  boundaries (per-line, per-file, per-chunk).
- `MdAppState::cancel_long_op()` sets the flag. `abort` / `C-g` call it.
- Spawned tasks clone the `Arc` before `move` into the async block.

---

## Rule 8: Every performance-sensitive change needs a bench

**Reason.** Without numbers, "it feels faster" is not evidence.

**How to apply.**

- Before changing a hot path, run `cargo bench -p rele-server` and
  record the number.
- After the change, run again and compare. If it regressed, the PR
  explains why the trade-off is worth it.
- Add a new bench for any new hot path you introduce.

**Enforcement.** CI gate (TODO) on > 10 % regression against the baseline.

---

## Rule 9: Parse once, query many

**Reason.** Comrak / tree-sitter / LSP hover results are expensive to
produce and cheap to store. Recomputing them per render wastes work.

**How to apply.**

- Markdown AST: parse on a worker thread, cache by
  `DocumentBuffer::version()`, preview renders from the cached snapshot.
- Tree-sitter tree: one per buffer, re-parsed incrementally from the
  change journal.
- LSP hover / completion: already cached on `MdAppState` until the
  cursor moves (see `update_preferred_column`).

---

## Rule 10: Prefer specific entities over god-state observers

**Reason.** `cx.observe(&state, |_, _, cx| cx.notify())` on a big
`MdAppState` entity means every view re-renders on every mutation —
even ones irrelevant to that view.

**How to apply.**

- Views observe only what they render. If that's not achievable with
  the current entity shape, use a **dirty-flag** field
  (e.g. `state.dirty_preview`) that the view reads and clears.
- When in doubt, measure: add a counter, log re-render counts, see if
  a cursor move triggers a preview re-render.

---

## Known blocking-call debt (audit at 2026-04-18)

30 sites across the workspace violate Rule 1 today. Clippy's
`disallowed_methods` lint (configured in `clippy.toml`) makes these
visible and prevents new ones from sneaking in, but existing call sites
are tracked here for incremental cleanup.

| File | Count | Typical issue |
|------|------:|---------------|
| `crates/app-gpui/src/state.rs`        | 10 | file-dialog handlers, `open_file_as_buffer` consumers |
| `crates/app-gpui/src/bin/rele.rs`     |  9 | `SaveFile` / `OpenFile` / `ImportDocx` actions |
| `crates/elisp/src/eval/builtins.rs`   |  3 | elisp `load` primitive, `insert-file-contents` |
| `crates/app-tui/src/state.rs`         |  2 | save + open paths |
| `crates/server/src/lsp/config.rs`     |  1 | config load (startup only — acceptable) |
| `crates/elisp/src/eval/mod.rs`        |  1 | elisp `load` wrapper |
| `crates/elisp-spec-tests/src/replay.rs` | 1 | test harness (acceptable) |
| `crates/app-tui/src/main.rs`          |  1 | initial file read (startup only — acceptable) |
| `crates/app-gpui/src/dired.rs`        |  1 | directory listing |
| `crates/app-gpui/src/commands.rs`     |  1 | command handler |

**Acceptable today:** startup-only paths (one-shot, never on hot path),
test harnesses, and calls inside `cx.spawn` async blocks (GPUI's
executor, off the main thread — though these should migrate to
`tokio::fs` over time).

**Not acceptable:** any call on a synchronous command handler path. The
10 in `state.rs` and 9 in `rele.rs` need individual review; some are
inside `cx.spawn` (OK-ish) and some are directly inside
`state.update(cx, ...)` closures (the bad shape). Cleanup should
proceed file by file with a bench to confirm the move-to-worker doesn't
regress cold-start latency.

## Follow-up projects (descoped from the initial perf PR)

These are sized like their own PRs and are tracked separately rather
than bundled with the initial rules:

### Tree-sitter for syntax highlighting (P1.1) — LANDED
`crates/server/src/syntax/` hosts `TreeSitterHighlighter` (markdown +
rust grammars) behind the `Highlighter` trait. The editor pane picks
tree-sitter for `.md` / `.rs` files, falls back to the regex
`highlight_line` for everything else.

**Incremental parsing is enabled.** `TextChange` carries pre-edit byte
offsets and byte columns, computed once inside `DocumentBuffer::insert`
/ `remove` before the rope mutates. The edit path calls
`notify_highlighter` before `lsp_did_change`, so the highlighter peeks
the journal, builds `tree_sitter::InputEdit`s from the byte offsets,
calls `tree.edit()` + incremental `parser.parse(..., Some(&tree))`, and
reuses every subtree unaffected by the edit.

Numbers (see `docs/perf-baseline.md`):

- Warm viewport query: ~70 µs, O(1) in buffer size.
- Incremental reparse on a 1000-section markdown: **4.9 ms**
  (was 40 ms full reparse — 8× speedup).
- Incremental reparse on a 1000-function Rust file: **2.0 ms**
  (was 27 ms — 14× speedup).

Both comfortably inside the 16 ms frame budget, so tree-sitter colours
stay up-to-date on every keystroke in large files.

Remaining nits (not urgent):
- Cold parse on file open is still O(n); for > 10 k-section markdown
  that's > 400 ms. Re-consider editor-side debounce when/if this
  becomes a workflow.
- The `LineHighlighterAdapter` (regex fallback) always re-scans the
  whole buffer on edit. Could be narrowed to the affected-line
  window using the change journal for parity with tree-sitter.

### Worker-thread markdown preview parse (P3.1)
Moving `comrak::parse_document` off the UI thread requires a Send-safe
intermediate — `comrak::Arena<AstNode>` is built on `RefCell` and
self-references so can't cross threads. Realistic paths:

1. Switch to `pulldown-cmark`, which emits a `Vec<Event>` that is
   cheaply made Send. Requires rewriting
   `crates/app-gpui/src/markdown/renderer.rs` (~500 LOC) from comrak
   AST walking to event consumption.
2. Serialize comrak AST to a custom Send intermediate on the worker,
   consume on the main thread. Parallel implementation work; still
   does the parse twice in practice (once to serialize, once to
   rebuild AST for rendering).

Neither is small. The **150 ms debounce** on `PreviewPane` already
coalesces burst typing, so a 1000-section markdown parses at most once
per typing pause (6.5 ms, see `markdown_parse` bench). That's 4 % CPU
during sustained editing — fine in practice. Revisit only if very
large docs (> 1 M words) become a common workflow.

### Elisp JIT / GC (P2 — separate project)
Cranelift JIT by default on hot bytecode functions, and incremental
GC. Deferred per user request.

## Review checklist

When reviewing a PR that touches the editor path:

- [ ] Any new blocking call on the UI thread? (Rule 1)
- [ ] New `document.text()` call — is it on a save/export path? (Rule 2)
- [ ] Any O(n)-in-buffer-size computation per frame or per keystroke? (Rule 3, 6)
- [ ] Syntax / decoration work scoped to the viewport? (Rule 4)
- [ ] Does it assume lines are short? (Rule 5)
- [ ] Long-running? Honours cancellation? (Rule 7)
- [ ] Bench before/after, numbers in the PR description? (Rule 8)
- [ ] View observes minimally? (Rule 10)
