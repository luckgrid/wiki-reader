//! Search overlay state (S1).

use wiki_reader_core::nav::NavStop;
use wiki_reader_core::search::{PageHit, TextHit};

use super::focus::FocusPane;
use super::regions::overlay::{clamp_scroll, ensure_visible};

/// Files vs full-text content mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMode {
    /// Fuzzy title/path (Files).
    #[default]
    Files,
    /// Full-text body scan (Content).
    Content,
}

/// Modal search panel.
#[derive(Debug, Clone)]
pub struct SearchOverlay {
    /// Query string.
    pub query: String,
    /// Files / Content.
    pub mode: SearchMode,
    /// Selected result index (absolute).
    pub selected: usize,
    /// List scroll offset.
    pub scroll: usize,
    /// Visible list rows from last draw (for PgUp/PgDn).
    pub list_height: usize,
    /// Page hits (when mode is Files).
    pub page_hits: Vec<PageHit>,
    /// Text hits (when mode is Content).
    pub text_hits: Vec<TextHit>,
    /// Distinct pages in `text_hits`, counted once per refresh (not per frame).
    pub text_files: usize,
    /// Focus before open (restored on Esc).
    pub prev_focus: FocusPane,
    /// Viewer cursor before open.
    pub prev_cursor: u32,
    /// Viewer scroll before open.
    pub prev_scroll: u32,
    /// Nav stop before open.
    pub prev_nav_stop: NavStop,
}

impl SearchOverlay {
    #[must_use]
    pub fn result_len(&self) -> usize {
        match self.mode {
            SearchMode::Files => self.page_hits.len(),
            SearchMode::Content => self.text_hits.len(),
        }
    }

    pub fn clamp_selected(&mut self) {
        let n = self.result_len();
        if n == 0 {
            self.selected = 0;
            self.scroll = 0;
        } else {
            self.selected = self.selected.min(n.saturating_sub(1));
            self.ensure_selection_visible();
        }
    }

    pub fn ensure_selection_visible(&mut self) {
        let visible = self.list_height.max(1);
        self.scroll = ensure_visible(self.selected, self.scroll, visible);
        self.scroll = clamp_scroll(self.scroll, visible, self.result_len());
    }
}
