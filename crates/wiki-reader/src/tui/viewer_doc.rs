//! Interim viewer document seam (raw lines now; `RenderedDoc` in P1-13).

use wiki_reader_core::Index;
use wiki_reader_core::index::Page;
use wiki_reader_core::nav::{Target, resolve, unresolved_relative_path};
use wiki_reader_core::parse::{self, MdLink};
use wiki_reader_core::provider::{FsProvider, PageKey};
use wiki_reader_render::{LinkClass, LinkId, LinkSpan};

use super::text_col;

/// Kind of a Tab-cycle focus item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    /// Inline markdown link.
    Link,
    /// Block action (frontmatter / table / code).
    BlockAction,
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
            FocusTarget::Link | FocusTarget::BlockAction => self.line,
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
    /// Styled display rows when available (rendered path).
    fn styled_lines(&self) -> Option<&[wiki_reader_render::StyledLine]> {
        None
    }
    /// Tab-cycle items (links + block actions + footer slots filled by the app).
    fn focus_items(&self) -> Vec<FocusItem> {
        let mut items: Vec<FocusItem> = self
            .link_spans()
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
            .collect();
        items.extend(self.block_focus_items());
        items.sort_by_key(|it| (it.line.unwrap_or(u32::MAX), it.cols.0));
        items
    }

    /// Block actions (rendered mode); empty for raw.
    fn block_focus_items(&self) -> Vec<FocusItem> {
        Vec::new()
    }
}

