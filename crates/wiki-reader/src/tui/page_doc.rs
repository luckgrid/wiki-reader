//! Active page document (raw or rendered).

use super::rendered_doc::RenderedViewerDoc;
use super::viewer_doc::{RawDoc, ViewerDoc};

/// Either raw source or laid-out rendered lines.
#[derive(Debug, Clone)]
pub enum PageDoc {
    /// Interim raw lines.
    Raw(RawDoc),
    /// Wrapped rendered layout.
    Rendered(RenderedViewerDoc),
}

impl PageDoc {
    #[must_use]
    pub fn word_count(&self) -> u32 {
        match self {
            Self::Raw(d) => d.word_count(),
            Self::Rendered(d) => d.word_count(),
        }
    }

    #[must_use]
    pub fn updated(&self) -> &str {
        match self {
            Self::Raw(d) => d.updated(),
            Self::Rendered(d) => d.updated(),
        }
    }

    #[must_use]
    pub fn link_target(&self, id: wiki_reader_render::LinkId) -> Option<&str> {
        match self {
            Self::Raw(d) => d.link_target(id),
            Self::Rendered(d) => d.link_target(id),
        }
    }

    /// 0-based source line for a 0-based display cursor.
    #[must_use]
    pub fn source_cursor(&self, display: u32) -> u32 {
        match self {
            Self::Raw(d) => d.source_line_of_row(display),
            Self::Rendered(d) => d.source_line_for_rendered(display).saturating_sub(1),
        }
    }

    /// 0-based display cursor for a 0-based source line.
    #[must_use]
    pub fn display_cursor(&self, source: u32) -> u32 {
        match self {
            Self::Raw(d) => d.first_row_of(source),
            Self::Rendered(d) => d.rendered_for_source(source.saturating_add(1)),
        }
    }
}

impl ViewerDoc for PageDoc {
    fn lines(&self) -> &[String] {
        match self {
            Self::Raw(d) => d.lines(),
            Self::Rendered(d) => d.lines(),
        }
    }

    fn block_starts(&self) -> &[u32] {
        match self {
            Self::Raw(d) => d.block_starts(),
            Self::Rendered(d) => d.block_starts(),
        }
    }

    fn heading_lines(&self) -> Vec<u32> {
        match self {
            Self::Raw(d) => d.heading_lines(),
            Self::Rendered(d) => d.heading_lines(),
        }
    }

    fn anchor_line(&self, slug: &str) -> Option<u32> {
        match self {
            Self::Raw(d) => d.anchor_line(slug),
            Self::Rendered(d) => d.anchor_line(slug),
        }
    }

    fn link_spans(&self) -> &[wiki_reader_render::LinkSpan] {
        match self {
            Self::Raw(d) => d.link_spans(),
            Self::Rendered(d) => d.link_spans(),
        }
    }

    fn styled_lines(&self) -> Option<&[wiki_reader_render::StyledLine]> {
        match self {
            Self::Raw(_) => None,
            Self::Rendered(d) => d.styled_lines(),
        }
    }

    fn block_focus_items(&self) -> Vec<super::viewer_doc::FocusItem> {
        match self {
            Self::Raw(d) => d.block_focus_items(),
            Self::Rendered(d) => d.block_focus_items(),
        }
    }
}
