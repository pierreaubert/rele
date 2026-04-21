//! Markdown syntax highlighting for the source editor.
//!
//! Tokenizes a line of markdown into colored spans for rendering.
//! Uses theme-aware colors for proper light/dark mode support.

use gpui::Rgba;
use rele_server::syntax::HighlightRange;

use super::theme_colors::MdThemeColors;

/// A colored text span within a single line.
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub text: String,
    pub color: Rgba,
}

/// Convert sparse tree-sitter `HighlightRange`s for a single line into the
/// contiguous `Vec<HighlightSpan>` format the editor renderer expects.
///
/// Gaps between ranges get filled with the default text color.
/// Overlapping ranges (rare; depends on the grammar) collapse by
/// preferring the range that ends later, then alphabetic order as
/// tiebreaker — but the renderer only needs *a* legal covering, so any
/// consistent choice works.
pub fn ts_ranges_to_spans(
    line_text: &str,
    ranges: &[HighlightRange],
    colors: &MdThemeColors,
) -> Vec<HighlightSpan> {
    if line_text.is_empty() {
        return Vec::new();
    }

    // Precompute char -> byte offset so we can slice by char column.
    let char_byte_offsets: Vec<usize> = line_text
        .char_indices()
        .map(|(b, _)| b)
        .chain(std::iter::once(line_text.len()))
        .collect();
    let char_count = char_byte_offsets.len().saturating_sub(1);

    // Walk ranges in order, emitting plain spans for gaps and coloured
    // spans for covered regions.
    let default = colors.text;
    let mut out: Vec<HighlightSpan> = Vec::new();
    let mut cursor: usize = 0;
    for r in ranges {
        let start = r.start_col.min(char_count);
        let end = r.end_col.min(char_count);
        if end <= cursor {
            // Overlap with an earlier range — skip, the earlier span
            // already covered this text.
            continue;
        }
        let start = start.max(cursor);
        if start > cursor {
            // Emit plain gap.
            let b_from = char_byte_offsets[cursor];
            let b_to = char_byte_offsets[start];
            out.push(HighlightSpan {
                text: line_text[b_from..b_to].to_string(),
                color: default,
            });
        }
        if end > start {
            let b_from = char_byte_offsets[start];
            let b_to = char_byte_offsets[end];
            out.push(HighlightSpan {
                text: line_text[b_from..b_to].to_string(),
                color: colors.color_for_highlight(r.kind),
            });
        }
        cursor = end;
    }
    // Trailing plain tail.
    if cursor < char_count {
        let b_from = char_byte_offsets[cursor];
        out.push(HighlightSpan {
            text: line_text[b_from..].to_string(),
            color: default,
        });
    }
    if out.is_empty() {
        // No ranges at all — one big plain span.
        out.push(HighlightSpan {
            text: line_text.to_string(),
            color: default,
        });
    }
    out
}

#[cfg(test)]
mod ts_spans_tests {
    use super::*;
    use rele_server::syntax::Highlight;

    fn colors() -> MdThemeColors {
        // Use a dummy; fields aren't compared, only used to produce Rgba.
        MdThemeColors {
            text: Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            text_muted: Rgba {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 1.0,
            },
            code_block_bg: Rgba {
                r: 0.9,
                g: 0.9,
                b: 0.9,
                a: 1.0,
            },
            table_bg: Rgba {
                r: 0.9,
                g: 0.9,
                b: 0.9,
                a: 1.0,
            },
            table_header_bg: Rgba {
                r: 0.85,
                g: 0.85,
                b: 0.85,
                a: 1.0,
            },
            border: Rgba {
                r: 0.8,
                g: 0.8,
                b: 0.8,
                a: 1.0,
            },
            hr: Rgba {
                r: 0.8,
                g: 0.8,
                b: 0.8,
                a: 1.0,
            },
            syn_heading: Rgba {
                r: 0.0,
                g: 0.3,
                b: 0.6,
                a: 1.0,
            },
            syn_bold: Rgba {
                r: 0.6,
                g: 0.2,
                b: 0.0,
                a: 1.0,
            },
            syn_italic: Rgba {
                r: 0.4,
                g: 0.2,
                b: 0.7,
                a: 1.0,
            },
            syn_strikethrough: Rgba {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 1.0,
            },
            syn_code: Rgba {
                r: 0.6,
                g: 0.3,
                b: 0.1,
                a: 1.0,
            },
            syn_link: Rgba {
                r: 0.0,
                g: 0.4,
                b: 0.2,
                a: 1.0,
            },
            syn_list_marker: Rgba {
                r: 0.4,
                g: 0.4,
                b: 0.4,
                a: 1.0,
            },
            syn_blockquote: Rgba {
                r: 0.4,
                g: 0.4,
                b: 0.4,
                a: 1.0,
            },
            syn_fence: Rgba {
                r: 0.6,
                g: 0.3,
                b: 0.1,
                a: 1.0,
            },
            syn_checkbox: Rgba {
                r: 0.0,
                g: 0.4,
                b: 0.2,
                a: 1.0,
            },
            cursor_bg: Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            cursor_text: Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            selection_bg: Rgba {
                r: 0.7,
                g: 0.8,
                b: 1.0,
                a: 1.0,
            },
            find_match_bg: Rgba {
                r: 1.0,
                g: 0.9,
                b: 0.3,
                a: 1.0,
            },
            find_match_text: Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        }
    }

