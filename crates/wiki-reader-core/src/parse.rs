//! Frontmatter split, pulldown-cmark walk, links, and headings.
//!
//! See [content model](../../../wiki/product/content-model.md).

use std::collections::{BTreeMap, HashSet};
use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

/// Non-fatal problem found while parsing a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Human-readable description.
    pub message: String,
}

/// How the frontmatter block was delimited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontmatterKind {
    /// `---` … `---` YAML.
    Yaml,
    /// `+++` … `+++` TOML.
    Toml,
}

/// One frontmatter value for the properties block (scalar or list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FmProp {
    /// Single string value.
    Scalar(String),
    /// YAML/TOML list rendered as a list in the viewer.
    List(Vec<String>),
}

/// Typed frontmatter keys plus unknowns kept raw for metadata disclosure.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Frontmatter {
    /// Addressable page id.
    pub id: Option<String>,
    /// Display title.
    pub title: Option<String>,
    /// Short summary.
    pub summary: Option<String>,
    /// Status badge string.
    pub status: Option<String>,
    /// Context tags.
    pub tags: Vec<String>,
    /// Work-unit labels.
    pub work_units: Vec<String>,
    /// Path globs this page applies to.
    pub applies_to: Vec<String>,
    /// Explicit related paths or ids.
    pub related: Vec<String>,
    /// Owner string.
    pub owner: Option<String>,
    /// Updated date/string.
    pub updated: Option<String>,
    /// Sibling order hint.
    pub nav_order: Option<f64>,
    /// Shorter tree/breadcrumb label.
    pub nav_title: Option<String>,
    /// Unknown keys, stringified for raw disclosure (ordered for stable display).
    pub unknown: BTreeMap<String, String>,
    /// Full key/value map in document order for the properties block (P2-15 / P2-23).
    pub props: Vec<(String, FmProp)>,
    /// Delimiter kind when a block was present.
    pub kind: Option<FrontmatterKind>,
}

/// A heading extracted from the markdown body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// Heading level (1–6).
    pub level: u8,
    /// Visible text.
    pub text: String,
    /// GitHub-style slug (unique within the page).
    pub slug: String,
    /// 1-based line in the original source.
    pub source_line: u32,
}

/// Link kind from a markdown walk (`WikiLink` deferred).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdLinkKind {
    /// Internal / relative / root-relative / anchor target.
    Link,
    /// `http:`, `https:`, or `mailto:` scheme.
    External,
}

/// A markdown link with source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MdLink {
    /// Link text as rendered.
    pub text: String,
    /// Raw destination (may include `#anchor`).
    pub target: String,
    /// 1-based line in the original source.
    pub source_line: u32,
    /// Internal vs external.
    pub kind: MdLinkKind,
    /// Nesting depth inside list items (`1` = top-level list); `0` if not in a list.
    pub depth: u32,
}

/// Result of parsing one page's source text.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPage {
    /// Parsed frontmatter (empty defaults when absent).
    pub frontmatter: Frontmatter,
    /// Markdown body after the frontmatter block (if any).
    pub body: String,
    /// First H1 text in the body, if any.
    pub h1: Option<String>,
    /// All headings with unique GitHub-style slugs.
    pub headings: Vec<Heading>,
    /// Markdown links (code-fence aware).
    pub links: Vec<MdLink>,
    /// 1-based source lines of each top-level block start (heading, paragraph,
    /// list, code, table, quote, rule).
    pub blocks: Vec<u32>,
    /// 1-based source line of the first body line (1 when no frontmatter).
    pub body_line_offset: u32,
    /// Word count of the body excluding frontmatter and code blocks.
    pub word_count: u32,
    /// Non-fatal parse issues.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse frontmatter and walk the markdown body for headings and links.
#[must_use]
pub fn parse(source: &str) -> ParsedPage {
    let mut diagnostics = Vec::new();
    let (frontmatter, body, body_line_offset) = split_frontmatter(source, &mut diagnostics);
    let (h1, headings, links, blocks, word_count) = walk_markdown(&body, body_line_offset);
    ParsedPage {
        frontmatter,
        body,
        h1,
        headings,
        links,
        blocks,
        body_line_offset,
        word_count,
        diagnostics,
    }
}

