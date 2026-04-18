//! Highlight trait and token types.
//!
//! An implementation reports coloured ranges within a given line range of a
//! buffer. Tree-sitter, regex, or any future backend implements the trait.

/// A semantic classification of a highlighted range. The view maps these
/// to concrete theme colours (see `MdThemeColors` in the GPUI client).
///
/// Kept small and generic so backends don't need to know the theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Highlight {
    /// Plain text — no special styling.
    Plain,
    /// Keyword (e.g. `fn`, `if`).
    Keyword,
    /// String literal.
    String,
    /// Numeric literal.
    Number,
    /// Comment.
    Comment,
    /// Function name (definition or call).
    Function,
    /// Type name.
    Type,
    /// Macro invocation.
    Macro,
    /// Markdown heading.
    Heading,
    /// Markdown emphasis (italic).
    Emphasis,
    /// Markdown strong (bold).
    Strong,
    /// Markdown code span / fenced code.
    Code,
    /// Markdown link.
    Link,
    /// Markdown list marker.
    ListMarker,
    /// Markdown fence marker (``` / ~~~).
    Fence,
    /// Markdown quote prefix (>).
    Quote,
    /// Task list checkbox marker (`- [ ]` / `- [x]`).
    Checkbox,
}

/// A classified range inside a line. `start_col` and `end_col` are
/// **character** offsets within the line (not bytes), which matches the
/// renderer's span iteration.
#[derive(Clone, Copy, Debug)]
pub struct HighlightRange {
    pub start_col: usize,
    pub end_col: usize,
    pub kind: Highlight,
}

/// A highlighter that colourises a buffer.
///
/// Implementations must be stateful in the sense that they cache an
/// internal representation of the buffer (parse tree, regex state, …) and
/// update it on edit via `on_edit`. The renderer then calls
/// `highlight_range` to pull spans for the visible lines.
///
/// The rope is passed by reference rather than as `&str` to avoid
/// materialising the whole buffer — tree-sitter reads through
/// `rope.chunk_at_byte()`, regex backends can iterate lines directly.
pub trait Highlighter {
    /// Called after an edit has been applied to the buffer, with the
    /// accumulated changes from `DocumentBuffer::drain_changes()`.
    /// Implementations that can't do incremental work may re-parse from
    /// scratch.
    fn on_edit(
        &mut self,
        rope: &ropey::Rope,
        changes: &[crate::document::buffer::TextChange],
        full_sync: bool,
    );

    /// Return the highlight ranges that intersect `[start_line..end_line)`
    /// in the buffer. Each element is `(line_index, ranges)` with
    /// `ranges` sorted by `start_col` ascending.
    fn highlight_range(
        &self,
        rope: &ropey::Rope,
        start_line: usize,
        end_line: usize,
    ) -> Vec<(usize, Vec<HighlightRange>)>;
}

/// A line-at-a-time highlighter shim. Backends that only know how to
/// classify one line at a time (and carry tiny state across, like "are
/// we in a fenced code block") implement this; `Highlighter` is derived
/// automatically.
///
/// This is what the existing regex-based markdown highlighter maps onto,
/// so the existing code can implement `LineHighlighter` and get a
/// `Highlighter` for free.
pub trait LineHighlighter {
    /// Per-call state carried across lines (e.g. "inside code block").
    type State: Default + Clone;

    /// Classify one line given the state entering that line.
    /// Returns the ranges found and the state leaving the line (which
    /// becomes the entering state of the next line).
    fn highlight_line(
        &self,
        line: &str,
        state: Self::State,
    ) -> (Vec<HighlightRange>, Self::State);
}
