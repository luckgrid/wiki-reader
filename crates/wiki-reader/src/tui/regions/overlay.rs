//! Shared centered overlay panel + scroll helpers (P2-20 / P2-21).

use ratatui::layout::Rect;

/// Centered panel rect, clamped to `[min_w, max_w]` × `[min_h, max_h]` and the area.
#[must_use]
pub fn centered_panel(area: Rect, max_w: u16, max_h: u16, min_w: u16, min_h: u16) -> Rect {
    let width = area.width.clamp(min_w, max_w).min(area.width);
    let height = area.height.clamp(min_h, max_h).min(area.height);
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Keep `selected` inside the visible window `[scroll, scroll + visible)`.
#[must_use]
pub fn ensure_visible(selected: usize, scroll: usize, visible: usize) -> usize {
    if visible == 0 {
        return 0;
    }
    if selected < scroll {
        selected
    } else if selected >= scroll.saturating_add(visible) {
        selected.saturating_add(1).saturating_sub(visible)
    } else {
        scroll
    }
}

/// Clamp scroll so it never past the end of a `len`-item list.
#[must_use]
pub fn clamp_scroll(scroll: usize, visible: usize, len: usize) -> usize {
    let max = len.saturating_sub(visible.max(1));
    scroll.min(max)
}
