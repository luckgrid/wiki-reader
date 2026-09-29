//! Five-region layout with responsive side-nav width.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Computed region rectangles for one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Regions {
    /// Header row.
    pub header: Rect,
    /// Side nav (zero-sized when hidden).
    pub side_nav: Rect,
    /// Viewer body (above footer).
    pub viewer: Rect,
    /// Sticky viewer footer (prev/next).
    pub footer: Rect,
    /// Status bar.
    pub status: Rect,
    /// Whether the nav is an overlay (narrow terminal).
    pub nav_overlay: bool,
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
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(3),    // mid
            Constraint::Length(1), // status
        ])
        .split(area);

    let header = vert[0];
    let mid = vert[1];
    let status = vert[2];

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

    let viewer_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(viewer_col);

    Regions {
        header,
        side_nav,
        viewer: viewer_split[0],
        footer: viewer_split[1],
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
}
