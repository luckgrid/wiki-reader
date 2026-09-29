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
        // Last rendered line whose source <= target (row that contains it), then
        // rewind to the first wrap of that source so multi-line blocks stay at top.
        map.iter()
            .enumerate()
            .rev()
            .find(|(_, s)| **s <= source_1based)
            .map_or(0, |(i, &src)| {
                let start = map[..i]
                    .iter()
                    .rposition(|&s| s != src)
                    .map_or(0, |p| p.saturating_add(1));
                u32::try_from(start).unwrap_or(0)
            })
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

#[cfg(test)]
mod tests {
    use super::super::viewer_doc::ViewerDoc;
    use super::*;
    use std::collections::HashMap;
    use std::path::Path;
    use wiki_reader_core::provider::PageKey;

    fn build(src: &str, width: u16) -> RenderedViewerDoc {
        let index = Index {
            collection_id: "t".into(),
            pages: HashMap::default(),
            edges: vec![],
            by_from: HashMap::default(),
            by_to: HashMap::default(),
            by_id: HashMap::default(),
            by_path: HashMap::default(),
            diagnostics: vec![],
        };
        let key = PageKey {
            collection_id: "t".into(),
            relative_path: Path::new("x.md").into(),
        };
        RenderedViewerDoc::build(src, None, &key, &index, width)
    }

    #[test]
    fn last_table_row_source_lands_on_that_row() {
        let src = "| h |\n| --- |\n| a |\n| NEEDLE |\n";
        let doc = build(src, 40);
        let page = super::super::page_doc::PageDoc::Rendered(doc.clone());
        // Source line 4 (1-based) → 0-based source 3.
        let display = page.display_cursor(3);
        let lines = page.lines();
        let idx = usize::try_from(display).unwrap();
        assert!(
            lines[idx].contains("NEEDLE"),
            "display {display} should be NEEDLE row, got {:?}",
            lines.get(idx)
        );
        // Toggle width (re-layout) still maps to NEEDLE via source.
        let doc2 = build(src, 28);
        let page2 = super::super::page_doc::PageDoc::Rendered(doc2);
        let display2 = page2.display_cursor(3);
        assert!(
            page2.lines()[usize::try_from(display2).unwrap()].contains("NEEDLE"),
            "resize must keep source 3 on NEEDLE"
        );
        // Round-trip: display → source → display stays on NEEDLE.
        let src0 = page.source_cursor(display);
        assert_eq!(src0, 3);
        assert_eq!(page.display_cursor(src0), display);
    }

    #[test]
    fn rendered_for_source_last_leq_on_gap() {
        // Blank line inside a block: last row with source <= blank should win.
        let src = "line one\n\nline three\n";
        let doc = build(src, 40);
        // Source line 2 is blank; map should land on "line one" (source 1), not "line three".
        let display = doc.rendered_for_source(2);
        let line = &doc.lines()[usize::try_from(display).unwrap()];
        assert!(
            line.contains("line one"),
            "blank source should stay on prior row, got {line:?}"
        );
    }
}
