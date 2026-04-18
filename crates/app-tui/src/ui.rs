use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::state::TuiAppState;

/// Render the full TUI: editor pane + status bar + message line.
pub fn draw(frame: &mut Frame, state: &mut TuiAppState) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // editor pane
            Constraint::Length(1), // status bar
            Constraint::Length(1), // message / minibuffer
        ])
        .split(area);

    draw_editor(frame, state, chunks[0]);
    draw_status_bar(frame, state, chunks[1]);
    draw_message_line(frame, state, chunks[2]);
}

/// Render the text editing area with line numbers and cursor.
fn draw_editor(frame: &mut Frame, state: &mut TuiAppState, area: Rect) {
    let viewport_height = area.height as usize;
    state.ensure_cursor_visible(viewport_height);

    let total_lines = state.document.len_lines();
    let gutter_width = line_number_width(total_lines);
    let text_width = (area.width as usize).saturating_sub(gutter_width + 1); // +1 for separator

    let cursor_line = state.cursor_line();
    let cursor_col = state.cursor_col();

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(viewport_height);

    // Collect diagnostic lines for gutter markers.
    let diag_lines: std::collections::HashSet<u32> = state
        .lsp_buffer_state
        .as_ref()
        .map(|s| {
            s.diagnostics
                .iter()
                .map(|d| d.range.start.line)
                .collect()
        })
        .unwrap_or_default();

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

        let has_diag = diag_lines.contains(&(line_idx as u32));
        let gutter_color = if has_diag {
            Color::Red
        } else if line_idx == cursor_line {
            Color::Yellow
        } else {
            Color::DarkGray
        };
        let gutter = format!("{:>width$} ", line_idx + 1, width = gutter_width);

        if line_idx == cursor_line {
            // Build spans with cursor highlight
            let mut spans = vec![Span::styled(
                gutter,
                Style::default().fg(gutter_color),
            )];

            let chars: Vec<char> = display_text.chars().collect();
            if cursor_col < chars.len() {
                let before: String = chars[..cursor_col].iter().collect();
                let cursor_char: String = chars[cursor_col..=cursor_col].iter().collect();
                let after: String = chars[cursor_col + 1..].iter().collect();
                spans.push(Span::raw(before));
                spans.push(Span::styled(
                    cursor_char,
                    Style::default()
                        .bg(Color::White)
                        .fg(Color::Black),
                ));
                if !after.is_empty() {
                    spans.push(Span::raw(after));
                }
            } else {
                // Cursor at end of line — show block cursor on space
                spans.push(Span::raw(display_text.to_string()));
                spans.push(Span::styled(
                    " ",
                    Style::default()
                        .bg(Color::White)
                        .fg(Color::Black),
                ));
            }
            lines.push(Line::from(spans));
        } else {
            let truncated: String = display_text.chars().take(text_width).collect();
            lines.push(Line::from(vec![
                Span::styled(gutter, Style::default().fg(gutter_color)),
                Span::raw(truncated),
            ]));
        }
    }

    let editor = Paragraph::new(lines);
    frame.render_widget(editor, area);
}

/// Render the Emacs-style mode line.
fn draw_status_bar(frame: &mut Frame, state: &TuiAppState, area: Rect) {
    let dirty = if state.document.is_dirty() { "**" } else { "--" };
    let line = state.cursor_line() + 1;
    let col = state.cursor_col();
    let total = state.document.len_lines();

    let diag_count = state
        .lsp_buffer_state
        .as_ref()
        .map(|s| s.diagnostics.len())
        .unwrap_or(0);
    let lsp_info = if let Some(ref status) = state.lsp_status {
        format!("  {status}")
    } else {
        String::new()
    };
    let diag_text = if diag_count > 0 {
        format!("  diag:{diag_count}")
    } else {
        String::new()
    };

    let left = format!(" {dirty} {}", state.current_buffer_name);
    let right = format!("L{line}:C{col}  ({total}L){diag_text}{lsp_info} ");

    let padding = (area.width as usize)
        .saturating_sub(left.len() + right.len());
    let mid = " ".repeat(padding);

    let status_line = Line::from(vec![
        Span::styled(
            format!("{left}{mid}{right}"),
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let bar = Paragraph::new(vec![status_line]);
    frame.render_widget(bar, area);
}

/// Render the message / minibuffer line at the bottom.
fn draw_message_line(frame: &mut Frame, state: &TuiAppState, area: Rect) {
    let text = state.message.as_deref().unwrap_or("");
    let line = Line::from(Span::raw(text.to_string()));
    let msg = Paragraph::new(vec![line]);
    frame.render_widget(msg, area);
}

/// How many characters wide to make the line number gutter.
fn line_number_width(total_lines: usize) -> usize {
    let digits = if total_lines == 0 {
        1
    } else {
        ((total_lines as f64).log10().floor() as usize) + 1
    };
    digits.max(3)
}
