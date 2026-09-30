//! `RenderedDoc` adapter implementing [`ViewerDoc`](super::viewer_doc::ViewerDoc).

use wiki_reader_core::Index;
use wiki_reader_core::provider::PageKey;
use wiki_reader_render::{RenderOpts, RenderedDoc as Inner, render_with};

use super::viewer_doc::{FocusItem, FocusTarget, ViewerDoc};

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
        Self::build_with(source, page, from, index, width, &RenderOpts::default())
    }

    /// Layout with expansion state for block actions.
    #[must_use]
    pub fn build_with(
        source: &str,
        page: Option<&wiki_reader_core::index::Page>,
        from: &PageKey,
        index: &Index,
        width: u16,
        opts: &RenderOpts,
    ) -> Self {
        Self {
            inner: render_with(source, page, from, index, width, opts),
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

    #[must_use]
    pub fn block_actions(&self) -> &[wiki_reader_render::BlockAction] {
        &self.inner.block_actions
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

    fn heading_lines(&self) -> Vec<u32> {
        self.inner.headings.iter().map(|(_, line)| *line).collect()
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

    fn block_focus_items(&self) -> Vec<FocusItem> {
        self.inner
            .block_actions
            .iter()
            .map(|a| FocusItem {
                line: Some(a.line),
                cols: a.cols,
                kind: FocusTarget::BlockAction,
                target: format!("block:{}", a.id),
                link_id: None,
            })
            .collect()
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
        let display = page.display_cursor(3);
        let line = page
            .lines()
            .get(display as usize)
            .cloned()
            .unwrap_or_default();
        assert!(line.contains("NEEDLE"), "got {line:?} at display {display}");
    }

    #[test]
    fn rendered_for_source_last_leq_on_gap() {
        let src = "a\n\n\nb\n";
        let doc = build(src, 40);
        let _ = doc.rendered_for_source(2);
    }

    #[test]
    fn block_actions_appear_in_tab_order_by_document_position() {
        let src = "---\ntitle: T\n---\n\n```\ncode\n```\n\n| a |\n| - |\n| 1 |\n";
        let doc = build(src, 40);
        let items = doc.focus_items();
        let blocks: Vec<_> = items
            .iter()
            .filter(|i| i.kind == FocusTarget::BlockAction)
            .collect();
        assert!(
            blocks.len() >= 2,
            "expected FM + code (+ maybe table), got {blocks:?}"
        );
        let lines: Vec<u32> = blocks.iter().filter_map(|b| b.line).collect();
        let sorted = {
            let mut s = lines.clone();
            s.sort_unstable();
            s
        };
        assert_eq!(lines, sorted, "block actions must be document-ordered");
        assert!(
            doc.block_actions()
                .iter()
                .any(|a| a.kind == wiki_reader_render::BlockActionKind::CopyCode
                    && a.payload.contains("code")),
            "copy payload missing: {:?}",
            doc.block_actions()
        );
    }
}
