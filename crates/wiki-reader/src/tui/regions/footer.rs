//! Viewer footer (F1): ‹ prev / next ›.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;

/// Draw prev/next labels; dim and non-hit when absent.
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    prev_label: Option<&str>,
    next_label: Option<&str>,
    theme: &Theme,
    hits: &mut HitMap,
) {
    if area.width == 0 {
        return;
    }

    let left = match prev_label {
        Some(l) => {
            let text = format!("‹ {l}");
            let w = u16::try_from(text.chars().count()).unwrap_or(u16::MAX);
            hits.push(
                Rect {
                    x: area.x,
                    y: area.y,
                    width: w.min(area.width / 2).max(1),
                    height: 1,
                },
                Hit::Prev,
            );
            Span::styled(text, theme.accent())
        }
        None => Span::styled("‹ —", theme.muted()),
    };

    let right = match next_label {
        Some(l) => {
            let text = format!("{l} ›");
            let w = u16::try_from(text.chars().count()).unwrap_or(u16::MAX);
            let x = area.x.saturating_add(area.width.saturating_sub(w));
            hits.push(
                Rect {
                    x,
                    y: area.y,
                    width: w.max(1),
                    height: 1,
                },
                Hit::Next,
            );
            Span::styled(text, theme.accent())
        }
        None => Span::styled("— ›", theme.muted()),
    };

    let left_len = u16::try_from(left.content.chars().count()).unwrap_or(0);
    let right_len = u16::try_from(right.content.chars().count()).unwrap_or(0);
    let gap = area
        .width
        .saturating_sub(left_len.saturating_add(right_len));
    let mid = Span::raw(" ".repeat(usize::from(gap)));

    frame.render_widget(Paragraph::new(Line::from(vec![left, mid, right])), area);
}
