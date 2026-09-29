//! Side nav region (placeholder list until P1-07c tree widget).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use wiki_reader_core::nav::{NavItem, NavTree, NodeId};
use wiki_reader_core::provider::PageKey;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;

/// Draw search row + flat visible tree rows; register hits.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    tree: &NavTree,
    expanded: &std::collections::HashSet<NodeId>,
    current: &PageKey,
    cursor: Option<&NodeId>,
    focused: bool,
    theme: &Theme,
    hits: &mut HitMap,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    hits.push(area, Hit::FocusNav);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(focused))
        .title("Nav");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    // Search row.
    let search_rect = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    hits.push(search_rect, Hit::NavSearchRow);
    let search = Line::from(Span::styled("⌕ Search…", theme.muted()));

    let mut rows: Vec<(NodeId, String, bool)> = Vec::new();
    collect_visible(&tree.items, expanded, 0, &mut rows);

    let mut lines = vec![search];
    let max_rows = usize::from(inner.height.saturating_sub(1));
    for (i, (id, label, is_group)) in rows.into_iter().take(max_rows).enumerate() {
        let y = inner
            .y
            .saturating_add(1)
            .saturating_add(u16::try_from(i).unwrap_or(0));
        let rect = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        if is_group {
            hits.push(rect, Hit::NavGroupToggle(id.clone()));
        } else {
            hits.push(rect, Hit::NavItem(id.clone()));
        }
        let marker = match &id {
            NodeId::Page(k) if k == current => "● ",
            _ => "",
        };
        let cursor_mark = if cursor == Some(&id) { "▌" } else { " " };
        let style = if cursor == Some(&id) {
            theme.accent()
        } else {
            theme.text()
        };
        let text: String = format!("{cursor_mark}{marker}{label}")
            .chars()
            .take(usize::from(inner.width))
            .collect();
        lines.push(Line::from(Span::styled(text, style)));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn collect_visible(
    items: &[NavItem],
    expanded: &std::collections::HashSet<NodeId>,
    depth: usize,
    out: &mut Vec<(NodeId, String, bool)>,
) {
    let indent = "  ".repeat(depth);
    for item in items {
        match item {
            NavItem::Page { id, label, .. } => {
                out.push((id.clone(), format!("{indent}{label}"), false));
            }
            NavItem::Group {
                id,
                label,
                children,
            } => {
                let open = expanded.contains(id);
                let tri = if open { "▾ " } else { "▸ " };
                out.push((id.clone(), format!("{indent}{tri}{label}"), true));
                if open {
                    collect_visible(children, expanded, depth + 1, out);
                }
            }
        }
    }
}
