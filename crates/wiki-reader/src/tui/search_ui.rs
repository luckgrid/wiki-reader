//! Search overlay state (S1).

use wiki_reader_core::nav::NavStop;
use wiki_reader_core::search::{PageHit, TextHit};

use super::focus::FocusPane;

/// Pages vs full-text mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMode {
    /// Fuzzy title/path.
    #[default]
    Pages,
    /// Full-text body scan.
    Text,
}

/// Modal search panel.
#[derive(Debug, Clone)]
pub struct SearchOverlay {
    /// Query string.
    pub query: String,
    /// Pages / Text.
    pub mode: SearchMode,
    /// Selected result index.
    pub selected: usize,
    /// Page hits (when mode is Pages).
    pub page_hits: Vec<PageHit>,
    /// Text hits (when mode is Text).
    pub text_hits: Vec<TextHit>,
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
            SearchMode::Pages => self.page_hits.len(),
            SearchMode::Text => self.text_hits.len(),
        }
    }

    pub fn clamp_selected(&mut self) {
        let n = self.result_len();
        if n == 0 {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(n.saturating_sub(1));
        }
    }
}
