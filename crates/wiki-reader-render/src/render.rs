//! ponytail: pulldown-cmark → wrapped lines; rich blocks land incrementally.

use std::collections::HashMap;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use unicode_width::UnicodeWidthStr;
use wiki_reader_core::Index;
use wiki_reader_core::index::Page;
use wiki_reader_core::nav::{Target, resolve};
use wiki_reader_core::provider::PageKey;

use crate::{LinkClass, LinkId, LinkSpan};

/// Layout output for the viewer (P1-13 baseline).
#[derive(Debug, Clone)]
pub struct RenderedDoc {
    /// Display lines at `width`.
    pub lines: Vec<String>,
    pub links: Vec<LinkSpan>,
    /// 1-based source line where each block starts.
    pub block_starts: Vec<u32>,
    /// Rendered line (0-based) → source line (1-based).
    pub source_map: Vec<u32>,
    /// Heading slug → source line (1-based).
    pub headings: Vec<(String, u32)>,
    pub word_count: u32,
    pub updated: String,
}

/// Render `page` body at `width` columns (wraps paragraphs).
#[must_use]
#[allow(clippy::too_many_lines)] // incremental element support
pub fn render(
    source: &str,
    page: Option<&Page>,
    from: &PageKey,
    index: &Index,
    width: u16,
) -> RenderedDoc {
    let w = usize::from(width.max(20));
    let (word_count, updated, parsed_links, blocks, headings) = if let Some(p) = page {
        (
            p.parsed.word_count,
            p.parsed
                .frontmatter
                .updated
                .clone()
                .unwrap_or_else(|| "—".into()),
            p.parsed.links.clone(),
            p.parsed.blocks.clone(),
            p.parsed
                .headings
                .iter()
                .map(|h| (h.slug.clone(), h.source_line))
                .collect::<Vec<_>>(),
        )
    } else {
        let parsed = wiki_reader_core::parse::parse(source);
        (
            parsed.word_count,
            parsed
                .frontmatter
                .updated
                .clone()
                .unwrap_or_else(|| "—".into()),
            parsed.links,
            parsed.blocks,
            parsed
                .headings
                .iter()
                .map(|h| (h.slug.clone(), h.source_line))
                .collect(),
        )
    };

    let body = strip_frontmatter(source);
    let mut out_lines: Vec<String> = Vec::new();
    let mut source_map: Vec<u32> = Vec::new();
    let mut link_spans: Vec<LinkSpan> = Vec::new();
    let mut link_id = 0u32;
    let mut link_buf = String::new();
    let mut link_raw = String::new();
    let mut in_link = false;
    let src_line = 1u32;

    let parser = Parser::new_ext(&body, Options::empty());
    for event in parser {
        #[allow(clippy::unnested_or_patterns, clippy::match_same_arms)]
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                in_link = true;
                link_raw = dest_url.to_string();
                link_buf.clear();
            }
            Event::End(TagEnd::Link) => {
                if in_link {
                    let class = match resolve(&link_raw, from, index).target {
                        Target::External(_) => LinkClass::External,
                        Target::Unresolved(_) => LinkClass::Broken,
                        _ => LinkClass::Internal,
                    };
                    let rendered_line = u32::try_from(out_lines.len()).unwrap_or(0);
                    let col_start = u16::try_from(
                        out_lines
                            .last()
                            .map_or(0, |l| l.as_str().width())
                            .min(usize::from(u16::MAX)),
                    )
                    .unwrap_or(0);
                    if let Some(last) = out_lines.last_mut() {
                        last.push_str(&link_buf);
                    } else {
                        out_lines.push(link_buf.clone());
                        source_map.push(src_line);
                    }
                    let col_end = u16::try_from(
                        out_lines
                            .last()
                            .map_or(0, |l| l.as_str().width())
                            .min(usize::from(u16::MAX)),
                    )
                    .unwrap_or(col_start);
                    link_spans.push(LinkSpan {
                        id: LinkId(link_id),
                        raw_target: link_raw.clone(),
                        class,
                        segments: vec![(rendered_line, (col_start, col_end))],
                    });
                    link_id = link_id.saturating_add(1);
                }
                in_link = false;
            }
            Event::Text(t) => {
                if in_link {
                    link_buf.push_str(&t);
                } else if let Some(last) = out_lines.last_mut() {
                    last.push_str(&t);
                } else {
                    push_wrapped(&mut out_lines, &mut source_map, src_line, &t, w);
                }
            }
            Event::Code(t) => {
                let s = format!("`{t}`");
                if in_link {
                    link_buf.push_str(&s);
                } else if let Some(last) = out_lines.last_mut() {
                    last.push_str(&s);
                } else {
                    push_wrapped(&mut out_lines, &mut source_map, src_line, &s, w);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_link {
                    link_buf.push(' ');
                } else {
                    out_lines.push(String::new());
                    source_map.push(src_line);
                }
            }
            Event::Start(Tag::Heading { .. }) | Event::End(TagEnd::Heading(_)) => {
                out_lines.push(String::new());
                source_map.push(src_line);
            }
            Event::End(TagEnd::Paragraph) => {
                out_lines.push(String::new());
                source_map.push(src_line);
            }
            Event::Start(Tag::Paragraph)
            | Event::Start(_)
            | Event::End(_)
            | Event::Rule
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::FootnoteReference(_)
            | Event::TaskListMarker(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_) => {}
        }
    }

    if out_lines.is_empty() {
        out_lines.push(String::new());
        source_map.push(1);
    }

    if link_spans.is_empty() && !parsed_links.is_empty() {
        link_spans = fallback_link_spans(&parsed_links, from, index, &out_lines, &source_map);
    }

    RenderedDoc {
        lines: out_lines,
        links: link_spans,
        block_starts: blocks,
        source_map,
        headings,
        word_count,
        updated,
    }
}

