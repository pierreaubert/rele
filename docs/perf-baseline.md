# Performance Baseline

Recorded baseline numbers from `cargo bench -p rele-server` on the development
machine (macOS, Apple Silicon). Regressions > 10 % against these numbers
should be called out in the PR that caused them.

## How to refresh

```bash
cargo bench -p rele-server --bench edit
cargo bench -p rele-server --bench search
cargo bench -p rele-server --bench markdown_parse
```

Results are written under `target/criterion/`. HTML reports at
`target/criterion/report/index.html`.

For quick iteration during a change, use `--quick` (lower confidence,
faster):

```bash
cargo bench -p rele-server --bench edit -- --quick
```

## Baseline numbers (2026-04-18, Apple Silicon, `--quick`)

Format: `bench name — N_lines — time_per_iter`.

### edit

| bench | 100 lines | 10k lines | 100k lines |
|-------|-----------|-----------|------------|
| `insert_single_char`       | _todo_  | _todo_  | _todo_  |
| `insert_paste_10kb`        | _todo_  | _todo_  | _todo_  |
| `remove_range_100`         | _todo_  | _todo_  | _todo_  |
| `snapshot_clone`           | _todo_  | _todo_  | _todo_  |
| `word_count` (cold)        | 4.6 µs  | 464 µs  | 4.6 ms  |
| `word_count` (warm/cached) | 3 ns    | 3 ns    | 3 ns    |

`word_count` caching yields ~1.5 million× speedup at 100k lines. This is
the shape of win Rule 3 is meant to protect.

### search

| bench | 1k lines | 10k lines | 100k lines |
|-------|----------|-----------|------------|
| `search_literal_forward`   | _todo_ | _todo_ | _todo_ |
| `search_literal_backward`  | _todo_ | _todo_ | _todo_ |
| `re_search_forward`        | _todo_ | _todo_ | _todo_ |

### markdown_parse

| bench | 10 sections | 100 sections | 1000 sections |
|-------|-------------|--------------|---------------|
| `parse_markdown` (comrak)  | 66 µs  | 645 µs | 6.5 ms |

### syntax (tree-sitter, `--quick`)

| bench | small | medium | large |
|-------|-------|--------|-------|
| `tree_sitter_parse_markdown_cold` | 912 µs (10 sec) | 4.4 ms (100 sec) | 40 ms (1 k sec) |
| `tree_sitter_parse_rust_cold`     | 11 ms (10 fn)   | 12.6 ms (100 fn) | 27 ms (1 k fn) |
| `tree_sitter_highlight_range_warm` (80-line viewport) | 67 µs (100 sec) | 67 µs (1 k sec) | — |
| `tree_sitter_incremental_rust_insert_char`     | 200 µs (100 fn) | **2.0 ms (1 k fn)** | — |
| `tree_sitter_incremental_markdown_insert_char` | 474 µs (100 sec) | **4.9 ms (1 k sec)** | — |

**Interpretation.**

- Warm viewport queries are **O(1) in buffer size** — confirms
  virtualization. We pay ~70 µs per frame regardless of how big the file
  is.
- Cold parse is hit only once (buffer open). On a 1000-section markdown
  that's 40 ms; user-perceivable but one-shot.
- **Incremental parse** is now the per-keystroke cost. At 1000 sections
  of markdown it's **4.9 ms** — comfortably inside the 16 ms frame
  budget. 1000-fn Rust is **2.0 ms**. Speedup over cold: **8× for
  markdown, 14× for Rust**.
- This means tree-sitter colors stay up-to-date on every keystroke even
  in large files, with room left for LSP diagnostics, cursor rendering,
  and input handling.
- For files larger than ~10 k sections, cold parse becomes painful
  (> 400 ms projected) and editor-side debounce may be worth
  re-considering. Not a common workflow today.

## Recording new numbers

Replace `_todo_` with real numbers once a full non-quick run is done
(`cargo bench`, which takes several minutes). Commit the updated baseline
in the PR that records it.

## Regression policy

- `cargo bench` in CI compares against this baseline (TODO: wire up).
- Any bench > 10 % slower must be explained in the PR; the PR either:
  - justifies the trade-off (e.g. new feature, correctness fix), and
    updates the baseline number above, **or**
  - the change is revised to not regress.
