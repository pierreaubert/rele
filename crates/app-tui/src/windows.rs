//! Emacs-style window management for the TUI.
//!
//! A [`Window`] is a binary tree whose leaves display content
//! ([`WindowContent`]) and whose internal nodes split their area
//! horizontally (`HSplit` — top / bottom) or vertically (`VSplit` —
//! left / right). Focus is tracked as an in-order leaf index — 0 is
//! the top-left-most leaf, incrementing left-to-right then
//! top-to-bottom.
//!
//! The initial implementation keeps every window rooted at the same
//! active buffer — splits are "mirror views" sharing cursor and scroll.
//! Per-window cursor / scroll / buffer binding is a follow-up; it needs
//! more surgery on the edit path than this first landing wants to
//! take on.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// What a single leaf displays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowContent {
    /// The active buffer's editor pane.
    Buffer,
    /// LSP diagnostics list for the active buffer. Shown as a
    /// read-only list of `severity line:col message` rows.
    Diagnostics,
}

/// How a split allocates space between its two children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitSize {
    /// Divide the area in half.
    Equal,
    /// Give the **second** child exactly this many rows (`HSplit`) or
    /// columns (`VSplit`). The first child gets the rest. If the area
    /// is too small to honour this, falls back to `Equal`.
    Lines(u16),
}

/// A window is either a single leaf displaying content, or a split
/// with two child windows.
#[derive(Clone, Debug)]
pub enum Window {
    Leaf(WindowContent),
    /// Horizontal split — children stack top / bottom. The `size`
    /// specifies how many rows the bottom child takes (when
    /// `Lines(n)`).
    HSplit(Box<Window>, Box<Window>, SplitSize),
    /// Vertical split — children sit left / right. The `size`
    /// specifies how many columns the right child takes.
    VSplit(Box<Window>, Box<Window>, SplitSize),
}

/// A concrete rectangle + the content to render in it, produced by
/// [`Window::layout`]. `focused` is true for exactly one leaf — the
/// one the user's keystrokes apply to.
#[derive(Clone, Copy, Debug)]
pub struct WindowLeafRect {
    pub rect: Rect,
    pub content: WindowContent,
    pub focused: bool,
}

/// Orientation of a divider between two adjacent windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeparatorKind {
    /// One-row horizontal divider (`─`), between top/bottom children
    /// of an `HSplit`.
    Horizontal,
    /// One-column vertical divider (`│`), between left/right children
    /// of a `VSplit`.
    Vertical,
}

/// A divider drawn between two adjacent windows.
///
/// For horizontal separators (between top/bottom panes) we carry the
/// content of the **window above** so the renderer can paint a
/// window-specific mode line there — matching Emacs, where every
/// window has its own mode line at its bottom edge. Vertical
/// separators stay simple (just a `│` bar) and leave these fields
/// `None`.
#[derive(Clone, Copy, Debug)]
pub struct SeparatorRect {
    pub rect: Rect,
    pub kind: SeparatorKind,
    /// The content kind of the window immediately above this
    /// horizontal separator, if any. `None` for vertical separators.
    pub above_content: Option<WindowContent>,
    /// Whether the window above this separator is currently focused.
    /// Used to style the mode line differently (brighter background
    /// for the focused window's mode line, dim for others).
    pub above_focused: bool,
}

impl Window {
    /// Walk the tree in in-order (L→R, T→B) and return the flat list
    /// of leaf rectangles plus the dividers between adjacent windows.
    ///
    /// Each internal split reserves 1 row (`HSplit`) or 1 column
    /// (`VSplit`) for a divider between its two children — so a
    /// `VSplit` of a 40-column area gives children 19 and 20 columns
    /// wide, plus the 1-col divider between them.
    pub fn layout(&self, area: Rect, focus: usize) -> (Vec<WindowLeafRect>, Vec<SeparatorRect>) {
        let mut leaves = Vec::new();
        let mut seps = Vec::new();
        let mut counter = 0usize;
        self.layout_into(area, focus, &mut counter, &mut leaves, &mut seps);
        (leaves, seps)
    }

