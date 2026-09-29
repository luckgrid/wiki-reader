//! Viewer cursor, scroll, and Tab-cycle state.

use super::App;
use crate::tui::viewer_doc::{FocusItem, FocusTarget, ViewerDoc, cycle};

impl App {
    pub(crate) fn viewer_move_line(&mut self, delta: i32) {
        self.focused_item = None;
        let max = i64::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(0);
        let cur = i64::from(self.cursor_line);
        let next = (cur + i64::from(delta)).clamp(0, max);
        self.cursor_line = u32::try_from(next).unwrap_or(0);
        self.ensure_cursor_visible();
    }

    pub(crate) fn viewer_block(&mut self, dir: i32) {
        self.focused_item = None;
        let blocks = self.doc.block_starts();
        // blocks are 1-based; cursor is 0-based
        let cur_src = self.cursor_line.saturating_add(1);
        if dir < 0 {
            let prev = blocks.iter().rev().find(|&&b| b < cur_src).copied();
            if let Some(b) = prev {
                self.cursor_line = b.saturating_sub(1);
            } else {
                self.cursor_line = 0;
            }
        } else {
            let next = blocks.iter().find(|&&b| b > cur_src).copied();
            if let Some(b) = next {
                self.cursor_line = b.saturating_sub(1);
            }
        }
        self.ensure_cursor_visible();
    }

    pub(crate) fn ensure_cursor_visible(&mut self) {
        let page_h = u32::from(self.viewer_rows.max(1));
        if self.cursor_line < self.scroll {
            self.scroll = self.cursor_line;
        } else if self.cursor_line >= self.scroll.saturating_add(page_h) {
            self.scroll = self.cursor_line.saturating_sub(page_h.saturating_sub(1));
        }
        self.clamp_viewer_scroll();
    }

    pub(crate) fn clamp_viewer_scroll(&mut self) {
        let max = u32::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(0);
        self.scroll = self.scroll.min(max);
    }

    pub(crate) fn focus_list(&self) -> Vec<FocusItem> {
        let mut items = self.doc.focus_items();
        let current = &self.navigator.tab().current().page;
        let tree = &self.navigator.nav().tree;
        if tree.prev(current).is_some() {
            items.push(FocusItem {
                line: None,
                cols: (0, 1),
                kind: FocusTarget::FooterPrev,
                target: String::new(),
            });
        }
        if tree.next(current).is_some() {
            items.push(FocusItem {
                line: None,
                cols: (0, 1),
                kind: FocusTarget::FooterNext,
                target: String::new(),
            });
        }
        items
    }

    pub(crate) fn viewer_tab(&mut self, backward: bool) {
        let items = self.focus_list();
        if self.focused_item.is_none() {
            self.focused_item = cycle::next_after(&items, self.cursor_line, backward);
        } else {
            self.focused_item = cycle::step(&items, self.focused_item, backward);
        }
        let Some(i) = self.focused_item else {
            return;
        };
        let Some(it) = items.get(i) else {
            return;
        };
        if let Some(line) = it.doc_line() {
            self.cursor_line = line;
            self.ensure_cursor_visible();
        }
        // Focused-item target is derived at draw time (not stored in `message`).
    }
}