/// Raw source document backed by provider text + parsed metadata.
///
/// Source lines are soft-wrapped into display rows by [`RawDoc::wrapped`];
/// `lines`, links, blocks and highlights are all in display-row terms, and the
/// `rows` map says which source line each row came from.
#[derive(Debug, Clone)]
pub struct RawDoc {
    /// Display rows (soft-wrapped source lines).
    lines: Vec<String>,
    /// Unwrapped source lines.
    src_lines: Vec<String>,
    /// Per display row: (0-based source line, char offset of the row in it).
    rows: Vec<(u32, usize)>,
    /// First display row of each source line.
    first_row: Vec<u32>,
    /// Gutter number per display row: `Some` on a source line's first row.
    numbers: Vec<Option<u32>>,
    /// Block starts, 1-based source lines.
    src_blocks: Vec<u32>,
    /// Block starts in 1-based display rows.
    blocks: Vec<u32>,
    /// Heading slugs with 1-based source lines.
    src_headings: Vec<(String, u32)>,
    /// Link geometry on source lines (one segment each).
    src_links: Vec<LinkSpan>,
    /// Link geometry on display rows.
    links: Vec<LinkSpan>,
    word_count: u32,
    updated: String,
    /// Syntect runs per source line (parallel to `src_lines`).
    src_highlights: Vec<Vec<crate::tui::highlight::HlSpan>>,
    /// Syntect runs per display row (parallel to `lines`).
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
        let mut doc = Self {
            lines: Vec::new(),
            src_lines: lines,
            rows: Vec::new(),
            first_row: Vec::new(),
            numbers: Vec::new(),
            src_blocks: blocks,
            blocks: Vec::new(),
            src_headings: headings,
            src_links: link_spans,
            links: Vec::new(),
            word_count,
            updated,
            src_highlights: Vec::new(),
            highlights: Vec::new(),
        };
        doc.relayout(0);
        doc
    }

    /// Soft-wrap source lines to `width` display columns (`0` = no wrapping).
    #[must_use]
    pub fn wrapped(mut self, width: u16) -> Self {
        self.relayout(width);
        self
    }

    /// Re-wrap in place to `width`, keeping links and syntax highlights.
    pub fn rewrap(&mut self, width: u16) {
        self.relayout(width);
    }

    /// Display column within its source line where a 0-based display row starts.
    #[must_use]
    pub fn row_col_offset(&self, row: u32) -> u16 {
        let Some(&(src, start)) = usize::try_from(row)
            .ok()
            .and_then(|i| self.rows.get(i))
            .or_else(|| self.rows.last())
        else {
            return 0;
        };
        let line = usize::try_from(src)
            .ok()
            .and_then(|i| self.src_lines.get(i));
        line.map_or(0, |l| {
            l.chars()
                .take(start)
                .fold(0u16, |w, ch| w.saturating_add(text_col::char_width(ch)))
        })
    }

    fn relayout(&mut self, width: u16) {
        self.lines.clear();
        self.rows.clear();
        self.first_row.clear();
        self.numbers.clear();
        // (source line, row index, display col range of the row within the line)
        let mut row_cols: Vec<Vec<(u32, u16, u16)>> = Vec::with_capacity(self.src_lines.len());
        for (src, line) in self.src_lines.iter().enumerate() {
            let src = u32::try_from(src).unwrap_or(u32::MAX);
            self.first_row
                .push(u32::try_from(self.lines.len()).unwrap_or(u32::MAX));
            let chars: Vec<char> = line.chars().collect();
            let starts = wrap_starts(&chars, usize::from(width));
            let mut cols = Vec::with_capacity(starts.len());
            let mut col_at = 0u16;
            for (k, &start) in starts.iter().enumerate() {
                let end = starts.get(k + 1).copied().unwrap_or(chars.len());
                let text: String = chars[start..end].iter().collect();
                let w = text_col::line_width(&text);
                cols.push((
                    u32::try_from(self.lines.len()).unwrap_or(u32::MAX),
                    col_at,
                    col_at.saturating_add(w),
                ));
                col_at = col_at.saturating_add(w);
                self.rows.push((src, start));
                self.numbers
                    .push((start == 0).then_some(src.saturating_add(1)));
                self.lines.push(text);
            }
            row_cols.push(cols);
        }
        let to_row = |src1: u32| -> u32 {
            let src0 = usize::try_from(src1.saturating_sub(1)).unwrap_or(usize::MAX);
            self.first_row
                .get(src0)
                .copied()
                .unwrap_or_else(|| u32::try_from(self.lines.len()).unwrap_or(u32::MAX))
                .saturating_add(1)
        };
        self.blocks = self.src_blocks.iter().map(|&b| to_row(b)).collect();
        self.links = self
            .src_links
            .iter()
            .map(|l| {
                let mut segments = Vec::new();
                for &(src, (c0, c1)) in &l.segments {
                    let rows = usize::try_from(src).ok().and_then(|i| row_cols.get(i));
                    match rows {
                        Some(rows) if c0 < c1 => {
                            for &(row, r0, r1) in rows {
                                let a = c0.max(r0);
                                let b = c1.min(r1);
                                if a < b {
                                    segments.push((row, (a - r0, b - r0)));
                                }
                            }
                        }
                        Some(rows) => {
                            let row = rows.first().map_or(src, |r| r.0);
                            segments.push((row, (c0, c1)));
                        }
                        None => segments.push((src, (c0, c1))),
                    }
                }
                LinkSpan {
                    segments,
                    ..l.clone()
                }
            })
            .collect();
        self.rebuild_highlights();
    }

    fn rebuild_highlights(&mut self) {
        self.highlights.clear();
        if self.src_highlights.is_empty() {
            return;
        }
        for (k, &(src, start)) in self.rows.iter().enumerate() {
            let end = self
                .rows
                .get(k + 1)
                .map_or(
                    usize::MAX,
                    |&(next_src, next)| {
                        if next_src == src { next } else { usize::MAX }
                    },
                );
            let runs = usize::try_from(src)
                .ok()
                .and_then(|i| self.src_highlights.get(i));
            self.highlights
                .push(runs.map_or_else(Vec::new, |r| slice_runs(r, start, end)));
        }
    }

    /// Replace syntect highlights (from a background job; one entry per source line).
    pub fn set_highlights(&mut self, highlights: Vec<Vec<crate::tui::highlight::HlSpan>>) {
        self.src_highlights = highlights;
        self.rebuild_highlights();
    }

    /// Gutter number per display row (`None` on soft-wrapped continuation rows).
    #[must_use]
    pub fn numbers(&self) -> &[Option<u32>] {
        &self.numbers
    }

    /// 0-based source line of a 0-based display row.
    #[must_use]
    pub fn source_line_of_row(&self, row: u32) -> u32 {
        usize::try_from(row)
            .ok()
            .and_then(|i| self.rows.get(i))
            .or_else(|| self.rows.last())
            .map_or(0, |r| r.0)
    }

    /// First 0-based display row of a 0-based source line.
    #[must_use]
    pub fn first_row_of(&self, source: u32) -> u32 {
        usize::try_from(source)
            .ok()
            .and_then(|i| self.first_row.get(i))
            .or_else(|| self.first_row.last())
            .copied()
            .unwrap_or(0)
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
        self.src_headings
            .iter()
            .map(|(_, line)| self.first_row_of(line.saturating_sub(1)).saturating_add(1))
            .collect()
    }

    fn anchor_line(&self, slug: &str) -> Option<u32> {
        self.src_headings
            .iter()
            .find(|(s, _)| s == slug)
            .map(|(_, line)| self.first_row_of(line.saturating_sub(1)).saturating_add(1))
    }

    fn link_spans(&self) -> &[LinkSpan] {
        &self.links
    }
}