    fn layout_into(
        &self,
        area: Rect,
        focus: usize,
        counter: &mut usize,
        leaves: &mut Vec<WindowLeafRect>,
        seps: &mut Vec<SeparatorRect>,
    ) {
        match self {
            Window::Leaf(content) => {
                let idx = *counter;
                *counter += 1;
                leaves.push(WindowLeafRect {
                    rect: area,
                    content: *content,
                    focused: idx == focus,
                });
            }
            Window::HSplit(top, bottom, size) => {
                // Resolve the split. `bottom_h` is how many rows we
                // want the bottom child to get; `top_h` is the rest
                // minus the divider row.
                let bottom_h = match size {
                    SplitSize::Equal => area.height.saturating_sub(1) / 2,
                    SplitSize::Lines(n) => {
                        // Sanity: leave at least 1 row for the top
                        // child plus 1 row for the separator.
                        let max = area.height.saturating_sub(2);
                        (*n).min(max).max(1)
                    }
                };
                // Too small to draw a separator at all: fall back to
                // ratatui's layout (no divider, proportional).
                if area.height < 3 {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Percentage(50), Constraint::Min(1)])
                        .split(area);
                    top.layout_into(chunks[0], focus, counter, leaves, seps);
                    bottom.layout_into(chunks[1], focus, counter, leaves, seps);
                    return;
                }
                let top_h = area.height - bottom_h - 1;
                let top_rect = Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: top_h,
                };
                let sep_rect = Rect {
                    x: area.x,
                    y: area.y + top_h,
                    width: area.width,
                    height: 1,
                };
                let bottom_rect = Rect {
                    x: area.x,
                    y: area.y + top_h + 1,
                    width: area.width,
                    height: bottom_h,
                };
                // Remember focus/content of the top child *before*
                // recursing so the separator can render a mode line
                // that describes the window immediately above it.
                let top_first_leaf_idx = *counter;
                top.layout_into(top_rect, focus, counter, leaves, seps);
                let top_leaves = &leaves[top_first_leaf_idx..];
                let (above_content, above_focused) = mode_line_target(top_leaves);
                seps.push(SeparatorRect {
                    rect: sep_rect,
                    kind: SeparatorKind::Horizontal,
                    above_content,
                    above_focused,
                });
                bottom.layout_into(bottom_rect, focus, counter, leaves, seps);
            }
            Window::VSplit(left, right, size) => {
                let right_w = match size {
                    SplitSize::Equal => area.width.saturating_sub(1) / 2,
                    SplitSize::Lines(n) => {
                        let max = area.width.saturating_sub(2);
                        (*n).min(max).max(1)
                    }
                };
                if area.width < 3 {
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(50), Constraint::Min(1)])
                        .split(area);
                    left.layout_into(chunks[0], focus, counter, leaves, seps);
                    right.layout_into(chunks[1], focus, counter, leaves, seps);
                    return;
                }
                let left_w = area.width - right_w - 1;
                let left_rect = Rect {
                    x: area.x,
                    y: area.y,
                    width: left_w,
                    height: area.height,
                };
                let sep_rect = Rect {
                    x: area.x + left_w,
                    y: area.y,
                    width: 1,
                    height: area.height,
                };
                let right_rect = Rect {
                    x: area.x + left_w + 1,
                    y: area.y,
                    width: right_w,
                    height: area.height,
                };
                left.layout_into(left_rect, focus, counter, leaves, seps);
                seps.push(SeparatorRect {
                    rect: sep_rect,
                    kind: SeparatorKind::Vertical,
                    above_content: None,
                    above_focused: false,
                });
                right.layout_into(right_rect, focus, counter, leaves, seps);
            }
        }
    }

    /// Number of leaves. Always ≥ 1.
    pub fn leaf_count(&self) -> usize {
        match self {
            Window::Leaf(_) => 1,
            Window::HSplit(a, b, _) | Window::VSplit(a, b, _) => {
                a.leaf_count() + b.leaf_count()
            }
        }
    }

    /// Content of the leaf at `focus` (in-order index).
    pub fn content_at(&self, focus: usize) -> Option<WindowContent> {
        self.content_at_inner(focus, &mut 0)
    }

    fn content_at_inner(&self, focus: usize, counter: &mut usize) -> Option<WindowContent> {
        match self {
            Window::Leaf(c) => {
                let idx = *counter;
                *counter += 1;
                if idx == focus { Some(*c) } else { None }
            }
            Window::HSplit(a, b, _) | Window::VSplit(a, b, _) => {
                a.content_at_inner(focus, counter)
                    .or_else(|| b.content_at_inner(focus, counter))
            }
        }
    }

    /// Split the focused leaf horizontally (new leaf appears below) —
    /// the Emacs `split-window-below` behaviour, bound to `C-x 2`.
    /// Returns the new focus index (one past the old, since leaves
    /// are now doubled).
    pub fn split_horizontal(&mut self, focus: usize) -> usize {
        self.split_at(focus, true)
    }

    /// Split the focused leaf vertically (new leaf appears to the
    /// right) — `split-window-right`, bound to `C-x 3`.
    pub fn split_vertical(&mut self, focus: usize) -> usize {
        self.split_at(focus, false)
    }

    fn split_at(&mut self, focus: usize, horizontal: bool) -> usize {
        let mut counter = 0usize;
        Self::split_at_inner(self, focus, horizontal, &mut counter);
        // The new leaf is appended directly after the old one in
        // in-order traversal, so focus stays on the original leaf.
        // (Emacs moves focus to the *new* window in C-x 2; adjust if
        // the user expects that. We keep it here for now.)
        focus
    }

    fn split_at_inner(
        window: &mut Window,
        focus: usize,
        horizontal: bool,
        counter: &mut usize,
    ) -> bool {
        match window {
            Window::Leaf(content) => {
                let idx = *counter;
                *counter += 1;
                if idx == focus {
                    let c = *content;
                    let new_child = Window::Leaf(c);
                    let replacement = if horizontal {
                        Window::HSplit(
                            Box::new(Window::Leaf(c)),
                            Box::new(new_child),
                            SplitSize::Equal,
                        )
                    } else {
                        Window::VSplit(
                            Box::new(Window::Leaf(c)),
                            Box::new(new_child),
                            SplitSize::Equal,
                        )
                    };
                    *window = replacement;
                    true
                } else {
                    false
                }
            }
            Window::HSplit(a, b, _) | Window::VSplit(a, b, _) => {
                if Self::split_at_inner(a, focus, horizontal, counter) {
                    return true;
                }
                Self::split_at_inner(b, focus, horizontal, counter)
            }
        }
    }

    /// Delete the focused leaf (`C-x 0`). Refuses when there's only
    /// one leaf (we never want to end up with no windows). Returns
    /// the new focus index, which points at the sibling that took
    /// the deleted leaf's place.
    pub fn delete_focused(&mut self, focus: usize) -> Option<usize> {
        if self.leaf_count() <= 1 {
            return None;
        }
        // Walk down collecting path; at the parent of the focused
        // leaf, replace it with the surviving sibling.
        let mut counter = 0usize;
        Self::delete_inner(self, focus, &mut counter);
        // After deletion, if the focused leaf was at index `focus`,
        // the sibling is now at that same index (or `focus - 1` if
        // the deleted leaf was the rightmost — clamp).
        let new_focus = focus.min(self.leaf_count().saturating_sub(1));
        Some(new_focus)
    }

    fn delete_inner(window: &mut Window, focus: usize, counter: &mut usize) -> DeleteResult {
        match window {
            Window::Leaf(_) => {
                let idx = *counter;
                *counter += 1;
                if idx == focus {
                    DeleteResult::DeleteMe
                } else {
                    DeleteResult::Untouched
                }
            }
            Window::HSplit(a, b, _) | Window::VSplit(a, b, _) => {
                let a_res = Self::delete_inner(a, focus, counter);
                if matches!(a_res, DeleteResult::DeleteMe) {
                    // Replace `window` with `b` — move it out.
                    let replacement = std::mem::replace(
                        b.as_mut(),
                        Window::Leaf(WindowContent::Buffer),
                    );
                    *window = replacement;
                    return DeleteResult::Collapsed;
                }
                if matches!(a_res, DeleteResult::Collapsed) {
                    return DeleteResult::Collapsed;
                }
                let b_res = Self::delete_inner(b, focus, counter);
                if matches!(b_res, DeleteResult::DeleteMe) {
                    let replacement = std::mem::replace(
                        a.as_mut(),
                        Window::Leaf(WindowContent::Buffer),
                    );
                    *window = replacement;
                    return DeleteResult::Collapsed;
                }
                b_res
            }
        }
    }

    /// Collapse the tree to just the focused leaf — `C-x 1`. Returns
    /// the new focus index (always 0).
    pub fn keep_only_focused(&mut self, focus: usize) -> usize {
        if let Some(c) = self.content_at(focus) {
            *self = Window::Leaf(c);
        }
        0
    }

    /// Move focus to the next leaf — `C-x o`. Wraps.
    pub fn focus_next(&self, focus: usize) -> usize {
        let count = self.leaf_count();
        if count == 0 { 0 } else { (focus + 1) % count }
    }

    /// Return a copy of the tree with leaves that match `hide`
    /// removed. Splits whose subtrees collapse to a single surviving
    /// child are replaced by that child, so an `HSplit(Buffer, Diag)`
    /// with `Diag` hidden becomes just `Buffer`.
    ///
    /// Returns `None` when *every* leaf in the tree matches `hide` —
    /// callers typically substitute a fallback (e.g.
    /// `Window::Leaf(Buffer)`) rather than render nothing.
    pub fn prune(&self, hide: &impl Fn(WindowContent) -> bool) -> Option<Window> {
        match self {
            Window::Leaf(c) => {
                if hide(*c) { None } else { Some(Window::Leaf(*c)) }
            }
            Window::HSplit(a, b, size) => match (a.prune(hide), b.prune(hide)) {
                (Some(l), Some(r)) => {
                    Some(Window::HSplit(Box::new(l), Box::new(r), *size))
                }
                (Some(l), None) => Some(l),
                (None, Some(r)) => Some(r),
                (None, None) => None,
            },
            Window::VSplit(a, b, size) => match (a.prune(hide), b.prune(hide)) {
                (Some(l), Some(r)) => {
                    Some(Window::VSplit(Box::new(l), Box::new(r), *size))
                }
                (Some(l), None) => Some(l),
                (None, Some(r)) => Some(r),
                (None, None) => None,
            },
        }
    }

    /// Given a focus index in `self`, return the in-order index of
    /// the same leaf in a tree produced by `self.prune(hide)`.
    ///
    /// If the originally-focused leaf itself satisfies `hide` (it's
    /// being pruned away), we fall back to 0 — the first visible
    /// leaf. Returns 0 for an empty pruned tree too, so callers can
    /// just clamp against the pruned `leaf_count` afterwards.
    pub fn remap_focus(
        &self,
        focus: usize,
        hide: &impl Fn(WindowContent) -> bool,
    ) -> usize {
        // Count visible leaves encountered before `focus` in in-order.
        // If the focused leaf itself is hidden, return the count so
        // far (which is the next visible leaf's index) or 0.
        let mut original_counter = 0usize;
        let mut visible_counter = 0usize;
        let mut hit_focus_on_hidden = false;
        self.remap_inner(
            focus,
            hide,
            &mut original_counter,
            &mut visible_counter,
            &mut hit_focus_on_hidden,
        );
        if hit_focus_on_hidden {
            // Focused leaf got pruned; fall back to the nearest
            // visible leaf before it, or 0 if none existed.
            // `visible_counter` is the count of visible leaves
            // encountered *strictly before* focus in in-order, so
            // the previous visible leaf is at index
            // `visible_counter - 1`. Clamp against the pruned
            // leaf_count in case the whole subtree before focus was
            // also pruned.
            let pruned_count =
                self.prune(hide).map(|w| w.leaf_count()).unwrap_or(0);
            visible_counter
                .saturating_sub(1)
                .min(pruned_count.saturating_sub(1))
        } else {
            visible_counter
        }
    }

    fn remap_inner(
        &self,
        focus: usize,
        hide: &impl Fn(WindowContent) -> bool,
        original_counter: &mut usize,
        visible_counter: &mut usize,
        hit_focus_on_hidden: &mut bool,
    ) -> bool {
        match self {
            Window::Leaf(c) => {
                let idx = *original_counter;
                *original_counter += 1;
                if idx == focus {
                    if hide(*c) {
                        *hit_focus_on_hidden = true;
                    }
                    true
                } else {
                    if !hide(*c) {
                        *visible_counter += 1;
                    }
                    false
                }
            }
            Window::HSplit(a, b, _) | Window::VSplit(a, b, _) => {
                if a.remap_inner(
                    focus,
                    hide,
                    original_counter,
                    visible_counter,
                    hit_focus_on_hidden,
                ) {
                    return true;
                }
                b.remap_inner(
                    focus,
                    hide,
                    original_counter,
                    visible_counter,
                    hit_focus_on_hidden,
                )
            }
        }
    }
}

