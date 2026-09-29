//! pulldown-cmark → styled wrapped lines (V1 elements).

use std::collections::HashMap;
use std::fmt::Write as _;

use pulldown_cmark::{
    BlockQuoteKind, CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};
use unicode_width::UnicodeWidthStr;
use wiki_reader_core::Index;
use wiki_reader_core::index::Page;
use wiki_reader_core::nav::{Target, resolve};
use wiki_reader_core::parse::{self, github_slug};
use wiki_reader_core::provider::PageKey;

use crate::{LinkClass, LinkId, LinkSpan};

/// Semantic style for a span (TUI maps to theme colours).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleKind {
    /// Body text.
    Plain,
    /// ATX heading level 1–6.
    Heading(u8),
    /// Italic.
    Emphasis,
    /// Bold.
    Strong,
    /// ~~strike~~.
    Strikethrough,
    /// `inline code`.
    InlineCode,
    /// Fenced / indented code block.
    CodeBlock,
    /// Link text (class carried separately on [`LinkSpan`]).
    Link,
    /// Blockquote / alert body.
    Quote,
    /// Horizontal rule glyph line.
    Rule,
    /// Table cell / border.
    Table,
    /// Task list checkbox marker.
    TaskMarker,
    /// Frontmatter metadata box.
    Frontmatter,
}

/// One styled run on a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledSpan {
    /// Display text.
    pub text: String,
    /// Semantic style.
    pub kind: StyleKind,
}

/// One display row after wrap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledLine {
    /// Runs on this row.
    pub spans: Vec<StyledSpan>,
    /// Source line (1-based) this row primarily came from.
    pub source_line: u32,
}

impl StyledLine {
    /// Plain display string.
    #[must_use]
    pub fn plain(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }
}

/// Layout output for the viewer.
#[derive(Debug, Clone)]
pub struct RenderedDoc {
    /// Display lines (plain text; matches [`StyledLine::plain`]).
    pub lines: Vec<String>,
    /// Styled rows (same length as `lines`).
    pub styled: Vec<StyledLine>,
    pub links: Vec<LinkSpan>,
    /// 1-based **rendered** line where each content block starts.
    pub block_starts: Vec<u32>,
    /// Rendered line (0-based) → source line (1-based), monotonic.
    pub source_map: Vec<u32>,
    /// Heading slug → 1-based **rendered** line.
    pub headings: Vec<(String, u32)>,
    pub word_count: u32,
    pub updated: String,
}

/// Render `source` at `width` columns.
#[must_use]
pub fn render(
    source: &str,
    page: Option<&Page>,
    from: &PageKey,
    index: &Index,
    width: u16,
) -> RenderedDoc {
    let w = usize::from(width.max(20));
    let parsed = match page {
        Some(p) => p.parsed.clone(),
        None => parse::parse(source),
    };
    let word_count = parsed.word_count;
    let updated = parsed
        .frontmatter
        .updated
        .clone()
        .unwrap_or_else(|| "—".into());

    let (body, body_line_offset) = body_and_offset(source);
    let mut state = LayoutState::new(w, body_line_offset, from, index);

    if parsed.frontmatter.title.is_some()
        || parsed.frontmatter.updated.is_some()
        || parsed.frontmatter.summary.is_some()
        || !parsed.frontmatter.tags.is_empty()
    {
        state.push_frontmatter_box(&parsed);
    }

    let options = Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_GFM;
    for (event, range) in Parser::new_ext(&body, options).into_offset_iter() {
        let src = offset_to_line(&body, body_line_offset, range.start);
        // Closing tags span the whole construct; prefer end for fence close etc.
        let src_end = offset_to_line(
            &body,
            body_line_offset,
            range.end.saturating_sub(1).max(range.start),
        );
        state.handle(event, src, src_end);
    }
    state.finish_block();

    if state.styled.is_empty() {
        state.push_empty(body_line_offset.max(1));
    }

    let lines: Vec<String> = state.styled.iter().map(StyledLine::plain).collect();
    let source_map: Vec<u32> = state.styled.iter().map(|l| l.source_line).collect();

    RenderedDoc {
        lines,
        styled: state.styled,
        links: state.links,
        block_starts: state.block_starts,
        source_map,
        headings: state.headings,
        word_count,
        updated,
    }
}

