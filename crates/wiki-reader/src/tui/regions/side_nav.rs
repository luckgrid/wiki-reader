//! Side nav: flat visible rows into our `HitMap` (ADR-0009 option B).
//!
//! `tui-tree-widget` was evaluated; syncing `TreeState` from core `NavState` each
//! frame duplicated the source of truth while our hit map already owns row rects.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use wiki_reader_core::nav::{NavItem, NavTree, NodeId};
use wiki_reader_core::provider::PageKey;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;

/// One visible side-nav row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavRow {
    /// Node id (page or group).
    pub id: NodeId,
    /// Display label (with indent / triangle).
    pub label: String,
    /// True when this row is a collapsible group header.
    pub is_group: bool,
}

/// Flatten the tree to currently visible rows (respecting `expanded`).
#[must_use]
pub fn visible_rows(tree: &NavTree, expanded: &std::collections::HashSet<NodeId>) -> Vec<NavRow> {
    let mut rows = Vec::new();
    collect_visible(&tree.items, expanded, 0, &mut rows);
    rows
}

/// Draw search row + flat visible tree rows; register hits.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    tree: &NavTree,
    expanded: &std::collections::HashSet<NodeId>,
    current: &PageKey,
    cursor: Option<&NodeId>,
    scroll: u16,
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

    let search_rect = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    hits.push(search_rect, Hit::NavSearchRow);
    let search = Line::from(Span::styled("⌕ Search…", theme.muted()));

    let rows = visible_rows(tree, expanded);
    let scroll = usize::from(scroll);
    let mut lines = vec![search];
    let max_rows = usize::from(inner.height.saturating_sub(1));
    for (i, row) in rows.into_iter().skip(scroll).take(max_rows).enumerate() {
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
        if row.is_group {
            hits.push(rect, Hit::NavGroupToggle(row.id.clone()));
        } else {
            hits.push(rect, Hit::NavItem(row.id.clone()));
        }
        let marker = match &row.id {
            NodeId::Page(k) if k == current => "● ",
            _ => "",
        };
        let cursor_mark = if cursor == Some(&row.id) {
            "▌"
        } else {
            " "
        };
        let style = if cursor == Some(&row.id) {
            theme.accent()
        } else {
            theme.text()
        };
        let text: String = format!("{cursor_mark}{marker}{}", row.label)
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
    out: &mut Vec<NavRow>,
) {
    let indent = "  ".repeat(depth);
    for item in items {
        match item {
            NavItem::Page { id, label, .. } => {
                out.push(NavRow {
                    id: id.clone(),
                    label: format!("{indent}{label}"),
                    is_group: false,
                });
            }
            NavItem::Group {
                id,
                label,
                children,
            } => {
                let open = expanded.contains(id);
                let tri = if open { "▾ " } else { "▸ " };
                out.push(NavRow {
                    id: id.clone(),
                    label: format!("{indent}{tri}{label}"),
                    is_group: true,
                });
                if open {
                    collect_visible(children, expanded, depth + 1, out);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use wiki_reader_core::Index;
    use wiki_reader_core::provider::FsProvider;

    fn worked_tree() -> NavTree {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example");
        let index = Index::build(&FsProvider::open(&root).unwrap()).unwrap();
        NavTree::build(&index)
    }

    #[test]
    fn visible_rows_collapsed_and_expanded() {
        let tree = worked_tree();
        let collapsed = visible_rows(&tree, &HashSet::new());
        assert!(collapsed.iter().any(|r| r.label.contains("Architecture")));
        assert!(!collapsed.iter().any(|r| r.label.contains("Token")));

        let mut expanded = HashSet::new();
        expanded.insert(NodeId::Group(PathBuf::from("architecture")));
        expanded.insert(NodeId::Group(PathBuf::from("architecture/design-system")));
        let open = visible_rows(&tree, &expanded);
        assert!(open.iter().any(|r| r.label.contains("Token")));
        insta::assert_debug_snapshot!(
            "worked_nav_expanded",
            open.iter().map(|r| &r.label).collect::<Vec<_>>()
        );
    }
}