/// Char offsets where each soft-wrapped row of `chars` starts (first is 0).
///
/// Rows break after the last space that fits, else hard at `width` columns, so
/// concatenating the rows gives the source line back exactly.
fn wrap_starts(chars: &[char], width: usize) -> Vec<usize> {
    let mut starts = vec![0];
    if width == 0 {
        return starts;
    }
    let widths: Vec<usize> = chars
        .iter()
        .map(|&c| usize::from(text_col::char_width(c)))
        .collect();
    let (mut start, mut w, mut brk, mut i) = (0usize, 0usize, None::<usize>, 0usize);
    while i < chars.len() {
        if w + widths[i] > width && i > start {
            let b = brk.take().filter(|&b| b > start).unwrap_or(i);
            starts.push(b);
            start = b;
            w = widths[b..i].iter().sum();
            continue;
        }
        w += widths[i];
        if chars[i] == ' ' {
            brk = Some(i + 1);
        }
        i += 1;
    }
    starts
}

/// Highlight runs restricted to chars `[start, end)` of their line.
fn slice_runs(
    runs: &[crate::tui::highlight::HlSpan],
    start: usize,
    end: usize,
) -> Vec<crate::tui::highlight::HlSpan> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for run in runs {
        let len = run.text.chars().count();
        let (a, b) = (at.max(start), (at + len).min(end));
        if a < b {
            out.push(crate::tui::highlight::HlSpan {
                style: run.style,
                text: run.text.chars().skip(a - at).take(b - a).collect(),
            });
        }
        at += len;
        if at >= end {
            break;
        }
    }
    out
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
        Target::Unsupported(_) => LinkClass::Unsupported,
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
        col = col.saturating_add(text_col::char_width(ch));
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
        wiki_reader_core::nav::Target::Unsupported(u) => {
            format!("unsupported link scheme: {u}")
        }
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

    #[must_use]
    fn sort_key(it: &FocusItem) -> u32 {
        it.line.unwrap_or(u32::MAX)
    }

    /// First item on or after `cursor_line` going forward (so an item on the
    /// cursor's own line, e.g. the frontmatter toggle on line 0, is reachable),
    /// or last item strictly before it going backward; wraps. Footer items sort last.
    #[must_use]
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
                .find(|(_, it)| sort_key(it) >= cursor_line)
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
        assert_eq!(doc.anchor_line("h2"), Some(doc.src_headings[1].1));
    }

    #[test]
    fn raw_wrap_mapping_handles_wide_tabs_empty_and_out_of_range() {
        let doc = RawDoc::from_source("ab漢c\tde\nshort", None).wrapped(4);
        assert_eq!(doc.lines[..doc.first_row[1] as usize].concat(), "ab漢c\tde");
        assert!(doc.lines.iter().all(|line| text_col::line_width(line) <= 4));
        assert_eq!(doc.source_line_of_row(0), 0);
        assert_eq!(doc.source_line_of_row(u32::MAX), 1);
        assert_eq!(doc.first_row_of(0), 0);
        assert_eq!(doc.first_row_of(u32::MAX), doc.first_row[1]);

        let empty = RawDoc::from_source("", None).wrapped(4);
        assert!(empty.lines.is_empty());
        assert_eq!(empty.source_line_of_row(u32::MAX), 0);
        assert_eq!(empty.first_row_of(u32::MAX), 0);
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

    #[test]
    fn unsupported_scheme_status_and_class() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/broken-links");
        let provider = wiki_reader_core::provider::FsProvider::open(&root).unwrap();
        let index = wiki_reader_core::Index::build(&provider).unwrap();
        let from = wiki_reader_core::provider::PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: PathBuf::from("README.md"),
        };
        let raw = "chatgpt-conversation://abc";
        assert_eq!(
            format_target_with_provider(raw, &from, &index, Some(&provider)),
            "unsupported link scheme: chatgpt-conversation://abc"
        );
        assert_eq!(link_class(raw, &from, &index), LinkClass::Unsupported);
    }
}