#[allow(clippy::struct_excessive_bools)] // layout FSM flags
struct LayoutState<'a> {
    width: usize,
    from: &'a PageKey,
    index: &'a Index,
    styled: Vec<StyledLine>,
    links: Vec<LinkSpan>,
    block_starts: Vec<u32>,
    headings: Vec<(String, u32)>,
    /// Current open line being built (spans not yet committed).
    cur: Vec<StyledSpan>,
    cur_src: u32,
    /// Stack of active inline styles (outer → inner).
    style_stack: Vec<StyleKind>,
    in_link: Option<LinkBuild>,
    list_stack: Vec<ListCtx>,
    in_code_block: bool,
    code_fence_lang: String,
    /// Next source line for code body rows (advances per content line).
    code_line_src: u32,
    in_quote: bool,
    /// Alert label when quote opens with `[!NOTE]` / goal / decision / risk.
    alert_label: Option<String>,
    list_marker_pending: bool,
    in_table: bool,
    table_row: Vec<String>,
    table_rows: Vec<Vec<String>>,
    table_header_done: bool,
    heading_level: Option<u8>,
    heading_text: String,
    used_slugs: HashMap<String, u32>,
    link_id: u32,
}

struct LinkBuild {
    raw: String,
    text: String,
    start_line: u32,
    start_col: u16,
    segments: Vec<(u32, (u16, u16))>,
}

struct ListCtx {
    ordered: bool,
    next_num: u64,
}

impl<'a> LayoutState<'a> {
    fn new(width: usize, body_line_offset: u32, from: &'a PageKey, index: &'a Index) -> Self {
        let _ = body_line_offset;
        Self {
            width,
            from,
            index,
            styled: Vec::new(),
            links: Vec::new(),
            block_starts: Vec::new(),
            headings: Vec::new(),
            cur: Vec::new(),
            cur_src: body_line_offset.max(1),
            style_stack: Vec::new(),
            in_link: None,
            list_stack: Vec::new(),
            in_code_block: false,
            code_fence_lang: String::new(),
            code_line_src: 1,
            in_quote: false,
            alert_label: None,
            list_marker_pending: false,
            in_table: false,
            table_row: Vec::new(),
            table_rows: Vec::new(),
            table_header_done: false,
            heading_level: None,
            heading_text: String::new(),
            used_slugs: HashMap::new(),
            link_id: 0,
        }
    }

    fn handle(&mut self, event: Event<'_>, src: u32, src_end: u32) {
        match event {
            Event::Start(tag) => self.start_tag(tag, src),
            Event::End(end) => self.end_tag(end, src, src_end),
            Event::Text(t) => self.text(&t, src),
            Event::Code(t) => self.inline_code(&t, src),
            Event::SoftBreak => self.soft_break(src),
            Event::HardBreak => {
                self.commit_line(src);
            }
            Event::Rule => {
                self.finish_block();
                self.mark_block(src);
                self.cur_src = src;
                self.push_span("─".repeat(self.width.min(40)), StyleKind::Rule, src);
                self.commit_line(src);
            }
            Event::TaskListMarker(checked) => {
                self.ensure_list_marker(src);
                let mark = if checked { "[x] " } else { "[ ] " };
                self.push_span(mark.to_owned(), StyleKind::TaskMarker, src);
            }
            Event::Html(h) | Event::InlineHtml(h) => {
                let _ = h;
            }
            Event::FootnoteReference(name) => {
                self.ensure_list_marker(src);
                self.push_span(format!("[^{name}]"), StyleKind::Plain, src);
            }
            Event::InlineMath(_) | Event::DisplayMath(_) => {}
        }
    }

