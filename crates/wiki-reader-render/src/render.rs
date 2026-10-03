//! pulldown-cmark → styled wrapped lines (V1 elements).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use pulldown_cmark::{
    BlockQuoteKind, CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};
use unicode_width::UnicodeWidthStr;
use wiki_reader_core::Index;
use wiki_reader_core::index::Page;
use wiki_reader_core::nav::{Target, resolve};
use wiki_reader_core::parse::{self, github_slug};
use wiki_reader_core::provider::PageKey;

use crate::diagrams::{
    DiagramEnv, DiagramTier, content_hash, diagram_lines_with_reason, select_tier,
};
use crate::images::{
    DiagramRequest, DiagramSize, DiagramSizeCache, DiagramTextReason, ImagePlan, ImageSlot,
    SlotSource, empty_diagram_size_cache, placeholder_text, plan_image, slot_geometry,
};
use crate::mermaid_raster::DiagramPalette;
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
    /// Fenced / indented code block body.
    CodeBlock,
    /// Code fence language label.
    CodeLang,
    /// Link text (class carried separately on [`LinkSpan`]).
    Link,
    /// Blockquote body.
    Quote,
    /// GFM alert (`NOTE`, `WARNING`, …); `kind` is a small tag id.
    Alert(u8),
    /// Horizontal rule glyph line.
    Rule,
    /// Table cell / border.
    Table,
    /// Table header row.
    TableHeader,
    /// List bullet / number marker.
    ListMarker,
    /// Task list checkbox marker.
    TaskMarker,
    /// Frontmatter metadata box (rules and label).
    Frontmatter,
    /// Frontmatter YAML key.
    FrontmatterKey,
    /// Frontmatter YAML value.
    FrontmatterValue,
    /// Frontmatter YAML punctuation (`:` and list dashes).
    FrontmatterPunct,
    /// Linked-from pane box edges and dividers.
    BacklinkBorder,
    /// Linked-from pane tag label (`Linked from`).
    BacklinkTag,
    /// Linked-from entry summary text.
    BacklinkSummary,
    /// Blank row reserved for an image the TUI draws over it.
    ImageSlot,
    /// `[image: alt] path — reason` shown instead of a picture.
    ImagePlaceholder,
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
    /// Focusable block actions (frontmatter / table / code), document order.
    pub block_actions: Vec<BlockAction>,
    /// Cell grid of every table, document order (P3-14 table viewer).
    pub tables: Vec<DocTable>,
    /// 1-based **rendered** line where each content block starts.
    pub block_starts: Vec<u32>,
    /// Rendered line (0-based) → source line (1-based), monotonic.
    pub source_map: Vec<u32>,
    /// Heading slug → 1-based **rendered** line.
    pub headings: Vec<(String, u32)>,
    /// Rows reserved for images, document order. Empty unless `RenderOpts::cell_px` is set.
    pub image_slots: Vec<ImageSlot>,
    /// Mermaid fences waiting on an off-thread size measure (image tier, cache miss).
    pub diagram_requests: Vec<DiagramRequest>,
    pub word_count: u32,
    pub updated: String,
}

/// One table's cell grid (plain cell text, links flattened), for the table viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocTable {
    /// Source line (1-based) of the header row.
    pub source_line: u32,
    /// 0-based display line of the table's first row.
    pub line: u32,
    /// Display rows the table occupies.
    pub height: u32,
    /// Header cells.
    pub header: Vec<String>,
    /// Body rows; every row has `header.len()` cells.
    pub rows: Vec<Vec<String>>,
}

/// Kind of a Tab-cycle block action (BA / P2-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockActionKind {
    /// Expand / collapse the frontmatter box.
    ToggleFrontmatter,
    /// Copy a fenced code block (OSC 52).
    CopyCode,
    /// Open the table in the modal viewer; `payload` is the index into [`RenderedDoc::tables`].
    ExpandTable,
    /// Open the Mermaid diagram in the image viewer; `payload` is the fence body.
    ExpandDiagram,
}

/// One focusable block action in document order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockAction {
    pub id: u32,
    pub kind: BlockActionKind,
    /// 0-based display line.
    pub line: u32,
    /// Display columns `[start, end)` on that line.
    pub cols: (u16, u16),
    /// For [`BlockActionKind::CopyCode`] and [`BlockActionKind::ExpandDiagram`]: code / fence
    /// body. For [`BlockActionKind::ExpandTable`]: table index. Else empty.
    pub payload: String,
}