/// GitHub-style heading slug (github-slugger rules).
///
/// Lowercase; keep Unicode letters/marks/numbers, `_`, and `-`; each space
/// becomes `-`; drop other punctuation; do not collapse hyphens.
#[must_use]
pub fn github_slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.trim().chars() {
        if c.is_whitespace() {
            out.push('-');
        } else if c == '_' || c == '-' || c.is_alphanumeric() || is_mark(c) {
            for lower in c.to_lowercase() {
                out.push(lower);
            }
        }
    }
    out
}

/// Combining marks (Mn/Mc/Me blocks) kept by github-slugger; no unicode crate.
fn is_mark(c: char) -> bool {
    matches!(
        c as u32,
        0x0300..=0x036F
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE20..=0xFE2F
    )
}

fn split_frontmatter(
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Frontmatter, String, u32) {
    let trimmed = source.strip_prefix('\u{feff}').unwrap_or(source);
    let Some((kind, delimiter)) = detect_fence(trimmed) else {
        return (Frontmatter::default(), trimmed.to_owned(), 1);
    };

    // Opening fence must be the first line.
    let rest = &trimmed[delimiter.len()..];
    let rest = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"));
    let Some(after_open) = rest else {
        return (Frontmatter::default(), trimmed.to_owned(), 1);
    };

    let Some(close) = find_closing_fence(after_open, delimiter) else {
        diagnostics.push(Diagnostic {
            message: format!("unclosed {delimiter} frontmatter; treating file as body"),
        });
        return (Frontmatter::default(), trimmed.to_owned(), 1);
    };

    let raw_fm = &after_open[..close.start];
    let body = after_open[close.end..].to_owned();
    // Lines: open fence (1) + fm lines + close fence → body starts after those.
    let fm_lines =
        u32::try_from(raw_fm.bytes().filter(|&b| b == b'\n').count()).unwrap_or(u32::MAX);
    let body_line_offset = 1 + fm_lines + 1 + 1; // open + fm newlines + close + next line

    let frontmatter = match kind {
        FrontmatterKind::Yaml => parse_yaml_frontmatter(raw_fm, diagnostics),
        FrontmatterKind::Toml => parse_toml_frontmatter(raw_fm, diagnostics),
    };

    (frontmatter, body, body_line_offset)
}

fn detect_fence(source: &str) -> Option<(FrontmatterKind, &'static str)> {
    if source.starts_with("---\n") || source.starts_with("---\r\n") || source == "---" {
        Some((FrontmatterKind::Yaml, "---"))
    } else if source.starts_with("+++\n") || source.starts_with("+++\r\n") || source == "+++" {
        Some((FrontmatterKind::Toml, "+++"))
    } else {
        None
    }
}

fn find_closing_fence(after_open: &str, delimiter: &str) -> Option<Range<usize>> {
    let mut offset = 0;
    for line in after_open.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        if content == delimiter {
            return Some(offset..offset + line.len());
        }
        offset += line.len();
    }
    None
}

fn parse_yaml_frontmatter(raw: &str, diagnostics: &mut Vec<Diagnostic>) -> Frontmatter {
    if raw.trim().is_empty() {
        return Frontmatter {
            kind: Some(FrontmatterKind::Yaml),
            ..Frontmatter::default()
        };
    }
    match serde_norway::from_str::<serde_norway::Value>(raw) {
        Ok(serde_norway::Value::Mapping(map)) => {
            let mut fm = Frontmatter {
                kind: Some(FrontmatterKind::Yaml),
                ..Frontmatter::default()
            };
            for (key, value) in map {
                let Some(name) = yaml_key_name(&key) else {
                    continue;
                };
                apply_frontmatter_key(&mut fm, &name, &YamlVal(&value), diagnostics);
            }
            fm
        }
        Ok(_) => {
            diagnostics.push(Diagnostic {
                message: "YAML frontmatter root must be a mapping".into(),
            });
            Frontmatter {
                kind: Some(FrontmatterKind::Yaml),
                ..Frontmatter::default()
            }
        }
        Err(err) => {
            diagnostics.push(Diagnostic {
                message: format!("invalid YAML frontmatter: {err}"),
            });
            Frontmatter {
                kind: Some(FrontmatterKind::Yaml),
                ..Frontmatter::default()
            }
        }
    }
}