    #[allow(clippy::too_many_lines)] // tag dispatch table
    fn start_tag(&mut self, tag: Tag<'_>, src: u32) {
        match tag {
            Tag::Paragraph => {
                self.finish_block();
                self.mark_block(src);
                self.cur_src = src;
                if self.in_quote {
                    self.push_span("│ ".into(), StyleKind::Quote, src);
                }
                self.ensure_list_marker(src);
            }
            Tag::Heading { level, .. } => {
                self.finish_block();
                self.mark_block(src);
                self.cur_src = src;
                let lv = heading_u8(level);
                self.heading_level = Some(lv);
                self.heading_text.clear();
                self.style_stack.push(StyleKind::Heading(lv));
                let hashes = "#".repeat(usize::from(lv));
                self.push_span(format!("{hashes} "), StyleKind::Heading(lv), src);
            }
            Tag::BlockQuote(kind) => {
                self.finish_block();
                self.mark_block(src);
                self.in_quote = true;
                self.alert_label = kind.map(|k| match k {
                    BlockQuoteKind::Note => "NOTE".into(),
                    BlockQuoteKind::Tip => "TIP".into(),
                    BlockQuoteKind::Important => "IMPORTANT".into(),
                    BlockQuoteKind::Warning => "WARNING".into(),
                    BlockQuoteKind::Caution => "CAUTION".into(),
                });
                if let Some(label) = self.alert_label.clone() {
                    self.push_span(format!("│ [{label}]"), StyleKind::Quote, src);
                    self.commit_line(src);
                }
            }
            Tag::CodeBlock(kind) => {
                self.finish_block();
                self.mark_block(src);
                self.in_code_block = true;
                self.code_line_src = src.saturating_add(1);
                self.code_fence_lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.cur_src = src;
                let label = if self.code_fence_lang.is_empty() {
                    "```".into()
                } else {
                    format!("```{}", self.code_fence_lang)
                };
                self.push_span(label, StyleKind::CodeBlock, src);
                self.commit_line(src);
            }
            Tag::List(start) => {
                self.finish_block();
                self.list_stack.push(ListCtx {
                    ordered: start.is_some(),
                    next_num: start.unwrap_or(1),
                });
            }
            Tag::Item => {
                self.finish_block();
                self.mark_block(src);
                self.cur_src = src;
                self.list_marker_pending = true;
            }
            Tag::Emphasis => self.style_stack.push(StyleKind::Emphasis),
            Tag::Strong => self.style_stack.push(StyleKind::Strong),
            Tag::Strikethrough => self.style_stack.push(StyleKind::Strikethrough),
            Tag::Link { dest_url, .. } => {
                let col = u16::try_from(self.cur_width().min(usize::from(u16::MAX))).unwrap_or(0);
                let line = u32::try_from(self.styled.len()).unwrap_or(0);
                self.in_link = Some(LinkBuild {
                    raw: dest_url.to_string(),
                    text: String::new(),
                    start_line: line,
                    start_col: col,
                    segments: Vec::new(),
                });
                self.style_stack.push(StyleKind::Link);
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                let alt = if title.is_empty() {
                    format!("[{dest_url}]")
                } else {
                    format!("[{title}]")
                };
                self.push_span(alt, StyleKind::Link, src);
            }
            Tag::Table(_) => {
                self.finish_block();
                self.mark_block(src);
                self.in_table = true;
                self.table_rows.clear();
                self.table_header_done = false;
            }
            Tag::TableHead | Tag::TableRow => {
                self.table_row.clear();
            }
            Tag::TableCell => {
                self.style_stack.push(StyleKind::Table);
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, end: TagEnd, src: u32, src_end: u32) {
        match end {
            TagEnd::Paragraph => {
                self.commit_line(src);
            }
            TagEnd::Heading(_) => {
                let level = self.heading_level.take().unwrap_or(1);
                let _ = self.style_stack.pop();
                let rendered = u32::try_from(self.styled.len()).unwrap_or(0);
                let slug = unique_slug(&github_slug(&self.heading_text), &mut self.used_slugs);
                self.headings.push((slug, rendered.saturating_add(1)));
                let _ = level;
                self.commit_line(src);
                self.heading_text.clear();
            }
            TagEnd::BlockQuote(_) => {
                self.in_quote = false;
                self.alert_label = None;
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                // Closing fence maps to its own source line (range end), not the open.
                let close_src = src_end.max(src);
                self.push_span("```".into(), StyleKind::CodeBlock, close_src);
                self.commit_line(close_src);
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
            }
            TagEnd::Item => {
                if !self.cur.is_empty() {
                    self.commit_line(src);
                }
                self.list_marker_pending = false;
                if let Some(list) = self.list_stack.last_mut()
                    && list.ordered
                {
                    list.next_num = list.next_num.saturating_add(1);
                }
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                let _ = self.style_stack.pop();
            }
            TagEnd::Link => {
                let _ = self.style_stack.pop();
                if let Some(mut lb) = self.in_link.take() {
                    // Finalize current-line segment without duplicating per-word rects.
                    let end_col =
                        u16::try_from(self.cur_width().min(usize::from(u16::MAX))).unwrap_or(0);
                    let line = u32::try_from(self.styled.len()).unwrap_or(0);
                    coalesce_link_segment(&mut lb.segments, line, lb.start_col, end_col);
                    let class = match resolve(&lb.raw, self.from, self.index).target {
                        Target::External(_) => LinkClass::External,
                        Target::Unresolved(_) => LinkClass::Broken,
                        _ => LinkClass::Internal,
                    };
                    // Drop empty / inverted segments
                    lb.segments.retain(|&(_, (a, b))| a < b);
                    self.links.push(LinkSpan {
                        id: LinkId(self.link_id),
                        raw_target: lb.raw,
                        class,
                        segments: lb.segments,
                    });
                    self.link_id = self.link_id.saturating_add(1);
                }
            }
            TagEnd::Table => {
                self.flush_table(src);
                self.in_table = false;
            }
            TagEnd::TableHead => {
                if !self.table_row.is_empty() {
                    self.table_rows.push(std::mem::take(&mut self.table_row));
                    self.table_header_done = true;
                }
            }
            TagEnd::TableRow => {
                if !self.table_row.is_empty() {
                    self.table_rows.push(std::mem::take(&mut self.table_row));
                }
            }
            TagEnd::TableCell => {
                let _ = self.style_stack.pop();
                let cell = self.drain_cur_text();
                self.table_row.push(cell);
            }
            _ => {}
        }
    }

    fn text(&mut self, t: &str, src: u32) {
        if self.in_code_block {
            // Drop the trailing empty split from a final newline (no blank │ row).
            let body = t.strip_suffix('\n').unwrap_or(t);
            if body.is_empty() && t.ends_with('\n') {
                return;
            }
            for line in body.split('\n') {
                let line_src = self.code_line_src;
                self.push_span("│ ".into(), StyleKind::CodeBlock, line_src);
                self.push_span(line.to_owned(), StyleKind::CodeBlock, line_src);
                self.commit_line(line_src);
                self.code_line_src = self.code_line_src.saturating_add(1);
            }
            return;
        }
        let text = if self.in_quote && self.alert_label.is_none() {
            if let Some((label, rest)) = parse_alert_prefix(t) {
                self.alert_label = Some(label.to_owned());
                // Replace the bare quote gutter with a labelled alert header.
                if self.cur.len() == 1
                    && self.cur[0].text == "│ "
                    && self.cur[0].kind == StyleKind::Quote
                {
                    self.cur.clear();
                    self.push_span(format!("│ [{label}] "), StyleKind::Quote, src);
                } else {
                    self.push_span(format!("[{label}] "), StyleKind::Quote, src);
                }
                rest.to_owned()
            } else {
                t.to_owned()
            }
        } else {
            t.to_owned()
        };
        if self.heading_level.is_some() {
            self.heading_text.push_str(&text);
        }
        if let Some(lb) = self.in_link.as_mut() {
            lb.text.push_str(&text);
        }
        self.ensure_list_marker(src);
        if !text.is_empty() {
            self.push_wrapping(&text, src);
        }
    }

    fn inline_code(&mut self, t: &CowStr<'_>, src: u32) {
        let s = format!("`{t}`");
        if let Some(lb) = self.in_link.as_mut() {
            lb.text.push_str(&s);
        }
        if self.heading_level.is_some() {
            self.heading_text.push_str(&s);
        }
        self.push_span(s, StyleKind::InlineCode, src);
    }

    fn soft_break(&mut self, src: u32) {
        self.push_wrapping(" ", src);
    }

    fn push_frontmatter_box(&mut self, parsed: &parse::ParsedPage) {
        self.mark_block(1);
        self.push_span("── frontmatter ──".into(), StyleKind::Frontmatter, 1);
        self.commit_line(1);
        if let Some(title) = &parsed.frontmatter.title {
            self.push_span(format!("title: {title}"), StyleKind::Frontmatter, 1);
            self.commit_line(1);
        }
        if let Some(updated) = &parsed.frontmatter.updated {
            self.push_span(format!("updated: {updated}"), StyleKind::Frontmatter, 1);
            self.commit_line(1);
        }
        if let Some(summary) = &parsed.frontmatter.summary {
            self.push_span(format!("summary: {summary}"), StyleKind::Frontmatter, 1);
            self.commit_line(1);
        }
        if !parsed.frontmatter.tags.is_empty() {
            self.push_span(
                format!("tags: {}", parsed.frontmatter.tags.join(", ")),
                StyleKind::Frontmatter,
                1,
            );
            self.commit_line(1);
        }
        self.push_span("─────────────────".into(), StyleKind::Frontmatter, 1);
        self.commit_line(1);
    }

    fn mark_block(&mut self, _src: u32) {
        let rendered = u32::try_from(self.styled.len()).unwrap_or(0);
        let line = rendered.saturating_add(1);
        if self.block_starts.last().copied() != Some(line) {
            self.block_starts.push(line);
        }
    }

    fn ensure_list_marker(&mut self, src: u32) {
        if !self.list_marker_pending {
            return;
        }
        self.list_marker_pending = false;
        self.prefix_list_marker(src);
    }

    fn prefix_list_marker(&mut self, src: u32) {
        let depth = self.list_stack.len();
        if depth == 0 {
            return;
        }
        let indent = "  ".repeat(depth.saturating_sub(1));
        let marker = {
            let list = self.list_stack.last().expect("depth>0");
            if list.ordered {
                format!("{}. ", list.next_num)
            } else {
                "• ".into()
            }
        };
        self.push_span(format!("{indent}{marker}"), self.current_kind(), src);
    }

    fn current_kind(&self) -> StyleKind {
        self.style_stack
            .last()
            .copied()
            .unwrap_or(if self.in_quote {
                StyleKind::Quote
            } else if self.in_code_block {
                StyleKind::CodeBlock
            } else {
                StyleKind::Plain
            })
    }

    fn cur_width(&self) -> usize {
        self.cur.iter().map(|s| s.text.width()).sum()
    }

    fn push_span(&mut self, text: String, kind: StyleKind, src: u32) {
        if text.is_empty() {
            return;
        }
        self.cur_src = src;
        // wrap if needed
        let mut rest = text;
        while !rest.is_empty() {
            let avail = self.width.saturating_sub(self.cur_width());
            if avail == 0 {
                self.commit_line(src);
                continue;
            }
            let (take, next) = split_at_width(&rest, avail);
            if take.is_empty() {
                // single glyph wider than avail — force commit
                self.commit_line(src);
                let (take2, next2) = split_at_width(&rest, self.width.max(1));
                self.cur.push(StyledSpan { text: take2, kind });
                rest = next2;
                continue;
            }
            let is_link = kind == StyleKind::Link && self.in_link.is_some();
            let start = u16::try_from(self.cur_width().min(usize::from(u16::MAX))).unwrap_or(0);
            self.cur.push(StyledSpan { text: take, kind });
            if is_link {
                let end =
                    u16::try_from(self.cur_width().min(usize::from(u16::MAX))).unwrap_or(start);
                let line = u32::try_from(self.styled.len()).unwrap_or(0);
                if start < end
                    && let Some(lb) = self.in_link.as_mut()
                {
                    coalesce_link_segment(&mut lb.segments, line, start, end);
                }
            }
            rest = next;
            if !rest.is_empty() {
                self.commit_line(src);
            }
        }
    }

    fn push_wrapping(&mut self, text: &str, src: u32) {
        let kind = self.current_kind();
        // Word-wrap on spaces when possible
        if text.contains(' ') || text.contains('\t') {
            let mut first = true;
            for word in text.split_inclusive(|c: char| c.is_whitespace()) {
                if !first && self.cur_width() > 0 && self.cur_width() + word.width() > self.width {
                    self.commit_line(src);
                }
                first = false;
                self.push_span(word.to_owned(), kind, src);
            }
        } else {
            self.push_span(text.to_owned(), kind, src);
        }
    }

    fn commit_line(&mut self, src: u32) {
        let spans = std::mem::take(&mut self.cur);
        let line = StyledLine {
            spans,
            source_line: if self.cur_src == 0 { src } else { self.cur_src },
        };
        // Next link glyphs start at column 0 of the following display row.
        if let Some(lb) = self.in_link.as_mut() {
            lb.start_line = u32::try_from(self.styled.len().saturating_add(1)).unwrap_or(0);
            lb.start_col = 0;
        }
        self.styled.push(line);
    }

    fn finish_block(&mut self) {
        if !self.cur.is_empty() {
            self.commit_line(self.cur_src);
        }
    }

    fn push_empty(&mut self, src: u32) {
        self.styled.push(StyledLine {
            spans: vec![StyledSpan {
                text: String::new(),
                kind: StyleKind::Plain,
            }],
            source_line: src,
        });
    }

    fn drain_cur_text(&mut self) -> String {
        let s: String = self.cur.iter().map(|sp| sp.text.as_str()).collect();
        self.cur.clear();
        s.trim().to_owned()
    }

    fn flush_table(&mut self, src: u32) {
        if self.table_rows.is_empty() {
            return;
        }
        let rows = std::mem::take(&mut self.table_rows);
        let cols = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let mut rows = rows;
        for row in &mut rows {
            while row.len() < cols {
                row.push(String::new());
            }
        }
        let widths = fair_share_widths(&rows, cols, self.width);
        let header_count = usize::from(self.table_header_done);
        for (i, row) in rows.iter().enumerate() {
            let line = format_table_row(row, &widths);
            self.cur_src = src;
            self.cur.push(StyledSpan {
                text: line,
                kind: StyleKind::Table,
            });
            self.commit_line(src);
            if i + 1 == header_count {
                let sep = format_table_separator(&widths);
                self.cur.push(StyledSpan {
                    text: sep,
                    kind: StyleKind::Table,
                });
                self.commit_line(src);
            }
        }
        self.table_header_done = false;
    }
}

fn body_and_offset(source: &str) -> (String, u32) {
    if let Some(body) = strip_fm(source) {
        let offset = source.len() - body.len();
        let lines_before = source[..offset].bytes().filter(|&b| b == b'\n').count();
        let body_line_offset = u32::try_from(lines_before).unwrap_or(0).saturating_add(1);
        return (body, body_line_offset.max(1));
    }
    (source.to_owned(), 1)
}

fn strip_fm(source: &str) -> Option<String> {
    let rest = if source.starts_with("---\n") {
        source.strip_prefix("---\n")?
    } else if source.starts_with("---\r\n") {
        source.strip_prefix("---\r\n")?
    } else if source.starts_with("+++\n") {
        source.strip_prefix("+++\n")?
    } else {
        return None;
    };
    let close = if source.starts_with("+++") {
        "\n+++\n"
    } else {
        "\n---\n"
    };
    let close_alt = if source.starts_with("+++") {
        "\n+++\r\n"
    } else {
        "\n---\r\n"
    };
    if let Some(idx) = rest.find(close) {
        return Some(rest[idx + close.len()..].to_owned());
    }
    if let Some(idx) = rest.find(close_alt) {
        return Some(rest[idx + close_alt.len()..].to_owned());
    }
    None
}

fn offset_to_line(body: &str, body_line_offset: u32, byte: usize) -> u32 {
    let mut byte = byte.min(body.len());
    while byte > 0 && !body.is_char_boundary(byte) {
        byte -= 1;
    }
    let idx = body[..byte].bytes().filter(|&b| b == b'\n').count();
    body_line_offset + u32::try_from(idx).unwrap_or(0)
}

fn heading_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn unique_slug(base: &str, used: &mut HashMap<String, u32>) -> String {
    let n = used.entry(base.to_owned()).or_insert(0);
    *n = n.saturating_add(1);
    if *n == 1 {
        base.to_owned()
    } else {
        format!("{base}-{}", *n - 1)
    }
}

fn split_at_width(s: &str, max: usize) -> (String, String) {
    if s.width() <= max {
        return (s.to_owned(), String::new());
    }
    let mut col = 0usize;
    for (i, ch) in s.char_indices() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if col + cw > max {
            if i == 0 {
                // force one char
                let next = i + ch.len_utf8();
                return (s[..next].to_owned(), s[next..].to_owned());
            }
            return (s[..i].to_owned(), s[i..].to_owned());
        }
        col += cw;
    }
    (s.to_owned(), String::new())
}

fn truncate_width(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_owned();
    }
    if max <= 1 {
        return "…".into();
    }
    let (head, _) = split_at_width(s, max.saturating_sub(1));
    format!("{head}…")
}

