//! Per-frame hit map: mouse → same [`Action`](super::action::Action) as keys.

use ratatui::layout::Rect;
use wiki_reader_core::nav::NodeId;
use wiki_reader_core::provider::PageKey;

use super::action::Action;

/// Clickable target registered while drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hit {
    /// Side-nav page row.
    NavItem(NodeId),
    /// Side-nav group expand/collapse.
    NavGroupToggle(NodeId),
    /// Search… row above the tree.
    NavSearchRow,
    /// Breadcrumb segment (clickable landing).
    Breadcrumb(PageKey),
    /// Header ◫.
    NavToggle,
    /// Header ✕.
    Quit,
    /// Viewer footer prev.
    Prev,
    /// Viewer footer next.
    Next,
    /// Viewer body line (0-based source line index).
    ViewerLine(u32),
    /// Link segment (`LinkId.0`).
    Link(u32),
    /// Search overlay result row.
    SearchResult(usize),
    /// Click outside the search panel (dismiss).
    SearchDismiss,
    /// Click focuses the nav pane.
    FocusNav,
    /// Click focuses the viewer pane.
    FocusViewer,
    /// Tab bar label (index into `Navigator::tabs`).
    Tab(usize),
    /// Tab bar close glyph for tab index.
    TabClose(usize),
}

/// Rect → hit entries; searched in reverse so topmost wins.
#[derive(Debug, Default, Clone)]
pub struct HitMap {
    entries: Vec<(Rect, Hit)>,
}

impl HitMap {
    /// Clear for a new frame.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Register a hit region.
    pub fn push(&mut self, rect: Rect, hit: Hit) {
        if rect.width > 0 && rect.height > 0 {
            self.entries.push((rect, hit));
        }
    }

    /// Topmost hit at `(x, y)`, if any.
    #[must_use]
    pub fn hit_at(&self, x: u16, y: u16) -> Option<&Hit> {
        self.entries
            .iter()
            .rev()
            .find(|(r, _)| {
                x >= r.x
                    && x < r.x.saturating_add(r.width)
                    && y >= r.y
                    && y < r.y.saturating_add(r.height)
            })
            .map(|(_, h)| h)
    }

    /// Map a hit to an [`Action`].
    #[must_use]
    #[allow(clippy::match_same_arms)] // Focus* distinct for later
    pub fn action_for(hit: &Hit) -> Action {
        match hit {
            Hit::Quit => Action::Quit,
            Hit::NavToggle => Action::ToggleNav,
            Hit::Prev => Action::PrevPage,
            Hit::Next => Action::NextPage,
            Hit::Breadcrumb(key) => Action::GoToPage(key.clone()),
            Hit::NavItem(NodeId::Page(key)) => Action::GoToPage(key.clone()),
            Hit::NavItem(id) => Action::ToggleGroup(id.clone()),
            Hit::NavGroupToggle(id) => Action::ToggleGroup(id.clone()),
            Hit::NavSearchRow => Action::OpenSearch,
            Hit::ViewerLine(line) => Action::SetCursorLine(*line),
            Hit::Link(id) => Action::FollowLinkId(*id),
            Hit::SearchResult(i) => Action::SearchActivateIndex(*i),
            Hit::SearchDismiss => Action::CloseSearch,
            Hit::FocusNav => Action::FocusNav,
            Hit::FocusViewer => Action::FocusViewer,
            Hit::Tab(i) => Action::SwitchTab(*i),
            Hit::TabClose(_) => Action::CloseTab, // index applied in apply_mouse
        }
    }

    /// Entries (tests + wheel pane detection).
    #[must_use]
    pub fn entries(&self) -> &[(Rect, Hit)] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topmost_wins() {
        let mut map = HitMap::default();
        map.push(Rect::new(0, 0, 10, 10), Hit::Quit);
        map.push(Rect::new(2, 2, 2, 2), Hit::NavToggle);
        assert_eq!(map.hit_at(3, 3), Some(&Hit::NavToggle));
        assert_eq!(map.hit_at(0, 0), Some(&Hit::Quit));
        assert_eq!(map.hit_at(20, 20), None);
    }
}