fn parse_toml_frontmatter(raw: &str, diagnostics: &mut Vec<Diagnostic>) -> Frontmatter {
    if raw.trim().is_empty() {
        return Frontmatter {
            kind: Some(FrontmatterKind::Toml),
            ..Frontmatter::default()
        };
    }
    match raw.parse::<toml::Table>() {
        Ok(table) => {
            let mut fm = Frontmatter {
                kind: Some(FrontmatterKind::Toml),
                ..Frontmatter::default()
            };
            for (name, value) in table {
                apply_frontmatter_key(&mut fm, &name, &TomlVal(&value), diagnostics);
            }
            fm
        }
        Err(err) => {
            diagnostics.push(Diagnostic {
                message: format!("invalid TOML frontmatter: {err}"),
            });
            Frontmatter {
                kind: Some(FrontmatterKind::Toml),
                ..Frontmatter::default()
            }
        }
    }
}

fn yaml_key_name(key: &serde_norway::Value) -> Option<String> {
    match key {
        serde_norway::Value::String(s) => Some(s.clone()),
        serde_norway::Value::Bool(b) => Some(b.to_string()),
        serde_norway::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Value adapter so YAML and TOML share one extract path.
trait FmValue {
    fn as_string(&self) -> Option<String>;
    fn as_f64(&self) -> Option<f64>;
    fn as_list_strings(&self) -> Option<Vec<String>>;
    fn to_display(&self) -> String;
}

struct YamlVal<'a>(&'a serde_norway::Value);
struct TomlVal<'a>(&'a toml::Value);

