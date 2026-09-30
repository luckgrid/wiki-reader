//! Tab bar above the viewer (≥2 tabs).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use wiki_reader_core::nav::Tab;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;

fn stem(tab: &Tab) -> String {
    tab.current()
        .page
        .relative_path
        .file_stem()
        .map_or_else(|| "?".into(), |s| s.to_string_lossy().into_owned())
}

/// Draw one-row tab bar; register [`Hit::Tab`] / [`Hit::TabClose`].
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    tabs: &[Tab],
    active: usize,
    theme: &Theme,
    hits: &mut HitMap,
) {
    if area.width == 0 || tabs.len() < 2 {
        return;
    }
    let mut spans = Vec::new();
    let mut x = area.x;
    for (i, tab) in tabs.iter().enumerate() {
        if i > 0 {
            let sep = "│";
            spans.push(Span::styled(sep, theme.muted()));
            x = x.saturating_add(1);
        }
        let label = stem(tab);
        let style = if i == active {
            theme.accent().add_modifier(ratatui::style::Modifier::BOLD)
        } else {
            theme.muted()
        };
        let w = u16::try_from(label.chars().count()).unwrap_or(1).max(1);
        hits.push(
            Rect {
                x,
                y: area.y,
                width: w,
                height: 1,
            },
            Hit::Tab(i),
        );
        spans.push(Span::styled(label, style));
        x = x.saturating_add(w);
        // Close glyph
        let cx = x;
        hits.push(
            Rect {
                x: cx,
                y: area.y,
                width: 1,
                height: 1,
            },
            Hit::TabClose(i),
        );
        spans.push(Span::styled("×", theme.muted()));
        x = x.saturating_add(1);
        if x >= area.x.saturating_add(area.width) {
            break;
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
