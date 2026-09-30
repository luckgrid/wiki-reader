//! Viewer cursor, scroll, and Tab-cycle state.

use super::App;
use crate::tui::action::Action;
use crate::tui::page_doc::PageDoc;
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

    pub(crate) fn viewer_heading(&mut self, dir: i32) {
        self.focused_item = None;
        let headings = self.doc.heading_lines();
        let cur = self.cursor_line.saturating_add(1);
        if dir < 0 {
            if let Some(h) = headings.iter().rev().find(|&&h| h < cur).copied() {
                self.cursor_line = h.saturating_sub(1);
            } else if let Some(&first) = headings.first() {
                self.cursor_line = first.saturating_sub(1);
            }
        } else if let Some(h) = headings.iter().find(|&&h| h > cur).copied() {
            self.cursor_line = h.saturating_sub(1);
        } else if let Some(&last) = headings.last() {
            self.cursor_line = last.saturating_sub(1);
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
                link_id: None,
            });
        }
        if tree.next(current).is_some() {
            items.push(FocusItem {
                line: None,
                cols: (0, 1),
                kind: FocusTarget::FooterNext,
                target: String::new(),
                link_id: None,
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

    pub(crate) fn viewer_activate(&mut self) {
        let items = self.focus_list();
        if let Some(i) = self.focused_item {
            let Some(it) = items.get(i).cloned() else {
                return;
            };
            match it.kind {
                FocusTarget::FooterPrev => self.update(Action::PrevPage),
                FocusTarget::FooterNext => self.update(Action::NextPage),
                FocusTarget::Link => self.follow_link_target(&it.target),
                FocusTarget::BlockAction => self.activate_block(&it.target),
            }
            return;
        }
        let line = self.cursor_line;
        let on_line: Vec<_> = self
            .doc
            .link_spans()
            .iter()
            .filter(|s| s.segments.iter().any(|(l, _)| *l == line))
            .collect();
        if on_line.len() == 1 {
            let raw = on_line[0].raw_target.clone();
            self.follow_link_target(&raw);
        }
    }

    fn activate_block(&mut self, target: &str) {
        let Some(id_str) = target.strip_prefix("block:") else {
            return;
        };
        let Ok(id) = id_str.parse::<u32>() else {
            return;
        };
        let PageDoc::Rendered(doc) = &self.doc else {
            return;
        };
        let Some(action) = doc.block_actions().iter().find(|a| a.id == id).cloned() else {
            return;
        };
        match action.kind {
            wiki_reader_render::BlockActionKind::CopyCode => {
                match self.clipboard.copy(&action.payload) {
                    Ok(()) => self.message = "copied".into(),
                    Err(err) => self.message = format!("copy failed: {err}"),
                }
            }
            wiki_reader_render::BlockActionKind::ToggleFrontmatter
            | wiki_reader_render::BlockActionKind::ToggleTable => {
                if !self.expanded_blocks.remove(&id) {
                    self.expanded_blocks.insert(id);
                }
                let key = self.navigator.tab().current().page.clone();
                let source = self.doc.source_cursor(self.cursor_line);
                let source_scroll = self.doc.source_cursor(self.scroll);
                self.reload_page_keeping_view(&key, source, source_scroll);
                // Keep focus on the same block id after re-layout.
                let items = self.focus_list();
                self.focused_item = items.iter().position(|it| {
                    it.kind == FocusTarget::BlockAction && it.target == format!("block:{id}")
                });
                if let Some(i) = self.focused_item
                    && let Some(line) = items[i].doc_line()
                {
                    self.cursor_line = line;
                    self.ensure_cursor_visible();
                }
            }
        }
    }

    pub(crate) fn follow_link_target(&mut self, raw: &str) {
        let effects = self.navigator.follow_link(raw, self.view_state());
        self.apply_effects(effects);
    }

    pub(crate) fn follow_link_id(&mut self, id: u32) {
        let Some(raw) = self
            .doc
            .link_target(wiki_reader_render::LinkId(id))
            .map(str::to_owned)
        else {
            return;
        };
        self.follow_link_target(&raw);
    }
}