fn push_wrapped(
    out_lines: &mut Vec<String>,
    source_map: &mut Vec<u32>,
    src_line: u32,
    text: &str,
    width: usize,
) {
    if text.is_empty() {
        out_lines.push(String::new());
        source_map.push(src_line);
        return;
    }
    for chunk in wrap_line(text, width) {
        out_lines.push(chunk);
        source_map.push(src_line);
    }
}

fn strip_frontmatter(source: &str) -> String {
    if source.starts_with("---\n")
        && let Some((_, rest)) = source.split_once("\n---\n")
    {
        return rest.to_owned();
    }
    source.to_owned()
}

fn wrap_line(text: &str, width: usize) -> Vec<String> {
    if text.width() <= width {
        return vec![text.to_owned()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut col = 0usize;
    for word in text.split_whitespace() {
        let ww = word.width();
        if col > 0 && col + 1 + ww > width {
            lines.push(std::mem::take(&mut current));
            col = 0;
        }
        if col > 0 {
            current.push(' ');
            col += 1;
        }
        current.push_str(word);
        col += ww;
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn fallback_link_spans(
    md_links: &[wiki_reader_core::parse::MdLink],
    from: &PageKey,
    index: &Index,
    lines: &[String],
    source_map: &[u32],
) -> Vec<LinkSpan> {
    let mut by_src: HashMap<u32, u32> = HashMap::new();
    for (r, src) in source_map.iter().enumerate() {
        by_src.entry(*src).or_insert(u32::try_from(r).unwrap_or(0));
    }
    md_links
        .iter()
        .enumerate()
        .map(|(i, md)| {
            let class = match resolve(&md.target, from, index).target {
                Target::External(_) => LinkClass::External,
                Target::Unresolved(_) => LinkClass::Broken,
                _ => LinkClass::Internal,
            };
            let rendered = by_src.get(&md.source_line).copied().unwrap_or(0);
            let w = lines
                .get(usize::try_from(rendered).unwrap_or(0))
                .map_or(0, |l| l.as_str().width());
            LinkSpan {
                id: LinkId(u32::try_from(i).unwrap_or(u32::MAX)),
                raw_target: md.target.clone(),
                class,
                segments: vec![(
                    rendered,
                    (0, u16::try_from(w.min(usize::from(u16::MAX))).unwrap_or(0)),
                )],
            }
        })
        .collect()
}
