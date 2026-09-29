//! Markdown → `RenderedDoc` (styled lines, link spans, source map).

mod link_span;
mod render;

pub use link_span::{LinkClass, LinkId, LinkSpan};
pub use render::{RenderedDoc, StyleKind, StyledLine, StyledSpan, render};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::Path;
    use wiki_reader_core::provider::{CollectionProvider, FsProvider, PageKey};

    fn empty_key() -> PageKey {
        PageKey {
            collection_id: "t".into(),
            relative_path: Path::new("x.md").into(),
        }
    }

    fn render_src(src: &str, width: u16) -> RenderedDoc {
        let index = wiki_reader_core::Index {
            collection_id: "t".into(),
            pages: HashMap::default(),
            edges: vec![],
            by_from: HashMap::default(),
            by_to: HashMap::default(),
            by_id: HashMap::default(),
            by_path: HashMap::default(),
            diagnostics: vec![],
        };
        render(src, None, &empty_key(), &index, width)
    }

    #[test]
    fn renders_heading_line() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example");
        let provider = FsProvider::open(&root).unwrap();
        let index = wiki_reader_core::Index::build(&provider).unwrap();
        let key = PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: std::path::PathBuf::from("architecture/README.md"),
        };
        let src = provider.read(&key).unwrap();
        let page = index.pages.get(&key);
        let doc = render(&src, page, &key, &index, 80);
        assert!(doc.lines.iter().any(|l| l.contains("Architecture")));
        assert!(!doc.links.is_empty());
    }

    #[test]
    fn paragraphs_wrap_separately_not_glued() {
        let src = "First paragraph with enough words to wrap at forty columns here.\n\nSecond paragraph also wraps independently of the first one.\n";
        let doc = render_src(src, 40);
        assert!(
            doc.lines.len() >= 4,
            "expected multiple wrapped lines, got {:?}",
            doc.lines
        );
        // No single monster line from glued paragraphs
        assert!(
            doc.lines.iter().all(|l| l.chars().count() <= 45),
            "line too wide: {:?}",
            doc.lines
        );
        let joined = doc.lines.join("\n");
        assert!(joined.contains("First paragraph"));
        assert!(joined.contains("Second paragraph"));
    }

    #[test]
    fn list_items_not_concatenated() {
        let src = "- a\n- b\n- c\n";
        let doc = render_src(src, 40);
        let plain = doc.lines.join("|");
        assert!(!plain.contains("abc"), "list items glued: {plain:?}");
        assert!(
            doc.lines
                .iter()
                .filter(|l| l.contains('a') || l.contains('b') || l.contains('c'))
                .count()
                >= 3
        );
    }

    #[test]
    fn code_block_preserves_newlines() {
        let src = "```\nlet x = 1;\nlet y = 2;\n```\n";
        let doc = render_src(src, 40);
        let plain = doc.lines.join("\n");
        assert!(
            plain.contains("let x = 1;") && plain.contains("let y = 2;"),
            "{plain}"
        );
        assert!(
            !doc.lines.iter().any(|l| l.contains("let x = 1;let y")),
            "code lines merged: {:?}",
            doc.lines
        );
    }

    #[test]
    fn table_not_raw_pipes_only() {
        let src = "| a | b |\n| --- | --- |\n| 1 | 2 |\n";
        let doc = render_src(src, 40);
        let plain = doc.lines.join("\n");
        assert!(
            doc.lines.iter().any(|l| l.contains('│') || l.contains('a')),
            "expected table layout, got {plain}"
        );
    }

    #[test]
    fn source_map_monotonic_and_nonzero() {
        let src = "# Hello\n\nPara one.\n\nPara two.\n";
        let doc = render_src(src, 40);
        assert!(!doc.source_map.is_empty());
        assert!(doc.source_map.iter().all(|&s| s >= 1));
        for w in doc.source_map.windows(2) {
            assert!(
                w[1] >= w[0],
                "source_map not monotonic: {:?}",
                doc.source_map
            );
        }
    }

    #[test]
    fn anchor_line_is_rendered_index_in_range() {
        let src = "# Existing Heading\n\nBody text here.\n";
        let doc = render_src(src, 40);
        let (_, line) = doc
            .headings
            .iter()
            .find(|(s, _)| s == "existing-heading")
            .expect("heading");
        let idx = line.saturating_sub(1) as usize;
        assert!(
            idx < doc.lines.len(),
            "anchor {line} out of range for {} lines",
            doc.lines.len()
        );
        assert!(doc.lines[idx].to_lowercase().contains("existing"));
    }

    #[test]
    fn block_starts_are_rendered_lines() {
        let src = "Para.\n\n# Head\n\nMore.\n";
        let doc = render_src(src, 40);
        assert!(!doc.block_starts.is_empty());
        for &b in &doc.block_starts {
            assert!(b >= 1);
            assert!((b as usize) <= doc.lines.len().saturating_add(1));
        }
    }

    #[test]
    fn styled_lines_cover_headings() {
        let src = "# Title\n\n*em* **strong**\n";
        let doc = render_src(src, 80);
        assert!(
            doc.styled
                .iter()
                .flat_map(|l| &l.spans)
                .any(|s| matches!(s.kind, StyleKind::Heading(1)))
        );
    }

    #[test]
    fn wrapped_link_has_segments() {
        let src =
            "See [this is a fairly long link label that should wrap](https://example.com) end.\n";
        let doc = render_src(src, 30);
        assert!(!doc.links.is_empty());
        let segs = &doc.links[0].segments;
        assert!(!segs.is_empty());
    }
}
