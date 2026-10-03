//! View tabs: a fully closed group in a dedicated three-row strip.
//! Active selection uses the accent with View focus and a darker fill with Nav focus.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use wiki_reader_core::nav::Tab;

use crate::tui::hit::{Hit, HitMap};
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

/// Middle row of a closed group `│ a.md × │ b.md × │`, with shared separators.
/// Registers [`Hit::Tab`] over each padded cell and
/// [`Hit::TabClose`] over its `×`.
///
/// `area` is the top strip. Reserve the active tab first, truncate labels to
/// fit, then admit whole remaining cells in order. Partial strips have no hits.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn titles(
    area: Rect,
    tabs: &[Tab],
    active: usize,
    pane_focused: bool,
    muted: bool,
    theme: &Theme,
    hits: &mut HitMap,
) -> Line<'static> {
    if area.height < 3 || area.width < 9 {
        return Line::default();
    }
    // Pane sides and the closed group's two borders.
    let inner_w = usize::from(area.width - 4);
    let border = theme.border(pane_focused);
    let fill = if pane_focused {
        Style::default().bg(theme.peach).fg(theme.on_peach)
    } else {
        Style::default()
            .bg(theme.tab_unfocused())
            .fg(ratatui::style::Color::White)
    };
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
                y: area.y + 1,
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
                y: area.y + 1,
                width: 1,
                height: 1,
            },
            Hit::TabClose(i),
        );
        let selected = i == active && !muted;
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

/// Paint the complete tab group; only middle-row labels and × have hits.
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
    let line = titles(area, tabs, active, pane_focused, muted, theme, hits);
    let width = u16::try_from(line.width()).unwrap_or(0);
    if width > 0 {
        crate::tui::regions::footer::draw_closed(
            frame,
            Rect::new(area.x + 1, area.y, width, area.height),
            line,
            theme.border(pane_focused),
        );
    }
}
