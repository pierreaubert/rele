use std::cell::Cell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::theme::ThemeExt;

use crate::markdown::{MdThemeColors, highlight_line, ts_ranges_to_spans};
use crate::state::MdAppState;

/// Estimated line height in pixels for scroll offset calculations.
///
/// Pub so that `MainView::sync_scroll` can project editor scroll offsets
/// onto source lines using the same grid the editor paints against.
pub const LINE_HEIGHT_PX: f32 = 20.0;

/// Lines of padding rendered above/below the visible viewport for smooth scrolling.
const VIRTUALIZATION_OVERSCAN: usize = 10;

/// Maximum number of characters of a single line that the editor will
/// render. Lines longer than this are truncated at render time (the
/// underlying `DocumentBuffer` is untouched) to keep pathological inputs —
/// minified JSON, compiled CSS, base64 blobs — from freezing GPUI or the
/// syntax highlighter.
///
/// Per `PERFORMANCE.md` Rule 5 — cap pathological inputs at render time.
const MAX_LINE_DISPLAY_CHARS: usize = 10_000;

/// Marker appended to truncated lines so the user knows it happened.
const LINE_TRUNCATED_MARKER: &str = " … [line truncated]";

/// Derive page jump size from a viewport height in pixels.
/// Falls back to 40 lines if height is unknown or zero.
fn page_lines_for_height(viewport_px: f32) -> usize {
    if viewport_px > 0.0 {
        ((viewport_px / LINE_HEIGHT_PX) as usize).max(1)
    } else {
        40
    }
}

/// Multi-line source editor pane with syntax highlighting, keyboard input,
/// line numbers, theme support, and mouse selection.
pub struct EditorPane {
    state: Entity<MdAppState>,
    pub scroll_handle: ScrollHandle,
    focus_handle: FocusHandle,
    /// Last known viewport height in pixels, used to derive page jump size.
    viewport_height: Rc<Cell<f32>>,
    /// Fingerprint of the state fields this view renders. The observer
    /// only fires `cx.notify()` when the fingerprint changes, so state
    /// mutations that don't affect the editor (e.g. preview settings,
    /// recent-files list) don't trigger an editor re-render.
    /// See `PERFORMANCE.md` Rule 10.
    last_fingerprint: Cell<EditorFingerprint>,
}

/// Fields the editor pane actually displays. Any change here should
/// trigger a re-render; anything outside should not.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
struct EditorFingerprint {
    doc_version: u64,
    cursor_pos: usize,
    anchor: Option<usize>,
    font_size_bits: u32,
    show_line_numbers: bool,
    find_bar_visible: bool,
    find_query_len: usize,
    isearch_active: bool,
    universal_arg: Option<usize>,
    completion_visible: bool,
    completion_selected: usize,
    hover_present: bool,
    /// Number of diagnostics on the active buffer — a change here is rare
    /// but does change what we paint (underlines).
    diag_count: usize,
}

impl EditorFingerprint {
    fn from_state(s: &MdAppState) -> Self {
        Self {
            doc_version: s.document.version(),
            cursor_pos: s.cursor.position,
            anchor: s.cursor.anchor,
            font_size_bits: s.font_size.to_bits(),
            show_line_numbers: s.show_line_numbers,
            find_bar_visible: s.find_bar_visible,
            find_query_len: s.find_query.len(),
            isearch_active: s.isearch.active,
            universal_arg: s.universal_arg,
            completion_visible: s.lsp_completion_visible,
            completion_selected: s.lsp_completion_selected,
            hover_present: s.lsp_hover_text.is_some(),
            diag_count: s
                .lsp_buffer_state
                .as_ref()
                .map(|b| b.diagnostics.len())
                .unwrap_or(0),
        }
    }
}

impl EditorPane {
    pub fn new(state: Entity<MdAppState>, cx: &mut Context<Self>) -> Self {
        let initial_fp = EditorFingerprint::from_state(state.read(cx));

        // Only notify when a field the editor renders has changed. This
        // shields the editor from re-rendering when unrelated state
        // mutates (e.g. preview settings, recent-files list, dired state).
        cx.observe(&state, |this, state, cx| {
            let fp = EditorFingerprint::from_state(state.read(cx));
            if fp != this.last_fingerprint.get() {
                this.last_fingerprint.set(fp);
                cx.notify();
            }
        })
        .detach();

        Self {
            state,
            scroll_handle: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
            viewport_height: Rc::new(Cell::new(0.0)),
            last_fingerprint: Cell::new(initial_fp),
        }
    }
}

