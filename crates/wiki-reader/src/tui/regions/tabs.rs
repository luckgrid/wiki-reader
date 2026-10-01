//! View tabs: outlined buttons on the pane's top border, styled like the
//! prev/next footer buttons. The active tab is filled peach while the View has
//! focus; with the Nav focused every tab stays outlined.

use ratatui::layout::{Alignment, Rect};
use ratatui::text::Line;
use wiki_reader_core::nav::Tab;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::regions::footer::{BUTTON_CHROME, button, col_width, ellipsis};
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

/// Top-border title with one button per tab; registers [`Hit::Tab`] over the
/// button and [`Hit::TabClose`] over its `×`.
///
/// `area` is the whole View pane. Tabs that do not fit in the border are
/// dropped from the right (the active tab is always kept).
#[must_use]
pub fn titles(
    area: Rect,
    tabs: &[Tab],
    active: usize,
    pane_focused: bool,
    theme: &Theme,
    hits: &mut HitMap,
) -> Line<'static> {
    let inner_w = usize::from(area.width.saturating_sub(2));
    let border = theme.border(pane_focused);
    let mut x = area.x.saturating_add(1);
    let mut used = 0usize;
    let mut spans = Vec::new();
    for (i, tab) in tabs.iter().enumerate() {
        let label = ellipsis(&stem(tab), MAX_LABEL);
        let text = format!("{label} ×");
        let w = col_width(&text) + BUTTON_CHROME;
        if used + w > inner_w && i != active {
            continue;
        }
        if used + w > inner_w {
            break;
        }
        let Ok(w16) = u16::try_from(w) else {
            break;
        };
        hits.push(
            Rect {
                x,
                y: area.y,
                width: w16,
                height: 1,
            },
            Hit::Tab(i),
        );
        // `┤ ` + label + ` ` then the ×.
        let close_x = x
            .saturating_add(2)
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
        spans.extend(button(&text, selected, border, theme).spans);
        x = x.saturating_add(w16);
        used += w;
    }
    Line::from(spans).alignment(Alignment::Left)
}