    #[test]
    fn empty_line_produces_no_spans() {
        let spans = ts_ranges_to_spans("", &[], &colors());
        assert!(spans.is_empty());
    }

    #[test]
    fn no_ranges_produces_one_plain_span() {
        let spans = ts_ranges_to_spans("hello", &[], &colors());
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "hello");
    }

    #[test]
    fn range_with_gaps_fills_with_plain() {
        // Input: "let x = 1;" with a Keyword on [0..3) ("let")
        let ranges = [HighlightRange {
            start_col: 0,
            end_col: 3,
            kind: Highlight::Keyword,
        }];
        let spans = ts_ranges_to_spans("let x = 1;", &ranges, &colors());
        // Expect two spans: "let" (keyword), " x = 1;" (plain).
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text, "let");
        assert_eq!(spans[1].text, " x = 1;");
    }

    #[test]
    fn spans_cover_line_contiguously() {
        let ranges = [
            HighlightRange {
                start_col: 0,
                end_col: 2,
                kind: Highlight::Keyword,
            },
            HighlightRange {
                start_col: 4,
                end_col: 7,
                kind: Highlight::Type,
            },
        ];
        let text = "fn  Foo(x)";
        let spans = ts_ranges_to_spans(text, &ranges, &colors());
        let joined: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined, text, "spans must cover the full line exactly");
    }
}

/// Highlight a single line of markdown source.
///
/// `in_code_block` indicates whether we're inside a fenced code block.
/// Returns the highlighted spans and updated code block state.
pub fn highlight_line(
    line: &str,
    in_code_block: bool,
    colors: &MdThemeColors,
) -> (Vec<HighlightSpan>, bool) {
    let trimmed = line.trim_start();

    // Code fence toggle (``` or ~~~)
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        return (
            vec![HighlightSpan {
                text: line.to_string(),
                color: colors.syn_fence,
            }],
            !in_code_block,
        );
    }

    // Inside code block
    if in_code_block {
        return (
            vec![HighlightSpan {
                text: line.to_string(),
                color: colors.syn_code,
            }],
            true,
        );
    }

    // Heading
    if trimmed.starts_with('#') {
        let hash_count = trimmed.chars().take_while(|c| *c == '#').count();
        if hash_count <= 6 && trimmed.chars().nth(hash_count) == Some(' ') {
            return (
                vec![HighlightSpan {
                    text: line.to_string(),
                    color: colors.syn_heading,
                }],
                false,
            );
        }
    }

    // Horizontal rule (CommonMark: 3+ of same char from {-, *, _}, optional spaces)
    if is_horizontal_rule(trimmed) {
        return (
            vec![HighlightSpan {
                text: line.to_string(),
                color: colors.text_muted,
            }],
            false,
        );
    }

    // Block quote
    if trimmed.starts_with('>') {
        return (
            vec![HighlightSpan {
                text: line.to_string(),
                color: colors.syn_blockquote,
            }],
            false,
        );
    }

    // Inline highlighting
    let spans = highlight_inline(line, colors);
    (spans, false)
}

