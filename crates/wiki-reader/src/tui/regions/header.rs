//! Header region (H1): breadcrumb + ◫ / ✕.

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
    if area.width == 0 || area.height == 0 {
        return;
    }

    let icon_w = area.width.min(layout::icon_trail_w());
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

    let glyphs = if area.width >= layout::icon_trail_w() {
        [("◫", Hit::NavToggle), ("✕", Hit::Quit)].as_slice()
    } else {
        [("✕", Hit::Quit)].as_slice()
    };
    frame.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect {
            width: trail_w,
            ..area
        },
    );
    // Render independently: no breadcrumb can push controls off-screen.
    for (rect, (glyph, hit)) in layout::icon_button_rects(area)
        .into_iter()
        .zip(glyphs.iter().cloned())
    {
        hits.push(rect, hit);
        let shown = if rect.width >= layout::ICON_W {
            layout::icon_button_label(glyph)
        } else {
            glyph.to_owned()
        };
        frame.render_widget(Paragraph::new(shown).style(theme.accent()), rect);
    }
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
    // Root … current
    let mut out = vec![crumbs[0].clone()];
    out.push(Crumb {
        label: "…".into(),
        target: None,
    });
    out.push(crumbs[crumbs.len() - 1].clone());
    out
}
