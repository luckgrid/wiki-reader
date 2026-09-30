//! Five-region layout with responsive side-nav width and chrome insets.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Rows consumed by nav pane chrome: top/bottom borders + gap + search + gap.
pub const NAV_CHROME_ROWS: u16 = 5;

/// Inner left padding in the viewer (cursor marker column; P2-19).
pub const VIEWER_LEFT_PAD: u16 = 1;

/// Computed region rectangles for one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Regions {
    /// Header content row (gaps above/below are blank when tall).
    pub header: Rect,
    /// Side nav (zero-sized when hidden).
    pub side_nav: Rect,
    /// Viewer pane (prev/next live on the bottom border).
    pub viewer: Rect,
    /// Status content row (gaps above/below are blank when tall).
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

/// Blank rows above and below header/status when the terminal is tall enough.
#[must_use]
pub fn vertical_gap(term_height: u16) -> u16 {
    u16::from(term_height >= 30)
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

/// Side-nav column width for a given terminal width.
#[must_use]
pub fn nav_width(term_width: u16, nav_forced: bool) -> Option<u16> {
    if term_width >= 120 {
        Some(30)
    } else if term_width >= 80 {
        Some(26)
    } else if nav_forced {
        // Overlay: use ~half, capped.
        Some(term_width.saturating_sub(2).clamp(16, 30))
    } else {
        None
    }
}

/// Split `area` into the five regions.
///
/// `nav_visible` is honored at every width: docked when ≥80, overlay when &lt;80.
#[must_use]
pub fn split(area: Rect, nav_visible: bool) -> Regions {
    let gap = vertical_gap(area.height);
    // gap + content + gap (or just content when gap == 0).
    let band = 1 + gap.saturating_mul(2);

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(band), // header band
            Constraint::Min(3),       // mid
            Constraint::Length(band), // status band
        ])
        .split(area);

    let header = Rect {
        x: vert[0].x,
        y: vert[0].y.saturating_add(gap),
        width: vert[0].width,
        height: 1,
    };
    let mid = vert[1];
    let status = Rect {
        x: vert[2].x,
        y: vert[2].y.saturating_add(gap),
        width: vert[2].width,
        height: 1,
    };

    let narrow = area.width < 80;
    let show_nav = nav_visible && nav_width(area.width, nav_visible).is_some();
    let nav_overlay = narrow && show_nav;

    let (side_nav, viewer_col) = if show_nav {
        let w = nav_width(area.width, nav_visible).unwrap_or(26);
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
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(w), Constraint::Min(10)])
                .split(mid);
            (cols[0], cols[1])
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
        assert_eq!(nav_width(120, false), Some(30));
        assert_eq!(nav_width(100, false), Some(26));
        assert_eq!(nav_width(60, false), None);
        assert!(nav_width(60, true).is_some());
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
    fn vertical_gap_at_30() {
        assert_eq!(vertical_gap(24), 0);
        assert_eq!(vertical_gap(29), 0);
        assert_eq!(vertical_gap(30), 1);
        assert_eq!(vertical_gap(40), 1);
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
        assert_eq!(split(wide, false).side_nav.width, 0);
        assert!(split(wide, true).side_nav.width > 0);
        assert!(!split(wide, true).nav_overlay);

        let narrow = Rect {
            x: 0,
            y: 0,
            width: 60,
            height: 24,
        };
        assert_eq!(split(narrow, false).side_nav.width, 0);
        assert!(split(narrow, true).side_nav.width > 0);
        assert!(split(narrow, true).nav_overlay);
    }

    #[test]
    fn split_places_header_below_gap_when_tall() {
        let short = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        assert_eq!(split(short, true).header.y, 0);

        let tall = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 40,
        };
        let r = split(tall, true);
        assert_eq!(r.header.y, 1);
        assert_eq!(r.status.y, 38); // band at bottom: y 37 gap, 38 status, 39 gap
    }
}
