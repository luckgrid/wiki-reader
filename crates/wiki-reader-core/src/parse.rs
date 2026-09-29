//! Frontmatter split, pulldown-cmark walk, links, and headings.
//!
//! See [content model](../../../wiki/product/content-model.md).

use std::collections::HashMap;
use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde::Deserialize;

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
    /// Unknown keys, stringified for raw disclosure.
    pub unknown: HashMap<String, String>,
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
    /// Non-fatal parse issues.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse frontmatter and walk the markdown body for headings and links.
#[must_use]
pub fn parse(source: &str) -> ParsedPage {
    let mut diagnostics = Vec::new();
    let (frontmatter, body, body_line_offset) = split_frontmatter(source, &mut diagnostics);
    let (h1, headings, links) = walk_markdown(&body, body_line_offset);
    ParsedPage {
        frontmatter,
        body,
        h1,
        headings,
        links,
        diagnostics,
    }
}

/// GitHub-style heading slug (lowercase, strip punctuation, spaces → `-`).
#[must_use]
pub fn github_slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_hyphen = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_hyphen = false;
        } else if (c == ' ' || c == '-') && !prev_hyphen && !out.is_empty() {
            out.push('-');
            prev_hyphen = true;
        }
        // punctuation dropped
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
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

#[derive(Debug, Deserialize, Default)]
struct YamlFm {
    id: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    status: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    work_units: Vec<String>,
    #[serde(default)]
    applies_to: Vec<String>,
    #[serde(default)]
    related: StringOrList,
    owner: Option<String>,
    updated: Option<String>,
    nav_order: Option<f64>,
    nav_title: Option<String>,
    #[serde(flatten)]
    unknown: HashMap<String, serde_norway::Value>,
}

#[derive(Debug, Deserialize, Default)]
struct TomlFm {
    id: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    status: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    work_units: Vec<String>,
    #[serde(default)]
    applies_to: Vec<String>,
    #[serde(default)]
    related: StringOrList,
    owner: Option<String>,
    updated: Option<String>,
    nav_order: Option<f64>,
    nav_title: Option<String>,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

/// Accept `related: "x"` or `related: ["a", "b"]`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct StringOrList(Vec<String>);

impl<'de> Deserialize<'de> for StringOrList {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Helper {
            One(String),
            Many(Vec<String>),
        }
        Ok(match Helper::deserialize(deserializer)? {
            Helper::One(s) => Self(vec![s]),
            Helper::Many(v) => Self(v),
        })
    }
}

fn parse_yaml_frontmatter(raw: &str, diagnostics: &mut Vec<Diagnostic>) -> Frontmatter {
    if raw.trim().is_empty() {
        return Frontmatter {
            kind: Some(FrontmatterKind::Yaml),
            ..Frontmatter::default()
        };
    }
    match serde_norway::from_str::<YamlFm>(raw) {
        Ok(y) => Frontmatter {
            id: y.id,
            title: y.title,
            summary: y.summary,
            status: y.status,
            tags: y.tags,
            work_units: y.work_units,
            applies_to: y.applies_to,
            related: y.related.0,
            owner: y.owner,
            updated: y.updated,
            nav_order: y.nav_order,
            nav_title: y.nav_title,
            unknown: y
                .unknown
                .into_iter()
                .map(|(k, v)| (k, value_to_string_yaml(&v)))
                .collect(),
            kind: Some(FrontmatterKind::Yaml),
        },
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
    match toml::from_str::<TomlFm>(raw) {
        Ok(t) => Frontmatter {
            id: t.id,
            title: t.title,
            summary: t.summary,
            status: t.status,
            tags: t.tags,
            work_units: t.work_units,
            applies_to: t.applies_to,
            related: t.related.0,
            owner: t.owner,
            updated: t.updated,
            nav_order: t.nav_order,
            nav_title: t.nav_title,
            unknown: t
                .unknown
                .into_iter()
                .map(|(k, v)| (k, value_to_string_toml(&v)))
                .collect(),
            kind: Some(FrontmatterKind::Toml),
        },
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

fn value_to_string_yaml(v: &serde_norway::Value) -> String {
    serde_norway::to_string(v).unwrap_or_else(|_| format!("{v:?}"))
}

fn value_to_string_toml(v: &toml::Value) -> String {
    v.to_string()
}

fn walk_markdown(body: &str, body_line_offset: u32) -> (Option<String>, Vec<Heading>, Vec<MdLink>) {
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
    let mut slug_counts: HashMap<String, u32> = HashMap::new();

    let mut in_code_block = false;
    let mut in_heading: Option<(u8, u32, String)> = None;
    let mut in_link: Option<(String, String, u32)> = None; // target, text, line

    for (event, range) in Parser::new_ext(body, options).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(_) | CodeBlockKind::Indented)) => {
                in_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
            }
            Event::Start(Tag::Heading { level, .. }) if !in_code_block => {
                in_heading = Some((level as u8, offset_to_line(range.start), String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, source_line, text)) = in_heading.take() {
                    let text = text.trim().to_owned();
                    if level == 1 && h1.is_none() {
                        h1 = Some(text.clone());
                    }
                    let base = github_slug(&text);
                    let slug = unique_slug(&base, &mut slug_counts);
                    headings.push(Heading {
                        level,
                        text,
                        slug,
                        source_line,
                    });
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) if !in_code_block => {
                in_link = Some((
                    dest_url.into_string(),
                    String::new(),
                    offset_to_line(range.start),
                ));
            }
            Event::End(TagEnd::Link) => {
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
                    });
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, _, ref mut text)) = in_heading {
                    text.push_str(&t);
                }
                if let Some((_, ref mut text, _)) = in_link {
                    text.push_str(&t);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, _, ref mut text)) = in_heading {
                    text.push(' ');
                }
                if let Some((_, ref mut text, _)) = in_link {
                    text.push(' ');
                }
            }
            _ => {}
        }
    }

    (h1, headings, links)
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

fn unique_slug(base: &str, counts: &mut HashMap<String, u32>) -> String {
    let n = counts.entry(base.to_owned()).or_insert(0);
    let slug = if *n == 0 {
        base.to_owned()
    } else {
        format!("{base}-{n}")
    };
    *n += 1;
    slug
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
        assert_eq!(github_slug("  Foo -- Bar  "), "foo-bar");
        let page = parse("# Dup\n\n## Dup\n\n## Dup\n");
        let slugs: Vec<_> = page.headings.iter().map(|h| h.slug.as_str()).collect();
        assert_eq!(slugs, vec!["dup", "dup-1", "dup-2"]);
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
}
