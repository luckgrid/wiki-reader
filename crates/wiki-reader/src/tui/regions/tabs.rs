//! Tab bar above the viewer (≥2 tabs).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
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

fn col_width(s: &str) -> u16 {
    u16::try_from(Span::raw(s).width()).unwrap_or(u16::MAX)
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
        let lw = col_width(&label).max(1);
        let bg = if i == active {
            theme.tab_active
        } else {
            theme.tab_inactive
        };
        let fg_style = if i == active {
            theme.accent().add_modifier(Modifier::BOLD)
        } else {
            theme.muted()
        };
        let chip = Style {
            bg: Some(bg),
            ..fg_style
        };
        // 1-col pad each side of the label; Tab hit covers the padded label.
        let tab_w = lw.saturating_add(2);
        hits.push(
            Rect {
                x,
                y: area.y,
                width: tab_w,
                height: 1,
            },
            Hit::Tab(i),
        );
        spans.push(Span::styled(" ", Style::default().bg(bg)));
        spans.push(Span::styled(label, chip));
        spans.push(Span::styled(" ", Style::default().bg(bg)));
        x = x.saturating_add(tab_w);
        hits.push(
            Rect {
                x,
                y: area.y,
                width: 1,
                height: 1,
            },
            Hit::TabClose(i),
        );
        spans.push(Span::styled("×", theme.muted().bg(bg)));
        x = x.saturating_add(1);
        if x >= area.x.saturating_add(area.width) {
            break;
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
