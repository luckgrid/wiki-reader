//! Interim viewer document seam (raw lines now; `RenderedDoc` in P1-13).

use wiki_reader_core::Index;
use wiki_reader_core::index::Page;
use wiki_reader_core::nav::{Target, resolve, unresolved_relative_path};
use wiki_reader_core::parse::{self, MdLink};
use wiki_reader_core::provider::{FsProvider, PageKey};
use wiki_reader_render::{LinkClass, LinkId, LinkSpan};

/// Kind of a Tab-cycle focus item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    /// Inline markdown link.
    Link,
    /// Viewer footer prev.
    FooterPrev,
    /// Viewer footer next.
    FooterNext,
}

/// One Tab-cycle target in the viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusItem {
    /// 0-based source line; `None` for footer buttons.
    pub line: Option<u32>,
    /// Display-column range on that line `[start, end)`.
    pub cols: (u16, u16),
    /// Item kind.
    pub kind: FocusTarget,
    /// Raw target string (path, url, or empty for footer).
    pub target: String,
    /// Link id when `kind == Link`.
    pub link_id: Option<LinkId>,
}

impl FocusItem {
    /// Document line for links; `None` for footer buttons.
    #[must_use]
    pub fn doc_line(&self) -> Option<u32> {
        match self.kind {
            FocusTarget::Link => self.line,
            FocusTarget::FooterPrev | FocusTarget::FooterNext => None,
        }
    }
}

/// Document view used by the viewer pane.
pub trait ViewerDoc {
    /// Source (or rendered) lines.
    fn lines(&self) -> &[String];
    /// Top-level block start lines (1-based source).
    fn block_starts(&self) -> &[u32];
    /// Heading display lines (1-based), document order.
    fn heading_lines(&self) -> Vec<u32>;
    /// Line (1-based) for a heading slug.
    fn anchor_line(&self, slug: &str) -> Option<u32>;
    /// Interactive link geometry.
    fn link_spans(&self) -> &[LinkSpan];
    /// Tab-cycle items (links + footer slots filled by the app).
    fn focus_items(&self) -> Vec<FocusItem> {
        self.link_spans()
            .iter()
            .map(|s| {
                let (line, cols) = s.segments.first().copied().unwrap_or((0, (0, 0)));
                FocusItem {
                    line: Some(line),
                    cols,
                    kind: FocusTarget::Link,
                    target: s.raw_target.clone(),
                    link_id: Some(s.id),
                }
            })
            .collect()
    }
}

/// Raw source document backed by provider text + parsed metadata.
#[derive(Debug, Clone)]
pub struct RawDoc {
    lines: Vec<String>,
    blocks: Vec<u32>,
    headings: Vec<(String, u32)>,
    links: Vec<LinkSpan>,
    word_count: u32,
    updated: String,
    /// Syntect highlight runs per source line (parallel to `lines`).
    pub highlights: Vec<Vec<crate::tui::highlight::HlSpan>>,
}

impl RawDoc {
    /// Build from raw source and the indexed page (if any).
    #[must_use]
    pub fn from_source(source: &str, page: Option<&Page>) -> Self {
        Self::from_source_ctx(source, page, None, None, None)
    }

    /// Build with link resolution context (normal app load path).
    #[must_use]
    pub fn from_source_ctx(
        source: &str,
        page: Option<&Page>,
        from: Option<&PageKey>,
        index: Option<&Index>,
        provider: Option<&FsProvider>,
    ) -> Self {
        let lines: Vec<String> = source.lines().map(str::to_owned).collect();
        let (blocks, headings, md_links, word_count, updated) = if let Some(p) = page {
            (
                p.parsed.blocks.clone(),
                p.parsed
                    .headings
                    .iter()
                    .map(|h| (h.slug.clone(), h.source_line))
                    .collect(),
                p.parsed.links.clone(),
                p.parsed.word_count,
                p.parsed
                    .frontmatter
                    .updated
                    .clone()
                    .unwrap_or_else(|| "—".into()),
            )
        } else {
            let parsed = parse::parse(source);
            (
                parsed.blocks,
                parsed
                    .headings
                    .iter()
                    .map(|h| (h.slug.clone(), h.source_line))
                    .collect(),
                parsed.links,
                parsed.word_count,
                parsed
                    .frontmatter
                    .updated
                    .clone()
                    .unwrap_or_else(|| "—".into()),
            )
        };
        let link_spans = link_spans_from_md(&lines, &md_links, from, index, provider);
        // ponytail: highlights filled async by App; draw falls back to plain until then
        Self {
            lines,
            blocks,
            headings,
            links: link_spans,
            word_count,
            updated,
            highlights: Vec::new(),
        }
    }

