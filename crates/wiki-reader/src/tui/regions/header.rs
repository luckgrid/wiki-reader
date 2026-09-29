//! Header region (H1): breadcrumb + ◫ / ✕.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use wiki_reader_core::nav::Crumb;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;

/// Draw the header and register breadcrumb / icon hits.
pub fn draw(frame: &mut Frame<'_>, area: Rect, crumbs: &[Crumb], theme: &Theme, hits: &mut HitMap) {
    if area.width == 0 {
        return;
    }

    let icon_w: u16 = 5; // " ◫ ✕"
    let trail_w = area.width.saturating_sub(icon_w);
    let trail = truncate_crumbs(crumbs, usize::from(trail_w));

    let mut spans = Vec::new();
    let mut x = area.x;
    for (i, crumb) in trail.iter().enumerate() {
        if i > 0 {
            let sep = " › ";
            spans.push(Span::styled(sep, theme.muted()));
            x = x.saturating_add(u16::try_from(sep.len()).unwrap_or(0));
        }
        let style = if crumb.target.is_some() {
            theme.accent()
        } else {
            theme.muted()
        };
        let label = crumb.label.as_str();
        let w = u16::try_from(label.chars().count()).unwrap_or(u16::MAX);
        if let Some(ref key) = crumb.target {
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

    // Right icons.
    let toggle_x = area.x.saturating_add(area.width.saturating_sub(4));
    let quit_x = area.x.saturating_add(area.width.saturating_sub(2));
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
    let pad = area.width.saturating_sub(
        line_spans
            .iter()
            .map(|s| u16::try_from(s.content.chars().count()).unwrap_or(0))
            .sum::<u16>()
            .saturating_add(4),
    );
    if pad > 0 {
        line_spans.push(Span::raw(" ".repeat(usize::from(pad))));
    }
    line_spans.push(Span::styled(" ◫", theme.accent()));
    line_spans.push(Span::styled(" ✕", theme.accent()));

    frame.render_widget(Paragraph::new(Line::from(line_spans)), area);
}

/// Keep root and current; drop middle crumbs with `…` when over width.
fn truncate_crumbs(crumbs: &[Crumb], max_chars: usize) -> Vec<Crumb> {
    if crumbs.is_empty() || max_chars < 3 {
        return crumbs.to_vec();
    }
    let full: usize = crumbs
        .iter()
        .map(|c| c.label.chars().count())
        .sum::<usize>()
        + crumbs.len().saturating_sub(1) * 3; // " › "
    if full <= max_chars {
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
