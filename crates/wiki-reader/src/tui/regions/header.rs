//! Header region (H1): breadcrumb + ⚙ / ◫ / ✕.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use wiki_reader_core::nav::Crumb;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::layout;
use crate::tui::regions::footer::ellipsis;
use crate::tui::theme::Theme;

/// Display columns for `s` (CJK/emoji-safe; matches ratatui cell width).
fn col_width(s: &str) -> u16 {
    u16::try_from(Span::raw(s).width()).unwrap_or(u16::MAX)
}

/// Draw the header and register breadcrumb / icon hits.
pub fn draw(frame: &mut Frame<'_>, area: Rect, crumbs: &[Crumb], theme: &Theme, hits: &mut HitMap) {
    let area = layout::chrome_pad(area);
    if area.width == 0 {
        return;
    }

    // Trailer: " ⚙ ◫ ✕" → 6 columns.
    let icon_w: u16 = 6;
    let trail_w = area.width.saturating_sub(icon_w);
    let trail = truncate_crumbs(crumbs, usize::from(trail_w));

    let mut spans = Vec::new();
    let mut x = area.x;
    let trail_end = area.x.saturating_add(trail_w);
    for (i, crumb) in trail.iter().enumerate() {
        let remaining = trail_end.saturating_sub(x);
        if remaining == 0 {
            break;
        }
        if i > 0 {
            let sep = " › ";
            if remaining <= col_width(sep) {
                spans.push(Span::styled("…", theme.muted()));
                break;
            }
            spans.push(Span::styled(sep, theme.muted()));
            x = x.saturating_add(col_width(sep));
        }
        // The last crumb is the current (read-only) page.
        let style = if i + 1 == trail.len() {
            theme.secondary()
        } else if crumb.target.is_some() {
            theme.accent()
        } else {
            theme.muted()
        };
        let remaining = trail_end.saturating_sub(x);
        // Reserve an ellipsis when more crumbs cannot fit after this label.
        let clipped = col_width(&crumb.label) > remaining
            || (i + 1 < trail.len() && col_width(&crumb.label) >= remaining);
        let label = if clipped {
            ellipsis(&format!("{}…", crumb.label), usize::from(remaining))
        } else {
            crumb.label.clone()
        };
        let w = col_width(&label);
        if let Some(key) = &crumb.target {
            hits.push(
                Rect {
                    x,
                    y: area.y,
                    width: w.max(1),
                    height: 1,
                },
                Hit::Breadcrumb(key.clone()),
            );
        }
        spans.push(Span::styled(label, style));
        x = x.saturating_add(w);
        if clipped {
            break;
        }
    }

    // Glyph columns within padded area (right-aligned): ⚙ ◫ ✕
    for (offset, hit) in [(5, Hit::OpenOptions), (3, Hit::NavToggle), (1, Hit::Quit)] {
        if area.width >= offset {
            hits.push(
                Rect {
                    x: area.right() - offset,
                    y: area.y,
                    width: 1,
                    height: 1,
                },
                hit,
            );
        }
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect {
            width: trail_w,
            ..area
        },
    );
    // Render independently: no breadcrumb can push controls off-screen.
    let icons = Rect {
        x: area.x.saturating_add(trail_w),
        width: area.width.min(icon_w),
        ..area
    };
    let controls: String = " ⚙ ◫ ✕"
        .chars()
        .skip(usize::from(icon_w - icons.width))
        .collect();
    frame.render_widget(Paragraph::new(controls).style(theme.accent()), icons);
}

/// Keep root and current; drop middle crumbs with `…` when over width.
fn truncate_crumbs(crumbs: &[Crumb], max_cols: usize) -> Vec<Crumb> {
    if crumbs.is_empty() || max_cols < 3 {
        return crumbs.to_vec();
    }
    let full: usize = crumbs
        .iter()
        .map(|c| usize::from(col_width(&c.label)))
        .sum::<usize>()
        + crumbs.len().saturating_sub(1) * usize::from(col_width(" › "));
    if full <= max_cols {
        return crumbs.to_vec();
    }
    if crumbs.len() <= 2 {
        return crumbs.to_vec();
    }
    let mut out = vec![crumbs[0].clone()];
    out.push(Crumb {
        label: "…".into(),
        target: None,
    });
    out.push(crumbs[crumbs.len() - 1].clone());
    out
}
