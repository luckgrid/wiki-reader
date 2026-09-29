//! `RenderedDoc` adapter implementing [`ViewerDoc`](super::viewer_doc::ViewerDoc).

use wiki_reader_core::Index;
use wiki_reader_core::provider::PageKey;
use wiki_reader_render::{RenderedDoc as Inner, render};

use super::viewer_doc::ViewerDoc;

/// Rendered markdown at a fixed width.
#[derive(Debug, Clone)]
pub struct RenderedViewerDoc {
    inner: Inner,
    #[allow(dead_code)] // P1-11 cursor mapping uses this next
    width: u16,
}

#[allow(dead_code)]
impl RenderedViewerDoc {
    /// Layout `source` for `from` at `width`.
    #[must_use]
    pub fn build(
        source: &str,
        page: Option<&wiki_reader_core::index::Page>,
        from: &PageKey,
        index: &Index,
        width: u16,
    ) -> Self {
        Self {
            inner: render(source, page, from, index, width),
            width,
        }
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn width(&self) -> u16 {
        self.width
    }

    #[must_use]
    pub fn word_count(&self) -> u32 {
        self.inner.word_count
    }

    #[must_use]
    pub fn updated(&self) -> &str {
        &self.inner.updated
    }

    #[must_use]
    pub fn link_target(&self, id: wiki_reader_render::LinkId) -> Option<&str> {
        self.inner
            .links
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.raw_target.as_str())
    }

    /// Source line (1-based) for rendered cursor line (0-based).
    #[must_use]
    pub fn source_line_for_rendered(&self, rendered_line: u32) -> u32 {
        self.inner
            .source_map
            .get(usize::try_from(rendered_line).unwrap_or(0))
            .copied()
            .unwrap_or(1)
    }

    /// Nearest rendered line (0-based) for a 1-based source line.
    #[must_use]
    pub fn rendered_for_source(&self, source_1based: u32) -> u32 {
        let map = &self.inner.source_map;
        if map.is_empty() {
            return 0;
        }
        // First rendered line whose source >= target; else last.
        map.iter()
            .enumerate()
            .find(|(_, s)| **s >= source_1based)
            .map_or_else(
                || u32::try_from(map.len().saturating_sub(1)).unwrap_or(0),
                |(i, _)| u32::try_from(i).unwrap_or(0),
            )
    }
}

impl ViewerDoc for RenderedViewerDoc {
    fn lines(&self) -> &[String] {
        &self.inner.lines
    }

    fn block_starts(&self) -> &[u32] {
        &self.inner.block_starts
    }

    fn anchor_line(&self, slug: &str) -> Option<u32> {
        self.inner
            .headings
            .iter()
            .find(|(s, _)| s == slug)
            .map(|(_, line)| *line)
    }

    fn link_spans(&self) -> &[wiki_reader_render::LinkSpan] {
        &self.inner.links
    }
}
