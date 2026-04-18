use lsp_types::{Diagnostic, DiagnosticSeverity};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use rele_server::syntax::{Highlight, HighlightRange};

use crate::state::TuiAppState;
use crate::windows::WindowContent;

/// Return the terminal-friendly marker shown in the gutter for a
/// diagnostic of this severity. Kept single-character so the gutter
/// width doesn't have to grow.
fn severity_marker(sev: Option<DiagnosticSeverity>) -> (&'static str, Color) {
    match sev {
        Some(DiagnosticSeverity::ERROR) => ("E", Color::LightRed),
        Some(DiagnosticSeverity::WARNING) => ("W", Color::LightYellow),
        Some(DiagnosticSeverity::INFORMATION) => ("i", Color::LightCyan),
        Some(DiagnosticSeverity::HINT) => ("?", Color::Gray),
        _ => (" ", Color::DarkGray),
    }
}

/// Rank a severity so we can compare them — the LSP enum's inner
/// value isn't public, but the four values are well-known constants.
/// Lower rank = more severe.
fn severity_rank(sev: DiagnosticSeverity) -> u8 {
    if sev == DiagnosticSeverity::ERROR {
        0
    } else if sev == DiagnosticSeverity::WARNING {
        1
    } else if sev == DiagnosticSeverity::INFORMATION {
        2
    } else if sev == DiagnosticSeverity::HINT {
        3
    } else {
        4
    }
}

/// Pick the "worst" severity among diagnostics — ERROR beats WARNING
/// beats INFO beats HINT. Returns `None` if the slice is empty.
/// How many characters of the cursor line we need to materialise.
///
/// We want `text_width` characters visible on a normal line, but if
/// the cursor sits past the wrap point (e.g. user C-e'd to the end of
/// a 10k-char line), the cursor block still has to render — so the
/// cap must be at least `cursor_col + 1`. Capped here and not inside
/// the span builder so the `Vec<char>` allocation itself is bounded:
/// a pathological 10 MB single-line file was previously allocating
/// the whole line per frame (PERFORMANCE.md rule 5).
fn cursor_line_char_cap(text_width: usize, cursor_col: usize) -> usize {
    text_width.max(cursor_col.saturating_add(1))
}

fn worst_severity(diags: &[&Diagnostic]) -> Option<DiagnosticSeverity> {
    diags
        .iter()
        .map(|d| d.severity.unwrap_or(DiagnosticSeverity::INFORMATION))
        .min_by_key(|s| severity_rank(*s))
}

/// Map a language-agnostic [`Highlight`] token to a ratatui [`Color`].
/// Colours picked from the terminal's 16-colour palette where possible so
/// they look right in any terminal theme; fall back to `Reset` (default
/// foreground) for plain text so the terminal's own colour wins on
/// unhighlighted regions.
fn color_for_highlight(kind: Highlight) -> Color {
    match kind {
        Highlight::Plain => Color::Reset,
        // Code-oriented kinds.
        Highlight::Keyword => Color::Magenta,
        Highlight::Type => Color::Yellow,
        Highlight::Function => Color::Cyan,
        Highlight::Macro => Color::LightMagenta,
        Highlight::String => Color::Green,
        Highlight::Number => Color::LightYellow,
        Highlight::Comment => Color::DarkGray,
        // Markdown-oriented kinds.
        Highlight::Heading => Color::LightBlue,
        Highlight::Emphasis => Color::LightMagenta,
        Highlight::Strong => Color::LightRed,
        Highlight::Code => Color::LightYellow,
        Highlight::Link => Color::LightCyan,
        Highlight::ListMarker => Color::LightGreen,
        Highlight::Fence => Color::Gray,
        Highlight::Quote => Color::Gray,
        Highlight::Checkbox => Color::LightGreen,
    }
}

