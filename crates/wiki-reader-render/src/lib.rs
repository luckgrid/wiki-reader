//! Markdown → `RenderedDoc` (lines, link spans, source map).

mod link_span;
mod render;

pub use link_span::{LinkClass, LinkId, LinkSpan};
pub use render::{RenderedDoc, render};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use wiki_reader_core::provider::{CollectionProvider, FsProvider};

    #[test]
    fn renders_heading_line() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example");
        let provider = FsProvider::open(&root).unwrap();
        let index = wiki_reader_core::Index::build(&provider).unwrap();
        let key = wiki_reader_core::provider::PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: std::path::PathBuf::from("architecture/README.md"),
        };
        let src = provider.read(&key).unwrap();
        let page = index.pages.get(&key);
        let doc = render(&src, page, &key, &index, 80);
        assert!(doc.lines.iter().any(|l| l.contains("Architecture")));
        assert!(!doc.links.is_empty());
    }
}
