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
}
