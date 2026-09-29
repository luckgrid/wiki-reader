//! Viewer body: interim raw source lines (cursor lands in P1-08b).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;

/// Draw raw lines into the viewer and register per-line hits.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    lines: &[String],
    scroll: u32,
    cursor_line: u32,
    focused: bool,
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
        let style = if line_no == cursor_line {
            theme.text().bg(theme.cursor_line)
        } else {
            theme.text()
        };
        out_lines.push(Line::from(Span::styled(display, style)));
    }

    frame.render_widget(Paragraph::new(out_lines), inner_area);
}