/// Render the full TUI: window tree + status bar + echo area.
///
/// Layout (top → bottom):
///   - window tree — split recursively per user commands
///     (`C-x 2` / `C-x 3`). `*Diagnostics*` is one of the window
///     kinds, not a permanent panel.
///   - status bar (1 row).
///   - message / minibuffer (1 row normally; grows when the
///     minibuffer is open with candidates).
pub fn draw(frame: &mut Frame, state: &mut TuiAppState) {
    let area = frame.area();

    // How tall the bottom "message / minibuffer" area should be.
    // 1 line normally; up to ~6 lines when the minibuffer is open with
    // candidates (one prompt line + up to N candidate lines).
    let minibuffer_height = if state.minibuffer.active
        && !state.minibuffer.candidates.is_empty()
    {
        let candidates = state.minibuffer.filtered.len().min(5);
        (candidates + 1) as u16
    } else {
        1
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),                       // window tree
            Constraint::Length(1),                    // status bar
            Constraint::Length(minibuffer_height),    // message / minibuffer
        ])
        .split(area);

    draw_window_tree(frame, state, chunks[0]);
    draw_status_bar(frame, state, chunks[1]);
    draw_message_line(frame, state, chunks[2]);
}

/// Walk the user's window tree, render each leaf, and paint the 1-char
/// divider bars between adjacent leaves. Only the **focused** leaf of
/// type `Buffer` sets the shared `last_viewport_height` — scroll
/// commands apply to the focused window.
///
/// **Auto-hide**: if the active buffer has zero LSP diagnostics we
/// prune `Diagnostics` leaves out of the render tree entirely so the
/// buffer window reclaims the space. The user's stored window layout
/// is untouched — when diagnostics reappear, the *Diagnostics* window
/// comes back in the same spot.
fn draw_window_tree(frame: &mut Frame, state: &mut TuiAppState, area: Rect) {
    let has_diagnostics = state
        .lsp_buffer_state
        .as_ref()
        .map(|s| !s.diagnostics.is_empty())
        .unwrap_or(false);

    // Build the tree we actually render (possibly pruned) and remap
    // focus. Keep it as an owned value so the pruned-and-unpruned
    // paths have matching types.
    let (tree, focus) = if has_diagnostics {
        (state.windows.clone(), state.focused_window)
    } else {
        let hide =
            |c: crate::windows::WindowContent| c == crate::windows::WindowContent::Diagnostics;
        let pruned = state
            .windows
            .prune(&hide)
            .unwrap_or_else(|| crate::windows::Window::Leaf(WindowContent::Buffer));
        let leaf_count = pruned.leaf_count();
        let new_focus = state
            .windows
            .remap_focus(state.focused_window, &hide)
            .min(leaf_count.saturating_sub(1));
        (pruned, new_focus)
    };

    let (leaves, separators) = tree.layout(area, focus);
    for leaf in leaves {
        match leaf.content {
            WindowContent::Buffer => {
                if leaf.focused {
                    // The focused buffer window drives scroll commands.
                    draw_editor(frame, state, leaf.rect);
                } else {
                    // Non-focused buffer windows mirror the focused
                    // one (same buffer, same cursor) until we have
                    // per-window cursor state. We still want them to
                    // paint, so call draw_editor but without updating
                    // `last_viewport_height`.
                    let prev_vh = state.last_viewport_height;
                    draw_editor(frame, state, leaf.rect);
                    state.last_viewport_height = prev_vh;
                }
            }
            WindowContent::Diagnostics => {
                draw_diagnostics_panel(frame, state, leaf.rect);
            }
        }
    }
    for sep in separators {
        draw_separator(frame, state, sep);
    }
}

/// Paint the divider between two adjacent windows.
///
/// - **Vertical** separator: a dim `│` column, purely decorative —
///   same as Emacs's `vertical-border` face.
/// - **Horizontal** separator: acts as the **mode line** for the
///   window above it. Displays the same info as the bottom status
///   bar (buffer name, position, language, diagnostics) but styled
///   according to whether the window above is focused. This matches
///   Emacs, where every window's mode line sits at its bottom edge
///   and becomes the "separator" when a split is present.
fn draw_separator(frame: &mut Frame, state: &TuiAppState, sep: crate::windows::SeparatorRect) {
    use crate::windows::SeparatorKind;
    match sep.kind {
        SeparatorKind::Vertical => {
            let cols = 1;
            let rows = sep.rect.height as usize;
            let style = Style::default().fg(Color::DarkGray);
            let lines: Vec<Line<'_>> = (0..rows)
                .map(|_| {
                    let text: String = std::iter::repeat('│').take(cols).collect();
                    Line::from(Span::styled(text, style))
                })
                .collect();
            frame.render_widget(Paragraph::new(lines), sep.rect);
        }
        SeparatorKind::Horizontal => {
            // Mode line for the window immediately above.
            let content = sep.above_content.unwrap_or(WindowContent::Buffer);
            render_mode_line(frame, state, sep.rect, content, sep.above_focused);
        }
    }
}

