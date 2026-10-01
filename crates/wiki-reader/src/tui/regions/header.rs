//! Header region (H1): breadcrumb + ○/◉ (syntax/formatted) / ◫ / ✕.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use wiki_reader_core::nav::Crumb;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::layout;
use crate::tui::theme::Theme;

/// Display columns for `s` (CJK/emoji-safe; matches ratatui cell width).
fn col_width(s: &str) -> u16 {
    u16::try_from(Span::raw(s).width()).unwrap_or(u16::MAX)
}

/// Draw the header and register breadcrumb / icon hits.
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    crumbs: &[Crumb],
    formatted: bool,
    theme: &Theme,
    hits: &mut HitMap,
) {
    let area = layout::chrome_pad(area);
    if area.width == 0 {
        return;
    }

    // Trailer: " ○ ◫ ✕" → 6 columns.
    let icon_w: u16 = 6;
    let trail_w = area.width.saturating_sub(icon_w);
    let trail = truncate_crumbs(crumbs, usize::from(trail_w));

    let mut spans = Vec::new();
    let mut x = area.x;
    for (i, crumb) in trail.iter().enumerate() {
        if i > 0 {
            let sep = " › ";
            spans.push(Span::styled(sep, theme.muted()));
            x = x.saturating_add(col_width(sep));
        }
        let style = if crumb.target.is_some() {
            theme.accent()
        } else {
            theme.muted()
        };
        let label = &crumb.label;
        let w = col_width(label).max(1);
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
        spans.push(Span::styled(label.to_owned(), style));
        x = x.saturating_add(w);
    }

    // Glyph columns within padded area (right-aligned): ○/◉ ◫ ✕
    let eye_x = area.x.saturating_add(area.width.saturating_sub(5));
    let toggle_x = area.x.saturating_add(area.width.saturating_sub(3));
    let quit_x = area.x.saturating_add(area.width.saturating_sub(1));
    hits.push(
        Rect {
            x: eye_x,
            y: area.y,
            width: 1,
            height: 1,
        },
        Hit::ViewToggle,
    );
    hits.push(
        Rect {
            x: toggle_x,
            y: area.y,
            width: 1,
            height: 1,
        },
        Hit::NavToggle,
    );
    hits.push(
        Rect {
            x: quit_x,
            y: area.y,
            width: 1,
            height: 1,
        },
        Hit::Quit,
    );

    let mut line_spans = spans;
    let trail_cols: u16 = line_spans.iter().map(|s| col_width(&s.content)).sum();
    let pad = area.width.saturating_sub(trail_cols.saturating_add(icon_w));
    if pad > 0 {
        line_spans.push(Span::raw(" ".repeat(usize::from(pad))));
    }
    // ◉ = formatted on; ○ = syntax (markers visible).
    let eye = if formatted { " ◉" } else { " ○" };
    line_spans.push(Span::styled(eye, theme.accent()));
    line_spans.push(Span::styled(" ◫", theme.accent()));
    line_spans.push(Span::styled(" ✕", theme.accent()));

    frame.render_widget(Paragraph::new(Line::from(line_spans)), area);
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
