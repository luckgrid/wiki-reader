//! View tabs: one outlined group on the pane's top border, styled like the
//! prev/next footer buttons. The active tab is filled peach while the View has
//! focus; with the Nav focused every tab stays outlined.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use wiki_reader_core::nav::Tab;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::regions::footer::{col_width, ellipsis};
use crate::tui::theme::Theme;

fn stem(tab: &Tab) -> String {
    tab.current()
        .page
        .relative_path
        .file_stem()
        .map_or_else(|| "?".into(), |s| s.to_string_lossy().into_owned())
}

/// Longest label a tab shows before it is ellipsized.
const MAX_LABEL: usize = 18;

/// Top-border title: one outlined group `┤ a × │ b × ├`, tabs separated by a
/// single `│`. Registers [`Hit::Tab`] over each padded cell and
/// [`Hit::TabClose`] over its `×`.
///
/// `area` is the whole View pane. Tabs that do not fit in the border are
/// dropped from the right (the active tab is always kept if it fits).
#[must_use]
pub fn titles(
    area: Rect,
    tabs: &[Tab],
    active: usize,
    pane_focused: bool,
    theme: &Theme,
    hits: &mut HitMap,
) -> Line<'static> {
    // `┤` and `├` take one column each.
    let inner_w = usize::from(area.width.saturating_sub(2)).saturating_sub(2);
    let border = theme.border(pane_focused);
    let fill = Style::default().bg(theme.peach).fg(theme.on_peach);
    let labels: Vec<_> = tabs
        .iter()
        .map(|tab| ellipsis(&stem(tab), MAX_LABEL))
        .collect();
    let widths: Vec<_> = labels
        .iter()
        .map(|label| col_width(&format!(" {label} × ")))
        .collect();

    // Reserve the active cell first, then fill remaining room in tab order.
    // Without the reservation, enough earlier tabs could hide the active one.
    let mut shown = vec![false; tabs.len()];
    let mut used = 0usize;
    if let Some(&w) = widths.get(active)
        && w <= inner_w
    {
        shown[active] = true;
        used = w;
    }
    for (i, &w) in widths.iter().enumerate() {
        if shown[i] {
            continue;
        }
        let sep = usize::from(used > 0);
        if used + sep + w <= inner_w {
            shown[i] = true;
            used += sep + w;
        }
    }

    let mut x = area.x.saturating_add(2);
    let mut cells: Vec<Span<'static>> = Vec::new();
    for (i, label) in labels.into_iter().enumerate().filter(|(i, _)| shown[*i]) {
        let text = format!(" {label} × ");
        let w = widths[i];
        let Ok(w16) = u16::try_from(w) else {
            continue;
        };
        if !cells.is_empty() {
            cells.push(Span::styled("│", border));
            x = x.saturating_add(1);
        }
        hits.push(
            Rect {
                x,
                y: area.y,
                width: w16,
                height: 1,
            },
            Hit::Tab(i),
        );
        // ` ` + label + ` ` then the ×.
        let close_x = x
            .saturating_add(1)
            .saturating_add(u16::try_from(col_width(&label) + 1).unwrap_or(0));
        hits.push(
            Rect {
                x: close_x,
                y: area.y,
                width: 1,
                height: 1,
            },
            Hit::TabClose(i),
        );
        let selected = i == active && pane_focused;
        cells.push(Span::styled(text, if selected { fill } else { border }));
        x = x.saturating_add(w16);
    }
    if cells.is_empty() {
        return Line::default();
    }
    let mut spans = vec![Span::styled("┤", border)];
    spans.extend(cells);
    spans.push(Span::styled("├", border));
    Line::from(spans).alignment(Alignment::Left)
}