impl FmValue for YamlVal<'_> {
    fn as_string(&self) -> Option<String> {
        match self.0 {
            serde_norway::Value::String(s) => Some(s.clone()),
            serde_norway::Value::Bool(b) => Some(b.to_string()),
            serde_norway::Value::Number(n) => Some(n.to_string()),
            serde_norway::Value::Null
            | serde_norway::Value::Sequence(_)
            | serde_norway::Value::Mapping(_)
            | serde_norway::Value::Tagged(_) => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self.0 {
            serde_norway::Value::Number(n) => n.as_f64(),
            serde_norway::Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    fn as_list_strings(&self) -> Option<Vec<String>> {
        match self.0 {
            serde_norway::Value::Sequence(seq) => Some(
                seq.iter()
                    .filter_map(|v| YamlVal(v).as_string())
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect(),
            ),
            serde_norway::Value::String(s) => {
                let t = s.trim();
                if t.is_empty() {
                    Some(vec![])
                } else {
                    Some(vec![t.to_owned()])
                }
            }
            serde_norway::Value::Bool(_) | serde_norway::Value::Number(_) => {
                self.as_string().map(|s| vec![s])
            }
            _ => None,
        }
    }

    fn to_display(&self) -> String {
        serde_norway::to_string(self.0)
            .unwrap_or_else(|_| format!("{:?}", self.0))
            .trim()
            .to_owned()
    }
}

impl FmValue for TomlVal<'_> {
    fn as_string(&self) -> Option<String> {
        match self.0 {
            toml::Value::String(s) => Some(s.clone()),
            toml::Value::Boolean(b) => Some(b.to_string()),
            toml::Value::Integer(i) => Some(i.to_string()),
            toml::Value::Float(f) => Some(f.to_string()),
            toml::Value::Datetime(d) => Some(d.to_string()),
            toml::Value::Array(_) | toml::Value::Table(_) => None,
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self.0 {
            toml::Value::Float(f) => Some(*f),
            // ponytail: nav_order fits f64; i64→f64 loss only past 2^53
            #[allow(clippy::cast_precision_loss)]
            toml::Value::Integer(i) => Some(*i as f64),
            toml::Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    fn as_list_strings(&self) -> Option<Vec<String>> {
        match self.0 {
            toml::Value::Array(arr) => Some(
                arr.iter()
                    .filter_map(|v| TomlVal(v).as_string())
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect(),
            ),
            toml::Value::String(s) => {
                let t = s.trim();
                if t.is_empty() {
                    Some(vec![])
                } else {
                    Some(vec![t.to_owned()])
                }
            }
            toml::Value::Boolean(_)
            | toml::Value::Integer(_)
            | toml::Value::Float(_)
            | toml::Value::Datetime(_) => self.as_string().map(|s| vec![s]),
            toml::Value::Table(_) => None,
        }
    }

    fn to_display(&self) -> String {
        self.0.to_string().trim().to_owned()
    }
}

fn apply_frontmatter_key(
    fm: &mut Frontmatter,
    name: &str,
    value: &impl FmValue,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = name.trim();
    if name.is_empty() {
        diagnostics.push(Diagnostic {
            message: "frontmatter key is blank; skipped".into(),
        });
        return;
    }
    match name {
        "id" | "title" | "summary" | "status" | "owner" | "updated" | "nav_title" => {
            match value.as_string() {
                Some(s) => {
                    let s = s.trim().to_owned();
                    if s.is_empty() {
                        return;
                    }
                    fm.props.push((name.to_owned(), FmProp::Scalar(s.clone())));
                    match name {
                        "id" => fm.id = Some(s),
                        "title" => fm.title = Some(s),
                        "summary" => fm.summary = Some(s),
                        "status" => fm.status = Some(s),
                        "owner" => fm.owner = Some(s),
                        "updated" => fm.updated = Some(s),
                        "nav_title" => fm.nav_title = Some(s),
                        _ => unreachable!(),
                    }
                }
                None => diagnostics.push(Diagnostic {
                    message: format!("frontmatter `{name}`: expected string-ish value; skipped"),
                }),
            }
        }
        "nav_order" => match value.as_f64() {
            Some(n) => {
                fm.nav_order = Some(n);
                fm.props
                    .push((name.to_owned(), FmProp::Scalar(n.to_string())));
            }
            None => diagnostics.push(Diagnostic {
                message: "frontmatter `nav_order`: expected number; skipped".into(),
            }),
        },
        "tags" | "work_units" | "applies_to" | "related" => match value.as_list_strings() {
            Some(list) => {
                fm.props.push((name.to_owned(), FmProp::List(list.clone())));
                match name {
                    "tags" => fm.tags = list,
                    "work_units" => fm.work_units = list,
                    "applies_to" => fm.applies_to = list,
                    "related" => fm.related = list,
                    _ => unreachable!(),
                }
            }
            None => diagnostics.push(Diagnostic {
                message: format!("frontmatter `{name}`: expected string or list; skipped"),
            }),
        },
        _ => {
            if let Some(list) = value.as_list_strings() {
                if !list.is_empty() {
                    fm.props.push((name.to_owned(), FmProp::List(list.clone())));
                    fm.unknown.insert(name.to_owned(), list.join(", "));
                }
                return;
            }
            let display = if let Some(s) = value.as_string() {
                s.trim().to_owned()
            } else {
                value.to_display()
            };
            if !display.is_empty() {
                fm.props
                    .push((name.to_owned(), FmProp::Scalar(display.clone())));
                fm.unknown.insert(name.to_owned(), display);
            }
        }
    }
}

fn walk_markdown(
    body: &str,
    body_line_offset: u32,
) -> (Option<String>, Vec<Heading>, Vec<MdLink>, Vec<u32>, u32) {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);

    let line_starts = line_start_offsets(body);
    let offset_to_line = |byte: usize| -> u32 {
        let idx = line_starts
            .partition_point(|&s| s <= byte)
            .saturating_sub(1);
        body_line_offset + u32::try_from(idx).unwrap_or(u32::MAX)
    };

    let mut h1 = None;
    let mut headings = Vec::new();
    let mut links = Vec::new();
    let mut blocks = Vec::new();
    let mut used_slugs: HashSet<String> = HashSet::new();
    let mut in_code_block = false;
    let mut list_depth: u32 = 0;
    let mut block_depth: u32 = 0;
    let mut in_heading: Option<(u8, u32, String)> = None;
    let mut in_link: Option<(String, String, u32)> = None;
    let mut word_buf = String::new();

    for (event, range) in Parser::new_ext(body, options).into_offset_iter() {
        match event {
            Event::Start(tag) if !in_code_block || matches!(tag, Tag::CodeBlock(_)) => {
                handle_block_start(
                    &tag,
                    range.start,
                    &offset_to_line,
                    &mut blocks,
                    &mut block_depth,
                    &mut list_depth,
                    &mut in_code_block,
                    &mut in_heading,
                    &mut in_link,
                );
            }
            Event::End(tag_end) => {
                handle_block_end(
                    tag_end,
                    &mut block_depth,
                    &mut list_depth,
                    &mut in_code_block,
                    &mut in_heading,
                    &mut h1,
                    &mut headings,
                    &mut used_slugs,
                    &mut in_link,
                    &mut links,
                );
            }
            Event::Rule if !in_code_block && block_depth == 0 => {
                push_block_line(&mut blocks, offset_to_line(range.start));
            }
            Event::Text(t) | Event::Code(t) => {
                append_inline(
                    &t,
                    &mut in_heading,
                    &mut in_link,
                    in_code_block,
                    &mut word_buf,
                );
            }
            Event::SoftBreak | Event::HardBreak => {
                append_break(&mut in_heading, &mut in_link, in_code_block, &mut word_buf);
            }
            _ => {}
        }
    }

    let word_count = u32::try_from(word_buf.split_whitespace().count()).unwrap_or(u32::MAX);
    (h1, headings, links, blocks, word_count)
}

fn push_block_line(blocks: &mut Vec<u32>, line: u32) {
    if blocks.last() != Some(&line) {
        blocks.push(line);
    }
}

fn enter_top_block(blocks: &mut Vec<u32>, block_depth: &mut u32, line: u32) {
    if *block_depth == 0 {
        push_block_line(blocks, line);
    }
    *block_depth = block_depth.saturating_add(1);
}

#[allow(clippy::too_many_arguments)]
fn handle_block_start(
    tag: &Tag<'_>,
    start: usize,
    offset_to_line: &dyn Fn(usize) -> u32,
    blocks: &mut Vec<u32>,
    block_depth: &mut u32,
    list_depth: &mut u32,
    in_code_block: &mut bool,
    in_heading: &mut Option<(u8, u32, String)>,
    in_link: &mut Option<(String, String, u32)>,
) {
    match tag {
        Tag::CodeBlock(CodeBlockKind::Fenced(_) | CodeBlockKind::Indented) => {
            enter_top_block(blocks, block_depth, offset_to_line(start));
            *in_code_block = true;
        }
        Tag::List(_) if !*in_code_block => {
            enter_top_block(blocks, block_depth, offset_to_line(start));
            *list_depth = list_depth.saturating_add(1);
        }
        Tag::Heading { level, .. } if !*in_code_block => {
            let line = offset_to_line(start);
            enter_top_block(blocks, block_depth, line);
            *in_heading = Some((*level as u8, line, String::new()));
        }
        Tag::Paragraph | Tag::BlockQuote(_) | Tag::Table(_) if !*in_code_block => {
            enter_top_block(blocks, block_depth, offset_to_line(start));
        }
        Tag::Link { dest_url, .. } if !*in_code_block => {
            *in_link = Some((dest_url.to_string(), String::new(), offset_to_line(start)));
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_block_end(
    tag_end: TagEnd,
    block_depth: &mut u32,
    list_depth: &mut u32,
    in_code_block: &mut bool,
    in_heading: &mut Option<(u8, u32, String)>,
    h1: &mut Option<String>,
    headings: &mut Vec<Heading>,
    used_slugs: &mut HashSet<String>,
    in_link: &mut Option<(String, String, u32)>,
    links: &mut Vec<MdLink>,
) {
    match tag_end {
        TagEnd::CodeBlock => {
            *in_code_block = false;
            *block_depth = block_depth.saturating_sub(1);
        }
        TagEnd::List(_) => {
            *list_depth = list_depth.saturating_sub(1);
            *block_depth = block_depth.saturating_sub(1);
        }
        TagEnd::Heading(_) => {
            *block_depth = block_depth.saturating_sub(1);
            if let Some((level, source_line, text)) = in_heading.take() {
                let text = text.trim().to_owned();
                if level == 1 && h1.is_none() {
                    *h1 = Some(text.clone());
                }
                let slug = unique_slug(&github_slug(&text), used_slugs);
                headings.push(Heading {
                    level,
                    text,
                    slug,
                    source_line,
                });
            }
        }
        TagEnd::Paragraph | TagEnd::BlockQuote(_) | TagEnd::Table => {
            *block_depth = block_depth.saturating_sub(1);
        }
        TagEnd::Link => {
            if let Some((target, text, source_line)) = in_link.take() {
                let kind = if is_external(&target) {
                    MdLinkKind::External
                } else {
                    MdLinkKind::Link
                };
                links.push(MdLink {
                    text: text.trim().to_owned(),
                    target,
                    source_line,
                    kind,
                    depth: *list_depth,
                });
            }
        }
        _ => {}
    }
}

fn append_inline(
    t: &str,
    in_heading: &mut Option<(u8, u32, String)>,
    in_link: &mut Option<(String, String, u32)>,
    in_code_block: bool,
    word_buf: &mut String,
) {
    if let Some((_, _, text)) = in_heading {
        text.push_str(t);
    }
    if let Some((_, text, _)) = in_link {
        text.push_str(t);
    }
    if !in_code_block {
        if !word_buf.is_empty() {
            word_buf.push(' ');
        }
        word_buf.push_str(t);
    }
}

fn append_break(
    in_heading: &mut Option<(u8, u32, String)>,
    in_link: &mut Option<(String, String, u32)>,
    in_code_block: bool,
    word_buf: &mut String,
) {
    if let Some((_, _, text)) = in_heading {
        text.push(' ');
    }
    if let Some((_, text, _)) = in_link {
        text.push(' ');
    }
    if !in_code_block {
        word_buf.push(' ');
    }
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

fn unique_slug(base: &str, used: &mut HashSet<String>) -> String {
    if used.insert(base.to_owned()) {
        return base.to_owned();
    }
    let mut n = 1u32;
    loop {
        let candidate = format!("{base}-{n}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

fn is_external(target: &str) -> bool {
    let lower = target.as_bytes();
    starts_with_ignore_case(lower, b"http:")
        || starts_with_ignore_case(lower, b"https:")
        || starts_with_ignore_case(lower, b"mailto:")
}

fn starts_with_ignore_case(hay: &[u8], prefix: &[u8]) -> bool {
    hay.len() >= prefix.len()
        && hay[..prefix.len()]
            .iter()
            .zip(prefix)
            .all(|(a, b)| a.to_ascii_lowercase() == *b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_frontmatter_parses_body() {
        let page = parse("# Hello\n\nSee [x](./x.md).\n");
        assert!(page.frontmatter.kind.is_none());
        assert_eq!(page.h1.as_deref(), Some("Hello"));
        assert_eq!(page.links.len(), 1);
        assert!(page.diagnostics.is_empty());
    }

    #[test]
    fn yaml_frontmatter_typed_keys() {
        let src = "---\nid: ABC\ntitle: T\nnav_order: 2\ntags: [a, b]\nrelated: [other]\ncustom: yes\n---\n\n# H1\n";
        let page = parse(src);
        assert_eq!(page.frontmatter.kind, Some(FrontmatterKind::Yaml));
        assert_eq!(page.frontmatter.id.as_deref(), Some("ABC"));
        assert_eq!(page.frontmatter.title.as_deref(), Some("T"));
        assert_eq!(page.frontmatter.nav_order, Some(2.0));
        assert_eq!(page.frontmatter.tags, vec!["a", "b"]);
        assert_eq!(page.frontmatter.related, vec!["other"]);
        assert!(page.frontmatter.unknown.contains_key("custom"));
        assert_eq!(page.h1.as_deref(), Some("H1"));
        assert!(page.diagnostics.is_empty());
    }

    #[test]
    fn toml_frontmatter() {
        let src = "+++\nid = \"T1\"\ntitle = \"Toml Title\"\n+++\n\nBody only.\n";
        let page = parse(src);
        assert_eq!(page.frontmatter.kind, Some(FrontmatterKind::Toml));
        assert_eq!(page.frontmatter.id.as_deref(), Some("T1"));
        assert_eq!(page.frontmatter.title.as_deref(), Some("Toml Title"));
        assert!(page.h1.is_none());
        assert!(page.diagnostics.is_empty());
    }

    #[test]
    fn malformed_yaml_is_warning_not_failure() {
        let src = "---\n: not valid: {{\n---\n\n# Still Here\n";
        let page = parse(src);
        assert!(!page.diagnostics.is_empty());
        assert_eq!(page.h1.as_deref(), Some("Still Here"));
        assert!(page.frontmatter.title.is_none());
    }

    #[test]
    fn unknown_keys_kept_raw() {
        let src = "---\ntitle: X\nextra_map:\n  a: 1\n---\n\n# X\n";
        let page = parse(src);
        assert!(page.frontmatter.unknown.contains_key("extra_map"));
    }

    #[test]
    fn links_inside_code_fences_ignored() {
        let src = "# T\n\n```md\n[nope](./secret.md)\n```\n\n[yes](./ok.md)\n";
        let page = parse(src);
        assert_eq!(page.links.len(), 1);
        assert_eq!(page.links[0].target, "./ok.md");
        assert_eq!(page.links[0].kind, MdLinkKind::Link);
    }

    #[test]
    fn external_link_kind() {
        let page = parse("[docs](https://example.com/a)\n");
        assert_eq!(page.links.len(), 1);
        assert_eq!(page.links[0].kind, MdLinkKind::External);
    }

    #[test]
    fn github_slug_and_uniqueness() {
        assert_eq!(github_slug("Hello World!"), "hello-world");
        assert_eq!(github_slug("  Foo -- Bar  "), "foo----bar");
        assert_eq!(github_slug("Café Résumé"), "café-résumé");
        assert_eq!(github_slug("snake_case name"), "snake_case-name");
        assert_eq!(github_slug("C++ & Rust"), "c--rust");
        let page = parse("# Dup\n\n## Dup\n\n## Dup\n\n## Dup-1\n\n## Dup\n");
        let slugs: Vec<_> = page.headings.iter().map(|h| h.slug.as_str()).collect();
        // Collision-safe: natural "dup-1" forces the 4th Dup to "dup-2", 5th to "dup-3".
        assert_eq!(slugs, vec!["dup", "dup-1", "dup-2", "dup-1-1", "dup-3"]);
    }

    #[test]
    fn bad_key_does_not_drop_siblings() {
        let src = "---\ntitle: Keep Me\ntags: draft\nnav_order: \"2\"\nid: 42\nrelated: spec\n---\n\n# H\n";
        let page = parse(src);
        assert_eq!(page.frontmatter.title.as_deref(), Some("Keep Me"));
        assert_eq!(page.frontmatter.tags, vec!["draft"]);
        assert_eq!(page.frontmatter.nav_order, Some(2.0));
        assert_eq!(page.frontmatter.id.as_deref(), Some("42"));
        assert_eq!(page.frontmatter.related, vec!["spec"]);
        assert!(page.diagnostics.is_empty());
    }

    #[test]
    fn blank_frontmatter_key_skipped() {
        // YAML null key / empty: ensure we don't panic and keep title.
        let src = "---\ntitle: T\n? ''\n: nope\n---\n\n# H\n";
        let page = parse(src);
        assert_eq!(page.frontmatter.title.as_deref(), Some("T"));
    }

    #[test]
    fn unknown_values_trimmed_and_ordered() {
        let src = "---\ntitle: X\nzebra: \"  z  \"\nalpha: 1\n---\n\n# X\n";
        let page = parse(src);
        let keys: Vec<_> = page.frontmatter.unknown.keys().cloned().collect();
        assert_eq!(keys, vec!["alpha", "zebra"]);
        assert_eq!(
            page.frontmatter.unknown.get("zebra").map(String::as_str),
            Some("z")
        );
    }

    #[test]
    fn link_source_line_accounts_for_frontmatter() {
        let src = "---\ntitle: T\n---\n\n# H\n\n[go](./x.md)\n";
        let page = parse(src);
        assert_eq!(page.links.len(), 1);
        // --- / title / --- / blank / # H / blank / link → line 7
        assert_eq!(page.links[0].source_line, 7);
    }

    #[test]
    fn worked_example_root_parses() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/worked-example/README.md");
        let src = std::fs::read_to_string(&path).expect("fixture");
        let page = parse(&src);
        assert_eq!(page.frontmatter.id.as_deref(), Some("FX-ROOT"));
        assert_eq!(
            page.frontmatter.title.as_deref(),
            Some("Worked Example Wiki")
        );
        assert_eq!(page.h1.as_deref(), Some("Worked Example Wiki"));
        assert!(!page.links.is_empty());
    }

    #[test]
    fn blocks_and_word_count_exclude_frontmatter_and_code() {
        let src = "\
---
title: MetaOne MetaTwo MetaThree
---

# Heading One

Intro paragraph with five words here.

```
code fence words ignored entirely
```

- list item alpha
";
        let page = parse(src);
        // heading, paragraph, code, list
        assert!(
            page.blocks.len() >= 4,
            "expected ≥4 top-level blocks, got {:?}",
            page.blocks
        );
        assert_eq!(page.blocks[0], page.headings[0].source_line);
        // "Heading One" (2) + "Intro paragraph with five words here" (6) + "list item alpha" (3) = 11
        assert_eq!(page.word_count, 11);
        // Frontmatter words not counted.
        assert!(!page.body.contains("MetaOne"));
    }
}
