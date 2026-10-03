//! Viewer cursor, scroll, and Tab-cycle state.

use super::App;
use crate::tui::action::Action;
use crate::tui::focus::FocusPane;
use crate::tui::page_doc::PageDoc;
use crate::tui::text_col;
use crate::tui::viewer_doc::{FocusItem, FocusTarget, ViewerDoc, cycle};

impl App {
    /// Any cursor movement drops Tab focus, the footer stickiness and a selection.
    pub(crate) fn clear_item_focus(&mut self) {
        self.focused_item = None;
        self.sticky_footer = None;
        self.selection = None;
        self.selecting = false;
        self.pending_link = None;
    }

    fn cursor_line_text(&self) -> String {
        let i = usize::try_from(self.cursor_line).unwrap_or(usize::MAX);
        self.doc.lines().get(i).cloned().unwrap_or_default()
    }

    /// The View cursor's column, snapped onto the current row.
    pub(crate) fn effective_col(&self) -> u16 {
        text_col::clamp_col(&self.cursor_line_text(), self.cursor_col)
    }

    /// ←/→ in the View. At the edge toward the nav, hand focus over (P3-11).
    pub(crate) fn viewer_move_col(&mut self, dir: i32) {
        use wiki_reader_core::config::NavPosition;

        let text = self.cursor_line_text();
        let col = text_col::clamp_col(&text, self.cursor_col);
        if dir < 0 {
            let Some(left) = text_col::step_left(&text, col) else {
                if self.nav_position == NavPosition::Left {
                    // Reveal a hidden nav so focus has somewhere visible to land.
                    if !self.nav_visible {
                        self.nav_user_override = true;
                        self.nav_visible = true;
                    }
                    self.update(Action::FocusNav);
                }
                return;
            };
            self.clear_item_focus();
            self.cursor_col = left;
        } else {
            let right = text_col::step_right(&text, col);
            if right == col && self.nav_position == NavPosition::Right {
                if !self.nav_visible {
                    self.nav_user_override = true;
                    self.nav_visible = true;
                }
                self.update(Action::FocusNav);
                return;
            }
            self.clear_item_focus();
            self.cursor_col = right;
        }
    }

    /// Document cell under a screen position, clamped into the text. Dragging
    /// above/below the pane scrolls it one row so selections can extend.
    pub(crate) fn pos_at(&mut self, x: u16, y: u16) -> crate::tui::selection::Pos {
        let g = self.viewer_geom;
        if y < g.top_y {
            self.scroll = self.scroll.saturating_sub(1);
        } else if g.rows > 0 && y >= g.top_y.saturating_add(g.rows) {
            self.scroll = self.scroll.saturating_add(1);
            self.clamp_viewer_scroll();
        }
        let max = u32::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(0);
        let row = u32::from(y.saturating_sub(g.top_y).min(g.rows.saturating_sub(1)));
        let line = self.scroll.saturating_add(row).min(max);
        let text = self
            .doc
            .lines()
            .get(usize::try_from(line).unwrap_or(usize::MAX))
            .cloned()
            .unwrap_or_default();
        let col = text_col::clamp_col(&text, x.saturating_sub(g.text_x));
        crate::tui::selection::Pos { line, col }
    }

    pub(crate) fn select_start(&mut self, line: u32, col: u16) {
        if self.focus != FocusPane::Viewer {
            self.navigator.nav_focus_lost();
            self.focus = FocusPane::Viewer;
        }
        // The pressed link (if any) survives: it is followed on release.
        let pressed = self.pending_link;
        self.clear_item_focus();
        self.pending_link = pressed;
        let max = u32::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(0);
        self.cursor_line = line.min(max);
        self.cursor_col = col;
        let pos = crate::tui::selection::Pos {
            line: self.cursor_line,
            col: self.effective_col(),
        };
        self.selection = Some(crate::tui::selection::Selection {
            anchor: pos,
            head: pos,
        });
        self.selecting = true;
    }