    /// Replace syntect highlights (from a background job).
    pub fn set_highlights(&mut self, highlights: Vec<Vec<crate::tui::highlight::HlSpan>>) {
        self.highlights = highlights;
    }

    /// Raw target for a link id.
    #[must_use]
    pub fn link_target(&self, id: LinkId) -> Option<&str> {
        self.links
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.raw_target.as_str())
    }

    /// Word count for the status bar.
    #[must_use]
    pub fn word_count(&self) -> u32 {
        self.word_count
    }

    /// Updated frontmatter or "—".
    #[must_use]
    pub fn updated(&self) -> &str {
        &self.updated
    }
}

impl ViewerDoc for RawDoc {
    fn lines(&self) -> &[String] {
        &self.lines
    }

    fn block_starts(&self) -> &[u32] {
        &self.blocks
    }

    fn heading_lines(&self) -> Vec<u32> {
        self.headings.iter().map(|(_, line)| *line).collect()
    }

    fn anchor_line(&self, slug: &str) -> Option<u32> {
        self.headings
            .iter()
            .find(|(s, _)| s == slug)
            .map(|(_, line)| *line)
    }

    fn link_spans(&self) -> &[LinkSpan] {
        &self.links
    }
}

fn link_spans_from_md(
    lines: &[String],
    md_links: &[MdLink],
    from: Option<&PageKey>,
    index: Option<&Index>,
    _provider: Option<&FsProvider>,
) -> Vec<LinkSpan> {
    md_links
        .iter()
        .enumerate()
        .map(|(i, md)| {
            let class = match (from, index) {
                (Some(f), Some(idx)) => link_class(&md.target, f, idx),
                _ => LinkClass::Internal,
            };
            let line_idx = usize::try_from(md.source_line.saturating_sub(1)).unwrap_or(0);
            let cols = match lines.get(line_idx) {
                Some(line) => {
                    let needle = format!("[{}]({})", md.text, md.target);
                    if let Some(start) = line.find(&needle) {
                        let end = start + needle.len();
                        (display_col_at(line, start), display_col_at(line, end))
                    } else {
                        (
                            0,
                            u16::try_from(ratatui::text::Span::raw(line.as_str()).width())
                                .unwrap_or(u16::MAX),
                        )
                    }
                }
                None => (0, 0),
            };
            LinkSpan {
                id: LinkId(u32::try_from(i).unwrap_or(u32::MAX)),
                raw_target: md.target.clone(),
                class,
                segments: vec![(md.source_line.saturating_sub(1), cols)],
            }
        })
        .collect()
}

fn link_class(raw: &str, from: &PageKey, index: &Index) -> LinkClass {
    let outcome = resolve(raw, from, index);
    match outcome.target {
        Target::External(_) => LinkClass::External,
        Target::Unresolved(_) => LinkClass::Broken,
        _ => LinkClass::Internal,
    }
}

fn display_col_at(line: &str, byte_offset: usize) -> u16 {
    let mut col = 0u16;
    let mut i = 0usize;
    for ch in line.chars() {
        if i >= byte_offset {
            break;
        }
        let w = u16::try_from(ratatui::text::Span::raw(ch.to_string()).width()).unwrap_or(1);
        col = col.saturating_add(w.max(1));
        i += ch.len_utf8();
    }
    col
}

/// Format a resolved target for the status bar (no provider hint).
#[must_use]
#[allow(dead_code)] // callers use `format_target_with_provider`
pub fn format_target(raw: &str, from: &PageKey, index: &Index) -> String {
    format_target_with_provider(raw, from, index, None)
}

