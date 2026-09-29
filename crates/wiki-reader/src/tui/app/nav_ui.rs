//! Side-nav keyboard cursor and tree interactions.

use wiki_reader_core::nav::{NavStop, NodeId};
use wiki_reader_core::provider::PageKey;

use super::App;
use crate::tui::regions::side_nav;

impl App {
    pub(crate) fn nav_rows(&self) -> Vec<side_nav::NavRow> {
        side_nav::visible_rows(&self.navigator.nav().tree, &self.navigator.nav().expanded)
    }

    pub(crate) fn on_search(&self) -> bool {
        matches!(self.navigator.nav().cursor, NavStop::Search)
    }

    pub(crate) fn nav_cursor_index(&self, rows: &[side_nav::NavRow]) -> Option<usize> {
        match &self.navigator.nav().cursor {
            NavStop::Search => None,
            NavStop::Node(cur) => rows.iter().position(|r| &r.id == cur),
        }
    }

    pub(crate) fn nav_step(&mut self, dir: i32) {
        let rows = self.nav_rows();
        let len = rows.len() + 1;
        let cur = if self.on_search() {
            0usize
        } else {
            self.nav_cursor_index(&rows).map_or(1, |i| i + 1)
        };
        let next = if dir < 0 {
            cur.saturating_sub(1)
        } else {
            (cur + 1).min(len.saturating_sub(1))
        };
        if next == 0 {
            self.navigator.set_nav_stop(NavStop::Search);
        } else if let Some(row) = rows.get(next - 1) {
            self.navigator.set_nav_cursor(row.id.clone());
        }
        self.ensure_nav_cursor_visible();
    }

    pub(crate) fn nav_jump_group(&mut self, dir: i32) {
        let rows = self.nav_rows();
        let cur = if self.on_search() {
            0usize
        } else {
            self.nav_cursor_index(&rows).map_or(1, |i| i + 1)
        };
        if dir < 0 {
            if cur <= 1 {
                self.navigator.set_nav_stop(NavStop::Search);
            } else {
                let mut found = false;
                for pos in (1..cur).rev() {
                    if rows[pos - 1].is_group {
                        self.navigator.set_nav_cursor(rows[pos - 1].id.clone());
                        found = true;
                        break;
                    }
                }
                if !found {
                    self.navigator.set_nav_stop(NavStop::Search);
                }
            }
        } else {
            for (idx, row) in rows.iter().enumerate() {
                let pos = idx + 1;
                if pos <= cur {
                    continue;
                }
                if row.is_group {
                    self.navigator.set_nav_cursor(row.id.clone());
                    break;
                }
            }
        }
        self.ensure_nav_cursor_visible();
    }

    pub(crate) fn nav_expand(&mut self) {
        let NavStop::Node(id) = self.navigator.nav().cursor.clone() else {
            return;
        };
        if matches!(id, NodeId::Group(_) | NodeId::OtherPages) {
            self.navigator.set_group_expanded(id, true);
        }
    }

    pub(crate) fn nav_collapse(&mut self) {
        let NavStop::Node(id) = self.navigator.nav().cursor.clone() else {
            return;
        };
        match id {
            NodeId::Group(_) | NodeId::OtherPages => {
                if self.navigator.nav().expanded.contains(&id) {
                    self.navigator.set_group_expanded(id, false);
                } else if let Some(parent) = self.navigator.nav().tree.parent_group(&id) {
                    self.navigator.set_nav_cursor(parent);
                    self.ensure_nav_cursor_visible();
                }
            }
            NodeId::Page(key) => {
                if let Some(parent) = self.navigator.nav().tree.parent_group(&NodeId::Page(key)) {
                    self.navigator.set_nav_cursor(parent);
                    self.ensure_nav_cursor_visible();
                }
            }
        }
    }

    pub(crate) fn nav_activate(&mut self) {
        match self.navigator.nav().cursor.clone() {
            NavStop::Search => {
                self.message = "search: coming in P1-10".into();
            }
            NavStop::Node(NodeId::Page(key)) => {
                self.navigator.set_nav_cursor(NodeId::Page(key.clone()));
                let effects = self.navigator.go_to_page(key, self.view_state());
                self.apply_effects(effects);
            }
            NavStop::Node(other) => {
                let open = !self.navigator.nav().expanded.contains(&other);
                self.navigator.set_group_expanded(other, open);
            }
        }
    }

    pub(crate) fn reveal_page_in_nav(&mut self, page: &PageKey) {
        let rows = self.nav_rows();
        if let Some(idx) = rows.iter().position(|r| r.id == NodeId::Page(page.clone())) {
            self.scroll_nav_to_index(idx);
        }
    }

    pub(crate) fn ensure_nav_cursor_visible(&mut self) {
        if self.on_search() {
            self.nav_scroll = 0;
            return;
        }
        let rows = self.nav_rows();
        if let Some(idx) = self.nav_cursor_index(&rows) {
            self.scroll_nav_to_index(idx);
        }
    }

    pub(crate) fn scroll_nav_to_index(&mut self, idx: usize) {
        let vh = usize::from(self.nav_viewport.max(1));
        let scroll = usize::from(self.nav_scroll);
        if idx < scroll {
            self.nav_scroll = u16::try_from(idx).unwrap_or(0);
        } else if idx >= scroll.saturating_add(vh) {
            self.nav_scroll = u16::try_from(idx.saturating_add(1).saturating_sub(vh)).unwrap_or(0);
        }
        self.clamp_nav_scroll();
    }

    pub(crate) fn clamp_nav_scroll(&mut self) {
        let n = self.nav_rows().len();
        let vh = usize::from(self.nav_viewport.max(1));
        let max = n.saturating_sub(vh);
        self.nav_scroll = self.nav_scroll.min(u16::try_from(max).unwrap_or(0));
    }
}
