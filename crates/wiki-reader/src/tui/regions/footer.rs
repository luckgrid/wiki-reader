//! View footer chrome: outlined ‹ prev / next › buttons and ? / ⚙ controls.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
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

/// One outlined button sitting on the border row: `┤ ‹ Name ├` in the pane's
/// border colour; selected (Tab-focused) fills the inside peach.
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

/// Reserve the right-hand ? / ⚙ controls, then split the rest between prev/next.
/// On tiny panes, hide links that cannot fit their chrome and retain only visible icons.
///
/// `pane_focused` is whether the View pane has focus; the button outline follows
/// the pane border colour. A missing side yields an empty title, so the border
/// runs unbroken.
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
    if area.height < 2 || area.width < 3 {
        return FooterTitles {
            left: Line::default(),
            right: Line::default(),
        };
    }
    let border_y = area.bottom() - 1;
    // Interior of the bottom border between corner glyphs.
    let inner_w = area.width.saturating_sub(2);
    let controls_w = inner_w.min(4);
    let half = (inner_w - controls_w) / 2;
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
            .saturating_add(area.width.saturating_sub(1 + controls_w).saturating_sub(w));
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

    let controls: String = " ? ⚙".chars().skip(usize::from(4 - controls_w)).collect();
    for (offset, hit) in [(3, Hit::OpenHelp), (1, Hit::OpenOptions)] {
        if inner_w >= offset {
            hits.push(Rect::new(area.right() - 1 - offset, border_y, 1, 1), hit);
        }
    }
    let mut right = right.unwrap_or_default();
    right.spans.push(Span::styled(controls, theme.accent()));
    FooterTitles {
        left: left.unwrap_or_default().alignment(Alignment::Left),
        right: right.alignment(Alignment::Right),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::hit::HitMap;
    use crate::tui::theme::Theme;

    #[test]
    fn tiny_footers_only_register_visible_non_overlapping_hits() {
        let theme = Theme::default();
        for width in 0..50 {
            for height in 0..4 {
                let area = Rect::new(5, 7, width, height);
                let mut hits = HitMap::default();
                let titles = titles(
                    area,
                    Some("Previous"),
                    Some("Next"),
                    None,
                    true,
                    &theme,
                    &mut hits,
                );
                assert!(
                    titles.left.width() + titles.right.width()
                        <= usize::from(width.saturating_sub(2))
                );
                for (i, (rect, _)) in hits.entries().iter().enumerate() {
                    assert!(height >= 2);
                    assert_eq!(rect.y, area.bottom() - 1);
                    assert!(rect.x > area.x && rect.right() < area.right());
                    for (other, _) in &hits.entries()[i + 1..] {
                        assert!(rect.intersection(*other).is_empty());
                    }
                }
            }
        }
    }

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
            let half = usize::from((w.saturating_sub(6)) / 2);
            assert!(lw <= half, "w={w}: left {lw} > half {half}");
            assert!(rw <= half + 4, "w={w}: right {rw} > budget {}", half + 4);
            assert!(lw + rw <= usize::from(w.saturating_sub(2)));
        }
    }
}