fn highlight_inline(line: &str, colors: &MdThemeColors) -> Vec<HighlightSpan> {
    let mut spans = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut current_text = String::new();

    let trimmed = line.trim_start();
    let indent_len = line.chars().count() - trimmed.chars().count();

    // Task list
    if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [ ] ") {
        let marker_end = indent_len + 6;
        let marker: String = chars[..marker_end.min(len)].iter().collect();
        spans.push(HighlightSpan {
            text: marker,
            color: colors.syn_checkbox,
        });
        i = marker_end.min(len);
    } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        let marker_end = indent_len + 2;
        let marker: String = chars[..marker_end.min(len)].iter().collect();
        spans.push(HighlightSpan {
            text: marker,
            color: colors.syn_list_marker,
        });
        i = marker_end.min(len);
    } else if let Some(num_end) = detect_ordered_list(trimmed) {
        let marker_end = indent_len + num_end;
        let marker: String = chars[..marker_end.min(len)].iter().collect();
        spans.push(HighlightSpan {
            text: marker,
            color: colors.syn_list_marker,
        });
        i = marker_end.min(len);
    }

    while i < len {
        let ch = chars[i];

        // Inline code
        if ch == '`' {
            flush_text(&mut current_text, colors.text, &mut spans);
            let end = find_closing(&chars, i + 1, '`');
            if let Some(end) = end {
                let code: String = chars[i..=end].iter().collect();
                spans.push(HighlightSpan {
                    text: code,
                    color: colors.syn_code,
                });
                i = end + 1;
                continue;
            }
        }

        // Bold **text** or __text__
        if i + 1 < len && ((ch == '*' && chars[i + 1] == '*') || (ch == '_' && chars[i + 1] == '_'))
        {
            let marker = ch;
            if let Some(end) = find_double_closing(&chars, i + 2, marker) {
                flush_text(&mut current_text, colors.text, &mut spans);
                let bold: String = chars[i..=end + 1].iter().collect();
                spans.push(HighlightSpan {
                    text: bold,
                    color: colors.syn_bold,
                });
                i = end + 2;
                continue;
            }
        }

        // Italic *text* or _text_
        if (ch == '*' || ch == '_')
            && i + 1 < len
            && chars[i + 1] != ch
            && let Some(end) = find_closing(&chars, i + 1, ch)
        {
            flush_text(&mut current_text, colors.text, &mut spans);
            let italic: String = chars[i..=end].iter().collect();
            spans.push(HighlightSpan {
                text: italic,
                color: colors.syn_italic,
            });
            i = end + 1;
            continue;
        }

        // Strikethrough ~~text~~
        if i + 1 < len
            && ch == '~'
            && chars[i + 1] == '~'
            && let Some(end) = find_double_closing(&chars, i + 2, '~')
        {
            flush_text(&mut current_text, colors.text, &mut spans);
            let strike: String = chars[i..=end + 1].iter().collect();
            spans.push(HighlightSpan {
                text: strike,
                color: colors.syn_strikethrough,
            });
            i = end + 2;
            continue;
        }

        // Link [text](url)
        if ch == '['
            && let Some(link_end) = find_link_end(&chars, i)
        {
            flush_text(&mut current_text, colors.text, &mut spans);
            let link: String = chars[i..=link_end].iter().collect();
            spans.push(HighlightSpan {
                text: link,
                color: colors.syn_link,
            });
            i = link_end + 1;
            continue;
        }

        current_text.push(ch);
        i += 1;
    }

    flush_text(&mut current_text, colors.text, &mut spans);
    spans
}

fn flush_text(current: &mut String, color: Rgba, spans: &mut Vec<HighlightSpan>) {
    if !current.is_empty() {
        spans.push(HighlightSpan {
            text: std::mem::take(current),
            color,
        });
    }
}

fn find_closing(chars: &[char], start: usize, marker: char) -> Option<usize> {
    chars[start..]
        .iter()
        .position(|&c| c == marker)
        .map(|i| start + i)
}

fn find_double_closing(chars: &[char], start: usize, marker: char) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == marker && chars[i + 1] == marker {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_link_end(chars: &[char], start: usize) -> Option<usize> {
    let bracket_close = find_closing(chars, start + 1, ']')?;
    if bracket_close + 1 < chars.len() && chars[bracket_close + 1] == '(' {
        let paren_close = find_closing(chars, bracket_close + 2, ')')?;
        return Some(paren_close);
    }
    None
}

/// CommonMark thematic break: a line containing 3 or more of the same
/// character from {`-`, `*`, `_`}, optionally interspersed with spaces/tabs,
/// and nothing else.
fn is_horizontal_rule(trimmed: &str) -> bool {
    if trimmed.is_empty() {
        return false;
    }
    // Determine the rule character (first non-space char)
    let rule_char = match trimmed.chars().find(|c| !c.is_whitespace()) {
        Some(c @ ('-' | '*' | '_')) => c,
        _ => return false,
    };
    let count = trimmed.chars().filter(|&c| c == rule_char).count();
    let all_valid = trimmed
        .chars()
        .all(|c| c == rule_char || c == ' ' || c == '\t');
    count >= 3 && all_valid
}

fn detect_ordered_list(trimmed: &str) -> Option<usize> {
    let mut i = 0;
    let chars: Vec<char> = trimmed.chars().collect();
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0
        && i < chars.len()
        && (chars[i] == '.' || chars[i] == ')')
        && i + 1 < chars.len()
        && chars[i + 1] == ' '
    {
        return Some(i + 2);
    }
    None
}
