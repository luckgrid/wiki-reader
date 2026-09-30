//! Viewer body: lines, link hits, optional raw gutter + syntect spans.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::highlight::HlSpan;
use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;
use crate::tui::viewer_doc::{FocusItem, FocusTarget};
use wiki_reader_render::{LinkClass, LinkId, LinkSpan, StyleKind, StyledLine};

/// Draw lines into the viewer and register link + line hits.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    lines: &[String],
    styled: Option<&[StyledLine]>,
    link_spans: &[LinkSpan],
    highlights: Option<&[Vec<HlSpan>]>,
    show_gutter: bool,
    scroll: u32,
    cursor_line: u32,
    match_highlight: Option<u32>,
    focused: bool,
    focused_item: Option<&FocusItem>,
    theme: &Theme,
    hits: &mut HitMap,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    hits.push(area, Hit::FocusViewer);

    let inner = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(focused))
        .title("Viewer");
    let inner_area = inner.inner(area);
    frame.render_widget(inner, area);

    let gutter_w: u16 = if show_gutter { 6 } else { 0 };
    let text_width = usize::from(inner_area.width.saturating_sub(gutter_w)).min(100);
    let visible_h = usize::from(inner_area.height);
    let scroll = usize::try_from(scroll).unwrap_or(0);
    let mut out_lines = Vec::with_capacity(visible_h);
    let focus_style = Style::default().bg(theme.focus_item).fg(theme.text);
    let focused_link = focused_item.and_then(|it| it.link_id);
    let gutter_style = theme.muted();
    let match_style = Style::default().bg(theme.focus_item).fg(theme.text);

    for row in 0..visible_h {
        let src_idx = scroll + row;
        let line_no = u32::try_from(src_idx).unwrap_or(u32::MAX);
        let y = inner_area
            .y
            .saturating_add(u16::try_from(row).unwrap_or(u16::MAX));

        let base = if match_highlight == Some(line_no) {
            match_style
        } else if line_no == cursor_line {
            theme.text().bg(theme.cursor_line)
        } else {
            theme.text()
        };

        hits.push(
            Rect {
                x: inner_area.x,
                y,
                width: inner_area.width,
                height: 1,
            },
            Hit::ViewerLine(line_no),
        );

        for span in link_spans {
            for &(seg_line, (c0, c1)) in &span.segments {
                if seg_line == line_no && c0 < c1 {
                    let x0 = gutter_w.saturating_add(c0);
                    let x1 = gutter_w.saturating_add(c1).min(inner_area.width);
                    if x0 >= inner_area.width || x1 <= x0 {
                        continue;
                    }
                    hits.push(
                        Rect {
                            x: inner_area.x.saturating_add(x0),
                            y,
                            width: x1.saturating_sub(x0),
                            height: 1,
                        },
                        Hit::Link(span.id.0),
                    );
                }
            }
        }

        let focused_block_cols = focused_item.and_then(|it| {
            if it.kind == FocusTarget::BlockAction && it.line == Some(line_no) {
                Some(it.cols)
            } else {
                None
            }
        });

        let mut spans: Vec<Span<'static>> = Vec::new();
        if show_gutter {
            let n = src_idx.saturating_add(1);
            spans.push(Span::styled(format!("{n:4}│ "), gutter_style));
        }

        if let Some(hl_lines) = highlights
            && let Some(hl) = hl_lines.get(src_idx)
        {
            let mut col = 0usize;
            for run in hl {
                if col >= text_width {
                    break;
                }
                let remain = text_width.saturating_sub(col);
                let clipped: String = run.text.chars().take(remain).collect();
                if clipped.is_empty() && run.text.is_empty() {
                    continue;
                }
                let mut st = run.style;
                if line_no == cursor_line {
                    st = st.bg(theme.cursor_line);
                }
                col += clipped.chars().count();
                spans.push(Span::styled(clipped, st));
            }
            if spans.len() == usize::from(show_gutter) {
                spans.push(Span::styled(String::new(), base));
            }
        } else if let Some(styled_lines) = styled
            && let Some(sl) = styled_lines.get(src_idx)
        {
            spans.extend(paint_styled_line(
                sl,
                line_no,
                text_width,
                link_spans,
                focused_link,
                focused_block_cols,
                base,
                focus_style,
                theme,
                line_no == cursor_line,
            ));
        } else {
            let raw = lines.get(src_idx).map_or("", String::as_str);
            let display: String = raw.chars().take(text_width).collect();
            spans.extend(styled_line(
                &display,
                line_no,
                link_spans,
                focused_link,
                focused_block_cols,
                base,
                focus_style,
                theme,
            ));
        }
        if line_no == cursor_line {
            // Paragraph only styles the glyphs; pad so the highlight spans the row.
            let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
            let pad = usize::from(inner_area.width).saturating_sub(used);
            if pad > 0 {
                spans.push(Span::styled(
                    " ".repeat(pad),
                    Style::default().bg(theme.cursor_line),
                ));
            }
        }
        out_lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(out_lines), inner_area);
}

