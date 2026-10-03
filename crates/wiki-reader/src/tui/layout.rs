//! Five-region layout with responsive side-nav width and chrome insets.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use wiki_reader_core::config::NavPosition;

/// Rows consumed by nav pane chrome: top/bottom borders + search bar + its seam rule.
pub const NAV_CHROME_ROWS: u16 = 4;

/// Inner left padding in the viewer (cursor marker column; P2-19).
pub const VIEWER_LEFT_PAD: u16 = 1;

/// Rows of a connected tab / prev-next bar, counting the pane border row it shares.
pub const VIEWER_BAR_ROWS: u16 = 3;

/// Rows the View reserves at its top and bottom edges for a pane `height` rows tall.
///
/// A bar is drawn only when all of its rows fit, top first, then bottom. An edge
/// without a bar keeps just the pane's one border row.
#[must_use]
pub fn viewer_edge_rows(height: u16) -> (u16, u16) {
    let top = if height > VIEWER_BAR_ROWS {
        VIEWER_BAR_ROWS
    } else {
        height.min(1)
    };
    let rest = height - top;
    let bottom = if rest >= VIEWER_BAR_ROWS {
        VIEWER_BAR_ROWS
    } else {
        rest.min(1)
    };
    (top, bottom)
}

/// Article rows after reserving the top and bottom edges.
#[must_use]
pub fn viewer_visible_rows(height: u16) -> u16 {
    let (top, bottom) = viewer_edge_rows(height);
    height - top - bottom
}

/// Computed region rectangles for one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Regions {
    /// Header content row.
    pub header: Rect,
    /// Side nav (zero-sized when hidden).
    pub side_nav: Rect,
    /// Viewer pane (tabs and prev/next occupy connected three-row bars).
    pub viewer: Rect,
    /// Status content row.
    pub status: Rect,
    /// Whether the nav is an overlay (narrow terminal).
    pub nav_overlay: bool,
}

/// 1-col left/right inset for header/status content and hits.
#[must_use]
pub fn chrome_pad(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    }
}

/// Wrap width for viewer text: borders + left pad, capped at 100.
#[must_use]
pub fn viewer_text_width(viewer: Rect) -> u16 {
    viewer
        .width
        .saturating_sub(2)
        .saturating_sub(VIEWER_LEFT_PAD)
        .min(100)
}

/// Minimum / maximum docked nav column width (P2-14).
pub const NAV_WIDTH_MIN: u16 = 16;
pub const NAV_WIDTH_MAX: u16 = 50;

/// Side-nav column width for a given terminal width.
///
/// `preferred` is the user-dragged width (session); ignored when the terminal is
/// narrow (&lt;80) so responsive collapse / overlay sizing stays intact.
#[must_use]
pub fn nav_width(term_width: u16, nav_forced: bool, preferred: Option<u16>) -> Option<u16> {
    if term_width >= 80 {
        let default = if term_width >= 120 { 30 } else { 26 };
        let cap = NAV_WIDTH_MAX.min(term_width.saturating_sub(20));
        Some(
            preferred
                .unwrap_or(default)
                .clamp(NAV_WIDTH_MIN, cap.max(NAV_WIDTH_MIN)),
        )
    } else if nav_forced {
        // Overlay: ignore preferred; use ~half, capped.
        Some(term_width.saturating_sub(2).clamp(16, 30))
    } else {
        None
    }
}

