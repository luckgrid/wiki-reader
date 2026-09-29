//! Viewer footer (F1): ‹ prev / next ›.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;
use crate::tui::viewer_doc::FocusTarget;

/// Draw prev/next labels; dim and non-hit when absent.
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    prev_label: Option<&str>,
    next_label: Option<&str>,
    focused: Option<FocusTarget>,
    theme: &Theme,
    hits: &mut HitMap,
) {
    if area.width == 0 {
        return;
    }

    let focus_prev = focused == Some(FocusTarget::FooterPrev);
    let focus_next = focused == Some(FocusTarget::FooterNext);

    let left = match prev_label {
        Some(l) => {
            let text = format!("‹ {l}");
            let w = u16::try_from(Span::raw(text.as_str()).width()).unwrap_or(u16::MAX);
            hits.push(
                Rect {
                    x: area.x,
                    y: area.y,
                    width: w.min(area.width / 2).max(1),
                    height: 1,
                },
                Hit::Prev,
            );
            let style = if focus_prev {
                theme.text().bg(theme.focus_item)
            } else {
                theme.accent()
            };
            Span::styled(text, style)
        }
        None => Span::styled("‹ —", theme.muted()),
    };

    let right = match next_label {
        Some(l) => {
            let text = format!("{l} ›");
            let w = u16::try_from(Span::raw(text.as_str()).width()).unwrap_or(u16::MAX);
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
            let style = if focus_next {
                theme.text().bg(theme.focus_item)
            } else {
                theme.accent()
            };
            Span::styled(text, style)
        }
        None => Span::styled("— ›", theme.muted()),
    };

    let left_len = u16::try_from(Span::raw(left.content.as_ref()).width()).unwrap_or(0);
    let right_len = u16::try_from(Span::raw(right.content.as_ref()).width()).unwrap_or(0);
    let gap = area
        .width
        .saturating_sub(left_len.saturating_add(right_len));
    let mid = Span::raw(" ".repeat(usize::from(gap)));

    frame.render_widget(Paragraph::new(Line::from(vec![left, mid, right])), area);
}