#[allow(clippy::too_many_arguments)]
fn paint_styled_line(
    sl: &StyledLine,
    line_no: u32,
    text_width: usize,
    link_spans: &[LinkSpan],
    focused_link: Option<LinkId>,
    focused_block_cols: Option<(u16, u16)>,
    base: Style,
    focus_style: Style,
    theme: &Theme,
    on_cursor: bool,
) -> Vec<Span<'static>> {
    let mut ranges: Vec<(u16, u16, Option<LinkClass>, bool)> = Vec::new();
    for span in link_spans {
        for &(seg_line, cols) in &span.segments {
            if seg_line == line_no && cols.0 < cols.1 {
                let is_focus = focused_link == Some(span.id);
                ranges.push((cols.0, cols.1, Some(span.class), is_focus));
            }
        }
    }
    if let Some((c0, c1)) = focused_block_cols
        && c0 < c1
    {
        ranges.push((c0, c1, None, true));
    }
    ranges.sort_by_key(|r| r.0);

    let mut spans = Vec::new();
    let mut col = 0u16;
    for run in &sl.spans {
        if usize::from(col) >= text_width {
            break;
        }
        for ch in run.text.chars() {
            if usize::from(col) >= text_width {
                break;
            }
            let w = u16::try_from(Span::raw(ch.to_string()).width())
                .unwrap_or(1)
                .max(1);
            let mut st = theme.style_kind(run.kind);
            if on_cursor {
                st = st.bg(theme.cursor_line);
            } else if matches!(
                run.kind,
                StyleKind::CodeBlock | StyleKind::CodeLang | StyleKind::InlineCode
            ) {
                // keep code bg
            } else {
                st = st.patch(base);
            }
            for &(c0, c1, class, is_focus) in &ranges {
                if col >= c0 && col < c1 {
                    st = if is_focus {
                        focus_style
                    } else if let Some(class) = class {
                        theme.link_class(class)
                    } else {
                        st
                    };
                    if on_cursor {
                        st = st.bg(theme.cursor_line);
                    }
                    break;
                }
            }
            spans.push(Span::styled(ch.to_string(), st));
            col = col.saturating_add(w);
        }
    }
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base));
    }
    spans
}

#[allow(clippy::too_many_arguments)] // link + block focus overlays on one pass
fn styled_line(
    display: &str,
    line_no: u32,
    link_spans: &[LinkSpan],
    focused_link: Option<LinkId>,
    focused_block_cols: Option<(u16, u16)>,
    base: Style,
    focus_style: Style,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let mut ranges: Vec<(u16, u16, Option<LinkClass>, bool)> = Vec::new();
    for span in link_spans {
        for &(seg_line, cols) in &span.segments {
            if seg_line == line_no && cols.0 < cols.1 {
                let is_focus = focused_link == Some(span.id);
                ranges.push((cols.0, cols.1, Some(span.class), is_focus));
            }
        }
    }
    if let Some((c0, c1)) = focused_block_cols
        && c0 < c1
    {
        ranges.push((c0, c1, None, true));
    }
    ranges.sort_by_key(|r| r.0);

    if ranges.is_empty() {
        return vec![Span::styled(display.to_owned(), base)];
    }

    let mut spans = Vec::new();
    let mut col = 0u16;
    let mut buf = String::new();
    let mut style = base;

    let flush = |buf: &mut String, style: Style, spans: &mut Vec<Span<'static>>| {
        if !buf.is_empty() {
            spans.push(Span::styled(std::mem::take(buf), style));
        }
    };

    for ch in display.chars() {
        let w = u16::try_from(Span::raw(ch.to_string()).width())
            .unwrap_or(1)
            .max(1);
        let mut next_style = base;
        for &(c0, c1, class, is_focus) in &ranges {
            if col >= c0 && col < c1 {
                next_style = if is_focus {
                    focus_style
                } else if let Some(class) = class {
                    theme.link_class(class)
                } else {
                    base
                };
                break;
            }
        }
        if next_style != style {
            flush(&mut buf, style, &mut spans);
            style = next_style;
        }
        buf.push(ch);
        col = col.saturating_add(w);
    }
    flush(&mut buf, style, &mut spans);
    spans
}
