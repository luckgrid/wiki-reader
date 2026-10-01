//! View footer chrome: outlined ‹ prev / next › buttons on the bottom border (P2-18).

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

/// Columns a button adds around its text: `┤ ` … ` ├`.
const BUTTON_CHROME: usize = 4;

/// One outlined button sitting on the border row: `┤ ‹ Name ├` in the pane's
/// border colour; selected (Tab-focused) fills the inside peach.
fn button(text: &str, selected: bool, border: Style, theme: &Theme) -> Line<'static> {
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

/// Build left/right bottom titles, each capped to half the border (never overlap).
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
    let border_y = area.y.saturating_add(area.height.saturating_sub(1));
    // Interior of the bottom border between corner glyphs.
    let inner_w = area.width.saturating_sub(2);
    let half = (inner_w / 2).max(1);
    let left_x = area.x.saturating_add(1);
    let budget = usize::from(half).saturating_sub(BUTTON_CHROME);
    let border = theme.border(pane_focused);

    let left = prev_label.map(|l| {
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

    let right = next_label.map(|l| {
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
