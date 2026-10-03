//! Compact View footer: connected two-row ‹ prev / next › controls.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;
use crate::tui::viewer_doc::FocusTarget;

/// Truncated button label rows and hit geometry for the View bottom bar.
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

/// Label row embedded in the pane edge; the strip adds its connected separator.
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

/// Build left/right label rows, each capped to half the bar (never overlap).
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
    if area.height < 2 || area.width < 3 {
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
    let border = theme.border(pane_focused).bg(theme.surface_muted);

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

/// Two connected rows, like a table title/separator: labels on the pane edge,
/// a shared horizontal rule toward the article, and a solid background throughout.
/// Each `(x, line)` uses an absolute column inside the pane's corner glyphs.
pub(crate) fn draw_strip(
    frame: &mut Frame<'_>,
    area: Rect,
    labels: Vec<(u16, Line<'static>)>,
    top: bool,
    border: Style,
) {
    if area.height < 2 || area.width < 2 {
        return;
    }
    let rule = "─".repeat(usize::from(area.width - 2));
    let edge = if top {
        format!("┌{rule}┐")
    } else {
        format!("└{rule}┘")
    };
    let seam = format!("├{rule}┤");
    let rows = if top {
        vec![edge, seam]
    } else {
        vec![seam, edge]
    };
    frame.render_widget(Paragraph::new(rows.join("\n")).style(border), area);
    for (x, mut label) in labels {
        if label.width() == 0 {
            continue;
        }
        let last = label.spans.len() - 1;
        let mut separator = Vec::new();
        for (i, span) in label.spans.iter_mut().enumerate() {
            let junction = i == 0 || i == last || span.content == "│";
            separator.push(Span::styled(
                if junction {
                    if top { "┴".into() } else { "┬".into() }
                } else {
                    "─".repeat(span.width())
                },
                border.patch(span.style),
            ));
        }
        let width = u16::try_from(label.width()).unwrap_or(0);
        label = label.alignment(Alignment::Left);
        let label_y = area.y + u16::from(!top);
        let seam_y = area.y + u16::from(top);
        frame.render_widget(
            Paragraph::new(label).style(border),
            Rect::new(x, label_y, width, 1),
        );
        frame.render_widget(
            Paragraph::new(Line::from(separator)),
            Rect::new(x, seam_y, width, 1),
        );
    }
}

/// Draw prev/next on the bottom edge with a shared separator toward the article.
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
    let mut labels = Vec::new();
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
        labels.push((x, line));
    }
    draw_strip(
        frame,
        area,
        labels,
        false,
        theme.border(pane_focused).bg(theme.surface_muted),
    );
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