impl Render for EditorPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let state = self.state.read(cx);
        let cursor_pos = state.cursor.position;
        let show_line_numbers = state.show_line_numbers;
        let find_query = state.find_query.clone();
        let selection = state.cursor.selection();
        let editor_font_size = state.font_size;

        // Capture status bar state before dropping the borrow
        let isearch_active = state.isearch.active;
        let isearch_text = if isearch_active {
            let dir = if state.isearch.direction == crate::state::IsearchDirection::Backward {
                " backward"
            } else {
                ""
            };
            format!("I-search{}: {}", dir, state.isearch.query)
        } else {
            String::new()
        };

        let universal_arg_text = match state.universal_arg {
            Some(n) => format!("C-u {}", n),
            None => String::new(),
        };
        let show_universal_arg = state.universal_arg.is_some();

        let doc = &state.document;
        let total_lines = doc.len_lines();
        let clamped_cursor = cursor_pos.min(doc.len_chars());
        let cursor_line = if doc.len_chars() > 0 {
            doc.char_to_line(clamped_cursor)
        } else {
            0
        };
        let cursor_col = clamped_cursor - doc.line_to_char(cursor_line);

        let gutter_font_size = editor_font_size - 1.0;
        let gutter_digit_width = gutter_font_size * 0.65;
        let gutter_width = if show_line_numbers {
            let digits = format!("{}", total_lines).len().max(3);
            digits as f32 * gutter_digit_width + 12.0
        } else {
            0.0
        };

        // Collect diagnostic line ranges for underline rendering.
        let diag_ranges: Vec<(u32, u32, u32, u32)> = state
            .lsp_buffer_state
            .as_ref()
            .map(|s| {
                s.diagnostics
                    .iter()
                    .map(|d| {
                        (
                            d.range.start.line,
                            d.range.start.character,
                            d.range.end.line,
                            d.range.end.character,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        let md_colors = MdThemeColors::from_theme(&theme);
        let bg_color = theme.background;
        let surface_color = theme.surface;
        let text_muted = theme.text_muted;
        let _accent_muted = theme.accent_muted; // kept for potential future use
        let border_color = theme.border;
        let text_color = theme.text_primary;

        // Viewport virtualization: compute visible line range from scroll offset
        let scroll_offset_y = (-self.scroll_handle.offset().y).into();
        let scroll_top: f32 = if scroll_offset_y > 0.0 {
            scroll_offset_y
        } else {
            0.0
        };
        let viewport_px = self.viewport_height.get().max(800.0); // fallback until measured
        let first_visible = (scroll_top / LINE_HEIGHT_PX) as usize;
        let visible_count = (viewport_px / LINE_HEIGHT_PX) as usize + 1;

        let render_start = first_visible.saturating_sub(VIRTUALIZATION_OVERSCAN);
        let render_end = (first_visible + visible_count + VIRTUALIZATION_OVERSCAN).min(total_lines);

        let mut line_elements: Vec<AnyElement> = Vec::with_capacity(render_end - render_start + 2);

        // Top spacer for lines above the rendered range
        if render_start > 0 {
            let spacer_height = render_start as f32 * LINE_HEIGHT_PX;
            line_elements.push(div().h(px(spacer_height)).w_full().into_any_element());
        }

        // Tree-sitter highlighting path: if the buffer has a grammar,
        // reparse (if stale) and query the visible range once. The per-
        // line loop below uses these ranges directly. Otherwise the
        // regex `highlight_line` runs per line with the `in_code_block`
        // state we derive by scanning from line 0 to `render_start`.
        let has_ts_highlighter = state.buffer_highlighter.is_some();
        // Release the immutable borrow so we can call the mutable
        // `ensure_highlight_fresh` below.
        let _ = state;
        if has_ts_highlighter {
            self.state.update(cx, |s, _cx| s.ensure_highlight_fresh());
        }
        let state = self.state.read(cx);
        let doc = &state.document; // rebind after state.update
        let ts_ranges_by_line: std::collections::HashMap<
            usize,
            Vec<rele_server::syntax::HighlightRange>,
        > = if let Some(h) = state.buffer_highlighter.as_ref() {
            h.highlight_range(doc.rope(), render_start, render_end)
                .into_iter()
                .collect()
        } else {
            std::collections::HashMap::new()
        };

        // Track code block state by scanning from line 0 to render_start.
        // Skipped entirely when tree-sitter is driving — the whole-file
        // parse tree carries the context naturally.
        let mut in_code_block = false;
        if !has_ts_highlighter {
            for i in 0..render_start {
                let line_slice = doc.line(i);
                let line_str: String = line_slice.to_string();
                let trimmed = line_str.trim_start();
                if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                    in_code_block = !in_code_block;
                }
            }
        }

        for i in render_start..render_end {
            let line_slice = doc.line(i);
            // Pull characters one chunk at a time and stop early if the line
            // exceeds MAX_LINE_DISPLAY_CHARS. This avoids materialising a
            // 10 MB `String` for pathological lines — we pay only for what
            // we display.
            let line_slice_len = line_slice.len_chars();
            let (line_text_owned, was_truncated): (String, bool) =
                if line_slice_len > MAX_LINE_DISPLAY_CHARS {
                    let mut s = String::with_capacity(MAX_LINE_DISPLAY_CHARS * 4);
                    for ch in line_slice.chars().take(MAX_LINE_DISPLAY_CHARS) {
                        s.push(ch);
                    }
                    s.push_str(LINE_TRUNCATED_MARKER);
                    (s, true)
                } else {
                    let s = line_slice.to_string();
                    let stripped = if s.ends_with('\n') {
                        s[..s.len() - 1].to_string()
                    } else {
                        s
                    };
                    (stripped, false)
                };
            let line_text: &str = &line_text_owned;
            let is_cursor_line = i == cursor_line;
            let line_char_offset = doc.line_to_char(i);
            // Char count of the DISPLAY text, not the underlying line.
            // Used for selection/cursor math on the rendered line only;
            // off-screen characters past the truncation marker are
            // un-clickable by design (user must scroll or wrap the file).
            let line_char_count = line_text.chars().count();
            let _ = was_truncated;
            let state_for_click = self.state.clone();
            let focus_for_click = self.focus_handle.clone();
            let state_for_drag = self.state.clone();

            // Prefer tree-sitter ranges when the buffer has a grammar;
            // fall back to the regex line highlighter otherwise. The
            // per-line `in_code_block` state is only advanced on the
            // regex path — tree-sitter handles fences globally.
            let (spans, new_code_block) = if has_ts_highlighter {
                let default = ts_ranges_by_line
                    .get(&i)
                    .map(|r| ts_ranges_to_spans(line_text, r, &md_colors))
                    .unwrap_or_else(|| ts_ranges_to_spans(line_text, &[], &md_colors));
                (default, in_code_block)
            } else {
                let (spans, next) = highlight_line(line_text, in_code_block, &md_colors);
                (spans, next)
            };
            in_code_block = new_code_block;

            let line_start = line_char_offset;
            let line_end = line_char_offset + line_char_count;

            // Compute per-character selection range within this line (in char cols)
            let sel_cols = selection.and_then(|(sel_start, sel_end)| {
                if sel_start < line_end && sel_end > line_start {
                    let col_start = sel_start.saturating_sub(line_start);
                    let col_end = (sel_end - line_start).min(line_char_count);
                    Some((col_start, col_end))
                } else {
                    None
                }
            });

            let mut line_div = div()
                .flex()
                .flex_row()
                .w_full()
                .min_h(px(20.0))
                .cursor_text();

            // Line number gutter (no mouse handler -- click on gutter selects line)
            if show_line_numbers {
                let state_for_gutter = self.state.clone();
                line_div = line_div.child(
                    div()
                        .min_w(px(gutter_width))
                        .pr_2()
                        .text_size(px(gutter_font_size))
                        .text_color(text_muted)
                        .font_family("monospace")
                        .flex_shrink_0()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_right()
                        .on_mouse_down(MouseButton::Left, move |ev, _window, cx| {
                            if ev.click_count >= 1 {
                                state_for_gutter.update(cx, |s, _cx| {
                                    s.select_line_at(line_char_offset);
                                });
                            }
                        })
                        .child(format!("{:>width$}", i + 1, width = 3)),
                );
            }

            if is_cursor_line {
                line_div = line_div.bg(surface_color);
            }

            // Collect diagnostic column ranges for this line.
            let line_diag_cols: Vec<(usize, usize)> = diag_ranges
                .iter()
                .filter(|(sl, _, el, _)| *sl <= i as u32 && *el >= i as u32)
                .map(|(sl, sc, el, ec)| {
                    let col_start = if *sl == i as u32 { *sc as usize } else { 0 };
                    let col_end = if *el == i as u32 {
                        *ec as usize
                    } else {
                        line_char_count
                    };
                    (col_start, col_end.max(col_start + 1))
                })
                .collect();

            // Build StyledText with syntax highlighting, selection, cursor, and find overlays
            let (text_content, runs) = build_line_text_runs(
                &spans,
                line_text,
                is_cursor_line,
                cursor_col,
                sel_cols,
                &find_query,
                &md_colors,
                &line_diag_cols,
            );

            let styled = StyledText::new(SharedString::from(text_content.clone())).with_runs(runs);
            let text_layout = styled.layout().clone();

            // Content area with StyledText and mouse handlers
            let text_for_click = text_content.clone();
            let text_for_drag = text_content;
            let content_div = div()
                .flex_grow()
                .min_w(px(0.0))
                .overflow_hidden()
                .text_size(px(editor_font_size))
                .font_family("monospace")
                .child(styled)
                .on_mouse_down(MouseButton::Left, {
                    let text_layout = text_layout.clone();
                    let state_click = state_for_click;
                    let focus = focus_for_click;
                    move |ev, window, cx| {
                        window.focus(&focus, cx);

                        let byte_idx = match text_layout.index_for_position(ev.position) {
                            Ok(idx) | Err(idx) => idx,
                        };
                        let col = byte_index_to_char_index(&text_for_click, byte_idx);
                        let char_pos = line_char_offset + col.min(line_char_count);

                        let click_count = ev.click_count;
                        let shift = ev.modifiers.shift;

                        state_click.update(cx, |s, _cx| match click_count {
                            2 => s.select_word_at(char_pos),
                            3 => s.select_line_at(char_pos),
                            _ => {
                                if shift {
                                    s.cursor.start_selection();
                                    s.cursor.position = char_pos;
                                } else {
                                    s.cursor.position = char_pos;
                                    s.cursor.clear_selection();
                                    s.cursor.anchor = Some(char_pos);
                                }
                            }
                        });
                    }
                })
                .on_mouse_move({
                    let text_layout = text_layout;
                    let state_drag = state_for_drag;
                    move |ev, window, cx| {
                        if ev.pressed_button.is_none() {
                            return;
                        }

                        let byte_idx = match text_layout.index_for_position(ev.position) {
                            Ok(idx) | Err(idx) => idx,
                        };
                        let col = byte_index_to_char_index(&text_for_drag, byte_idx);
                        let char_pos = line_char_offset + col.min(line_char_count);

                        state_drag.update(cx, |s, _cx| {
                            if s.cursor.anchor.is_some() {
                                s.cursor.position = char_pos;
                            }
                        });
                        window.refresh();
                    }
                });

            line_div = line_div.child(content_div);
            line_elements.push(line_div.into_any_element());
        }

        // Bottom spacer for lines below the rendered range
        if render_end < total_lines {
            let spacer_height = (total_lines - render_end) as f32 * LINE_HEIGHT_PX;
            line_elements.push(div().h(px(spacer_height)).w_full().into_any_element());
        }

        // Key handler state references
        // Completion popup state
        let completion_visible = state.lsp_completion_visible;
        let completion_items: Vec<(String, bool)> = if completion_visible {
            state
                .lsp_completion_items
                .iter()
                .enumerate()
                .take(10)
                .map(|(idx, item)| (item.label.clone(), idx == state.lsp_completion_selected))
                .collect()
        } else {
            Vec::new()
        };

        // Hover text (shown below completion popup or standalone)
        let hover_text = state.lsp_hover_text.clone();

        let state_for_keys = self.state.clone();
        let focus_for_keys = self.focus_handle.clone();
        let scroll_for_keys = self.scroll_handle.clone();
        let viewport_height_for_keys = self.viewport_height.clone();

        // Measure viewport height for page-jump and virtualization
        let viewport_height_for_measure = self.viewport_height.clone();

        div()
            .id("editor-pane")
            .size_full()
            .bg(bg_color)
            .overflow_y_scroll()
            .track_scroll(&self.scroll_handle)
            .track_focus(&self.focus_handle)
            .focusable()
            .p_2()
            .on_scroll_wheel({
                let state_scroll = self.state.clone();
                move |_ev, _window, cx| {
                    // Poke state so MainView re-renders and sync_scroll fires.
                    state_scroll.update(cx, |_s, _cx| {});
                }
            })
            .on_mouse_move({
                let vh = viewport_height_for_measure;
                move |ev, window, _cx| {
                    // Use the window's viewport bounds to estimate editor height.
                    // This fires often enough to keep the value fresh.
                    let bounds = window.bounds();
                    let h: f32 = bounds.size.height.into();
                    if h > 0.0 {
                        vh.set(h);
                    }
                    let _ = ev;
                }
            })
            // Keyboard input
            .on_key_down(move |event, window, cx| {
                if !focus_for_keys.is_focused(window) {
                    return;
                }
                cx.stop_propagation();

                let key = event.keystroke.key.as_str();
                let shift = event.keystroke.modifiers.shift;
                let ctrl = event.keystroke.modifiers.control;
                let cmd = event.keystroke.modifiers.platform; // Cmd on macOS
                let alt = event.keystroke.modifiers.alt;
                let modifier = ctrl || cmd; // platform-agnostic "primary modifier"

                // ---- 0. Mini-buffer intercept (top priority when active) ----
                let mb_active = state_for_keys.read(cx).minibuffer.active;
                if mb_active {
                    let mb_is_yesno = state_for_keys
                        .read(cx)
                        .minibuffer
                        .prompt
                        .as_ref()
                        .map(|p| p.is_yes_no())
                        .unwrap_or(false);
                    if mb_is_yesno {
                        match key {
                            "y" | "Y" => {
                                state_for_keys.update(cx, |s, _| s.minibuffer_submit_yes_no(true))
                            }
                            "n" | "N" | "escape" => {
                                state_for_keys.update(cx, |s, _| s.minibuffer_submit_yes_no(false))
                            }
                            "g" if ctrl => state_for_keys.update(cx, |s, _| s.minibuffer_cancel()),
                            _ => {}
                        }
                    } else {
                        match key {
                            "g" if ctrl => state_for_keys.update(cx, |s, _| s.minibuffer_cancel()),
                            "escape" => state_for_keys.update(cx, |s, _| s.minibuffer_cancel()),
                            "enter" => state_for_keys.update(cx, |s, _| s.minibuffer_submit()),
                            "backspace" => {
                                state_for_keys.update(cx, |s, _| s.minibuffer.backspace())
                            }
                            "delete" => {
                                state_for_keys.update(cx, |s, _| s.minibuffer.delete_forward())
                            }
                            "left" => state_for_keys.update(cx, |s, _| s.minibuffer.move_left()),
                            "right" => state_for_keys.update(cx, |s, _| s.minibuffer.move_right()),
                            "home" => {
                                state_for_keys.update(cx, |s, _| s.minibuffer.move_to_start())
                            }
                            "end" => state_for_keys.update(cx, |s, _| s.minibuffer.move_to_end()),
                            "tab" => state_for_keys.update(cx, |s, _| s.minibuffer.complete()),
                            "down" => state_for_keys.update(cx, |s, _| s.minibuffer.select_next()),
                            "up" => state_for_keys.update(cx, |s, _| s.minibuffer.select_prev()),
                            "p" if alt => {
                                state_for_keys.update(cx, |s, _| s.minibuffer.history_prev())
                            }
                            "n" if alt => {
                                state_for_keys.update(cx, |s, _| s.minibuffer.history_next())
                            }
                            _ if !ctrl && !cmd && !alt => {
                                if let Some(ch) = &event.keystroke.key_char {
                                    let ch = ch.clone();
                                    state_for_keys.update(cx, |s, _| {
                                        for c in ch.chars() {
                                            s.minibuffer.add_char(c);
                                        }
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                    window.refresh();
                    return;
                }

                // ---- 1. Isearch intercept (top priority when active) ----
                let isearch_active = state_for_keys.read(cx).isearch.active;
                if isearch_active {
                    match key {
                        "s" if ctrl => state_for_keys.update(cx, |s, _| s.isearch_next()),
                        "r" if ctrl => state_for_keys.update(cx, |s, _| s.isearch_prev()),
                        "g" if ctrl => state_for_keys.update(cx, |s, _| s.isearch_abort()),
                        "enter" | "escape" => state_for_keys.update(cx, |s, _| s.isearch_exit()),
                        "backspace" => state_for_keys.update(cx, |s, _| s.isearch_backspace()),
                        _ if !ctrl && !cmd && !alt => {
                            if let Some(ch) = &event.keystroke.key_char {
                                let ch = ch.clone();
                                state_for_keys.update(cx, |s, _| {
                                    for c in ch.chars() {
                                        s.isearch_add_char(c);
                                    }
                                });
                            }
                        }
                        _ => {} // ignore other modified keys during isearch
                    }
                    window.refresh();
                    return;
                }

                // ---- 2. Dired key map (when current buffer is dired) ----
                let is_dired = state_for_keys.read(cx).current_buffer_kind
                    == crate::document::BufferKind::Dired;
                if is_dired {
                    match key {
                        "n" | "down" => state_for_keys.update(cx, |s, _| s.dired_next()),
                        "p" | "up" => state_for_keys.update(cx, |s, _| s.dired_prev()),
                        "enter" => state_for_keys.update(cx, |s, _| s.dired_open_selected()),
                        "d" => state_for_keys.update(cx, |s, _| s.dired_mark_delete()),
                        "u" => state_for_keys.update(cx, |s, _| s.dired_unmark()),
                        "x" => state_for_keys.update(cx, |s, _| s.dired_execute_marks()),
                        "g" if !ctrl && !cmd && !alt => {
                            state_for_keys.update(cx, |s, _| {
                                let _ = s.dired_refresh();
                            });
                        }
                        "^" => state_for_keys.update(cx, |s, _| s.dired_up_directory()),
                        "q" => state_for_keys.update(cx, |s, _| {
                            s.kill_current_buffer();
                        }),
                        _ => {} // ignore other keys in dired
                    }
                    window.refresh();
                    return;
                }

                // ---- 3. Universal arg digit accumulation ----
                let is_accumulating = state_for_keys.read(cx).universal_arg_accumulating;
                if is_accumulating && !ctrl && !cmd && !alt {
                    if let Some(digit) = key.parse::<u8>().ok().filter(|&d| d <= 9) {
                        state_for_keys.update(cx, |s, _| {
                            s.universal_arg_digit(digit);
                        });
                        window.refresh();
                        return;
                    }
                    // Non-digit exits accumulation but falls through to handle the key
                    state_for_keys.update(cx, |s, _| {
                        s.universal_arg_accumulating = false;
                    });
                }

                // ---- 4. C-v page down (emacs-only, before clipboard) ----
                if key == "v" && ctrl && !cmd {
                    let is_emacs = state_for_keys.read(cx).keymap_preset
                        == gpui_keybinding::KeymapPreset::Emacs;
                    if is_emacs {
                        let page = page_lines_for_height(viewport_height_for_keys.get());
                        state_for_keys.update(cx, |s, _| {
                            for _ in 0..page {
                                s.move_down(shift);
                            }
                        });
                        window.refresh();
                        return;
                    }
                }

                // ---- 5. C-l recenter ----
                if key == "l" && ctrl && !cmd {
                    let st = state_for_keys.read(cx);
                    let cursor_ln = if st.document.len_chars() > 0 {
                        let pos = st.cursor.position.min(st.document.len_chars());
                        st.document.char_to_line(pos)
                    } else {
                        0
                    };
                    let _ = st;
                    // Scroll so cursor line is roughly centered (~20 visible lines)
                    let target = cursor_ln.saturating_sub(20) as f32 * LINE_HEIGHT_PX;
                    scroll_for_keys.set_offset(point(scroll_for_keys.offset().x, px(-target)));
                    window.refresh();
                    return;
                }

                // Undo/redo and clipboard operations need cx, handle separately
                if key == "z" && modifier && shift {
                    state_for_keys.update(cx, |s, _cx| s.redo());
                    window.refresh();
                    return;
                }
                if key == "z" && modifier {
                    state_for_keys.update(cx, |s, _cx| s.undo());
                    window.refresh();
                    return;
                }

                // Clipboard shortcuts: only the *platform* modifier
                // (Cmd on macOS, the configured "secondary" elsewhere)
                // — NOT plain Ctrl. Otherwise `Ctrl+X` gets eaten as
                // "cut" and the Emacs `C-x` chord prefix never fires,
                // which kills `C-x C-c`, `C-x C-s`, `C-x C-f`, etc.
                let clipboard_op = match key {
                    "c" if cmd => Some("copy"),
                    "x" if cmd && !shift => Some("cut"),
                    "v" if cmd => Some("paste"),
                    _ => None,
                };

                if let Some(op) = clipboard_op {
                    match op {
                        "copy" => {
                            let text = state_for_keys.read(cx).selected_text();
                            if let Some(text) = text {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                            }
                        }
                        "cut" => {
                            let text = state_for_keys.update(cx, |s, _cx| s.delete_selection());
                            if let Some(text) = text {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                            }
                        }
                        "paste" => {
                            if let Some(clipboard) = cx.read_from_clipboard()
                                && let Some(text) = clipboard.text()
                            {
                                state_for_keys.update(cx, |s, _cx| {
                                    s.kill_ring.push(text.clone());
                                    s.insert_text(&text);
                                });
                            }
                        }
                        _ => {}
                    }
                } else {
                    let page_lines = page_lines_for_height(viewport_height_for_keys.get());
                    // Set inside the state.update closure when `C-x C-c`
                    // fires; checked + acted on after the closure
                    // returns since `cx.quit()` needs the outer `cx`.
                    let want_quit = std::cell::Cell::new(false);
                    state_for_keys.update(cx, |s, _cx| {
                        // Esc as Meta prefix (classic Emacs terminal convention):
                        // pressing Escape then a key is equivalent to Alt+key.
                        let esc_meta = s.meta_pending;
                        if esc_meta {
                            s.meta_pending = false;
                        }
                        let alt = alt || esc_meta;

                        // Reset kill ring flags for non-kill operations
                        let is_kill_op = (key == "k" && ctrl)
                            || (key == "w" && (ctrl || alt))
                            || (key == "d" && alt)
                            || (key == "backspace" && alt);

                        if !is_kill_op {
                            s.kill_ring.clear_kill_flag();
                        }

                        // Handle C-x r <key> (rectangle operations)
                        if s.c_x_r_pending {
                            s.c_x_r_pending = false;
                            s.c_x_pending = false;
                            match key {
                                "k" => s.kill_rectangle(),
                                "y" => s.yank_rectangle(),
                                "d" => s.delete_rectangle(),
                                _ => {} // unknown C-x r <key>: ignore
                            }
                            return;
                        }

                        // Handle C-x <key> prefix (Emacs two-key chords)
                        if s.c_x_pending {
                            s.c_x_pending = false;
                            match key {
                                "r" => {
                                    s.c_x_r_pending = true;
                                    return;
                                }
                                "u" => s.undo(),
                                "h" => s.select_all(),
                                // C-x b — switch to buffer (minibuffer)
                                "b" => {
                                    s.minibuffer_start_switch_buffer();
                                }
                                // C-x k — kill buffer (minibuffer)
                                "k" => {
                                    s.minibuffer_start_kill_buffer();
                                }
                                // C-x C-f — find file (minibuffer)
                                "f" if ctrl => {
                                    s.minibuffer_start_find_file();
                                }
                                // C-x C-s — save current buffer.
                                "s" if ctrl => {
                                    let _ = s.save_file_from_elisp();
                                }
                                // C-x C-c — quit the editor. The
                                // top-level keymap binds this to
                                // `Quit` but the per-key on_key_down
                                // intercepts `C-x` first, which
                                // shadows the chord; dispatch the
                                // action directly here so the
                                // binding works on the first
                                // keystroke after launch.
                                "c" if ctrl => {
                                    want_quit.set(true);
                                }
                                // C-x right — next buffer
                                "right" => {
                                    s.switch_to_next_buffer();
                                }
                                // C-x left — previous buffer
                                "left" => {
                                    s.switch_to_prev_buffer();
                                }
                                // C-x ( — start recording a macro
                                "(" => s.macro_start_recording(),
                                // C-x ) — stop recording a macro
                                ")" => s.macro_stop_recording(),
                                // C-x e — execute last macro (C-u N for count)
                                "e" => {
                                    let n = s.take_universal_arg();
                                    s.macro_execute_last(n);
                                }
                                _ => {} // unknown C-x <key>: ignore
                            }
                            return;
                        }

                        match key {
                            // Arrow keys (with Alt for word movement)
                            "left" if alt => {
                                s.record_action(crate::macros::RecordedAction::MoveWordLeft);
                                s.move_word_left(shift);
                            }
                            "right" if alt => {
                                s.record_action(crate::macros::RecordedAction::MoveWordRight);
                                s.move_word_right(shift);
                            }
                            "left" => {
                                s.record_action(crate::macros::RecordedAction::MoveLeft);
                                s.move_left(shift);
                            }
                            "right" => {
                                s.record_action(crate::macros::RecordedAction::MoveRight);
                                s.move_right(shift);
                            }
                            "up" => {
                                s.record_action(crate::macros::RecordedAction::MoveUp);
                                s.move_up(shift);
                            }
                            "down" => {
                                s.record_action(crate::macros::RecordedAction::MoveDown);
                                s.move_down(shift);
                            }

                            // Page Up/Down
                            "pageup" => {
                                for _ in 0..page_lines {
                                    s.move_up(shift);
                                }
                            }
                            "pagedown" => {
                                for _ in 0..page_lines {
                                    s.move_down(shift);
                                }
                            }

                            // Home/End
                            "home" if modifier => s.move_to_doc_start(shift),
                            "end" if modifier => s.move_to_doc_end(shift),
                            "home" => s.move_to_line_start(shift),
                            "end" => s.move_to_line_end(shift),

                            // Emacs-style navigation (C-a/e/f/b/p/n) — active in all presets.
                            // These use Ctrl (not Cmd) so they don't conflict with
                            // Cmd+A (Select All), Cmd+F (Find), Cmd+B (Bold), etc.
                            "a" if ctrl && !cmd => {
                                s.move_to_line_start(shift);
                            }
                            "e" if ctrl && !cmd => {
                                s.move_to_line_end(shift);
                            }
                            "f" if ctrl && !cmd => {
                                let n = s.take_universal_arg();
                                for _ in 0..n {
                                    s.move_right(shift);
                                }
                            }
                            "b" if ctrl && !cmd => {
                                let n = s.take_universal_arg();
                                for _ in 0..n {
                                    s.move_left(shift);
                                }
                            }
                            "p" if ctrl && !cmd => {
                                let n = s.take_universal_arg();
                                for _ in 0..n {
                                    s.move_up(shift);
                                }
                            }
                            "n" if ctrl && !cmd => {
                                let n = s.take_universal_arg();
                                for _ in 0..n {
                                    s.move_down(shift);
                                }
                            }

                            // M-f / M-b: word movement
                            "f" if alt && !ctrl && !cmd => {
                                let n = s.take_universal_arg();
                                for _ in 0..n {
                                    s.move_word_right(shift);
                                }
                            }
                            "b" if alt && !ctrl && !cmd => {
                                let n = s.take_universal_arg();
                                for _ in 0..n {
                                    s.move_word_left(shift);
                                }
                            }

                            // M-v: page up
                            "v" if alt && !ctrl && !cmd => {
                                for _ in 0..page_lines {
                                    s.move_up(shift);
                                }
                            }

                            // C-g: abort
                            "g" if ctrl && !cmd => s.abort(),

                            // C-u: universal argument
                            "u" if ctrl && !cmd => s.universal_argument(),

                            // M-u: upcase word (with universal arg)
                            "u" if alt && !ctrl && !cmd => {
                                let n = s.take_universal_arg();
                                for _ in 0..n {
                                    s.upcase_word();
                                }
                            }

                            // M-l: downcase word (with universal arg)
                            "l" if alt && !ctrl && !cmd => {
                                let n = s.take_universal_arg();
                                for _ in 0..n {
                                    s.downcase_word();
                                }
                            }

                            // M-x: command prompt (via mini-buffer + command registry)
                            "x" if alt && !ctrl && !cmd => s.minibuffer_start_command(),

                            // C-x: prefix key for two-key chords (Emacs only)
                            "x" if ctrl
                                && !cmd
                                && s.keymap_preset == gpui_keybinding::KeymapPreset::Emacs =>
                            {
                                s.c_x_pending = true;
                            }

                            // C-s / C-r: isearch entry (emacs-only)
                            "s" if ctrl && !cmd => {
                                if s.keymap_preset == gpui_keybinding::KeymapPreset::Emacs {
                                    s.isearch_start(crate::state::IsearchDirection::Forward);
                                }
                            }
                            "r" if ctrl && !cmd => {
                                if s.keymap_preset == gpui_keybinding::KeymapPreset::Emacs {
                                    s.isearch_start(crate::state::IsearchDirection::Backward);
                                }
                            }

                            // Emacs kill ring — C-k/C-w/C-y gated by preset
                            "k" if ctrl
                                && s.keymap_preset == gpui_keybinding::KeymapPreset::Emacs =>
                            {
                                s.kill_to_end_of_line();
                            }
                            "w" if ctrl
                                && s.keymap_preset == gpui_keybinding::KeymapPreset::Emacs =>
                            {
                                s.kill_region();
                            }
                            "y" if ctrl
                                && s.keymap_preset == gpui_keybinding::KeymapPreset::Emacs =>
                            {
                                s.yank();
                            }
                            // Alt-based kill ring ops — safe for all presets
                            "d" if alt => s.kill_word_forward(),
                            "w" if alt => s.copy_region(),
                            "y" if alt => s.yank_pop(),

                            // Emacs mark and exchange — safe for all presets
                            "space" if ctrl => s.set_mark(),

                            // C-t: transpose chars (Emacs)
                            "t" if ctrl
                                && !cmd
                                && s.keymap_preset == gpui_keybinding::KeymapPreset::Emacs =>
                            {
                                s.transpose_chars();
                            }

                            // M-t: transpose words
                            "t" if alt && !ctrl && !cmd => s.transpose_words(),

                            // M-z: zap to char
                            "z" if alt && !ctrl && !cmd => s.zap_to_char_start(),

                            // Emacs buffer boundaries
                            "<" if alt => s.move_to_doc_start(shift),
                            ">" if alt => s.move_to_doc_end(shift),

                            // Deletion with universal arg
                            "backspace" if alt => {
                                s.record_action(crate::macros::RecordedAction::KillWordBackward);
                                s.kill_word_backward();
                            }
                            "backspace" => {
                                s.record_action(crate::macros::RecordedAction::Backspace);
                                s.backspace();
                            }
                            "delete" => {
                                s.record_action(crate::macros::RecordedAction::DeleteForward);
                                s.delete_forward();
                            }
                            "d" if ctrl => {
                                let n = s.take_universal_arg();
                                for _ in 0..n {
                                    s.record_action(crate::macros::RecordedAction::DeleteForward);
                                    s.delete_forward();
                                }
                            }

                            // Enter (newline)
                            "enter" => {
                                s.record_action(crate::macros::RecordedAction::Newline);
                                s.insert_text("\n");
                            }

                            // Tab
                            "tab" if shift => {
                                let pos = s.cursor.position;
                                if s.document.len_chars() > 0 {
                                    let line =
                                        s.document.char_to_line(pos.min(s.document.len_chars()));
                                    let line_start = s.document.line_to_char(line);
                                    let line_text: String = s.document.line(line).into();
                                    let spaces =
                                        line_text.chars().take_while(|c| *c == ' ').count().min(4);
                                    if spaces > 0 {
                                        s.history
                                            .push_undo(s.document.snapshot(), s.cursor.position);
                                        s.document.remove(line_start, line_start + spaces);
                                        s.cursor.position =
                                            s.cursor.position.saturating_sub(spaces);
                                    }
                                }
                            }
                            "tab" => s.insert_text("    "),

                            // Select all
                            "a" if modifier => s.select_all(),

                            // Escape — set meta_pending so next key acts as M-<key>
                            "escape" if !ctrl && !cmd => {
                                // Cancel any pending prefix first
                                s.c_x_pending = false;
                                s.c_x_r_pending = false;
                                s.meta_pending = true;
                            }

                            // Text input
                            _ => {
                                if !modifier
                                    && !ctrl
                                    && !esc_meta
                                    && let Some(ch) = &event.keystroke.key_char
                                {
                                    if s.zap_to_char_pending {
                                        if let Some(c) = ch.chars().next() {
                                            s.zap_to_char(c);
                                        }
                                    } else {
                                        s.kill_ring.reset_flags();
                                        // Record each character for macro replay
                                        for c in ch.chars() {
                                            s.record_action(
                                                crate::macros::RecordedAction::InsertChar(c),
                                            );
                                        }
                                        s.insert_text(ch);
                                    }
                                }
                            }
                        }
                    });
                    if want_quit.get() {
                        // Outside the state.update closure so we can
                        // touch the App context — `cx.quit()` shuts
                        // the editor down.
                        cx.quit();
                    }
                }
                window.refresh();
            })
            // Mouse up: finalize selection
            .on_mouse_up(MouseButton::Left, {
                let state = self.state.clone();
                move |_ev, _window, cx| {
                    state.update(cx, |s, _cx| {
                        if s.cursor.anchor == Some(s.cursor.position) {
                            s.cursor.clear_selection();
                        }
                    });
                }
            })
            .child(div().flex().flex_col().w_full().children(line_elements))
            // Completion popup
            .when(completion_visible, |el| {
                let items: Vec<AnyElement> = completion_items
                    .iter()
                    .map(|(label, selected)| {
                        let bg = if *selected { surface_color } else { bg_color };
                        div()
                            .px_2()
                            .py(px(1.0))
                            .bg(bg)
                            .text_size(px(12.0))
                            .font_family("monospace")
                            .text_color(text_color)
                            .child(label.clone())
                            .into_any_element()
                    })
                    .collect();
                // Position the popup just below the cursor line. The editor
                // pane has `.p_2()` (8px padding) and the gutter offset pushes
                // content right. Y is relative to the scrolled document, so
                // we subtract the current scroll offset.
                let char_width = editor_font_size * 0.6;
                let popup_y = ((cursor_line + 1) as f32 * LINE_HEIGHT_PX) - scroll_top + 8.0;
                let popup_x = 8.0 + gutter_width + (cursor_col as f32 * char_width);
                el.child(
                    div()
                        .absolute()
                        .top(px(popup_y))
                        .left(px(popup_x))
                        .w(px(300.0))
                        .max_h(px(200.0))
                        .overflow_hidden()
                        .bg(bg_color)
                        .border_1()
                        .border_color(border_color)
                        .shadow_md()
                        .children(items),
                )
            })
            // Hover tooltip: above the cursor line so it doesn't overlap typing.
            .when_some(hover_text, |el, text| {
                let first_line = text.lines().next().unwrap_or("").to_string();
                let char_width = editor_font_size * 0.6;
                let hover_y = (cursor_line as f32 * LINE_HEIGHT_PX) - scroll_top - 24.0 + 8.0;
                let hover_x = 8.0 + gutter_width + (cursor_col as f32 * char_width);
                el.child(
                    div()
                        .absolute()
                        .top(px(hover_y.max(0.0)))
                        .left(px(hover_x))
                        .max_w(px(500.0))
                        .px_2()
                        .py_1()
                        .bg(surface_color)
                        .border_1()
                        .border_color(border_color)
                        .shadow_md()
                        .text_size(px(12.0))
                        .font_family("monospace")
                        .text_color(text_color)
                        .child(first_line),
                )
            })
            // Isearch status bar
            .when(isearch_active, |el| {
                el.child(
                    div()
                        .flex()
                        .px_3()
                        .py_1()
                        .bg(surface_color)
                        .border_t_1()
                        .border_color(border_color)
                        .text_size(px(13.0))
                        .font_family("monospace")
                        .text_color(text_color)
                        .child(isearch_text),
                )
            })
            // Universal arg indicator
            .when(show_universal_arg, |el| {
                el.child(
                    div()
                        .flex()
                        .px_3()
                        .py_1()
                        .bg(surface_color)
                        .border_t_1()
                        .border_color(border_color)
                        .text_size(px(13.0))
                        .font_family("monospace")
                        .text_color(text_muted)
                        .child(universal_arg_text),
                )
            })
    }
}

/// Convert a byte index (from TextLayout) to a char index within the line text.
fn byte_index_to_char_index(line_text: &str, byte_index: usize) -> usize {
    line_text[..byte_index.min(line_text.len())].chars().count()
}

/// Convert a char-based column offset to a byte offset within the line text.
fn char_col_to_byte_offset(line_text: &str, char_col: usize) -> usize {
    line_text
        .char_indices()
        .nth(char_col)
        .map(|(byte_pos, _)| byte_pos)
        .unwrap_or(line_text.len())
}

/// Build line text and TextRuns for a single editor line.
///
/// Bakes in syntax highlighting, selection background, cursor block, and find
/// match highlights as TextRun properties. Returns the text string and runs
/// vector ready for StyledText.
fn build_line_text_runs(
    spans: &[crate::markdown::HighlightSpan],
    line_text: &str,
    is_cursor_line: bool,
    cursor_col: usize,
    sel_cols: Option<(usize, usize)>,
    find_query: &str,
    colors: &MdThemeColors,
    diag_cols: &[(usize, usize)],
) -> (String, Vec<TextRun>) {
    let font = Font {
        family: SharedString::from("monospace"),
        features: FontFeatures::default(),
        fallbacks: None,
        weight: FontWeight::NORMAL,
        style: FontStyle::Normal,
    };

    // Build the display text. For cursor at end-of-line, append a space for the
    // cursor block. For empty lines, use a single space for height.
    let line_char_count = line_text.chars().count();
    let needs_cursor_space = is_cursor_line && cursor_col >= line_char_count;
    let text = if line_text.is_empty() {
        " ".to_string()
    } else if needs_cursor_space {
        format!("{} ", line_text)
    } else {
        line_text.to_string()
    };

    // Phase 1: Build base color map (one Hsla per byte) from syntax spans
    let text_bytes = text.len();
    let mut fg_colors: Vec<Hsla> = vec![Hsla::from(colors.text); text_bytes];
    let mut bg_colors: Vec<Option<Hsla>> = vec![None; text_bytes];

    // Map span colors onto byte positions
    let mut byte_offset = 0;
    for span in spans {
        let span_color = Hsla::from(span.color);
        let span_byte_len = span.text.len();
        for c in &mut fg_colors[byte_offset..(byte_offset + span_byte_len).min(text_bytes)] {
            *c = span_color;
        }
        byte_offset += span_byte_len;
    }

    // Phase 2: Overlay selection background
    if let Some((sel_start_col, sel_end_col)) = sel_cols {
        let sel_start_byte = char_col_to_byte_offset(&text, sel_start_col);
        let sel_end_byte = char_col_to_byte_offset(&text, sel_end_col);
        let sel_bg = Hsla::from(colors.selection_bg);
        for c in &mut bg_colors[sel_start_byte..sel_end_byte.min(text_bytes)] {
            *c = Some(sel_bg);
        }
    }

    // Phase 3: Overlay find match highlights
    if !find_query.is_empty() {
        let find_fg = Hsla::from(colors.find_match_text);
        let find_bg = Hsla::from(colors.find_match_bg);
        let mut search_start = 0;
        while let Some(pos) = text[search_start..].find(find_query) {
            let match_start = search_start + pos;
            let match_end = match_start + find_query.len();
            for b in match_start..match_end.min(text_bytes) {
                fg_colors[b] = find_fg;
                bg_colors[b] = Some(find_bg);
            }
            search_start = match_end;
        }
    }

    // Phase 4: Overlay diagnostic underlines
    let mut underlines: Vec<Option<UnderlineStyle>> = vec![None; text_bytes];
    for &(diag_start_col, diag_end_col) in diag_cols {
        let byte_start = char_col_to_byte_offset(&text, diag_start_col);
        let byte_end = char_col_to_byte_offset(&text, diag_end_col);
        let diag_underline = UnderlineStyle {
            thickness: px(1.0),
            color: Some(gpui::red()),
            wavy: true,
        };
        for b in byte_start..byte_end.min(text_bytes) {
            underlines[b] = Some(diag_underline);
        }
    }

    // Phase 5: Overlay cursor block (highest priority)
    if is_cursor_line {
        let cursor_byte_start = char_col_to_byte_offset(&text, cursor_col);
        let cursor_byte_end = char_col_to_byte_offset(&text, cursor_col + 1);
        let cursor_fg = Hsla::from(colors.cursor_text);
        let cursor_bg = Hsla::from(colors.cursor_bg);
        for b in cursor_byte_start..cursor_byte_end.min(text_bytes) {
            fg_colors[b] = cursor_fg;
            bg_colors[b] = Some(cursor_bg);
        }
    }

    // Phase 6: Compress consecutive bytes with identical styling into TextRuns
    let mut runs: Vec<TextRun> = Vec::new();
    if text_bytes > 0 {
        let mut run_start = 0;
        let mut cur_fg = fg_colors[0];
        let mut cur_bg = bg_colors[0];
        let mut cur_ul = underlines[0];

        for b in 1..text_bytes {
            if fg_colors[b] != cur_fg || bg_colors[b] != cur_bg || underlines[b] != cur_ul {
                runs.push(TextRun {
                    len: b - run_start,
                    font: font.clone(),
                    color: cur_fg,
                    background_color: cur_bg,
                    underline: cur_ul,
                    strikethrough: None,
                });
                run_start = b;
                cur_fg = fg_colors[b];
                cur_bg = bg_colors[b];
                cur_ul = underlines[b];
            }
        }
        // Final run
        runs.push(TextRun {
            len: text_bytes - run_start,
            font,
            color: cur_fg,
            background_color: cur_bg,
            underline: cur_ul,
            strikethrough: None,
        });
    }

    (text, runs)
}
