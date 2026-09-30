//! Viewer body: lines, link hits, optional raw gutter + syntect spans.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::highlight::HlSpan;
use crate::tui::hit::{Hit, HitMap};
use crate::tui::layout::VIEWER_LEFT_PAD;
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
    prev_label: Option<&str>,
    next_label: Option<&str>,
    footer_focus: Option<FocusTarget>,
    theme: &Theme,
    hits: &mut HitMap,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    hits.push(area, Hit::FocusViewer);

    let footer = crate::tui::regions::footer::titles(
        area,
        prev_label,
        next_label,
        footer_focus,
        theme,
        hits,
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(focused))
        .title("Viewer")
        .title_bottom(footer.left)
        .title_bottom(footer.right);
    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Left pad column holds the ▌ cursor marker (P2-19); text starts one col in.
    let content = Rect {
        x: inner_area.x.saturating_add(VIEWER_LEFT_PAD),
        y: inner_area.y,
        width: inner_area.width.saturating_sub(VIEWER_LEFT_PAD),
        height: inner_area.height,
    };

    let gutter_w: u16 = if show_gutter { 6 } else { 0 };
    let text_width = usize::from(content.width.saturating_sub(gutter_w)).min(100);
    let visible_h = usize::from(content.height);
    let scroll = usize::try_from(scroll).unwrap_or(0);
    let mut out_lines = Vec::with_capacity(visible_h);
    let focus_style = Style::default().bg(theme.focus_item).fg(theme.text);
    let focused_link = focused_item.and_then(|it| it.link_id);
    let gutter_style = theme.muted();
    let match_style = Style::default().bg(theme.focus_item).fg(theme.text);
    let cursor_bg = Style::default().bg(theme.cursor_line).fg(theme.text);

    for row in 0..visible_h {
        let src_idx = scroll + row;
        let line_no = u32::try_from(src_idx).unwrap_or(u32::MAX);
        let y = content
            .y
            .saturating_add(u16::try_from(row).unwrap_or(u16::MAX));

        let on_cursor = line_no == cursor_line;
        let on_match = match_highlight == Some(line_no);
        let base = if on_match {
            match_style
        } else if on_cursor {
            cursor_bg
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
                    let x1 = gutter_w.saturating_add(c1).min(content.width);
                    if x0 >= content.width || x1 <= x0 {
                        continue;
                    }
                    hits.push(
                        Rect {
                            x: content.x.saturating_add(x0),
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

        // Marker column: ▌ on cursor (not on match-only highlight).
        let mut spans: Vec<Span<'static>> = Vec::new();
        if on_cursor && !on_match {
            spans.push(Span::styled("▌", cursor_bg));
        } else if on_match {
            spans.push(Span::styled(" ", match_style));
        } else {
            spans.push(Span::raw(" "));
        }

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
                if on_cursor && !on_match {
                    st = st.bg(theme.cursor_line);
                } else if on_match {
                    st = st.bg(theme.focus_item);
                }
                col += clipped.chars().count();
                spans.push(Span::styled(clipped, st));
            }
            if spans.len() == 1 + usize::from(show_gutter) {
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
                on_cursor && !on_match,
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
        if on_cursor || on_match {
            // Pad highlight across the full inner width (marker + content).
            let used: usize = spans
                .iter()
                .map(|s| Span::raw(s.content.as_ref()).width())
                .sum();
            let pad = usize::from(inner_area.width).saturating_sub(used);
            if pad > 0 {
                let pad_style = if on_match {
                    match_style
                } else {
                    Style::default().bg(theme.cursor_line)
                };
                spans.push(Span::styled(" ".repeat(pad), pad_style));
            }
        }
        out_lines.push(Line::from(spans));
    }

    // Paint into full inner (marker column + content).
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

    let style_at = |col: u16, kind: StyleKind| -> Style {
        let mut st = theme.style_kind(kind);
        if on_cursor {
            st = st.bg(theme.cursor_line);
        } else if !matches!(
            kind,
            StyleKind::CodeBlock | StyleKind::CodeLang | StyleKind::InlineCode | StyleKind::Quote
        ) {
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
        st
    };

    let mut spans = Vec::new();
    let mut col = 0u16;
    let mut buf = String::new();
    let mut style = base;
    let flush = |buf: &mut String, style: Style, spans: &mut Vec<Span<'static>>| {
        if !buf.is_empty() {
            spans.push(Span::styled(std::mem::take(buf), style));
        }
    };

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
            let st = style_at(col, run.kind);
            if buf.is_empty() {
                style = st;
            } else if st != style {
                flush(&mut buf, style, &mut spans);
                style = st;
            }
            buf.push(ch);
            col = col.saturating_add(w);
        }
    }
    flush(&mut buf, style, &mut spans);

    // Full-row code/quote shading (glyphs alone leave a ragged right edge).
    if !on_cursor {
        let shade = sl.spans.iter().find_map(|s| match s.kind {
            StyleKind::CodeBlock | StyleKind::CodeLang | StyleKind::InlineCode => {
                Some(theme.code_bg)
            }
            StyleKind::Quote => Some(theme.quote_bar),
            _ => None,
        });
        if let Some(bg) = shade {
            let pad = text_width.saturating_sub(usize::from(col));
            if pad > 0 {
                spans.push(Span::styled(" ".repeat(pad), Style::default().bg(bg)));
            }
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
