//! Interim viewer document seam (raw lines now; `RenderedDoc` in P1-13).

use wiki_reader_core::Index;
use wiki_reader_core::index::Page;
use wiki_reader_core::nav::resolve;
use wiki_reader_core::parse::{self, MdLink};
use wiki_reader_core::provider::PageKey;

/// Kind of a Tab-cycle focus item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusKind {
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
    /// 0-based source line.
    pub line: u32,
    /// Column range on that line (byte/char index; best-effort).
    pub cols: (u16, u16),
    /// Item kind.
    pub kind: FocusKind,
    /// Raw target string (path, url, or empty for footer).
    pub target: String,
}

/// Document view used by the viewer pane.
pub trait ViewerDoc {
    /// Source (or rendered) lines.
    fn lines(&self) -> &[String];
    /// Top-level block start lines (1-based source).
    fn block_starts(&self) -> &[u32];
    /// Line (1-based) for a heading slug.
    fn anchor_line(&self, slug: &str) -> Option<u32>;
    /// Tab-cycle items (links + footer slots filled by the app).
    fn focus_items(&self) -> Vec<FocusItem>;
}

/// Raw source document backed by provider text + parsed metadata.
#[derive(Debug, Clone)]
pub struct RawDoc {
    lines: Vec<String>,
    blocks: Vec<u32>,
    headings: Vec<(String, u32)>,
    items: Vec<FocusItem>,
    word_count: u32,
    updated: String,
}

impl RawDoc {
    /// Build from raw source and the indexed page (if any).
    #[must_use]
    pub fn from_source(source: &str, page: Option<&Page>) -> Self {
        let lines: Vec<String> = source.lines().map(str::to_owned).collect();
        let (blocks, headings, items, word_count, updated) = if let Some(p) = page {
            let blocks = p.parsed.blocks.clone();
            let headings = p
                .parsed
                .headings
                .iter()
                .map(|h| (h.slug.clone(), h.source_line))
                .collect();
            let items = link_items(&lines, &p.parsed.links);
            (
                blocks,
                headings,
                items,
                p.parsed.word_count,
                p.parsed
                    .frontmatter
                    .updated
                    .clone()
                    .unwrap_or_else(|| "—".into()),
            )
        } else {
            let parsed = parse::parse(source);
            let headings = parsed
                .headings
                .iter()
                .map(|h| (h.slug.clone(), h.source_line))
                .collect();
            let items = link_items(&lines, &parsed.links);
            (
                parsed.blocks,
                headings,
                items,
                parsed.word_count,
                parsed
                    .frontmatter
                    .updated
                    .clone()
                    .unwrap_or_else(|| "—".into()),
            )
        };
        Self {
            lines,
            blocks,
            headings,
            items,
            word_count,
            updated,
        }
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

    fn anchor_line(&self, slug: &str) -> Option<u32> {
        self.headings
            .iter()
            .find(|(s, _)| s == slug)
            .map(|(_, line)| *line)
    }

    fn focus_items(&self) -> Vec<FocusItem> {
        self.items.clone()
    }
}

fn link_items(lines: &[String], md_links: &[MdLink]) -> Vec<FocusItem> {
    md_links
        .iter()
        .map(|md| {
            let line_idx = usize::try_from(md.source_line.saturating_sub(1)).unwrap_or(0);
            let cols = match lines.get(line_idx) {
                Some(line) => {
                    let needle = format!("[{}]({})", md.text, md.target);
                    if let Some(start) = line.find(&needle) {
                        let end = start + needle.len();
                        (
                            u16::try_from(start).unwrap_or(0),
                            u16::try_from(end).unwrap_or(u16::MAX),
                        )
                    } else {
                        (0, u16::try_from(line.chars().count()).unwrap_or(u16::MAX))
                    }
                }
                None => (0, 0),
            };
            FocusItem {
                line: md.source_line.saturating_sub(1),
                cols,
                kind: FocusKind::Link,
                target: md.target.clone(),
            }
        })
        .collect()
}

/// Format a resolved target for the status bar.
#[must_use]
pub fn format_target(raw: &str, from: &PageKey, index: &Index) -> String {
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
        wiki_reader_core::nav::Target::Unresolved(u) => format!("? not found: {u}"),
    }
}

/// Tab-cycle helpers (P1-08c).
pub mod cycle {
    use super::FocusItem;

    /// Next item after `cursor_line` (wrap; footer items last in `items`).
    #[must_use]
    pub fn next_after(items: &[FocusItem], cursor_line: u32, backward: bool) -> Option<usize> {
        if items.is_empty() {
            return None;
        }
        if backward {
            // Previous: last item with line < cursor, else wrap to last.
            items
                .iter()
                .enumerate()
                .rev()
                .find(|(_, it)| it.line < cursor_line)
                .map(|(i, _)| i)
                .or(Some(items.len() - 1))
        } else {
            items
                .iter()
                .enumerate()
                .find(|(_, it)| it.line > cursor_line)
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
                line: 2,
                cols: (0, 1),
                kind: FocusKind::Link,
                target: "a".into(),
            },
            FocusItem {
                line: 5,
                cols: (0, 1),
                kind: FocusKind::Link,
                target: "b".into(),
            },
            FocusItem {
                line: 99,
                cols: (0, 1),
                kind: FocusKind::FooterPrev,
                target: String::new(),
            },
            FocusItem {
                line: 99,
                cols: (0, 1),
                kind: FocusKind::FooterNext,
                target: String::new(),
            },
        ];
        // After cursor line 3 → first item with line > 3 = index 1
        assert_eq!(next_after(&items, 3, false), Some(1));
        // Wrap forward from end
        assert_eq!(step(&items, Some(3), false), Some(0));
        // Wrap backward
        assert_eq!(step(&items, Some(0), true), Some(3));
        // Start after cursor
        assert_eq!(next_after(&items, 0, false), Some(0));
        // Clear on arrow is App responsibility (focused_item = None in viewer_move_line).
    }

    #[test]
    fn broken_links_fixture_yields_link_items() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/broken-links");
        let provider = wiki_reader_core::provider::FsProvider::open(&root).unwrap();
        let index = wiki_reader_core::Index::build(&provider).unwrap();
        let key = wiki_reader_core::provider::PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: std::path::PathBuf::from("README.md"),
        };
        let src = provider.read(&key).unwrap();
        let page = index.pages.get(&key);
        let doc = RawDoc::from_source(&src, page);
        assert!(
            !doc.focus_items().is_empty(),
            "expected link items on broken-links README"
        );
    }
}
