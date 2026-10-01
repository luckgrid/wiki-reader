//! Shared centered overlay panel + scroll helpers (P2-20 / P2-21).

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};

use super::status::clip_spans;
use crate::tui::theme::Theme;

/// Columns of blank padding inside a popup, left and right.
pub const POPUP_PAD: u16 = 2;

/// Peach-bordered popup frame with `title` on the top border.
#[must_use]
pub fn popup_block(title: Line<'static>, theme: &Theme) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.peach))
        .title(title)
}

/// Peach bold style for popup titles and accents.
#[must_use]
pub fn popup_accent(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.peach)
        .add_modifier(Modifier::BOLD)
}

/// One popup row of `inner_w` columns: padding, `spans` (clipped), then fill.
/// With `bg` the highlight spans the whole row, padding included.
#[must_use]
pub fn popup_row(spans: Vec<Span<'static>>, inner_w: u16, bg: Option<Color>) -> Line<'static> {
    let paint = |style: Style| bg.map_or(style, |c| style.bg(c));
    let content_max = usize::from(inner_w.saturating_sub(POPUP_PAD * 2));
    let total: usize = spans
        .iter()
        .map(|s| usize::from(crate::tui::text_col::line_width(&s.content)))
        .sum();
    // A cut-off row ends in `…` instead of stopping dead at the border.
    let clipped = if total > content_max && content_max > 1 {
        let tail_style = spans.last().map_or_else(Style::default, |s| s.style);
        let mut cut = clip_spans(spans, content_max - 1);
        cut.push(Span::styled("…", tail_style));
        cut
    } else {
        clip_spans(spans, content_max)
    };
    let used: usize = clipped
        .iter()
        .map(|s| usize::from(crate::tui::text_col::line_width(&s.content)))
        .sum();
    let left_pad = usize::from(POPUP_PAD.min(inner_w));
    let pad = " ".repeat(left_pad);
    let fill = usize::from(inner_w).saturating_sub(left_pad + used);
    let mut out = vec![Span::styled(pad, paint(Style::default()))];
    out.extend(
        clipped
            .into_iter()
            .map(|sp| Span::styled(sp.content, paint(sp.style))),
    );
    out.push(Span::styled(" ".repeat(fill), paint(Style::default())));
    Line::from(out)
}

/// Centered panel rect, clamped to `[min_w, max_w]` × `[min_h, max_h]` and the area.
#[must_use]
pub fn centered_panel(area: Rect, max_w: u16, max_h: u16, min_w: u16, min_h: u16) -> Rect {
    let width = area.width.clamp(min_w, max_w).min(area.width);
    let height = area.height.clamp(min_h, max_h).min(area.height);
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Keep `selected` inside the visible window `[scroll, scroll + visible)`.
#[must_use]
pub fn ensure_visible(selected: usize, scroll: usize, visible: usize) -> usize {
    if visible == 0 {
        return 0;
    }
    if selected < scroll {
        selected
    } else if selected >= scroll.saturating_add(visible) {
        selected.saturating_add(1).saturating_sub(visible)
    } else {
        scroll
    }
}

/// Clamp scroll so it never past the end of a `len`-item list.
#[must_use]
pub fn clamp_scroll(scroll: usize, visible: usize, len: usize) -> usize {
    let max = len.saturating_sub(visible.max(1));
    scroll.min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_rows_never_exceed_tiny_widths() {
        for width in 0..=5 {
            let row = popup_row(vec![Span::raw("漢abcdef")], width, Some(Color::Blue));
            assert_eq!(row.width(), usize::from(width));
            assert!(
                row.spans
                    .iter()
                    .all(|span| span.style.bg == Some(Color::Blue))
            );
        }
    }
}
