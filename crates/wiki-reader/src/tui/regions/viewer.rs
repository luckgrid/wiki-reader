//! Viewer body: interim raw source lines with K3 focus highlight.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;
use crate::tui::viewer_doc::{FocusItem, FocusTarget};

/// Draw raw lines into the viewer and register per-line hits.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    lines: &[String],
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

    for row in 0..visible_h {
        let src_idx = scroll + row;
        let line_no = u32::try_from(src_idx).unwrap_or(u32::MAX);
        let y = inner_area
            .y
            .saturating_add(u16::try_from(row).unwrap_or(u16::MAX));
        hits.push(
            Rect {
                x: inner_area.x,
                y,
                width: inner_area.width,
                height: 1,
            },
            Hit::ViewerLine(line_no),
        );

        let raw = lines.get(src_idx).map_or("", String::as_str);
        let display: String = raw.chars().take(text_width).collect();
        let base = if line_no == cursor_line {
            theme.text().bg(theme.cursor_line)
        } else {
            theme.text()
        };

        let spans = match focused_item {
            Some(it)
                if it.kind == FocusTarget::Link
                    && it.doc_line() == Some(line_no)
                    && it.cols.0 < it.cols.1 =>
            {
                highlight_cols(&display, it.cols, base, focus_style)
            }
            _ => vec![Span::styled(display, base)],
        };
        out_lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(out_lines), inner_area);
}

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
        if buf.is_empty() {
            return;
        }
        let style = if focus_mode { focus } else { base };
        spans.push(Span::styled(std::mem::take(buf), style));
    };

    for ch in display.chars() {
        let at_focus = col >= cols.0 && col < cols.1;
        if at_focus != mode_focus && !buf.is_empty() {
            flush(&mut buf, mode_focus, &mut spans, base, focus);
            mode_focus = at_focus;
        } else if buf.is_empty() {
            mode_focus = at_focus;
        }
        buf.push(ch);
        let w = u16::try_from(Span::raw(ch.to_string()).width()).unwrap_or(1);
        col = col.saturating_add(w.max(1));
    }
    flush(&mut buf, mode_focus, &mut spans, base, focus);
    if spans.is_empty() {
        spans.push(Span::styled(display.to_owned(), base));
    }
    spans
}