/// The "window immediately above this horizontal separator" has a
/// deterministic leaf. We use the *last* leaf that was added to
/// `leaves` while laying out the above subtree — that's the
/// bottom-right-most leaf by the in-order traversal, which is always
/// the one adjacent to (and therefore owning) the separator below it.
fn mode_line_target(
    above_leaves: &[WindowLeafRect],
) -> (Option<WindowContent>, bool) {
    match above_leaves.last() {
        Some(l) => (Some(l.content), l.focused),
        None => (None, false),
    }
}

/// Result of a recursive delete walk.
enum DeleteResult {
    /// Not affected by the deletion.
    Untouched,
    /// This node itself is the focused leaf and must be removed by
    /// the parent.
    DeleteMe,
    /// A deletion has already happened in this subtree; the parent
    /// should not recurse further.
    Collapsed,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(c: WindowContent) -> Window {
        Window::Leaf(c)
    }

    fn rect() -> Rect {
        Rect { x: 0, y: 0, width: 80, height: 24 }
    }

    #[test]
    fn single_leaf_has_one_leaf() {
        let w = leaf(WindowContent::Buffer);
        assert_eq!(w.leaf_count(), 1);
        let (leaves, seps) = w.layout(rect(), 0);
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].rect, rect());
        assert!(leaves[0].focused);
        // No separator for a single leaf.
        assert!(seps.is_empty());
    }

    #[test]
    fn horizontal_split_stacks_with_separator_between() {
        let mut w = leaf(WindowContent::Buffer);
        let f = w.split_horizontal(0);
        assert_eq!(w.leaf_count(), 2);
        let (leaves, seps) = w.layout(rect(), f);
        assert_eq!(leaves.len(), 2);
        assert_eq!(seps.len(), 1);
        // Top child at y=0; separator just below; bottom child below
        // that. Heights should sum to area height.
        assert_eq!(leaves[0].rect.y, 0);
        let top_end = leaves[0].rect.y + leaves[0].rect.height;
        assert_eq!(seps[0].rect.y, top_end);
        assert_eq!(seps[0].rect.height, 1);
        assert_eq!(seps[0].kind, SeparatorKind::Horizontal);
        assert_eq!(leaves[1].rect.y, top_end + 1);
        assert_eq!(
            leaves[0].rect.height + 1 + leaves[1].rect.height,
            rect().height
        );
    }

    #[test]
    fn vertical_split_places_children_side_by_side_with_separator() {
        let mut w = leaf(WindowContent::Buffer);
        let f = w.split_vertical(0);
        let (leaves, seps) = w.layout(rect(), f);
        assert_eq!(leaves.len(), 2);
        assert_eq!(seps.len(), 1);
        assert_eq!(leaves[0].rect.x, 0);
        let left_end = leaves[0].rect.x + leaves[0].rect.width;
        assert_eq!(seps[0].rect.x, left_end);
        assert_eq!(seps[0].rect.width, 1);
        assert_eq!(seps[0].kind, SeparatorKind::Vertical);
        assert_eq!(leaves[1].rect.x, left_end + 1);
        assert_eq!(
            leaves[0].rect.width + 1 + leaves[1].rect.width,
            rect().width
        );
    }

    #[test]
    fn separator_emitted_per_split() {
        // Tree with two splits: one H, one V. Expect 2 separators.
        let mut w = leaf(WindowContent::Buffer);
        w.split_horizontal(0);   // 2 leaves, 1 H sep
        w.split_vertical(1);     // 3 leaves, 1 H sep + 1 V sep
        let (leaves, seps) = w.layout(rect(), 0);
        assert_eq!(leaves.len(), 3);
        assert_eq!(seps.len(), 2);
        // One of each orientation.
        assert!(seps.iter().any(|s| s.kind == SeparatorKind::Horizontal));
        assert!(seps.iter().any(|s| s.kind == SeparatorKind::Vertical));
    }

    #[test]
    fn content_at_returns_leaf_content() {
        let mut w = leaf(WindowContent::Buffer);
        w.split_horizontal(0);
        // Second leaf gets WindowContent::Buffer too (clone of first).
        assert_eq!(w.content_at(0), Some(WindowContent::Buffer));
        assert_eq!(w.content_at(1), Some(WindowContent::Buffer));
        assert_eq!(w.content_at(2), None);
    }

    #[test]
    fn delete_of_only_leaf_refuses() {
        let mut w = leaf(WindowContent::Buffer);
        assert!(w.delete_focused(0).is_none());
        assert_eq!(w.leaf_count(), 1);
    }

    #[test]
    fn delete_collapses_split_to_survivor() {
        let mut w = leaf(WindowContent::Buffer);
        w.split_vertical(0); // now 2 leaves side-by-side
        // Replace leaf 1 with a Diagnostics marker so we can check
        // which one survives.
        // (Skip: content_at shows both are `Buffer`; we just confirm
        // the count drops.)
        let new_focus = w.delete_focused(0).unwrap();
        assert_eq!(w.leaf_count(), 1);
        assert_eq!(new_focus, 0);
    }

    #[test]
    fn keep_only_focused_flattens_tree() {
        let mut w = leaf(WindowContent::Buffer);
        w.split_horizontal(0);
        w.split_vertical(1); // 3 leaves now
        assert_eq!(w.leaf_count(), 3);
        let new_focus = w.keep_only_focused(2);
        assert_eq!(w.leaf_count(), 1);
        assert_eq!(new_focus, 0);
    }

    #[test]
    fn focus_next_wraps() {
        let mut w = leaf(WindowContent::Buffer);
        w.split_horizontal(0);
        w.split_vertical(1);
        // 3 leaves total.
        assert_eq!(w.focus_next(0), 1);
        assert_eq!(w.focus_next(1), 2);
        assert_eq!(w.focus_next(2), 0);
    }

    #[test]
    fn focused_leaf_is_marked_in_layout() {
        let mut w = leaf(WindowContent::Buffer);
        w.split_vertical(0);
        let (leaves, _seps) = w.layout(rect(), 1);
        assert!(!leaves[0].focused);
        assert!(leaves[1].focused);
    }

    #[test]
    fn lines_split_size_gives_bottom_child_exactly_n_rows() {
        // Build an HSplit with Lines(5) — bottom child should get 5 rows.
        let w = Window::HSplit(
            Box::new(leaf(WindowContent::Buffer)),
            Box::new(leaf(WindowContent::Diagnostics)),
            SplitSize::Lines(5),
        );
        let (leaves, seps) = w.layout(rect(), 0); // 80x24
        assert_eq!(leaves.len(), 2);
        assert_eq!(leaves[1].rect.height, 5);
        // Top child gets height - 5 (diag) - 1 (separator) = 18.
        assert_eq!(leaves[0].rect.height, 24 - 5 - 1);
        // Separator is 1 row between them.
        assert_eq!(seps[0].rect.height, 1);
    }

    #[test]
    fn lines_split_size_clamps_when_area_too_small() {
        // 6-row area, requesting Lines(5) — should still leave ≥1 row
        // for the top child.
        let area = Rect { x: 0, y: 0, width: 80, height: 6 };
        let w = Window::HSplit(
            Box::new(leaf(WindowContent::Buffer)),
            Box::new(leaf(WindowContent::Diagnostics)),
            SplitSize::Lines(5),
        );
        let (leaves, _seps) = w.layout(area, 0);
        // area=6, sep=1, min top=1, so bottom=5 fits (6-1-5 = 0)? Actually
        // max for bottom is area.height - 2 = 4. Clamped to 4.
        assert_eq!(leaves[0].rect.height, 1);
        assert_eq!(leaves[1].rect.height, 4);
    }

    #[test]
    fn horizontal_separator_carries_above_content() {
        // HSplit of buffer/diagnostics — separator should know the
        // above window is a Buffer and is focused (focus=0).
        let w = Window::HSplit(
            Box::new(leaf(WindowContent::Buffer)),
            Box::new(leaf(WindowContent::Diagnostics)),
            SplitSize::Lines(5),
        );
        let (_leaves, seps) = w.layout(rect(), 0);
        assert_eq!(seps.len(), 1);
        assert_eq!(seps[0].above_content, Some(WindowContent::Buffer));
        assert!(seps[0].above_focused);
    }

    #[test]
    fn horizontal_separator_marks_unfocused_above() {
        let w = Window::HSplit(
            Box::new(leaf(WindowContent::Buffer)),
            Box::new(leaf(WindowContent::Diagnostics)),
            SplitSize::Equal,
        );
        // Focus is on leaf 1 (bottom) — above window is NOT focused.
        let (_leaves, seps) = w.layout(rect(), 1);
        assert_eq!(seps[0].above_content, Some(WindowContent::Buffer));
        assert!(!seps[0].above_focused);
    }

    #[test]
    fn vertical_separator_has_no_above_content() {
        let w = Window::VSplit(
            Box::new(leaf(WindowContent::Buffer)),
            Box::new(leaf(WindowContent::Buffer)),
            SplitSize::Equal,
        );
        let (_leaves, seps) = w.layout(rect(), 0);
        assert_eq!(seps[0].above_content, None);
        assert!(!seps[0].above_focused);
    }

    #[test]
    fn prune_removes_matching_leaves() {
        let w = Window::HSplit(
            Box::new(leaf(WindowContent::Buffer)),
            Box::new(leaf(WindowContent::Diagnostics)),
            SplitSize::Lines(5),
        );
        let pruned = w
            .prune(&|c| c == WindowContent::Diagnostics)
            .expect("at least one leaf should survive");
        // Pruned to just the Buffer leaf.
        assert_eq!(pruned.leaf_count(), 1);
        assert_eq!(pruned.content_at(0), Some(WindowContent::Buffer));
    }

    #[test]
    fn prune_returns_none_when_all_leaves_hidden() {
        let w = leaf(WindowContent::Diagnostics);
        let pruned = w.prune(&|c| c == WindowContent::Diagnostics);
        assert!(pruned.is_none());
    }

    #[test]
    fn prune_collapses_nested_split_to_sibling() {
        // HSplit(Buffer, HSplit(Diagnostics, Buffer))
        // With Diagnostics hidden, inner split collapses to Buffer;
        // outer split has two Buffer children.
        let inner = Window::HSplit(
            Box::new(leaf(WindowContent::Diagnostics)),
            Box::new(leaf(WindowContent::Buffer)),
            SplitSize::Equal,
        );
        let w = Window::HSplit(
            Box::new(leaf(WindowContent::Buffer)),
            Box::new(inner),
            SplitSize::Equal,
        );
        let pruned = w
            .prune(&|c| c == WindowContent::Diagnostics)
            .unwrap();
        assert_eq!(pruned.leaf_count(), 2);
        assert_eq!(pruned.content_at(0), Some(WindowContent::Buffer));
        assert_eq!(pruned.content_at(1), Some(WindowContent::Buffer));
    }

    #[test]
    fn remap_focus_adjusts_after_hidden_leaves() {
        // Buffer(0), Diagnostics(1), Buffer(2) — focus on leaf 2.
        // After pruning Diagnostics, leaf 2 becomes visible leaf 1.
        let w = Window::HSplit(
            Box::new(leaf(WindowContent::Buffer)),
            Box::new(Window::HSplit(
                Box::new(leaf(WindowContent::Diagnostics)),
                Box::new(leaf(WindowContent::Buffer)),
                SplitSize::Equal,
            )),
            SplitSize::Equal,
        );
        let hide = |c: WindowContent| c == WindowContent::Diagnostics;
        let new = w.remap_focus(2, &hide);
        assert_eq!(new, 1);
    }

    /// Regression: when the focused leaf was hidden, the fallback
    /// was `visible_counter.saturating_sub(0)`, which is a no-op and
    /// pointed at the next visible leaf *after* the hidden one.
    /// The doc promises "nearest visible leaf before it" — which is
    /// the last visible leaf encountered before `focus` in in-order
    /// traversal, i.e. `visible_counter - 1`.
    #[test]
    fn remap_focus_pruned_focus_lands_on_visible_leaf_before() {
        // Tree: Buffer(0) | HSplit(Buffer(1), HSplit(Diagnostics(2), Buffer(3)))
        // Focus on the Diagnostics leaf (original index 2).
        let w = Window::HSplit(
            Box::new(leaf(WindowContent::Buffer)),
            Box::new(Window::HSplit(
                Box::new(leaf(WindowContent::Buffer)),
                Box::new(Window::HSplit(
                    Box::new(leaf(WindowContent::Diagnostics)),
                    Box::new(leaf(WindowContent::Buffer)),
                    SplitSize::Equal,
                )),
                SplitSize::Equal,
            )),
            SplitSize::Equal,
        );
        let hide = |c: WindowContent| c == WindowContent::Diagnostics;
        // Pruned tree has 3 Buffer leaves at indices 0, 1, 2. The
        // focused Diagnostics sat between the 2nd and 3rd Buffer.
        // "Nearest visible leaf before" is the 2nd Buffer = index 1.
        let new = w.remap_focus(2, &hide);
        assert_eq!(
            new, 1,
            "remap_focus should land on the visible leaf before the hidden one"
        );
    }

    #[test]
    fn remap_focus_falls_back_when_focused_leaf_hidden() {
        // Focus on Diagnostics (leaf 1); after pruning, it's gone —
        // new focus should land on a visible leaf (0).
        let w = Window::HSplit(
            Box::new(leaf(WindowContent::Buffer)),
            Box::new(leaf(WindowContent::Diagnostics)),
            SplitSize::Equal,
        );
        let hide = |c: WindowContent| c == WindowContent::Diagnostics;
        let new = w.remap_focus(1, &hide);
        assert_eq!(new, 0);
    }

    #[test]
    fn prune_preserves_layout_when_no_leaves_match() {
        let w = Window::HSplit(
            Box::new(leaf(WindowContent::Buffer)),
            Box::new(leaf(WindowContent::Buffer)),
            SplitSize::Lines(3),
        );
        let pruned = w
            .prune(&|c| c == WindowContent::Diagnostics)
            .unwrap();
        // Structure preserved: still an HSplit with 2 leaves.
        assert_eq!(pruned.leaf_count(), 2);
    }

    #[test]
    fn diagnostics_content_can_live_in_a_leaf() {
        // Build the expected default layout: main buffer top-left,
        // diagnostics bottom.
        let w = Window::HSplit(
            Box::new(leaf(WindowContent::Buffer)),
            Box::new(leaf(WindowContent::Diagnostics)),
            SplitSize::Equal,
        );
        assert_eq!(w.leaf_count(), 2);
        assert_eq!(w.content_at(0), Some(WindowContent::Buffer));
        assert_eq!(w.content_at(1), Some(WindowContent::Diagnostics));
    }
}
