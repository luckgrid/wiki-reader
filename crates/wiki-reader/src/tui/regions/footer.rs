//! Viewer footer chrome: ‹ prev / next › on the viewer bottom border (P2-18).

use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};

use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;
use crate::tui::viewer_doc::FocusTarget;

/// Truncated border titles and hit geometry for the viewer bottom edge.
pub struct FooterTitles {
    /// Left-aligned prev title.
    pub left: Line<'static>,
    /// Right-aligned next title.
    pub right: Line<'static>,
}

fn col_width(s: &str) -> usize {
    Span::raw(s).width()
}

/// Ellipsis-truncate `s` to at most `max` display columns (`…` when clipped).
fn ellipsis(s: &str, max: usize) -> String {
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

/// Build left/right bottom titles, each capped to half the border (never overlap).
#[must_use]
pub fn titles(
    area: Rect,
    prev_label: Option<&str>,
    next_label: Option<&str>,
    focused: Option<FocusTarget>,
    theme: &Theme,
    hits: &mut HitMap,
) -> FooterTitles {
    let border_y = area.y.saturating_add(area.height.saturating_sub(1));
    // Interior of the bottom border between corner glyphs.
    let inner_w = area.width.saturating_sub(2);
    let half = (inner_w / 2).max(1);
    let left_x = area.x.saturating_add(1);
    let right_budget = usize::from(half);
    let left_budget = usize::from(half);

    let focus_prev = focused == Some(FocusTarget::FooterPrev);
    let focus_next = focused == Some(FocusTarget::FooterNext);

    let (left_text, left_style, left_hit) = match prev_label {
        Some(l) => {
            let body = ellipsis(l, left_budget.saturating_sub(2)); // "‹ " + body
            let text = format!("‹ {body}");
            let style = if focus_prev {
                theme.text().bg(theme.focus_item)
            } else {
                theme.accent()
            };
            (text, style, true)
        }
        None => ("‹ —".to_owned(), theme.muted(), false),
    };
    let left_w = col_width(&left_text).min(left_budget).max(1);
    if left_hit {
        hits.push(
            Rect {
                x: left_x,
                y: border_y,
                width: u16::try_from(left_w).unwrap_or(1).min(half).max(1),
                height: 1,
            },
            Hit::Prev,
        );
    }

    let (right_text, right_style, right_hit) = match next_label {
        Some(l) => {
            let body = ellipsis(l, right_budget.saturating_sub(2)); // body + " ›"
            let text = format!("{body} ›");
            let style = if focus_next {
                theme.text().bg(theme.focus_item)
            } else {
                theme.accent()
            };
            (text, style, true)
        }
        None => ("— ›".to_owned(), theme.muted(), false),
    };
    let right_w = col_width(&right_text).min(right_budget).max(1);
    if right_hit {
        let w = u16::try_from(right_w).unwrap_or(1).min(half).max(1);
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
    }

    FooterTitles {
        left: Line::from(Span::styled(left_text, left_style)).alignment(Alignment::Left),
        right: Line::from(Span::styled(right_text, right_style)).alignment(Alignment::Right),
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