/// Content-stable block id (kind + text fingerprint). Survives edits above the block.
fn content_block_id(kind: BlockActionKind, text: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    h ^= match kind {
        BlockActionKind::ToggleFrontmatter => 1,
        BlockActionKind::CopyCode => 3,
        BlockActionKind::ExpandTable => 4,
        BlockActionKind::ExpandDiagram => 5,
    };
    h = h.wrapping_mul(0x0100_0193);
    for b in text.as_bytes().iter().take(256) {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Optional expansion state for re-layout.
#[derive(Debug, Clone)]
pub struct RenderOpts {
    /// Expanded block-action ids (frontmatter / tables).
    pub expanded: std::collections::HashSet<u32>,
    /// Diagram render preference (ADR-0004).
    pub diagram_mode: wiki_reader_core::config::DiagramMode,
    /// Collection root. Local images resolve (and must stay) under it (ADR-0017); with `None`
    /// every image renders as a text placeholder.
    pub image_root: Option<PathBuf>,
    /// Terminal cell size in pixels. `Some` only when a graphics protocol is usable: block images
    /// then reserve rows ([`RenderedDoc::image_slots`]); `None` keeps the text placeholder.
    pub cell_px: Option<(u16, u16)>,
    /// Confirmed graphics protocol from the TUI probe (`cell_px.is_some()`). Never from `HERDR_ENV`.
    pub graphics: bool,
    /// `$TMUX` set — forces the text tier (ADR-0004).
    pub tmux: bool,
    /// `HERDR_ENV=1` — Kitty-only image preference when graphics are confirmed.
    pub herdr: bool,
    /// Mermaid colours (part of the size/slot cache key), filled from the active theme.
    pub diagram_palette: DiagramPalette,
    /// Shared Mermaid natural-size cache filled by the image worker.
    pub diagram_sizes: Arc<DiagramSizeCache>,
    /// Tallest image/diagram slot in rows (default [`crate::MAX_SLOT_ROWS`]).
    pub max_slot_rows: u16,
}

impl Default for RenderOpts {
    fn default() -> Self {
        Self {
            expanded: std::collections::HashSet::new(),
            diagram_mode: wiki_reader_core::config::DiagramMode::Auto,
            image_root: None,
            cell_px: None,
            graphics: false,
            tmux: false,
            herdr: false,
            diagram_palette: DiagramPalette::default(),
            diagram_sizes: empty_diagram_size_cache(),
            max_slot_rows: crate::MAX_SLOT_ROWS,
        }
    }
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
    render_with(source, page, from, index, width, &RenderOpts::default())
}

/// Render with expansion state for block actions.
#[must_use]
pub fn render_with(
    source: &str,
    page: Option<&Page>,
    from: &PageKey,
    index: &Index,
    width: u16,
    opts: &RenderOpts,
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
    state.expanded.clone_from(&opts.expanded);
    state.diagram_mode = opts.diagram_mode;
    state.image_root.clone_from(&opts.image_root);
    state.cell_px = opts.cell_px;
    state.graphics = opts.graphics;
    state.tmux = opts.tmux;
    state.herdr = opts.herdr;
    state.diagram_palette = opts.diagram_palette;
    state.diagram_sizes = Arc::clone(&opts.diagram_sizes);
    state.max_slot_rows = opts.max_slot_rows;

    if parsed.frontmatter.kind.is_some()
        || !parsed.frontmatter.props.is_empty()
        || parsed.frontmatter.title.is_some()
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
    let line_starts = line_start_offsets(&body);
    // Collected so a paragraph holding only an image can look ahead and become an image block.
    let events: Vec<_> = Parser::new_ext(&body, options).into_offset_iter().collect();
    let mut i = 0;
    while i < events.len() {
        let (event, range) = &events[i];
        let src = offset_to_line(&line_starts, body_line_offset, range.start);
        let src_end = offset_to_line(
            &line_starts,
            body_line_offset,
            range.end.saturating_sub(1).max(range.start),
        );
        if let Some(block) = state.image_only_paragraph(&events, i) {
            state.image_block(&block.dest, &block.alt, src);
            i = block.next;
            continue;
        }
        if let Some(image) = image_at(&events, i) {
            state.inline_image(&image.alt, src);
            i = image.end + 1;
            continue;
        }
        state.handle(event.clone(), src, src_end);
        i += 1;
    }
    state.finish_block();
    state.append_backlinks(from);

    if state.styled.is_empty() {
        state.push_empty(body_line_offset.max(1));
    }

    let lines: Vec<String> = state.styled.iter().map(StyledLine::plain).collect();
    let source_map: Vec<u32> = state.styled.iter().map(|l| l.source_line).collect();

    RenderedDoc {
        lines,
        styled: state.styled,
        links: state.links,
        block_actions: state.block_actions,
        tables: state.tables,
        block_starts: state.block_starts,
        source_map,
        headings: state.headings,
        image_slots: state.image_slots,
        diagram_requests: state.diagram_requests,
        word_count,
        updated,
    }
}

/// An image's destination and alt text, and the index of its `End(Image)` event.
struct ImageSpan {
    dest: String,
    alt: String,
    end: usize,
}

/// `Start(Image)` at `events[i]` with its alt text (plain text of the children).
fn image_at(events: &[(Event<'_>, std::ops::Range<usize>)], i: usize) -> Option<ImageSpan> {
    let Event::Start(Tag::Image { dest_url, .. }) = &events.get(i)?.0 else {
        return None;
    };
    let mut alt = String::new();
    let mut j = i + 1;
    loop {
        match &events.get(j)?.0 {
            Event::End(TagEnd::Image) => break,
            Event::Text(t) | Event::Code(t) => alt.push_str(t),
            Event::SoftBreak | Event::HardBreak => alt.push(' '),
            Event::Start(Tag::Image { .. }) => return None,
            _ => {}
        }
        j += 1;
    }
    Some(ImageSpan {
        dest: dest_url.to_string(),
        alt,
        end: j,
    })
}

/// A paragraph whose only content is one image.
struct ImageParagraph {
    dest: String,
    alt: String,
    /// Index of the first event after the paragraph.
    next: usize,
}

#[allow(clippy::struct_excessive_bools)] // layout FSM flags
struct LayoutState<'a> {
    width: usize,
    from: &'a PageKey,
    index: &'a Index,
    styled: Vec<StyledLine>,
    links: Vec<LinkSpan>,
    block_actions: Vec<BlockAction>,
    tables: Vec<DocTable>,
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
    /// Accumulated code body for `CopyCode` payload.
    code_body: String,
    /// Next source line for code body rows (advances per content line).
    code_line_src: u32,
    in_quote: bool,
    /// Alert label when quote opens with `[!NOTE]` / goal / decision / risk.
    alert_label: Option<String>,
    /// True after the first body block inside the current quote (gaps keep `│`).
    quote_body_started: bool,
    list_marker_pending: bool,
    in_table: bool,
    table_row: Vec<String>,
    /// Source line for the row currently being collected.
    table_row_src: u32,
    /// (source line, cells) per header/body row.
    table_rows: Vec<(u32, Vec<String>)>,
    table_header_done: bool,
    /// Links collected inside table cells; remapped to display cols in `flush_table`.
    table_links: Vec<TableCellLink>,
    heading_level: Option<u8>,
    heading_text: String,
    used_slugs: HashMap<String, u32>,
    link_id: u32,
    expanded: std::collections::HashSet<u32>,
    diagram_mode: wiki_reader_core::config::DiagramMode,
    image_root: Option<PathBuf>,
    cell_px: Option<(u16, u16)>,
    graphics: bool,
    tmux: bool,
    herdr: bool,
    diagram_palette: DiagramPalette,
    diagram_sizes: Arc<DiagramSizeCache>,
    max_slot_rows: u16,
    image_slots: Vec<ImageSlot>,
    diagram_requests: Vec<DiagramRequest>,
}

/// A link whose display columns are only known after the table is laid out.
struct TableCellLink {
    /// Index into `table_rows` once the row is pushed; while building, `table_rows.len()`.
    row: usize,
    cell: usize,
    /// Display-column range within the cell's plain text.
    start: u16,
    end: u16,
    raw: String,
    class: LinkClass,
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
            block_actions: Vec::new(),
            tables: Vec::new(),
            block_starts: Vec::new(),
            headings: Vec::new(),
            cur: Vec::new(),
            cur_src: body_line_offset.max(1),
            style_stack: Vec::new(),
            in_link: None,
            list_stack: Vec::new(),
            in_code_block: false,
            code_fence_lang: String::new(),
            code_body: String::new(),
            code_line_src: 1,
            in_quote: false,
            alert_label: None,
            quote_body_started: false,
            list_marker_pending: false,
            in_table: false,
            table_row: Vec::new(),
            table_row_src: 1,
            table_rows: Vec::new(),
            table_header_done: false,
            table_links: Vec::new(),
            heading_level: None,
            heading_text: String::new(),
            used_slugs: HashMap::new(),
            link_id: 0,
            expanded: std::collections::HashSet::new(),
            diagram_mode: wiki_reader_core::config::DiagramMode::Auto,
            image_root: None,
            cell_px: None,
            graphics: false,
            tmux: false,
            herdr: false,
            diagram_palette: DiagramPalette::default(),
            diagram_sizes: empty_diagram_size_cache(),
            max_slot_rows: crate::MAX_SLOT_ROWS,
            image_slots: Vec::new(),
            diagram_requests: Vec::new(),
        }
    }

    /// `Start(Paragraph) Start(Image) … End(Image) End(Paragraph)` at `events[i]`, outside
    /// quotes, lists and tables (slots are not indented).
    fn image_only_paragraph(
        &self,
        events: &[(Event<'_>, std::ops::Range<usize>)],
        i: usize,
    ) -> Option<ImageParagraph> {
        if !matches!(events[i].0, Event::Start(Tag::Paragraph))
            || self.in_quote
            || self.in_table
            || !self.list_stack.is_empty()
        {
            return None;
        }
        let image = image_at(events, i + 1)?;
        matches!(events.get(image.end + 1)?.0, Event::End(TagEnd::Paragraph)).then(|| {
            ImageParagraph {
                dest: image.dest,
                alt: image.alt,
                next: image.end + 2,
            }
        })
    }

    /// An image inside running text, a list, a quote or a table: only block-level images are
    /// drawn, so this stays a terse `[image: alt]`.
    fn inline_image(&mut self, alt: &str, src: u32) {
        self.ensure_list_marker(src);
        let alt = alt.trim();
        let text = if alt.is_empty() {
            "[image]".to_owned()
        } else {
            format!("[image: {alt}]")
        };
        // Inside a link the text must stay link-styled so the link keeps its click target.
        let kind = if self.in_link.is_some() {
            StyleKind::Link
        } else {
            StyleKind::ImagePlaceholder
        };
        self.push_span(text, kind, src);
    }

    /// Reserve rows for a local image, or show the text placeholder (ADR-0017).
    fn image_block(&mut self, dest: &str, alt: &str, src: u32) {
        self.ensure_block_gap(src);
        self.mark_block(src);
        self.cur_src = src;
        let max_cols = u16::try_from(self.width).unwrap_or(u16::MAX);
        let plan = plan_image(
            self.image_root.as_deref(),
            &self.from.relative_path,
            dest,
            self.cell_px,
            max_cols,
            self.max_slot_rows,
        );
        match plan {
            ImagePlan::Slot { source, cols, rows } => {
                let line = u32::try_from(self.styled.len()).unwrap_or(0);
                for _ in 0..rows {
                    self.styled.push(StyledLine {
                        spans: vec![StyledSpan {
                            text: String::new(),
                            kind: StyleKind::ImageSlot,
                        }],
                        source_line: src,
                    });
                }
                // A real gap row: slot rows are blank, so the next block would otherwise
                // see a blank line and skip its own gap, leaving text against the picture.
                self.styled.push(StyledLine {
                    spans: Vec::new(),
                    source_line: src,
                });
                self.image_slots.push(ImageSlot {
                    line,
                    rows,
                    cols,
                    source,
                    alt: alt.to_owned(),
                });
            }
            ImagePlan::Placeholder(reason) => {
                self.push_span(
                    placeholder_text(alt, dest, &reason),
                    StyleKind::ImagePlaceholder,
                    src,
                );
                self.finish_block();
            }
        }
    }

    /// Mermaid fence: image slot when sized and legible, else text/source with a reason.
    #[allow(clippy::too_many_lines)] // layout FSM: slot vs text + CopyCode in one place
    fn emit_mermaid(&mut self, body: &str, close_src: u32) {
        let width = u16::try_from(self.width).unwrap_or(80);
        let env = DiagramEnv {
            tmux: self.tmux,
            herdr: self.herdr,
            kitty_graphics: self.graphics && self.cell_px.is_some(),
        };
        let tier = select_tier(self.diagram_mode, &env);
        let hash = content_hash(body);
        let palette = self.diagram_palette;

        let image_plan = if tier == DiagramTier::Image {
            match self.diagram_sizes.get(hash, palette) {
                Some(DiagramSize::Natural { px_w, px_h }) => {
                    let cell_px = self.cell_px.unwrap_or((8, 17));
                    if crate::mermaid_raster::is_legible(
                        px_w,
                        px_h,
                        cell_px,
                        width,
                        self.max_slot_rows,
                    ) {
                        let (cols, rows) =
                            slot_geometry(px_w, px_h, cell_px, width, self.max_slot_rows);
                        Some((cols, rows))
                    } else {
                        // Width-scoped: do not overwrite the natural-size cache entry.
                        None
                    }
                }
                Some(DiagramSize::Text(_)) => None,
                None => {
                    if let Some(cell_px) = self.cell_px {
                        self.diagram_requests.push(DiagramRequest {
                            hash,
                            source: body.to_owned(),
                            width,
                            palette,
                            cell_px,
                        });
                    }
                    None
                }
            }
        } else {
            None
        };

        let start_line = u32::try_from(self.styled.len()).unwrap_or(0);
        if let Some((cols, rows)) = image_plan {
            self.ensure_block_gap(close_src);
            self.mark_block(close_src);
            for _ in 0..rows {
                self.styled.push(StyledLine {
                    spans: vec![StyledSpan {
                        text: String::new(),
                        kind: StyleKind::ImageSlot,
                    }],
                    source_line: close_src,
                });
            }
            self.styled.push(StyledLine {
                spans: Vec::new(),
                source_line: close_src,
            });
            self.image_slots.push(ImageSlot {
                line: start_line,
                rows,
                cols,
                source: SlotSource::Mermaid {
                    hash,
                    palette,
                    source: body.to_owned(),
                },
                alt: "diagram".into(),
            });
        } else {
            let reason_owned: Option<String> = match tier {
                DiagramTier::Image if self.cell_px.is_none() || !self.graphics => {
                    Some("no graphics protocol".into())
                }
                DiagramTier::Image => match self.diagram_sizes.get(hash, palette) {
                    Some(DiagramSize::Text(DiagramTextReason::Failed(msg))) => Some(msg),
                    Some(
                        DiagramSize::Text(DiagramTextReason::TooWide) | DiagramSize::Natural { .. },
                    ) => Some("too wide".into()),
                    None => None, // measuring: plain text tier until the cache warms
                },
                DiagramTier::Text
                    if env.tmux
                        && matches!(
                            self.diagram_mode,
                            wiki_reader_core::config::DiagramMode::Image
                        ) =>
                {
                    Some("tmux: text tier".into())
                }
                _ => None,
            };
            let reason = reason_owned.as_deref();
            let paint_tier = if tier == DiagramTier::Image {
                DiagramTier::Text
            } else {
                tier
            };
            let (lines, _) = diagram_lines_with_reason(body, paint_tier, width, reason);
            for (i, line) in lines.iter().enumerate() {
                let src_line = close_src.saturating_add(u32::try_from(i).unwrap_or(0));
                self.push_raw_line(line.clone(), StyleKind::CodeBlock, src_line);
            }
        }

        let end = u16::try_from(
            body.lines()
                .next()
                .map_or(8usize, |l| l.chars().count())
                .min(usize::from(u16::MAX)),
        )
        .unwrap_or(8);
        // Expand first: at equal columns the stable focus sort keeps it ahead of copy, so Enter on
        // the line expands while a click still copies (last hit wins).
        let fingerprint = format!("mermaid\n{body}");
        self.block_actions.push(BlockAction {
            id: content_block_id(BlockActionKind::ExpandDiagram, &fingerprint),
            kind: BlockActionKind::ExpandDiagram,
            line: start_line,
            cols: (0, end.max(1)),
            payload: body.to_owned(),
        });
        let id = content_block_id(BlockActionKind::CopyCode, &fingerprint);
        self.block_actions.push(BlockAction {
            id,
            kind: BlockActionKind::CopyCode,
            line: start_line,
            cols: (0, end.max(1)),
            payload: body.to_owned(),
        });
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
                self.ensure_block_gap(src);
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
                // List items stay tight; quotes keep the bar on gaps; else blank gap.
                if !self.list_stack.is_empty() {
                    self.finish_block();
                } else if self.in_quote {
                    self.quote_paragraph_gap(src);
                } else {
                    self.ensure_block_gap(src);
                }
                self.mark_block(src);
                self.cur_src = src;
                if self.in_quote {
                    self.push_span("│ ".into(), StyleKind::Quote, src);
                }
                self.ensure_list_marker(src);
            }
            Tag::Heading { level, .. } => {
                let lv = heading_u8(level);
                self.ensure_block_gap(src);
                // H2 and under: one more blank above than below (the gap below is the
                // normal single block gap), so a heading groups with its body.
                let n = self.styled.len();
                if lv >= 2
                    && n >= 2
                    && self.last_line_blank()
                    && !self.styled[n - 2].spans.is_empty()
                {
                    self.styled.push(StyledLine {
                        spans: Vec::new(),
                        source_line: src,
                    });
                }
                self.mark_block(src);
                self.cur_src = src;
                self.heading_level = Some(lv);
                self.heading_text.clear();
                self.style_stack.push(StyleKind::Heading(lv));
                // Rendered mode drops the `#` markers; extra top gap for H1/H2
                // so levels read without hashes. Raw view shows the syntax.
                if lv <= 2 && !self.styled.is_empty() {
                    self.ensure_block_gap(src);
                }
            }
            Tag::BlockQuote(kind) => {
                self.ensure_block_gap(src);
                self.mark_block(src);
                self.in_quote = true;
                self.quote_body_started = false;
                self.alert_label = kind.map(|k| match k {
                    BlockQuoteKind::Note => "NOTE".into(),
                    BlockQuoteKind::Tip => "TIP".into(),
                    BlockQuoteKind::Important => "IMPORTANT".into(),
                    BlockQuoteKind::Warning => "WARNING".into(),
                    BlockQuoteKind::Caution => "CAUTION".into(),
                });
                if let Some(label) = self.alert_label.clone() {
                    let kind = StyleKind::Alert(alert_kind_id(&label));
                    self.push_span(format!("│ [{label}]"), kind, src);
                    self.commit_line(src);
                }
            }
            Tag::CodeBlock(kind) => {
                self.ensure_block_gap(src);
                self.mark_block(src);
                self.in_code_block = true;
                self.code_body.clear();
                self.code_line_src = src.saturating_add(1);
                self.code_fence_lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.cur_src = src;
                if crate::diagrams::is_mermaid_lang(&self.code_fence_lang) {
                    // Body accumulated; paint on TagEnd via text/source tier.
                    return;
                }
                let label = if self.code_fence_lang.is_empty() {
                    "── code ──".into()
                } else {
                    format!("── {} ──", self.code_fence_lang)
                };
                let line = u32::try_from(self.styled.len()).unwrap_or(0);
                self.push_span(label, StyleKind::CodeLang, src);
                let end = u16::try_from(self.cur_width().min(usize::from(u16::MAX))).unwrap_or(1);
                // Provisional id from lang; rewritten when body is known.
                let id = content_block_id(BlockActionKind::CopyCode, &self.code_fence_lang);
                self.block_actions.push(BlockAction {
                    id,
                    kind: BlockActionKind::CopyCode,
                    line,
                    cols: (0, end.max(1)),
                    payload: String::new(),
                });
                self.commit_line(src);
            }
            Tag::List(start) => {
                // Gap once for the whole list; items stay tight.
                if self.list_stack.is_empty() {
                    if self.in_quote {
                        self.quote_paragraph_gap(src);
                    } else {
                        self.ensure_block_gap(src);
                    }
                } else {
                    self.finish_block();
                }
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
                // Marker before start_col so the link hit does not cover the bullet.
                self.ensure_list_marker(src);
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
            Tag::Table(_) => {
                self.ensure_block_gap(src);
                self.mark_block(src);
                self.in_table = true;
                self.table_rows.clear();
                self.table_links.clear();
                self.table_header_done = false;
            }
            Tag::TableHead | Tag::TableRow => {
                self.table_row.clear();
                self.table_row_src = src;
            }
            Tag::TableCell => {
                self.style_stack.push(StyleKind::Table);
            }
            // Images never get here: `image_at` consumes them whole before events are handled.
            _ => {}
        }
    }

    #[allow(clippy::too_many_lines)] // mermaid branch + existing ends
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
                self.commit_line(src);
                if level <= 2 {
                    self.push_span("─".repeat(self.width.min(40)), StyleKind::Rule, src);
                    self.commit_line(src);
                }
                // H3+: nothing extra below; the next block's normal gap is one blank.
                self.heading_text.clear();
            }
            TagEnd::BlockQuote(_) => {
                self.in_quote = false;
                self.alert_label = None;
                self.quote_body_started = false;
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                let close_src = src_end.max(src);
                if crate::diagrams::is_mermaid_lang(&self.code_fence_lang) {
                    let body = std::mem::take(&mut self.code_body);
                    self.emit_mermaid(&body, close_src);
                    self.code_fence_lang.clear();
                    return;
                }
                // No closing fence line: the label above frames the block.
                if let Some(action) = self
                    .block_actions
                    .iter_mut()
                    .rev()
                    .find(|a| a.kind == BlockActionKind::CopyCode && a.payload.is_empty())
                {
                    let body = std::mem::take(&mut self.code_body);
                    let fp = format!("{}\n{body}", self.code_fence_lang);
                    action.id = content_block_id(BlockActionKind::CopyCode, &fp);
                    action.payload = body;
                }
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
                    let end_col =
                        u16::try_from(self.cur_width().min(usize::from(u16::MAX))).unwrap_or(0);
                    let class = match resolve(&lb.raw, self.from, self.index).target {
                        Target::External(_) => LinkClass::External,
                        Target::Unsupported(_) => LinkClass::Unsupported,
                        Target::Unresolved(_) => LinkClass::Broken,
                        _ => LinkClass::Internal,
                    };
                    if self.in_table {
                        // Cell text isn't on a display line yet; map in flush_table.
                        self.table_links.push(TableCellLink {
                            row: self.table_rows.len(),
                            cell: self.table_row.len(),
                            start: lb.start_col,
                            end: end_col,
                            raw: lb.raw,
                            class,
                        });
                    } else {
                        let line = u32::try_from(self.styled.len()).unwrap_or(0);
                        coalesce_link_segment(&mut lb.segments, line, lb.start_col, end_col);
                        lb.segments.retain(|&(_, (a, b))| a < b);
                        self.links.push(LinkSpan {
                            id: LinkId(self.link_id),
                            raw_target: lb.raw,
                            class,
                            segments: lb.segments,
                            backlink: false,
                        });
                        self.link_id = self.link_id.saturating_add(1);
                    }
                }
            }
            TagEnd::Table => {
                self.flush_table();
                self.in_table = false;
            }
            TagEnd::TableHead => {
                if !self.table_row.is_empty() {
                    self.table_rows
                        .push((self.table_row_src, std::mem::take(&mut self.table_row)));
                    self.table_header_done = true;
                }
            }
            TagEnd::TableRow => {
                if !self.table_row.is_empty() {
                    self.table_rows
                        .push((self.table_row_src, std::mem::take(&mut self.table_row)));
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
            if !self.code_body.is_empty() {
                self.code_body.push('\n');
            }
            self.code_body.push_str(body);
            // Mermaid: accumulate only; TagEnd paints diagram_lines once.
            if crate::diagrams::is_mermaid_lang(&self.code_fence_lang) {
                self.code_line_src = self
                    .code_line_src
                    .saturating_add(u32::try_from(body.split('\n').count()).unwrap_or(1));
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
                let kind = StyleKind::Alert(alert_kind_id(label));
                // Replace the bare quote gutter with a labelled alert header.
                if self.cur.len() == 1
                    && self.cur[0].text == "│ "
                    && self.cur[0].kind == StyleKind::Quote
                {
                    self.cur.clear();
                    self.push_span(format!("│ [{label}] "), kind, src);
                } else {
                    self.push_span(format!("[{label}] "), kind, src);
                }
                rest.to_owned()
            } else {
                t.to_owned()
            }
        } else {
            t.to_owned()
        };
        // Colour emoji ignore terminal fg; substitute only a leading marker so
        // mid-sentence use stays literal. U+2212 takes the text colour like `+`.
        let at_start = self.list_marker_pending || self.cur.is_empty();
        self.ensure_list_marker(src);
        let text = if at_start {
            leading_plus_minus(&text)
        } else {
            text
        };
        if self.heading_level.is_some() {
            self.heading_text.push_str(&text);
        }
        if let Some(lb) = self.in_link.as_mut() {
            lb.text.push_str(&text);
        }
        if !text.is_empty() {
            self.push_wrapping(&text, src);
        }
    }

    fn inline_code(&mut self, t: &CowStr<'_>, src: u32) {
        self.ensure_list_marker(src);
        let s = t.to_string();
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
        let fm_fp = parsed
            .frontmatter
            .props
            .iter()
            .map(|(k, v)| match v {
                parse::FmProp::Scalar(s) => format!("{k}={s}"),
                parse::FmProp::List(xs) => format!("{k}=[{}]", xs.join(",")),
            })
            .collect::<Vec<_>>()
            .join(";");
        let id = content_block_id(BlockActionKind::ToggleFrontmatter, &fm_fp);
        let expanded = self.expanded.contains(&id);
        let line = u32::try_from(self.styled.len()).unwrap_or(0);
        // Top and bottom rules share one width (the full pane) so the box reads as a unit.
        let rule_w = self.width.max(1);
        let arrow = if expanded { '▾' } else { '▸' };
        let label = format!("── frontmatter {arrow} ");
        let fill = rule_w.saturating_sub(label.width());
        self.push_span(
            format!("{label}{}", "─".repeat(fill)),
            StyleKind::Frontmatter,
            1,
        );
        let end = u16::try_from(self.cur_width().min(usize::from(u16::MAX))).unwrap_or(1);
        self.block_actions.push(BlockAction {
            id,
            kind: BlockActionKind::ToggleFrontmatter,
            line,
            cols: (0, end.max(1)),
            payload: String::new(),
        });
        self.commit_line(1);
        if !expanded {
            return;
        }
        let key_w = parsed
            .frontmatter
            .props
            .iter()
            .map(|(k, _)| k.width())
            .max()
            .unwrap_or(0)
            .max(1);
        for (key, val) in &parsed.frontmatter.props {
            match val {
                parse::FmProp::Scalar(s) => {
                    let pad = " ".repeat(key_w.saturating_sub(key.width()));
                    self.push_span(key.clone(), StyleKind::FrontmatterKey, 1);
                    self.push_span(":".into(), StyleKind::FrontmatterPunct, 1);
                    self.push_span(format!("{pad} {s}"), StyleKind::FrontmatterValue, 1);
                    self.commit_line(1);
                }
                parse::FmProp::List(xs) => {
                    let pad = " ".repeat(key_w.saturating_sub(key.width()));
                    self.push_span(key.clone(), StyleKind::FrontmatterKey, 1);
                    self.push_span(format!(":{pad}"), StyleKind::FrontmatterPunct, 1);
                    self.commit_line(1);
                    for item in xs {
                        self.push_span("  - ".into(), StyleKind::FrontmatterPunct, 1);
                        self.push_span(item.clone(), StyleKind::FrontmatterValue, 1);
                        self.commit_line(1);
                    }
                }
            }
        }
        self.push_span("─".repeat(rule_w), StyleKind::Frontmatter, 1);
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
        self.push_span(format!("{indent}{marker}"), StyleKind::ListMarker, src);
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

    /// Push a full display row without wrapping (diagrams, table rows).
    fn push_raw_line(&mut self, text: String, kind: StyleKind, src: u32) {
        self.finish_block();
        self.cur_src = src;
        self.cur.push(StyledSpan { text, kind });
        self.commit_line(src);
    }

    fn push_span(&mut self, text: String, kind: StyleKind, src: u32) {
        if text.is_empty() {
            return;
        }
        self.cur_src = src;
        if self.in_table {
            // Cell text is wrapped to its column by `flush_table`; wrapping here
            // would hoist the overflow out of the table as a paragraph line.
            self.push_span_piece(&text, kind, src);
            return;
        }
        let mut rest = text;
        while !rest.is_empty() {
            let avail = self.width.saturating_sub(self.cur_width());
            if avail == 0 {
                self.commit_quote_wrap(src);
                continue;
            }
            if rest.width() <= avail {
                self.push_span_piece(&rest, kind, src);
                break;
            }
            let (take, next) = split_at_word_boundary(&rest, avail);
            if take.is_empty() {
                // Token does not fit on this line: move to next if we already have content.
                let only_quote_gutter =
                    self.in_quote && self.cur.len() == 1 && self.cur[0].text == "│ ";
                if self.cur_width() > 0 && !only_quote_gutter {
                    self.commit_quote_wrap(src);
                    continue;
                }
                // Hard-break tokens wider than the remaining pane (after optional gutter).
                let hard_w = if only_quote_gutter {
                    avail.max(1)
                } else {
                    self.width.max(1)
                };
                let (take2, next2) = split_at_width(&rest, hard_w);
                self.push_span_piece(&take2, kind, src);
                rest = next2;
                if !rest.is_empty() {
                    self.commit_quote_wrap(src);
                }
                continue;
            }
            self.push_span_piece(&take, kind, src);
            rest = next;
            if !rest.is_empty() {
                self.commit_quote_wrap(src);
            }
        }
    }

    /// Commit a wrapped line and re-prefix the quote bar on the continuation.
    fn commit_quote_wrap(&mut self, src: u32) {
        self.commit_line(src);
        if self.in_quote {
            self.cur_src = src;
            self.cur.push(StyledSpan {
                text: "│ ".into(),
                kind: StyleKind::Quote,
            });
        }
    }

    /// Inter-paragraph gap inside a quote keeps `│`; first body block stays tight.
    fn quote_paragraph_gap(&mut self, src: u32) {
        self.finish_block();
        if self.quote_body_started && !self.last_line_blank() {
            let gap_src = self.styled.last().map_or(src, |l| l.source_line);
            self.cur_src = gap_src;
            self.cur.push(StyledSpan {
                text: "│".into(),
                kind: StyleKind::Quote,
            });
            self.commit_line(gap_src);
        }
        self.quote_body_started = true;
    }

    fn push_span_piece(&mut self, take: &str, kind: StyleKind, _src: u32) {
        if take.is_empty() {
            return;
        }
        let is_link =
            self.in_link.is_some() && matches!(kind, StyleKind::Link | StyleKind::BacklinkSummary);
        let start = u16::try_from(self.cur_width().min(usize::from(u16::MAX))).unwrap_or(0);
        self.cur.push(StyledSpan {
            text: take.to_owned(),
            kind,
        });
        // Table cell links are remapped in flush_table; don't record display lines here.
        if is_link && !self.in_table {
            let end = u16::try_from(self.cur_width().min(usize::from(u16::MAX))).unwrap_or(start);
            let line = u32::try_from(self.styled.len()).unwrap_or(0);
            if start < end
                && let Some(lb) = self.in_link.as_mut()
            {
                coalesce_link_segment(&mut lb.segments, line, start, end);
            }
        }
    }

    fn push_wrapping(&mut self, text: &str, src: u32) {
        self.push_span(text.to_owned(), self.current_kind(), src);
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

    fn last_line_blank(&self) -> bool {
        self.styled
            .last()
            .is_none_or(|l| l.spans.is_empty() || l.spans.iter().all(|s| s.text.trim().is_empty()))
    }

    /// Blank line before a top-level block when the previous line has content.
    ///
    /// The gap inherits the previous line's source so it does not join the
    /// following block's source run (cursor restore uses run starts).
    fn ensure_block_gap(&mut self, src: u32) {
        self.finish_block();
        if self.styled.is_empty() || self.last_line_blank() {
            return;
        }
        let gap_src = self.styled.last().map_or(src, |l| l.source_line);
        self.styled.push(StyledLine {
            spans: Vec::new(),
            source_line: gap_src,
        });
    }

    /// Append a "Linked from" pane (B1). Box-drawn section: tag header, title
    /// rows (links), optional summaries, dividers between entries. Omitted when
    /// empty. Raw mode never calls this.
    fn append_backlinks(&mut self, page: &PageKey) {
        let mut sources: Vec<&PageKey> = self
            .index
            .backlinks(page)
            .into_iter()
            .filter_map(|i| self.index.edges.get(i))
            .map(|e| &e.from)
            .filter(|from| *from != page)
            .collect();
        sources.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        sources.dedup();
        if sources.is_empty() {
            return;
        }
        let src = self.styled.last().map_or(1, |l| l.source_line.max(1));
        self.finish_block();
        // Two blank rows above the pane.
        self.commit_line(src);
        self.commit_line(src);
        self.mark_block(src);
        let heading_line = u32::try_from(self.styled.len()).unwrap_or(0);
        self.push_pane_top_border(src, "Linked from");
        self.headings.push((
            unique_slug("linked-from", &mut self.used_slugs),
            heading_line.saturating_add(1),
        ));
        let last = sources.len().saturating_sub(1);
        for (i, from) in sources.into_iter().enumerate() {
            let title = self.index.pages.get(from).map_or_else(
                || {
                    from.relative_path.file_stem().map_or_else(
                        || from.relative_path.display().to_string(),
                        |s| s.to_string_lossy().into_owned(),
                    )
                },
                |p| p.title.clone(),
            );
            let summary = self.index.pages.get(from).and_then(|p| {
                p.parsed
                    .frontmatter
                    .summary
                    .clone()
                    .filter(|s| !s.is_empty())
                    .or_else(|| first_paragraph(&p.parsed.body))
            });
            let target = format!(
                "/{}",
                from.relative_path.to_string_lossy().replace('\\', "/")
            );
            self.push_backlink_entry(&title, summary.as_deref(), &target, src);
            if i < last {
                self.push_span(
                    pane_h_border(self.width, '├', '┤'),
                    StyleKind::BacklinkBorder,
                    src,
                );
                self.commit_line(src);
            }
        }
        self.push_span(
            pane_h_border(self.width, '└', '┘'),
            StyleKind::BacklinkBorder,
            src,
        );
        self.commit_line(src);
    }

    /// Title (link) + optional one-line summary; `LinkSpan` covers title and summary.
    /// Entry rows are `│ content │` so the pane has side borders; segments exclude the glyphs.
    fn push_backlink_entry(&mut self, title: &str, summary: Option<&str>, target: &str, src: u32) {
        let line = u32::try_from(self.styled.len()).unwrap_or(0);
        self.in_link = Some(LinkBuild {
            raw: target.to_owned(),
            text: title.to_owned(),
            start_line: line,
            start_col: 0,
            segments: Vec::new(),
        });

        // `│ ` + content + ` │` — need at least 4 columns for sides.
        let side = 4usize;
        let with_sides = self.width >= side;
        let avail = if with_sides {
            self.width.saturating_sub(side).max(1)
        } else {
            self.width.max(1)
        };

        self.push_backlink_row(title, StyleKind::Link, avail, src, true, with_sides);
        if let Some(summary) = summary.filter(|s| !s.is_empty()) {
            // One line only — pad/ellipsis so a trailing period never orphans.
            let piece = if summary.width() > avail {
                let room = avail.saturating_sub(1).max(1);
                let (take, _) = split_at_width(summary, room);
                let mut s = take;
                s.push('…');
                s
            } else {
                summary.to_owned()
            };
            self.push_backlink_row(
                &piece,
                StyleKind::BacklinkSummary,
                avail,
                src,
                false,
                with_sides,
            );
        }

        if let Some(mut lb) = self.in_link.take() {
            lb.segments.retain(|&(_, (a, b))| a < b);
            self.links.push(LinkSpan {
                id: LinkId(self.link_id),
                raw_target: lb.raw,
                class: LinkClass::Internal,
                segments: lb.segments,
                backlink: true,
            });
            self.link_id = self.link_id.saturating_add(1);
        }
    }

    /// One entry row; with sides: `│ content… │`, padded to `avail` for a solid focus block.
    fn push_backlink_row(
        &mut self,
        text: &str,
        kind: StyleKind,
        avail: usize,
        src: u32,
        ellipsis_ok: bool,
        with_sides: bool,
    ) {
        if with_sides {
            self.push_span("│ ".into(), StyleKind::BacklinkBorder, src);
        }
        let text_w = text.width();
        if ellipsis_ok && text_w > avail {
            let room = avail.saturating_sub(1).max(1);
            let (take, _) = split_at_width(text, room);
            if !take.is_empty() {
                self.push_span(take, kind, src);
            }
            self.push_span("…".into(), kind, src);
            // Pad after … — split_at_width can undershoot `room` on a wide glyph.
            let leading = if with_sides { 2 } else { 0 };
            let used = self.cur_width().saturating_sub(leading);
            if used < avail {
                self.push_span(" ".repeat(avail.saturating_sub(used)), kind, src);
            }
        } else {
            let (take, _) = split_at_width(text, avail);
            self.push_span(take, kind, src);
            let leading = if with_sides { 2 } else { 0 };
            let used = self.cur_width().saturating_sub(leading);
            if used < avail {
                self.push_span(" ".repeat(avail.saturating_sub(used)), kind, src);
            }
        }
        if with_sides {
            self.push_span(" │".into(), StyleKind::BacklinkBorder, src);
        }
        self.commit_line(src);
    }

    /// Top border as Border + Tag + Border spans so the label can be themed.
    fn push_pane_top_border(&mut self, src: u32, tag: &str) {
        let width = self.width;
        if width == 0 {
            return;
        }
        if width == 1 {
            self.push_span("┌".into(), StyleKind::BacklinkBorder, src);
            self.commit_line(src);
            return;
        }
        let prefix = format!("┌ {tag} ");
        let suffix_w = 1; // ┐
        if prefix.width() + suffix_w > width {
            self.push_span(
                pane_h_border(width, '┌', '┐'),
                StyleKind::BacklinkBorder,
                src,
            );
            self.commit_line(src);
            return;
        }
        let fill = width
            .saturating_sub(prefix.width())
            .saturating_sub(suffix_w);
        self.push_span("┌ ".into(), StyleKind::BacklinkBorder, src);
        self.push_span(tag.to_owned(), StyleKind::BacklinkTag, src);
        self.push_span(
            format!(" {}┐", "─".repeat(fill)),
            StyleKind::BacklinkBorder,
            src,
        );
        self.commit_line(src);
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

    /// Lay the table out, then record its cell grid and an expand action.
    fn flush_table(&mut self) {
        let start = self.styled.len();
        let has_header = self.table_header_done;
        let grid = self.table_rows.clone();
        self.layout_table();
        let Some((source_line, first)) = grid.first().cloned() else {
            return;
        };
        let cols = grid.iter().map(|(_, c)| c.len()).max().unwrap_or(1).max(1);
        let pad = |mut row: Vec<String>| {
            row.resize(cols, String::new());
            row
        };
        let mut rows = grid.into_iter().map(|(_, c)| pad(c));
        // ponytail: a table without a header row (can't happen with GFM) gets an empty header.
        let header = if has_header {
            rows.next().unwrap_or_default()
        } else {
            vec![String::new(); cols]
        };
        let rows: Vec<Vec<String>> = rows.collect();
        let fingerprint = format!(
            "{}\n{}",
            first.join("\t"),
            rows.first().map_or(String::new(), |r| r.join("\t"))
        );
        let line = u32::try_from(start).unwrap_or(0);
        let width = self.styled.get(start).map_or(1, |l| {
            u16::try_from(l.plain().width().min(usize::from(u16::MAX))).unwrap_or(1)
        });
        self.block_actions.push(BlockAction {
            id: content_block_id(BlockActionKind::ExpandTable, &fingerprint),
            kind: BlockActionKind::ExpandTable,
            line,
            cols: (0, width.max(1)),
            payload: self.tables.len().to_string(),
        });
        self.tables.push(DocTable {
            source_line,
            line,
            height: u32::try_from(self.styled.len() - start).unwrap_or(0),
            header,
            rows,
        });
    }

    fn layout_table(&mut self) {
        if self.table_rows.is_empty() {
            self.table_links.clear();
            return;
        }
        let mut rows = std::mem::take(&mut self.table_rows);
        let pending = std::mem::take(&mut self.table_links);
        let cols = rows
            .iter()
            .map(|(_, cells)| cells.len())
            .max()
            .unwrap_or(1)
            .max(1);
        for (_, row) in &mut rows {
            while row.len() < cols {
                row.push(String::new());
            }
        }
        let cell_rows: Vec<&[String]> = rows.iter().map(|(_, c)| c.as_slice()).collect();
        let Some(widths) = fair_share_widths(&cell_rows, cols, self.width) else {
            // Mins do not fit: fall back to a source-like dump with a note.
            let src = rows.first().map_or(1, |(s, _)| *s);
            let mut note = format!(
                "│ table too wide for pane ({} cols); showing cells unwrapped",
                self.width
            );
            while !note.is_empty() {
                if note.width() <= self.width {
                    self.push_raw_line(std::mem::take(&mut note), StyleKind::Table, src);
                    break;
                }
                let (take, next) = split_at_width(&note, self.width.max(1));
                self.push_raw_line(take, StyleKind::Table, src);
                note = next;
            }
            for (row_src, row) in &rows {
                self.push_raw_line(format!("│ {}", row.join(" | ")), StyleKind::Table, *row_src);
            }
            self.table_header_done = false;
            // ponytail: no hit targets in the unwrapped dump; upgrade: map cell offsets.
            return;
        };
        let header_count = usize::from(self.table_header_done);
        let top_src = rows.first().map_or(1, |(s, _)| *s);
        self.push_raw_line(format_table_top(&widths), StyleKind::Table, top_src);
        for (i, (row_src, row)) in rows.iter().enumerate() {
            let kind = if i < header_count {
                StyleKind::TableHeader
            } else {
                StyleKind::Table
            };
            let lines = format_table_row_wrapped(row, &widths);
            let first_line = u32::try_from(self.styled.len()).unwrap_or(0);
            for line in &lines {
                self.push_raw_line(line.clone(), kind, *row_src);
            }
            // Remap cell-relative links onto the laid-out display columns.
            for link in pending.iter().filter(|l| l.row == i) {
                let segs = table_link_segments(row, &widths, link.cell, link.start, link.end);
                let mut segments: Vec<(u32, (u16, u16))> = segs
                    .into_iter()
                    .map(|(r, c0, c1)| (first_line.saturating_add(r), (c0, c1)))
                    .filter(|&(_, (a, b))| a < b)
                    .collect();
                if segments.is_empty() {
                    continue;
                }
                // Merge adjacent runs on the same line.
                segments.sort_by_key(|s| (s.0, s.1.0));
                let mut merged: Vec<(u32, (u16, u16))> = Vec::new();
                for seg in segments {
                    if let Some(last) = merged.last_mut()
                        && last.0 == seg.0
                        && last.1.1 >= seg.1.0
                    {
                        last.1.1 = last.1.1.max(seg.1.1);
                    } else {
                        merged.push(seg);
                    }
                }
                self.links.push(LinkSpan {
                    id: LinkId(self.link_id),
                    raw_target: link.raw.clone(),
                    class: link.class,
                    segments: merged,
                    backlink: false,
                });
                self.link_id = self.link_id.saturating_add(1);
            }
            if i + 1 == header_count {
                let mut sep_src = row_src.saturating_add(1);
                if let Some((next_src, _)) = rows.get(i + 1) {
                    sep_src = sep_src.min(*next_src).max(*row_src);
                }
                self.push_raw_line(format_table_separator(&widths), StyleKind::Table, sep_src);
            } else if i + 1 < rows.len() && i + 1 != header_count {
                // Light row separator between body rows.
                self.push_raw_line(format_table_row_sep(&widths), StyleKind::Table, *row_src);
            }
        }
        let bottom_src = rows.last().map_or(top_src, |(s, _)| *s);
        self.push_raw_line(format_table_bottom(&widths), StyleKind::Table, bottom_src);
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

fn line_start_offsets(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

fn offset_to_line(line_starts: &[usize], body_line_offset: u32, byte: usize) -> u32 {
    let idx = line_starts
        .partition_point(|&s| s <= byte)
        .saturating_sub(1);
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

/// Prefer breaking at the last whitespace that fits; empty take means "no break in avail".
fn split_at_word_boundary(s: &str, max: usize) -> (String, String) {
    if max == 0 {
        return (String::new(), s.to_owned());
    }
    if s.width() <= max {
        return (s.to_owned(), String::new());
    }
    let (hard, _) = split_at_width(s, max);
    if let Some(pos) = hard.rfind(|c: char| c.is_whitespace()) {
        let take = hard[..pos].trim_end().to_owned();
        if !take.is_empty() {
            let rest = s[pos..].trim_start().to_owned();
            return (take, rest);
        }
    }
    // No whitespace in the fitting prefix — caller may soft-wrap to next line.
    (String::new(), s.to_owned())
}

fn longest_word_width(s: &str) -> usize {
    s.split_whitespace()
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0)
        .max(1)
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

fn alert_kind_id(label: &str) -> u8 {
    match label {
        "NOTE" => 1,
        "TIP" => 2,
        "IMPORTANT" => 3,
        "WARNING" => 4,
        "CAUTION" => 5,
        "GOAL" => 6,
        "DECISION" => 7,
        "RISK" => 8,
        _ => 0,
    }
}

/// Returns `None` when minimum (longest-word) widths cannot fit the pane.
fn fair_share_widths(rows: &[&[String]], cols: usize, total_width: usize) -> Option<Vec<usize>> {
    // │ {cell} │ × cols → borders = cols+1, leading+trailing space per cell (= 2*cols)
    let chrome = cols
        .saturating_add(1)
        .saturating_add(cols.saturating_mul(2));
    let avail = total_width.saturating_sub(chrome);
    if avail < cols {
        return None;
    }
    // Uncapped longest-word mins; if they don't fit, caller dumps unwrapped.
    let mins: Vec<usize> = (0..cols)
        .map(|c| {
            rows.iter()
                .map(|r| r.get(c).map_or(1, |s| longest_word_width(s)))
                .max()
                .unwrap_or(1)
                .max(1)
        })
        .collect();
    let min_sum: usize = mins.iter().sum();
    if min_sum > avail {
        return None;
    }
    let natural: Vec<usize> = (0..cols)
        .map(|c| {
            rows.iter()
                .map(|r| r.get(c).map_or(0, |s| s.width()))
                .max()
                .unwrap_or(1)
                .max(mins[c])
        })
        .collect();
    let sum: usize = natural.iter().sum();
    if sum <= avail {
        return Some(natural);
    }
    let mut widths: Vec<usize> = natural
        .iter()
        .enumerate()
        .map(|(i, &n)| {
            let scaled = ((n * avail) / sum).max(1);
            scaled.max(mins[i])
        })
        .collect();
    let mut used: usize = widths.iter().sum();
    while used > avail {
        if let Some((i, _)) = widths
            .iter()
            .enumerate()
            .filter(|(i, w)| **w > mins[*i])
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
    Some(widths)
}

fn wrap_cell(cell: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if cell.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut rest = cell.to_owned();
    while !rest.is_empty() {
        if rest.width() <= width {
            lines.push(std::mem::take(&mut rest));
            break;
        }
        let (take, next) = split_at_word_boundary(&rest, width);
        if take.is_empty() {
            let (hard, next2) = split_at_width(&rest, width);
            lines.push(hard);
            rest = next2;
        } else {
            lines.push(take);
            rest = next;
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn format_table_row_wrapped(row: &[String], widths: &[usize]) -> Vec<String> {
    let wrapped: Vec<Vec<String>> = row
        .iter()
        .zip(widths.iter())
        .map(|(cell, &w)| wrap_cell(cell, w))
        .collect();
    let height = wrapped.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let mut out_lines = Vec::with_capacity(height);
    for r in 0..height {
        let mut out = String::from("│");
        for (ci, &w) in widths.iter().enumerate() {
            let part = wrapped
                .get(ci)
                .and_then(|c| c.get(r))
                .map_or("", String::as_str);
            let pad = w.saturating_sub(part.width());
            let _ = write!(out, " {part}{} │", " ".repeat(pad));
        }
        out_lines.push(out);
    }
    out_lines
}

/// Map a cell-relative `[start, end)` display-col range onto laid-out table lines.
/// Each display line is `│` + per cell ` {part}{pad} │`.
fn table_link_segments(
    row: &[String],
    widths: &[usize],
    cell: usize,
    start: u16,
    end: u16,
) -> Vec<(u32, u16, u16)> {
    if start >= end || cell >= row.len() || cell >= widths.len() {
        return Vec::new();
    }
    let cell_text = &row[cell];
    let w = widths[cell];
    let parts = wrap_cell(cell_text, w);
    // Leading │ + each prior cell's (1 + width + 2) chrome.
    let mut cell_origin = 1u16;
    for &pw in widths.iter().take(cell) {
        cell_origin = cell_origin.saturating_add(
            1u16.saturating_add(u16::try_from(pw).unwrap_or(u16::MAX))
                .saturating_add(2),
        );
    }
    // Content starts one space after the cell's left edge.
    let content_origin = cell_origin.saturating_add(1);

    let mut out = Vec::new();
    // `wrap_cell` trims the whitespace at each soft break, so a part's column in the
    // cell text is found in the source, not by summing the widths of earlier parts.
    let mut byte = 0usize;
    for (r, part) in parts.iter().enumerate() {
        let rest = cell_text.get(byte..).unwrap_or("");
        let skip = if rest.starts_with(part.as_str()) {
            0
        } else {
            rest.len() - rest.trim_start().len()
        };
        let start_byte = byte + skip;
        byte = start_byte + part.len();
        let part_w = u16::try_from(part.width()).unwrap_or(u16::MAX);
        let before = cell_text.get(..start_byte).unwrap_or(cell_text);
        let part_start = u16::try_from(before.width()).unwrap_or(u16::MAX);
        let part_end = part_start.saturating_add(part_w);
        let a = start.max(part_start);
        let b = end.min(part_end);
        if a < b {
            let local0 = a.saturating_sub(part_start);
            let local1 = b.saturating_sub(part_start);
            out.push((
                u32::try_from(r).unwrap_or(0),
                content_origin.saturating_add(local0),
                content_origin.saturating_add(local1),
            ));
        }
    }
    out
}

fn format_table_border(widths: &[usize], left: char, mid: char, right: char) -> String {
    let mut out = String::from(left);
    for (i, &w) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(w.saturating_add(2)));
        if i + 1 == widths.len() {
            out.push(right);
        } else {
            out.push(mid);
        }
    }
    out
}

fn format_table_top(widths: &[usize]) -> String {
    format_table_border(widths, '┌', '┬', '┐')
}

fn format_table_bottom(widths: &[usize]) -> String {
    format_table_border(widths, '└', '┴', '┘')
}

fn format_table_separator(widths: &[usize]) -> String {
    // Match data row: leading space + w content + trailing space.
    format_table_border(widths, '├', '┼', '┤')
}

fn format_table_row_sep(widths: &[usize]) -> String {
    format_table_border(widths, '├', '┼', '┤')
}

/// Horizontal pane border: `left` + fill + `right`, clipped to `width` columns.
fn pane_h_border(width: usize, left: char, right: char) -> String {
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return left.to_string();
    }
    format!("{left}{}{right}", "─".repeat(width.saturating_sub(2)))
}

/// Leading ➕/➖ at item or paragraph start → ASCII `+` / U+2212 `−`.
fn leading_plus_minus(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some('➕') => format!("+{}", chars.as_str()),
        Some('➖') => format!("\u{2212}{}", chars.as_str()),
        _ => text.to_owned(),
    }
}

/// First body paragraph as plain text (no markup). Skips leading fences, tables,
/// lists, and quotes/alerts via pulldown-cmark.
fn first_paragraph(body: &str) -> Option<String> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_GFM;
    let mut nest = 0i32;
    let mut in_para = false;
    let mut out = String::new();

    for ev in Parser::new_ext(body, options) {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    if nest == 0 {
                        in_para = true;
                    }
                }
                Tag::Heading { .. }
                | Tag::CodeBlock(_)
                | Tag::List(_)
                | Tag::Table(_)
                | Tag::BlockQuote(_)
                | Tag::Item
                | Tag::FootnoteDefinition(_) => {
                    nest += 1;
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => {
                    if in_para {
                        break;
                    }
                }
                TagEnd::Heading(_)
                | TagEnd::CodeBlock
                | TagEnd::List(_)
                | TagEnd::Table
                | TagEnd::BlockQuote(_)
                | TagEnd::Item
                | TagEnd::FootnoteDefinition => {
                    nest -= 1;
                }
                _ => {}
            },
            Event::Text(t) | Event::Code(t) if in_para => out.push_str(&t),
            Event::SoftBreak | Event::HardBreak if in_para => out.push(' '),
            _ => {}
        }
    }
    let s = out.trim().to_owned();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(test)]
mod offset_line_tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::Path;

    fn empty_key() -> PageKey {
        PageKey {
            collection_id: "t".into(),
            relative_path: Path::new("x.md").into(),
        }
    }

    fn empty_index() -> Index {
        Index {
            collection_id: "t".into(),
            pages: HashMap::default(),
            edges: vec![],
            by_from: HashMap::default(),
            by_to: HashMap::default(),
            by_id: HashMap::default(),
            by_path: HashMap::default(),
            diagnostics: vec![],
        }
    }

    #[test]
    fn offset_to_line_uses_partition_point_over_line_starts() {
        let text = "a\nbb\nccc\n";
        let starts = line_start_offsets(text);
        assert_eq!(starts, vec![0, 2, 5, 9]);
        assert_eq!(offset_to_line(&starts, 0, 0), 0);
        assert_eq!(offset_to_line(&starts, 0, 1), 0);
        assert_eq!(offset_to_line(&starts, 0, 2), 1);
        assert_eq!(offset_to_line(&starts, 0, 5), 2);
        assert_eq!(offset_to_line(&starts, 1, 5), 3);
    }

    /// Wall-clock ceiling: per-call newline scans over ~170 KB blow past 10 s in
    /// debug; the indexed lookup finishes in ~100 ms. Call-count tests cannot
    /// catch that regression (lookups stay linear either way).
    #[test]
    fn render_large_page_finishes_under_ceiling() {
        use std::time::Instant;
        let chunk = "# H\n\nPara with [link](https://ex.com) and a list:\n\n- a\n- b\n\n| a | b |\n| - | - |\n| 1 | 2 |\n\n```\ncode\n```\n\n";
        let src = chunk.repeat(1700);
        assert!(src.len() >= 160_000, "fixture too small ({})", src.len());
        let index = empty_index();
        let key = empty_key();
        let _ = render(chunk, None, &key, &index, 80);
        let start = Instant::now();
        let doc = render(&src, None, &key, &index, 80);
        let elapsed = start.elapsed();
        assert!(!doc.lines.is_empty());
        assert!(
            elapsed.as_secs() < 10,
            "render took {elapsed:?}, expected < 10s (debug); quadratic offset→line would be ~100s"
        );
    }
}