    /// Idle tick during a drag: while the pointer rests above or below the pane,
    /// keep scrolling and extending the selection.
    pub(crate) fn drag_autoscroll(&mut self) {
        let Some((x, y)) = self.drag_at else {
            return;
        };
        let g = self.viewer_geom;
        if y >= g.top_y && y < g.top_y.saturating_add(g.rows) {
            return;
        }
        let pos = self.pos_at(x, y);
        self.select_extend(pos.line, pos.col);
    }

    pub(crate) fn select_extend(&mut self, line: u32, col: u16) {
        let Some(sel) = self.selection.as_mut() else {
            return;
        };
        sel.head = crate::tui::selection::Pos { line, col };
        self.cursor_line = line;
        self.cursor_col = col;
    }

    /// Finish a drag: copy a non-empty selection, or treat a bare click on a
    /// link as following it.
    pub(crate) fn select_end(&mut self) {
        self.selecting = false;
        self.drag_at = None;
        let link = self.pending_link.take();
        let Some(sel) = self.selection else {
            return;
        };
        if sel.is_empty() {
            self.selection = None;
            if let Some(id) = link {
                self.follow_link_id(id);
            }
            return;
        }
        let text = crate::tui::selection::extract(&self.doc, &sel, self.layout_width);
        if text.is_empty() {
            return;
        }
        let n = text.chars().count();
        match self.clipboard.copy(&text) {
            Ok(()) => self.message = format!("copied {n} chars (OSC 52)"),
            Err(err) => self.message = format!("copy failed: {err}"),
        }
    }

    pub(crate) fn restore_sticky_footer(&mut self) {
        let Some(want) = self.sticky_footer else {
            return;
        };
        let items = self.focus_list();
        let idx = items.iter().position(|it| it.kind == want).or_else(|| {
            let other = match want {
                FocusTarget::FooterPrev => FocusTarget::FooterNext,
                FocusTarget::FooterNext => FocusTarget::FooterPrev,
                _ => return None,
            };
            items.iter().position(|it| it.kind == other)
        });
        self.focused_item = idx;
    }

    pub(crate) fn focus_footer(&mut self) {
        use crate::tui::focus::FocusPane;
        if self.focus != FocusPane::Viewer {
            self.navigator.nav_focus_lost();
            self.focus = FocusPane::Viewer;
        }
        let items = self.focus_list();
        // Prefer next; on the last page only prev exists.
        let prefer = if items.iter().any(|it| it.kind == FocusTarget::FooterNext) {
            FocusTarget::FooterNext
        } else {
            FocusTarget::FooterPrev
        };
        self.focused_item = items.iter().position(|it| it.kind == prefer).or_else(|| {
            items
                .iter()
                .position(|it| matches!(it.kind, FocusTarget::FooterPrev | FocusTarget::FooterNext))
        });
    }

    pub(crate) fn viewer_move_line(&mut self, delta: i32) {
        self.clear_item_focus();
        let max = i64::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(0);
        let cur = i64::from(self.cursor_line);
        let next = (cur + i64::from(delta)).clamp(0, max);
        self.cursor_line = u32::try_from(next).unwrap_or(0);
        self.ensure_cursor_visible();
    }

    pub(crate) fn viewer_block(&mut self, dir: i32) {
        self.clear_item_focus();
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
        self.clear_item_focus();
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
            self.cursor_col = it.cols.0;
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
                FocusTarget::FooterPrev => {
                    self.sticky_footer = Some(FocusTarget::FooterPrev);
                    self.update(Action::PrevPage);
                }
                FocusTarget::FooterNext => {
                    self.sticky_footer = Some(FocusTarget::FooterNext);
                    self.update(Action::NextPage);
                }
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
            return;
        }
        // No link to follow: a block action (frontmatter toggle, copy) on this line.
        if let Some(it) = items
            .iter()
            .find(|it| it.kind == FocusTarget::BlockAction && it.line == Some(line))
        {
            self.activate_block(&it.target);
        }
    }

    pub(crate) fn activate_block(&mut self, target: &str) {
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
                    Ok(()) => self.message = "sent to clipboard (OSC 52)".into(),
                    Err(err) => self.message = format!("copy failed: {err}"),
                }
            }
            wiki_reader_render::BlockActionKind::ToggleFrontmatter => {
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