/// Render the always-on *Diagnostics* panel between the status bar and
/// the minibuffer. Shows up to `DIAG_PANEL_MAX_ROWS` rows, oldest-first
/// (sorted by line). The row covering the cursor's current line is
/// highlighted so the user can see where their cursor sits relative to
/// the diagnostic list.
///
/// The last row is an overflow indicator (`… (+N more)`) when the
/// buffer has more diagnostics than fit. Use `M-g n` / `M-g p` to cycle
/// through all of them, not just the visible ones.
fn draw_diagnostics_panel(frame: &mut Frame, state: &TuiAppState, area: Rect) {
    let Some(lsp) = state.lsp_buffer_state.as_ref() else {
        return;
    };
    let cursor_line = state.cursor_line() as u32;

    // Sort by (line, col) for stable top-to-bottom reading. Server may
    // emit diagnostics in an arbitrary order.
    let mut sorted: Vec<&Diagnostic> = lsp.diagnostics.iter().collect();
    sorted.sort_by_key(|d| (d.range.start.line, d.range.start.character));
    let total = sorted.len();

    // Header.
    let header = Line::from(vec![
        Span::styled(
            " *Diagnostics* ",
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {total} total ", ),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    // How many diagnostic rows fit. Area height includes the header,
    // so leave one row for that. Previously we capped at
    // DIAG_PANEL_MAX_ROWS; now the window can be any size the user
    // gave us via splits, so respect that. Still reserve a row for
    // the overflow marker when we can't show everything.
    let area_rows = area.height as usize;
    let avail = area_rows.saturating_sub(1); // minus header
    let overflow = total > avail;
    let visible = if overflow {
        avail.saturating_sub(1) // reserve last row for "… (+N more)"
    } else {
        avail.min(total)
    };

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(visible + 2);
    lines.push(header);

    // Usable width for the message text, after the left gutter
    // (`E `, 2) + line:col column (~7) + separator (2).
    let width = area.width as usize;
    let message_width = width.saturating_sub(12).max(1);

    for d in sorted.iter().take(visible) {
        let sev = d.severity.unwrap_or(DiagnosticSeverity::INFORMATION);
        let (marker, sev_color) = severity_marker(Some(sev));
        let line = d.range.start.line;
        let col = d.range.start.character;
        let on_cursor = d.range.start.line <= cursor_line
            && d.range.end.line >= cursor_line;
        let row_style = if on_cursor {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        // Truncate the message to fit the row width (minus marker +
        // position prefix). LSP messages often contain embedded
        // newlines; take the first line only.
        let msg_first = d.message.lines().next().unwrap_or("");
        let msg_truncated: String = msg_first.chars().take(message_width).collect();
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {marker} "),
                row_style.fg(sev_color),
            ),
            Span::styled(
                format!("{:>3}:{:<3} ", line + 1, col + 1),
                row_style.fg(Color::DarkGray),
            ),
            Span::styled(msg_truncated, row_style),
        ]));
    }

    if overflow {
        let extra = total - visible;
        lines.push(Line::from(Span::styled(
            format!(" … (+{extra} more — M-g n / M-g p to navigate)"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        )));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

/// Render the text editing area with line numbers, cursor, and
/// tree-sitter syntax colouring.
fn draw_editor(frame: &mut Frame, state: &mut TuiAppState, area: Rect) {
    let viewport_height = area.height as usize;
    // Expose to commands (scroll-up/down use this as page size).
    state.last_viewport_height = viewport_height;
    state.ensure_cursor_visible(viewport_height);

    // Make sure tree-sitter has parsed the current document version
    // before we query highlights for the visible range. No-op if the
    // edit hooks already updated it.
    state.ensure_highlight_fresh();

    let total_lines = state.document.len_lines();
    let gutter_width = line_number_width(total_lines);
    let text_width = (area.width as usize).saturating_sub(gutter_width + 1); // +1 for separator

    let cursor_line = state.cursor_line();
    let cursor_col = state.cursor_col();

    // Query tree-sitter once for the whole visible window. Returns a
    // map: line -> ranges. Lines without any captures get no entry.
    // The query is O(visible-range-bytes), not O(buffer-size), so this
    // is cheap even on large files.
    let visible_end = (state.scroll_line + viewport_height).min(total_lines);
    let ts_ranges_by_line: std::collections::HashMap<usize, Vec<HighlightRange>> = state
        .buffer_highlighter
        .as_ref()
        .map(|h| {
            h.highlight_range(state.document.rope(), state.scroll_line, visible_end)
                .into_iter()
                .collect()
        })
        .unwrap_or_default();

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(viewport_height);

    // Group diagnostics by starting line so the inner loop can look up
    // "what's on this line" in O(1). A single diagnostic that spans
    // multiple lines only appears on its starting line for the gutter
    // marker; the inline underline covers the line range it actually
    // touches (handled below).
    let mut diag_by_line: std::collections::HashMap<u32, Vec<&Diagnostic>> =
        std::collections::HashMap::new();
    if let Some(lsp) = state.lsp_buffer_state.as_ref() {
        for d in &lsp.diagnostics {
            for line in d.range.start.line..=d.range.end.line {
                diag_by_line.entry(line).or_default().push(d);
            }
        }
    }

    for row in 0..viewport_height {
        let line_idx = state.scroll_line + row;
        if line_idx >= total_lines {
            // Tilde for lines past end of buffer (like vim ~)
            let gutter = format!("{:>width$} ", "~", width = gutter_width);
            lines.push(Line::from(vec![
                Span::styled(gutter, Style::default().fg(Color::DarkGray)),
            ]));
            continue;
        }

        let line_text = state.document.line(line_idx).to_string();
        // Strip trailing newline for display
        let display_text = line_text.trim_end_matches('\n');

        // Build the gutter: 1-char severity marker + line number.
        // The marker is ' ' (one space) when there's no diagnostic on
        // this line, so the gutter width stays constant and the line
        // numbers align.
        let line_diags: &[&Diagnostic] = diag_by_line
            .get(&(line_idx as u32))
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let line_severity = worst_severity(line_diags);
        let (marker_char, marker_color) = severity_marker(line_severity);
        let gutter_color = if line_severity.is_some() {
            marker_color
        } else if line_idx == cursor_line {
            Color::Yellow
        } else {
            Color::DarkGray
        };
        let gutter = format!(
            "{}{:>width$} ",
            marker_char,
            line_idx + 1,
            width = gutter_width.saturating_sub(1)
        );
        let empty_ranges: Vec<HighlightRange> = Vec::new();
        let line_ranges: &[HighlightRange] = ts_ranges_by_line
            .get(&line_idx)
            .map(|v| v.as_slice())
            .unwrap_or(&empty_ranges);

        let mut spans = vec![Span::styled(
            gutter,
            Style::default().fg(gutter_color),
        )];

        // Compute the character columns covered by diagnostics on this
        // line. For a single-line diagnostic we honour its start/end
        // character positions; for a multi-line diagnostic we cover
        // from the line start (or the diagnostic's start if this is
        // its first line) to the line end (or its end col on the last
        // line). Columns are in characters, not bytes — tree-sitter
        // and LSP both speak UTF-16, but our display is char-indexed
        // and markdown highlight spans are too, so this stays
        // consistent.
        let total_display_chars = display_text.chars().count();
        let diag_overlay: Vec<DiagnosticCover> = line_diags
            .iter()
            .filter_map(|d| {
                let sev = d.severity.unwrap_or(DiagnosticSeverity::INFORMATION);
                let line = line_idx as u32;
                let start_col = if d.range.start.line == line {
                    d.range.start.character as usize
                } else {
                    0
                };
                let end_col = if d.range.end.line == line {
                    d.range.end.character as usize
                } else {
                    total_display_chars
                };
                if end_col <= start_col {
                    return None;
                }
                Some(DiagnosticCover {
                    start_col,
                    end_col,
                    severity: sev,
                })
            })
            .collect();

        if line_idx == cursor_line {
            // Cursor line: respect syntax colour, overlay diagnostic
            // ranges, then overlay the cursor block at cursor_col.
            // Cap the char vector so a pathologically long line
            // (e.g. a 10 MB minified JSON line) doesn't allocate
            // millions of entries per frame — take enough to cover
            // both `text_width` and at least the cursor column so
            // the block is still visible if the cursor sits past
            // the fold. See `PERFORMANCE.md` rule 5.
            let cap = cursor_line_char_cap(text_width, cursor_col);
            let chars: Vec<char> = display_text.chars().take(cap).collect();
            push_highlighted_spans(
                &mut spans,
                &chars,
                line_ranges,
                &diag_overlay,
                Some(cursor_col),
            );
            // If cursor sits past the last char, append a trailing
            // block so the block is visible at EOL.
            if cursor_col >= chars.len() {
                spans.push(Span::styled(
                    " ",
                    Style::default().bg(Color::White).fg(Color::Black),
                ));
            }
        } else {
            // Non-cursor line: syntax-highlight + diagnostic overlay,
            // truncated to text_width.
            let truncated: Vec<char> = display_text.chars().take(text_width).collect();
            push_highlighted_spans(
                &mut spans,
                &truncated,
                line_ranges,
                &diag_overlay,
                None,
            );
        }
        lines.push(Line::from(spans));
    }

    let editor = Paragraph::new(lines);
    frame.render_widget(editor, area);
}

/// A diagnostic's coverage on a single display line. Char-indexed;
/// `[start_col..end_col)` is the span that gets the diagnostic
/// underline / colour overlay.
#[derive(Clone, Copy)]
struct DiagnosticCover {
    start_col: usize,
    end_col: usize,
    severity: DiagnosticSeverity,
}

/// Split `line_chars` into ratatui `Span`s coloured by `ranges`,
/// optionally overlaying diagnostic spans (`diag_overlay`) and a
/// cursor block, appending to `spans`.
///
/// Style precedence (lowest → highest):
///   1. Syntax colour from `ranges`
///   2. Diagnostic underline + color from `diag_overlay`
///   3. Cursor block at `cursor_col`
fn push_highlighted_spans(
    spans: &mut Vec<Span<'_>>,
    line_chars: &[char],
    ranges: &[HighlightRange],
    diag_overlay: &[DiagnosticCover],
    cursor_col: Option<usize>,
) {
    let total = line_chars.len();
    // Walk the line in order, emitting each sub-span.
    let mut col = 0usize;
    let mut range_iter = ranges.iter().peekable();

    while col < total {
        // Find the syntax style at this column, if any.
        let syntax_style = ranges
            .iter()
            .find(|r| r.start_col <= col && col < r.end_col)
            .map(|r| Style::default().fg(color_for_highlight(r.kind)));

        // Find the diagnostic covering this column, if any. We pick
        // the most severe if multiple overlap. The style layers on top
        // of the syntax style — we keep the syntax foreground but add
        // an underline and, for errors specifically, a red underline
        // colour so it's visually distinct from warnings.
        let diag_here = diag_overlay
            .iter()
            .filter(|d| d.start_col <= col && col < d.end_col)
            .min_by_key(|d| severity_rank(d.severity));

        // Find end of this stretch: end of covering syntax range,
        // start of next syntax range, end of covering diagnostic,
        // start of next diagnostic, cursor col, or EOL. The segment
        // within these boundaries has uniform styling.
        let mut next_boundary = total;
        if let Some(r) = ranges.iter().find(|r| r.start_col <= col && col < r.end_col) {
            next_boundary = next_boundary.min(r.end_col);
        }
        if let Some(r) = range_iter.peek() {
            if r.start_col > col {
                next_boundary = next_boundary.min(r.start_col);
            }
        }
        for d in diag_overlay {
            if d.start_col <= col && col < d.end_col {
                next_boundary = next_boundary.min(d.end_col);
            } else if d.start_col > col {
                next_boundary = next_boundary.min(d.start_col);
            }
        }
        if let Some(cc) = cursor_col {
            if cc > col {
                next_boundary = next_boundary.min(cc);
            } else if cc == col {
                // Emit exactly one char as the cursor, no style merging.
                let text: String = line_chars[col..col + 1].iter().collect();
                spans.push(Span::styled(
                    text,
                    Style::default().bg(Color::White).fg(Color::Black),
                ));
                col += 1;
                continue;
            }
        }
        next_boundary = next_boundary.max(col + 1).min(total);

        let text: String = line_chars[col..next_boundary].iter().collect();
        // Merge syntax + diagnostic styles. The diagnostic layer adds
        // an undercurl/underline and (for errors only) a red underline
        // colour, so errors stand out even when the token was already
        // coloured for syntax.
        let mut style = syntax_style.unwrap_or_default();
        if let Some(d) = diag_here {
            style = style.add_modifier(Modifier::UNDERLINED);
            if d.severity == DiagnosticSeverity::ERROR {
                style = style.underline_color(Color::LightRed);
            } else if d.severity == DiagnosticSeverity::WARNING {
                style = style.underline_color(Color::LightYellow);
            }
        }
        if syntax_style.is_none() && diag_here.is_none() {
            spans.push(Span::raw(text));
        } else {
            spans.push(Span::styled(text, style));
        }
        col = next_boundary;
        // Advance range_iter past any range that ended at or before col.
        while let Some(r) = range_iter.peek() {
            if r.end_col <= col {
                range_iter.next();
            } else {
                break;
            }
        }
    }
}

/// Render the Emacs-style mode line.
fn draw_status_bar(frame: &mut Frame, state: &TuiAppState, area: Rect) {
    // The bottom status bar describes the *focused* window (Emacs
    // behaviour: `mode-line-format` is active-window-dependent).
    let content = state
        .windows
        .content_at(state.focused_window)
        .unwrap_or(WindowContent::Buffer);
    render_mode_line(frame, state, area, content, /* focused = */ true);
}

/// Render a mode-line style horizontal bar. Used for:
/// - the bottom status bar (describes the focused window)
/// - each horizontal separator (describes the window above it)
///
/// The `focused` flag controls the background colour so the user can
/// tell at a glance which window is currently active — same as
/// Emacs's `mode-line` vs. `mode-line-inactive` faces.
fn render_mode_line(
    frame: &mut Frame,
    state: &TuiAppState,
    area: Rect,
    content: WindowContent,
    focused: bool,
) {
    let (left, right) = match content {
        WindowContent::Buffer => {
            let dirty = if state.document.is_dirty() { "**" } else { "--" };
            let line = state.cursor_line() + 1;
            let col = state.cursor_col();
            let total = state.document.len_lines();
            let diag_count = state
                .lsp_buffer_state
                .as_ref()
                .map(|s| s.diagnostics.len())
                .unwrap_or(0);
            // Language / major mode: derive from file extension (same
            // mapping the tree-sitter highlighter uses) so the user
            // sees e.g. `(Rust)` or `(Markdown)` beside the position.
            let lang = state
                .document
                .file_path()
                .and_then(|p| p.extension())
                .and_then(|e| e.to_str())
                .map(|ext| match ext {
                    "rs" => "Rust",
                    "md" | "markdown" => "Markdown",
                    "py" => "Python",
                    "ts" | "tsx" => "TypeScript",
                    "js" | "jsx" => "JavaScript",
                    "toml" => "TOML",
                    "json" => "JSON",
                    other => other,
                })
                .unwrap_or("Fundamental");
            let lsp_info = state
                .lsp_status
                .as_deref()
                .map(|s| format!("  {s}"))
                .unwrap_or_default();
            let diag_text = if diag_count > 0 {
                format!("  diag:{diag_count}")
            } else {
                String::new()
            };
            let left = format!(" {dirty} {}", state.current_buffer_name);
            let right =
                format!("L{line}:C{col}  ({total}L)  ({lang}){diag_text}{lsp_info} ");
            (left, right)
        }
        WindowContent::Diagnostics => {
            let count = state
                .lsp_buffer_state
                .as_ref()
                .map(|s| s.diagnostics.len())
                .unwrap_or(0);
            (" *Diagnostics* ".to_string(), format!("{count} total "))
        }
    };

    let padding = (area.width as usize).saturating_sub(left.len() + right.len());
    let mid = " ".repeat(padding);

    // `mode-line` (focused) vs `mode-line-inactive` (other windows).
    let style = if focused {
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(Color::Black).fg(Color::Gray)
    };

    let line = Line::from(vec![Span::styled(format!("{left}{mid}{right}"), style)]);
    frame.render_widget(Paragraph::new(vec![line]), area);
}

/// Render the message line, or the mini-buffer when it's active.
///
/// Priority (highest first):
///   1. Mini-buffer prompt + candidates (C-x C-f, M-x, …).
///   2. Transient `state.message` (set by commands — e.g. "Wrote
///      foo.md"). These clear after one frame.
///   3. LSP diagnostic on the cursor line — lets the user see the
///      error without running a separate command. Mirrors flymake's
///      echo-area behaviour.
///   4. Nothing.
fn draw_message_line(frame: &mut Frame, state: &TuiAppState, area: Rect) {
    if state.minibuffer.active {
        draw_minibuffer(frame, state, area);
        return;
    }

    // 2. Transient message.
    if let Some(msg) = state.message.as_deref() {
        let line = Line::from(Span::raw(msg.to_string()));
        frame.render_widget(Paragraph::new(vec![line]), area);
        return;
    }

    // 3. Diagnostic under the cursor.
    if let Some(diag) = state.diagnostic_at_cursor() {
        // Colour by the severity of the worst diagnostic on the line.
        // `severity_marker` already does that lookup.
        let sev = state
            .lsp_buffer_state
            .as_ref()
            .and_then(|lsp| {
                let cur = state.cursor_line() as u32;
                let here: Vec<&Diagnostic> = lsp
                    .diagnostics
                    .iter()
                    .filter(|d| d.range.start.line <= cur && d.range.end.line >= cur)
                    .collect();
                worst_severity(&here)
            });
        let (_, color) = severity_marker(sev);
        let line = Line::from(Span::styled(
            diag,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
        frame.render_widget(Paragraph::new(vec![line]), area);
        return;
    }

    // 4. Empty line.
    frame.render_widget(Paragraph::new(vec![Line::from("")]), area);
}

/// Render the mini-buffer prompt + input + candidate list.
fn draw_minibuffer(frame: &mut Frame, state: &TuiAppState, area: Rect) {
    let mb = &state.minibuffer;
    let prompt_label = mb
        .prompt
        .as_ref()
        .map(|p| p.label())
        .unwrap_or_else(|| "> ".to_string());

    // Build the prompt line with a visible cursor marker.
    // We don't have terminal cursor positioning here so we use a block
    // character at the cursor position.
    let cursor_pos = mb.cursor_pos.min(mb.input.chars().count());
    let before: String = mb.input.chars().take(cursor_pos).collect();
    let at_cursor = mb.input.chars().nth(cursor_pos).unwrap_or(' ');
    let after: String = mb.input.chars().skip(cursor_pos + 1).collect();

    let prompt_line = Line::from(vec![
        Span::styled(
            prompt_label,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(before),
        Span::styled(
            at_cursor.to_string(),
            Style::default().bg(Color::White).fg(Color::Black),
        ),
        Span::raw(after),
    ]);

    // Candidate list (above the prompt line if we have vertical room).
    let mut lines: Vec<Line<'_>> = Vec::new();
    let candidate_rows = (area.height as usize).saturating_sub(1);
    if candidate_rows > 0 && !mb.filtered.is_empty() {
        let max_shown = candidate_rows.min(5).min(mb.filtered.len());
        // Keep the selected candidate visible by windowing around it.
        let total = mb.filtered.len();
        let window_start = mb.selected.saturating_sub(max_shown / 2).min(total.saturating_sub(max_shown));
        for row in 0..max_shown {
            let fi = window_start + row;
            let Some(&candidate_idx) = mb.filtered.get(fi) else {
                break;
            };
            let Some(text) = mb.candidates.get(candidate_idx) else {
                continue;
            };
            let style = if fi == mb.selected {
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            lines.push(Line::from(Span::styled(text.clone(), style)));
        }
    }
    lines.push(prompt_line);

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// How many characters wide to make the line number gutter.
///
/// Layout: `[severity-marker][digits][space]`. The severity marker
/// takes the first column; numeric digits fill the middle; a single
/// trailing space separates the gutter from the line text.
fn line_number_width(total_lines: usize) -> usize {
    let digits = if total_lines == 0 {
        1
    } else {
        ((total_lines as f64).log10().floor() as usize) + 1
    };
    // +1 for the severity marker column. `max(3)` on digits keeps
    // short files from having an absurdly narrow gutter.
    digits.max(3) + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_rank_orders_most_severe_first() {
        // Lower rank = higher priority. Errors should rank lowest.
        assert!(severity_rank(DiagnosticSeverity::ERROR)
            < severity_rank(DiagnosticSeverity::WARNING));
        assert!(severity_rank(DiagnosticSeverity::WARNING)
            < severity_rank(DiagnosticSeverity::INFORMATION));
        assert!(severity_rank(DiagnosticSeverity::INFORMATION)
            < severity_rank(DiagnosticSeverity::HINT));
    }

    #[test]
    fn worst_severity_picks_error_over_warning() {
        fn d(sev: DiagnosticSeverity) -> Diagnostic {
            Diagnostic {
                range: lsp_types::Range {
                    start: lsp_types::Position { line: 0, character: 0 },
                    end: lsp_types::Position { line: 0, character: 1 },
                },
                severity: Some(sev),
                code: None,
                code_description: None,
                source: None,
                message: String::new(),
                related_information: None,
                tags: None,
                data: None,
            }
        }
        let warn = d(DiagnosticSeverity::WARNING);
        let err = d(DiagnosticSeverity::ERROR);
        let hint = d(DiagnosticSeverity::HINT);
        let pool = vec![&warn, &err, &hint];
        assert_eq!(worst_severity(&pool), Some(DiagnosticSeverity::ERROR));
    }

    /// Regression: `draw_editor` used to do
    /// `display_text.chars().collect()` on the cursor line, with no
    /// `take(text_width)`. A 10 MB cursor line then allocated 10 M
    /// chars per frame.
    #[test]
    fn cursor_line_char_cap_bounds_pathological_line() {
        // Typical viewport: 80-col window, cursor at col 3.
        assert_eq!(cursor_line_char_cap(80, 3), 80);
        // Cursor past the viewport: need to cover up to cursor + 1 so
        // the block is visible (user's `C-e` on a long line).
        assert_eq!(cursor_line_char_cap(80, 200), 201);
        // Zero-width (before first draw): still give the cursor block
        // somewhere to sit.
        assert_eq!(cursor_line_char_cap(0, 0), 1);
    }

    #[test]
    fn severity_marker_covers_all_severities() {
        // Sanity: every severity maps to a non-empty symbol.
        for sev in [
            DiagnosticSeverity::ERROR,
            DiagnosticSeverity::WARNING,
            DiagnosticSeverity::INFORMATION,
            DiagnosticSeverity::HINT,
        ] {
            let (marker, _) = severity_marker(Some(sev));
            assert!(!marker.is_empty());
        }
        // `None` should still produce a valid (whitespace) marker so
        // the gutter width stays consistent.
        let (marker, _) = severity_marker(None);
        assert_eq!(marker.len(), 1);
    }
}