/// Merge contiguous link glyph runs on the same display line into one rect.
fn coalesce_link_segment(segs: &mut Vec<(u32, (u16, u16))>, line: u32, start: u16, end: u16) {
    if start >= end {
        return;
    }
    if let Some(last) = segs.last_mut()
        && last.0 == line
    {
        last.1.0 = last.1.0.min(start);
        last.1.1 = last.1.1.max(end);
        return;
    }
    segs.push((line, (start, end)));
}

fn parse_alert_prefix(t: &str) -> Option<(&str, &str)> {
    let trimmed = t.trim_start();
    for label in [
        "NOTE",
        "TIP",
        "IMPORTANT",
        "WARNING",
        "CAUTION",
        "GOAL",
        "DECISION",
        "RISK",
    ] {
        let needle = format!("[!{label}]");
        if let Some(rest) = trimmed.strip_prefix(&needle) {
            return Some((label, rest.trim_start()));
        }
        let lower = format!("[!{}]", label.to_lowercase());
        if let Some(rest) = trimmed.strip_prefix(&lower) {
            return Some((label, rest.trim_start()));
        }
    }
    None
}

fn fair_share_widths(rows: &[Vec<String>], cols: usize, total_width: usize) -> Vec<usize> {
    // │ cell │ cell │ → borders = cols+1, plus one space pad each side of cell (= 2*cols)
    let chrome = cols
        .saturating_add(1)
        .saturating_add(cols.saturating_mul(2));
    let avail = total_width.saturating_sub(chrome).max(cols);
    let natural: Vec<usize> = (0..cols)
        .map(|c| {
            rows.iter()
                .map(|r| r.get(c).map_or(0, |s| s.width()))
                .max()
                .unwrap_or(1)
                .max(1)
        })
        .collect();
    let sum: usize = natural.iter().sum();
    if sum <= avail {
        return natural;
    }
    // Scale down proportionally, then fix remainder; floor at 1.
    let mut widths: Vec<usize> = natural
        .iter()
        .map(|&n| ((n * avail) / sum).max(1))
        .collect();
    let mut used: usize = widths.iter().sum();
    while used > avail {
        if let Some((i, _)) = widths
            .iter()
            .enumerate()
            .filter(|(_, w)| **w > 1)
            .max_by_key(|(_, w)| **w)
        {
            widths[i] -= 1;
            used -= 1;
        } else {
            break;
        }
    }
    while used < avail {
        if let Some((i, _)) = natural
            .iter()
            .enumerate()
            .max_by_key(|(i, n)| **n - widths[*i])
        {
            widths[i] += 1;
            used += 1;
        } else {
            break;
        }
    }
    widths
}

fn format_table_row(row: &[String], widths: &[usize]) -> String {
    let mut out = String::from("│");
    for (cell, &w) in row.iter().zip(widths.iter()) {
        let clipped = truncate_width(cell, w);
        let pad = w.saturating_sub(clipped.width());
        let _ = write!(out, " {clipped}{}│", " ".repeat(pad));
    }
    out
}

fn format_table_separator(widths: &[usize]) -> String {
    let mut out = String::from("├");
    for (i, &w) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(w.saturating_add(2)));
        if i + 1 == widths.len() {
            out.push('┤');
        } else {
            out.push('┼');
        }
    }
    out
}
