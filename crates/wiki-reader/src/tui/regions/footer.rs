//! View footer: a connected three-row bar with ‹ prev and next › cells.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use crate::tui::hit::{Hit, HitMap};
use crate::tui::regions::bar::{self, BAR_ROWS, Cell};
use crate::tui::theme::Theme;
use crate::tui::viewer_doc::FocusTarget;

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

/// One space on either side of a plain navigation label.
pub(crate) const BUTTON_CHROME: usize = 2;

pub(crate) fn button(
    text: &str,
    selected: bool,
    pane_focused: bool,
    theme: &Theme,
) -> Line<'static> {
    Line::from(Span::styled(
        format!(" {text} "),
        theme.chrome_label_style(selected, pane_focused),
    ))
}

/// Build the prev (left) and next (right) cells and register their hits on the
/// label row (the middle row of the bar).
///
/// Each cell is sized to its label and capped so the two cells and their dividers
/// never overlap. `pane_focused` is whether the View pane has focus. A missing side
/// yields no cell, so the bar stays empty there.
#[must_use]
pub fn cells(
    area: Rect,
    prev_label: Option<&str>,
    next_label: Option<&str>,
    focused: Option<FocusTarget>,
    pane_focused: bool,
    theme: &Theme,
    hits: &mut HitMap,
) -> Vec<Cell> {
    if area.height < BAR_ROWS || area.width < 3 {
        return Vec::new();
    }
    let label_y = area.y + 1;
    let inner_w = area.width.saturating_sub(2);
    // Two cells plus the dividers beside them fit in the interior.
    let cap = inner_w.saturating_sub(2) / 2;
    let budget = usize::from(cap).saturating_sub(BUTTON_CHROME);
    let mut out = Vec::new();

    if let Some(label) = prev_label.filter(|_| cap >= 5) {
        let text = format!("‹ {}", ellipsis(label, budget.saturating_sub(2)));
        let w = u16::try_from(col_width(&text) + BUTTON_CHROME)
            .unwrap_or(1)
            .clamp(1, cap);
        let x = area.x + 1;
        hits.push(
            Rect {
                x,
                y: label_y,
                width: w,
                height: 1,
            },
            Hit::Prev,
        );
        out.push(Cell {
            x,
            width: w,
            line: button(
                &text,
                focused == Some(FocusTarget::FooterPrev),
                pane_focused,
                theme,
            ),
        });
    }

    if let Some(label) = next_label.filter(|_| cap >= 5) {
        let text = format!("{} ›", ellipsis(label, budget.saturating_sub(2)));
        let w = u16::try_from(col_width(&text) + BUTTON_CHROME)
            .unwrap_or(1)
            .clamp(1, cap);
        let x = area.right() - 1 - w;
        hits.push(
            Rect {
                x,
                y: label_y,
                width: w,
                height: 1,
            },
            Hit::Next,
        );
        out.push(Cell {
            x,
            width: w,
            line: button(
                &text,
                focused == Some(FocusTarget::FooterNext),
                pane_focused,
                theme,
            ),
        });
    }
    out
}

/// Draw the bottom bar (`area` is its three rows) in the pane's border style.
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
    let cells = cells(area, prev, next, focused, pane_focused, theme, hits);
    bar::draw(frame, area, false, &cells, theme.border(pane_focused));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::hit::HitMap;
    use crate::tui::theme::Theme;

    #[test]
    fn long_titles_do_not_overlap_at_narrow_widths() {
        let theme = Theme::default();
        for w in [8u16, 12, 40, 60, 80] {
            let area = Rect {
                x: 0,
                y: 0,
                width: w,
                height: BAR_ROWS,
            };
            let mut hits = HitMap::default();
            let cells = cells(
                area,
                Some("A very long previous page title that would collide"),
                Some("Another extremely long next page title here"),
                None,
                true,
                &theme,
                &mut hits,
            );
            for pair in cells.windows(2) {
                // A divider column sits between the cells.
                assert!(
                    pair[0].x + pair[0].width < pair[1].x,
                    "w={w}: cells touch or overlap"
                );
            }
            for cell in &cells {
                assert!(cell.x > 0 && cell.x + cell.width < w);
                assert_eq!(cell.line.width(), usize::from(cell.width));
            }
            for (rect, _) in hits.entries() {
                assert_eq!(rect.y, 1, "hits sit on the label row");
            }
        }
    }
}
