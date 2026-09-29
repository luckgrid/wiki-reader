//! Viewer body: interim raw source lines with link hits and K3 focus highlight.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;
use crate::tui::viewer_doc::FocusItem;
use wiki_reader_render::{LinkClass, LinkId, LinkSpan};

/// Draw raw lines into the viewer and register link + line hits.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    lines: &[String],
    link_spans: &[LinkSpan],
    scroll: u32,
    cursor_line: u32,
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

    let text_width = usize::from(inner_area.width).min(100);
    let visible_h = usize::from(inner_area.height);
    let scroll = usize::try_from(scroll).unwrap_or(0);
    let mut out_lines = Vec::with_capacity(visible_h);
    let focus_style = Style::default().bg(theme.focus_item).fg(theme.text);
    let focused_link = focused_item.and_then(|it| it.link_id);

    for row in 0..visible_h {
        let src_idx = scroll + row;
        let line_no = u32::try_from(src_idx).unwrap_or(u32::MAX);
        let y = inner_area
            .y
            .saturating_add(u16::try_from(row).unwrap_or(u16::MAX));

        let raw = lines.get(src_idx).map_or("", String::as_str);
        let display: String = raw.chars().take(text_width).collect();
        let base = if line_no == cursor_line {
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
                    let x = inner_area.x.saturating_add(c0);
                    let w = c1.saturating_sub(c0);
                    hits.push(
                        Rect {
                            x,
                            y,
                            width: w,
                            height: 1,
                        },
                        Hit::Link(span.id.0),
                    );
                }
            }
        }

        let spans = styled_line(
            &display,
            line_no,
            link_spans,
            focused_link,
            base,
            focus_style,
            theme,
        );
        out_lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(out_lines), inner_area);
}

fn styled_line(
    display: &str,
    line_no: u32,
    link_spans: &[LinkSpan],
    focused_link: Option<LinkId>,
    base: Style,
    focus_style: Style,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let mut ranges: Vec<(u16, u16, LinkClass, bool)> = Vec::new();
    for span in link_spans {
        for &(seg_line, cols) in &span.segments {
            if seg_line == line_no && cols.0 < cols.1 {
                let is_focus = focused_link == Some(span.id);
                ranges.push((cols.0, cols.1, span.class, is_focus));
            }
        }
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
                } else {
                    theme.link_class(class)
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

#[allow(dead_code)]
fn highlight_cols(
    display: &str,
    cols: (u16, u16),
    base: Style,
    focus: Style,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut col = 0u16;
    let mut buf = String::new();
    let mut mode_focus = false;

    let flush = |buf: &mut String,
                 focus_mode: bool,
                 spans: &mut Vec<Span<'static>>,
                 base: Style,
                 focus: Style| {
        if !buf.is_empty() {
            spans.push(Span::styled(
                std::mem::take(buf),
                if focus_mode { focus } else { base },
            ));
        }
    };

    for ch in display.chars() {
        let w = u16::try_from(Span::raw(ch.to_string()).width())
            .unwrap_or(1)
            .max(1);
        let in_focus = col >= cols.0 && col < cols.1;
        if in_focus != mode_focus {
            flush(&mut buf, mode_focus, &mut spans, base, focus);
            mode_focus = in_focus;
        }
        buf.push(ch);
        col = col.saturating_add(w);
    }
    flush(&mut buf, mode_focus, &mut spans, base, focus);
    spans
}
