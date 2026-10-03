//! View tabs: bordered cells in a connected three-row bar; the active tab is bold peach.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use wiki_reader_core::nav::Tab;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::regions::bar::{self, BAR_ROWS, Cell};
use crate::tui::regions::footer::{col_width, ellipsis};
use crate::tui::theme::Theme;

fn filename(tab: &Tab) -> String {
    tab.current()
        .page
        .relative_path
        .file_name()
        .map_or_else(|| "?".into(), |s| s.to_string_lossy().into_owned())
}

/// Longest label a tab shows before it is ellipsized.
const MAX_LABEL: usize = 18;

/// One bordered cell per tab: ` name.md × `. Registers [`Hit::Tab`] over each padded
/// cell and [`Hit::TabClose`] over its `×`, on the label row (the bar's middle row).
///
/// `area` is the bar's three rows. Reserve the active tab first, truncate labels to
/// fit, then admit whole remaining cells in order; a cell and the divider before it
/// must fit. Partial strips have no hits.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn cells(
    area: Rect,
    tabs: &[Tab],
    active: usize,
    pane_focused: bool,
    muted: bool,
    theme: &Theme,
    hits: &mut HitMap,
) -> Vec<Cell> {
    if area.height < BAR_ROWS || area.width < 7 {
        return Vec::new();
    }
    let label_y = area.y + 1;
    let inner_w = usize::from(area.width - 2);
    let labels: Vec<_> = tabs
        .iter()
        .map(|tab| ellipsis(&filename(tab), MAX_LABEL.min(inner_w.saturating_sub(4))))
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
        let divider = usize::from(used > 0);
        if used + divider + w <= inner_w {
            shown[i] = true;
            used += divider + w;
        }
    }

    let mut x = area.x.saturating_add(1);
    let mut out: Vec<Cell> = Vec::new();
    for (i, label) in labels.into_iter().enumerate().filter(|(i, _)| shown[*i]) {
        let text = format!(" {label} × ");
        let Ok(w16) = u16::try_from(widths[i]) else {
            continue;
        };
        if !out.is_empty() {
            // The divider between cells.
            x = x.saturating_add(1);
        }
        hits.push(
            Rect {
                x,
                y: label_y,
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
                y: label_y,
                width: 1,
                height: 1,
            },
            Hit::TabClose(i),
        );
        let selected = i == active && !muted;
        out.push(Cell {
            x,
            width: w16,
            line: Line::from(Span::styled(
                text,
                theme.chrome_label_style(selected, pane_focused),
            )),
        });
        x = x.saturating_add(w16);
    }
    out
}

/// Draw the top bar (`area` is its three rows) in the pane's border style.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    tabs: &[Tab],
    active: usize,
    pane_focused: bool,
    muted: bool,
    theme: &Theme,
    hits: &mut HitMap,
) {
    let cells = cells(area, tabs, active, pane_focused, muted, theme, hits);
    bar::draw(frame, area, true, &cells, theme.border(pane_focused));
}
