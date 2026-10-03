//! Three-row View footer strip: fully closed ‹ prev / next › buttons.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;
use crate::tui::viewer_doc::FocusTarget;

/// Truncated button middle rows and hit geometry for the View bottom strip.
pub struct FooterTitles {
    /// Left-aligned prev title.
    pub left: Line<'static>,
    /// Right-aligned next title.
    pub right: Line<'static>,
}

pub(crate) fn col_width(s: &str) -> usize {
    Span::raw(s).width()
}

/// Ellipsis-truncate `s` to at most `max` display columns (`…` when clipped).
pub(crate) fn ellipsis(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if col_width(s) <= max {
        return s.to_owned();
    }
    let ell = "…";
    let ell_w = col_width(ell);
    if max <= ell_w {
        return ell.chars().take(max).collect();
    }
    let budget = max - ell_w;
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let w = col_width(&ch.to_string());
        if used + w > budget {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push_str(ell);
    out
}

/// Columns a button adds around its text: `┤ ` … ` ├`.
pub(crate) const BUTTON_CHROME: usize = 4;

/// Button middle row; `draw_closed` replaces end glyphs with vertical sides.
/// Selected (Tab-focused) labels fill the inside peach.
pub(crate) fn button(text: &str, selected: bool, border: Style, theme: &Theme) -> Line<'static> {
    let inner = if selected {
        Style::default().bg(theme.peach).fg(theme.on_peach)
    } else {
        border
    };
    Line::from(vec![
        Span::styled("┤", border),
        Span::styled(format!(" {text} "), inner),
        Span::styled("├", border),
    ])
}

/// Build left/right middle rows, each capped to half the strip (never overlap).
///
/// `pane_focused` is whether the View pane has focus; the button outline follows
/// the pane border colour. A missing side yields an empty title, so the border
/// remains empty.
#[must_use]
pub fn titles(
    area: Rect,
    prev_label: Option<&str>,
    next_label: Option<&str>,
    focused: Option<FocusTarget>,
    pane_focused: bool,
    theme: &Theme,
    hits: &mut HitMap,
) -> FooterTitles {
    if area.height < 3 || area.width < 3 {
        return FooterTitles {
            left: Line::default(),
            right: Line::default(),
        };
    }
    let border_y = area.y + 1;
    // Interior of the bottom border between corner glyphs.
    let inner_w = area.width.saturating_sub(2);
    let half = (inner_w / 2).max(1);
    let left_x = area.x.saturating_add(1);
    let budget = usize::from(half).saturating_sub(BUTTON_CHROME);
    let border = theme.border(pane_focused);

    let left = prev_label.filter(|_| half >= 7).map(|l| {
        let text = format!("‹ {}", ellipsis(l, budget.saturating_sub(2)));
        let w = col_width(&text) + BUTTON_CHROME;
        hits.push(
            Rect {
                x: left_x,
                y: border_y,
                width: u16::try_from(w).unwrap_or(1).min(half).max(1),
                height: 1,
            },
            Hit::Prev,
        );
        button(
            &text,
            focused == Some(FocusTarget::FooterPrev),
            border,
            theme,
        )
    });

    let right = next_label.filter(|_| half >= 7).map(|l| {
        let text = format!("{} ›", ellipsis(l, budget.saturating_sub(2)));
        let w = u16::try_from(col_width(&text) + BUTTON_CHROME)
            .unwrap_or(1)
            .min(half)
            .max(1);
        let x = area
            .x
            .saturating_add(area.width.saturating_sub(1).saturating_sub(w));
        hits.push(
            Rect {
                x,
                y: border_y,
                width: w,
                height: 1,
            },
            Hit::Next,
        );
        button(
            &text,
            focused == Some(FocusTarget::FooterNext),
            border,
            theme,
        )
    });

    FooterTitles {
        left: left.unwrap_or_default().alignment(Alignment::Left),
        right: right.unwrap_or_default().alignment(Alignment::Right),
    }
}

/// Paint a closed three-row box/group. The middle line supplies cell styles;
/// separator spans become shared top/bottom junctions.
pub(crate) fn draw_closed(
    frame: &mut Frame<'_>,
    area: Rect,
    mut middle: Line<'static>,
    border: Style,
) {
    if area.height < 3 || middle.width() == 0 {
        return;
    }
    let last = middle.spans.len() - 1;
    let mut top = String::new();
    let mut bottom = String::new();
    for (i, span) in middle.spans.iter_mut().enumerate() {
        if i == 0 {
            top.push('┌');
            bottom.push('└');
            span.content = "│".into();
        } else if i == last {
            top.push('┐');
            bottom.push('┘');
            span.content = "│".into();
        } else if span.content == "│" {
            top.push('┬');
            bottom.push('┴');
        } else {
            top.push_str(&"─".repeat(span.width()));
            bottom.push_str(&"─".repeat(span.width()));
        }
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(top, border),
            middle.alignment(Alignment::Left),
            Line::styled(bottom, border),
        ]),
        area,
    );
}

/// Draw prev/next in a dedicated strip, with hits on the middle row only.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    prev: Option<&str>,
    next: Option<&str>,
    focused: Option<FocusTarget>,
    pane_focused: bool,
    theme: &Theme,
    hits: &mut HitMap,
) {
    let titles = titles(area, prev, next, focused, pane_focused, theme, hits);
    for (line, right) in [(titles.left, false), (titles.right, true)] {
        let width = u16::try_from(line.width()).unwrap_or(0);
        if width == 0 {
            continue;
        }
        let x = if right {
            area.right() - 1 - width
        } else {
            area.x + 1
        };
        draw_closed(
            frame,
            Rect::new(x, area.y, width, area.height),
            line,
            theme.border(pane_focused),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::hit::HitMap;
    use crate::tui::theme::Theme;

    #[test]
    fn long_titles_do_not_overlap_at_narrow_widths() {
        let theme = Theme::default();
        for w in [40u16, 60, 80] {
            let area = Rect {
                x: 0,
                y: 0,
                width: w,
                height: 10,
            };
            let mut hits = HitMap::default();
            let t = titles(
                area,
                Some("A very long previous page title that would collide"),
                Some("Another extremely long next page title here"),
                None,
                true,
                &theme,
                &mut hits,
            );
            let lw = t.left.width();
            let rw = t.right.width();
            let half = usize::from((w.saturating_sub(2)) / 2);
            assert!(lw <= half, "w={w}: left {lw} > half {half}");
            assert!(rw <= half, "w={w}: right {rw} > half {half}");
            assert!(lw + rw <= usize::from(w.saturating_sub(2)));
        }
    }
}
