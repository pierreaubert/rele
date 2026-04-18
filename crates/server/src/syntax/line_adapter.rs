//! Adapter: a [`LineHighlighter`] (one-line-at-a-time, with tiny state
//! carried across) becomes a [`Highlighter`] (whole-buffer,
//! range-queried). Used for the regex-based markdown highlighter and any
//! future language that doesn't warrant a tree-sitter grammar.
//!
//! Strategy: on `on_edit`, re-scan the whole buffer line by line,
//! carrying `LineHighlighter::State` across lines, and cache the
//! per-line ranges keyed by document version. `highlight_range` slices
//! into the cache.
//!
//! This is O(total_lines) on every edit, which is why tree-sitter is the
//! default for languages with a grammar. For markdown where the regex
//! rules are simple and per-line, the constant factor is tiny.

use crate::document::buffer::TextChange;
use crate::syntax::highlight::{HighlightRange, Highlighter, LineHighlighter};

/// Wrap a `LineHighlighter` as a `Highlighter`. Caches per-line ranges;
/// invalidates the cache on every `on_edit`.
pub struct LineHighlighterAdapter<L: LineHighlighter> {
    inner: L,
    /// `cache[i]` holds the ranges for buffer line `i`. Length follows
    /// the rope's line count at the last `on_edit`.
    cache: Vec<Vec<HighlightRange>>,
}

impl<L: LineHighlighter> LineHighlighterAdapter<L> {
    pub fn new(inner: L) -> Self {
        Self {
            inner,
            cache: Vec::new(),
        }
    }
}

impl<L: LineHighlighter> std::fmt::Debug for LineHighlighterAdapter<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineHighlighterAdapter")
            .field("cached_lines", &self.cache.len())
            .finish()
    }
}

impl<L: LineHighlighter> Highlighter for LineHighlighterAdapter<L> {
    fn on_edit(
        &mut self,
        rope: &ropey::Rope,
        _changes: &[TextChange],
        _full_sync: bool,
    ) {
        // Simple correct path: rescan the whole buffer. Changes are
        // ignored because LineHighlighter is line-local with tiny state;
        // re-running it is cheap and avoids having to recompute where
        // the state boundary shifted.
        let total_lines = rope.len_lines();
        self.cache.clear();
        self.cache.reserve(total_lines);
        let mut state = L::State::default();
        for i in 0..total_lines {
            let line_slice = rope.line(i);
            let line_str = line_slice.to_string();
            let stripped = line_str.strip_suffix('\n').unwrap_or(&line_str);
            let (ranges, next_state) = self.inner.highlight_line(stripped, state.clone());
            self.cache.push(ranges);
            state = next_state;
        }
    }

    fn highlight_range(
        &self,
        _rope: &ropey::Rope,
        start_line: usize,
        end_line: usize,
    ) -> Vec<(usize, Vec<HighlightRange>)> {
        let end = end_line.min(self.cache.len());
        let start = start_line.min(end);
        (start..end)
            .filter_map(|i| {
                let ranges = self.cache.get(i).cloned().unwrap_or_default();
                if ranges.is_empty() {
                    None
                } else {
                    Some((i, ranges))
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::highlight::Highlight;
    use ropey::Rope;

    /// Minimal LineHighlighter that classifies every line as `Plain`
    /// unless it starts with `#`, in which case the `#` prefix is a
    /// `Heading`. State tracks whether we're inside a ```-fenced block,
    /// so we can verify state carries across lines.
    struct Toy;
    #[derive(Clone, Default)]
    struct ToyState {
        in_fence: bool,
    }
    impl LineHighlighter for Toy {
        type State = ToyState;
        fn highlight_line(
            &self,
            line: &str,
            state: ToyState,
        ) -> (Vec<HighlightRange>, ToyState) {
            if line.starts_with("```") {
                return (
                    vec![HighlightRange {
                        start_col: 0,
                        end_col: line.chars().count(),
                        kind: Highlight::Fence,
                    }],
                    ToyState {
                        in_fence: !state.in_fence,
                    },
                );
            }
            if state.in_fence {
                return (
                    vec![HighlightRange {
                        start_col: 0,
                        end_col: line.chars().count(),
                        kind: Highlight::Code,
                    }],
                    state,
                );
            }
            if let Some(rest) = line.strip_prefix("# ") {
                let _ = rest;
                return (
                    vec![HighlightRange {
                        start_col: 0,
                        end_col: 1,
                        kind: Highlight::Heading,
                    }],
                    state,
                );
            }
            (Vec::new(), state)
        }
    }

    #[test]
    fn adapter_caches_and_carries_state_across_lines() {
        let mut adapter = LineHighlighterAdapter::new(Toy);
        let rope = Rope::from_str("# heading\n```\ncode1\ncode2\n```\nplain\n");
        adapter.on_edit(&rope, &[], true);
        let out = adapter.highlight_range(&rope, 0, 6);
        // Line 0: heading marker
        assert!(out.iter().any(
            |(l, r)| *l == 0 && r.iter().any(|h| h.kind == Highlight::Heading)
        ));
        // Line 2 and 3 are inside the fence -> Code
        assert!(out.iter().any(
            |(l, r)| *l == 2 && r.iter().any(|h| h.kind == Highlight::Code)
        ));
        assert!(out.iter().any(
            |(l, r)| *l == 3 && r.iter().any(|h| h.kind == Highlight::Code)
        ));
        // Line 5 (plain) has no ranges -> not present in output
        assert!(!out.iter().any(|(l, _)| *l == 5));
    }

    #[test]
    fn adapter_range_clipped_to_window() {
        let mut adapter = LineHighlighterAdapter::new(Toy);
        let rope = Rope::from_str("# a\n# b\n# c\n");
        adapter.on_edit(&rope, &[], true);
        let out = adapter.highlight_range(&rope, 1, 2);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, 1);
    }

    #[test]
    fn adapter_reedit_refreshes_cache() {
        let mut adapter = LineHighlighterAdapter::new(Toy);
        let rope1 = Rope::from_str("# heading\n");
        adapter.on_edit(&rope1, &[], true);
        let out1 = adapter.highlight_range(&rope1, 0, 1);
        assert!(!out1.is_empty());

        let rope2 = Rope::from_str("plain line\n");
        adapter.on_edit(&rope2, &[], true);
        let out2 = adapter.highlight_range(&rope2, 0, 1);
        // New rope has no headings; cache should reflect that.
        assert!(out2.is_empty());
    }
}