/// Split `area` into the regions.
///
/// `nav_visible` is honored at every width: docked when ≥80, overlay when &lt;80.
/// Docked nav follows `position`; narrow overlay always opens from the left.
#[must_use]
pub fn split(
    area: Rect,
    nav_visible: bool,
    preferred_nav: Option<u16>,
    position: NavPosition,
) -> Regions {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(0),    // mid: header/status retained on short terminals
            Constraint::Length(1), // status
        ])
        .split(area);

    let header = vert[0];
    let mid = vert[1];
    let status = vert[2];

    let narrow = area.width < 80;
    let show_nav = nav_visible && nav_width(area.width, nav_visible, preferred_nav).is_some();
    let nav_overlay = narrow && show_nav;

    let (side_nav, viewer_col) = if show_nav {
        let w = nav_width(area.width, nav_visible, preferred_nav).unwrap_or(26);
        if nav_overlay {
            // Overlay sits on the left; viewer still gets full mid underneath.
            let nav = Rect {
                x: mid.x,
                y: mid.y,
                width: w.min(mid.width),
                height: mid.height,
            };
            (nav, mid)
        } else {
            let constraints = match position {
                NavPosition::Left => [Constraint::Length(w), Constraint::Min(10)],
                NavPosition::Right => [Constraint::Min(10), Constraint::Length(w)],
            };
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(constraints)
                .split(mid);
            match position {
                NavPosition::Left => (cols[0], cols[1]),
                NavPosition::Right => (cols[1], cols[0]),
            }
        }
    } else {
        (
            Rect {
                x: mid.x,
                y: mid.y,
                width: 0,
                height: 0,
            },
            mid,
        )
    };

    let viewer = viewer_col;

    Regions {
        header,
        side_nav,
        viewer,
        status,
        nav_overlay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responsive_nav_widths() {
        assert_eq!(nav_width(120, false, None), Some(30));
        assert_eq!(nav_width(100, false, None), Some(26));
        assert_eq!(nav_width(60, false, None), None);
        assert!(nav_width(60, true, None).is_some());
        assert_eq!(nav_width(120, false, Some(40)), Some(40));
        assert_eq!(nav_width(120, false, Some(10)), Some(NAV_WIDTH_MIN));
        assert_eq!(nav_width(120, false, Some(99)), Some(NAV_WIDTH_MAX));
        // Narrow ignores preferred.
        assert_eq!(nav_width(60, true, Some(40)), Some(30));
    }

    #[test]
    fn viewer_bars_are_dropped_whole_on_short_panes() {
        // (height, top edge rows, bottom edge rows, article rows)
        for (h, top, bottom, rows) in [
            (0, 0, 0, 0),
            (1, 1, 0, 0),
            (2, 1, 1, 0),
            (3, 1, 1, 1),
            (4, 3, 1, 0),
            (5, 3, 1, 1),
            (6, 3, 3, 0),
            (7, 3, 3, 1),
            (24, 3, 3, 18),
        ] {
            assert_eq!(viewer_edge_rows(h), (top, bottom), "height {h}");
            assert_eq!(viewer_visible_rows(h), rows, "height {h}");
        }
    }

    #[test]
    fn chrome_pad_insets_one_each_side() {
        let a = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 1,
        };
        let p = chrome_pad(a);
        assert_eq!(p.x, 1);
        assert_eq!(p.width, 78);
        assert_eq!(p.y, 0);
    }

    #[test]
    fn viewer_text_width_subtracts_borders_and_left_pad() {
        let v = Rect {
            x: 30,
            y: 1,
            width: 50,
            height: 20,
        };
        // 50 - 2 borders - 1 left pad = 47
        assert_eq!(viewer_text_width(v), 47);
        let wide = Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 10,
        };
        assert_eq!(viewer_text_width(wide), 100);
    }

    #[test]
    fn split_honors_nav_visible_at_wide_and_narrow() {
        let wide = Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 24,
        };
        assert_eq!(
            split(wide, false, None, NavPosition::Left).side_nav.width,
            0
        );
        assert!(split(wide, true, None, NavPosition::Left).side_nav.width > 0);
        assert!(!split(wide, true, None, NavPosition::Left).nav_overlay);

        let narrow = Rect {
            x: 0,
            y: 0,
            width: 60,
            height: 24,
        };
        assert_eq!(
            split(narrow, false, None, NavPosition::Left).side_nav.width,
            0
        );
        assert!(split(narrow, true, None, NavPosition::Left).side_nav.width > 0);
        assert!(split(narrow, true, None, NavPosition::Left).nav_overlay);
    }

    #[test]
    fn split_header_and_status_hug_the_panes() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 40,
        };
        let r = split(area, true, None, NavPosition::Left);
        assert_eq!(r.header.y, 0);
        assert_eq!(r.side_nav.y, 1);
        assert_eq!(r.status.y, 39);
        assert_eq!(r.viewer.y + r.viewer.height, 39);
    }

    #[test]
    fn split_honors_preferred_nav_width() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 24,
        };
        assert_eq!(
            split(area, true, Some(40), NavPosition::Left)
                .side_nav
                .width,
            40
        );
        assert_eq!(
            split(area, true, Some(8), NavPosition::Left).side_nav.width,
            NAV_WIDTH_MIN
        );
    }

    #[test]
    fn split_puts_nav_on_the_right_when_configured() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 24,
        };
        let left = split(area, true, Some(30), NavPosition::Left);
        let right = split(area, true, Some(30), NavPosition::Right);
        assert_eq!(left.side_nav.x, 0);
        assert_eq!(left.viewer.x, 30);
        assert_eq!(right.viewer.x, 0);
        assert_eq!(right.side_nav.x, 90);
        assert_eq!(right.side_nav.width, 30);
    }
}