/// Status text for a link, including non-markdown file hints.
#[must_use]
pub fn format_target_with_provider(
    raw: &str,
    from: &PageKey,
    index: &Index,
    provider: Option<&FsProvider>,
) -> String {
    let outcome = resolve(raw, from, index);
    match outcome.target {
        wiki_reader_core::nav::Target::Page(key, anchor) => {
            let path = key.relative_path.display();
            match anchor {
                Some(a) => format!("→ {path}#{a}"),
                None => format!("→ {path}"),
            }
        }
        wiki_reader_core::nav::Target::Anchor(a) => format!("→ #{a}"),
        wiki_reader_core::nav::Target::External(url) => format!("↗ {url}"),
        wiki_reader_core::nav::Target::Unresolved(u) => {
            if let (Some(p), Some(rel)) = (provider, unresolved_relative_path(raw, from))
                && p.non_markdown_file_exists(&rel)
            {
                "not a markdown page".into()
            } else {
                format!("? not found: {u}")
            }
        }
    }
}

/// Tab-cycle helpers (P1-08c).
pub mod cycle {
    use super::FocusItem;

    /// Next item after `cursor_line` (wrap; footer items last in `items`).
    #[must_use]
    fn sort_key(it: &FocusItem) -> u32 {
        it.line.unwrap_or(u32::MAX)
    }

    pub fn next_after(items: &[FocusItem], cursor_line: u32, backward: bool) -> Option<usize> {
        if items.is_empty() {
            return None;
        }
        if backward {
            items
                .iter()
                .enumerate()
                .rev()
                .find(|(_, it)| sort_key(it) < cursor_line)
                .map(|(i, _)| i)
                .or(Some(items.len() - 1))
        } else {
            items
                .iter()
                .enumerate()
                .find(|(_, it)| sort_key(it) > cursor_line)
                .map(|(i, _)| i)
                .or(Some(0))
        }
    }

    /// Step from current focused index.
    #[must_use]
    pub fn step(items: &[FocusItem], current: Option<usize>, backward: bool) -> Option<usize> {
        if items.is_empty() {
            return None;
        }
        match current {
            None => {
                if backward {
                    Some(items.len() - 1)
                } else {
                    Some(0)
                }
            }
            Some(i) if backward => Some(if i == 0 { items.len() - 1 } else { i - 1 }),
            Some(i) => Some(if i + 1 >= items.len() { 0 } else { i + 1 }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::cycle::{next_after, step};
    use super::*;
    use std::path::PathBuf;
    use wiki_reader_core::provider::CollectionProvider;

    #[test]
    fn block_jumps_and_word_count() {
        let src = "\
---
title: T
---

# H1

para one

```
code
```

## H2

more words here
";
        let doc = RawDoc::from_source(src, None);
        assert!(doc.block_starts().len() >= 3);
        assert!(doc.word_count() > 0);
        assert_eq!(doc.anchor_line("h2"), Some(doc.headings[1].1));
    }

    #[test]
    fn tab_cycle_rules() {
        let items = vec![
            FocusItem {
                line: Some(2),
                cols: (0, 1),
                kind: FocusTarget::Link,
                target: "a".into(),
                link_id: Some(LinkId(0)),
            },
            FocusItem {
                line: Some(5),
                cols: (0, 1),
                kind: FocusTarget::Link,
                target: "b".into(),
                link_id: Some(LinkId(1)),
            },
            FocusItem {
                line: None,
                cols: (0, 1),
                kind: FocusTarget::FooterPrev,
                target: String::new(),
                link_id: None,
            },
            FocusItem {
                line: None,
                cols: (0, 1),
                kind: FocusTarget::FooterNext,
                target: String::new(),
                link_id: None,
            },
        ];
        assert_eq!(next_after(&items, 3, false), Some(1));
        assert_eq!(step(&items, Some(3), false), Some(0));
        assert_eq!(step(&items, Some(0), true), Some(3));
        assert_eq!(next_after(&items, 0, false), Some(0));
    }

    #[test]
    fn broken_links_fixture_yields_link_items() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/broken-links");
        let provider = wiki_reader_core::provider::FsProvider::open(&root).unwrap();
        let index = wiki_reader_core::Index::build(&provider).unwrap();
        let key = wiki_reader_core::provider::PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: PathBuf::from("README.md"),
        };
        let src = provider.read(&key).unwrap();
        let page = index.pages.get(&key);
        let doc = RawDoc::from_source_ctx(&src, page, Some(&key), Some(&index), Some(&provider));
        assert!(
            !doc.link_spans().is_empty(),
            "expected link spans on broken-links README"
        );
    }
}
