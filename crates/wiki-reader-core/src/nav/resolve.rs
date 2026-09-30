//! Link resolution (L2).
//!
//! See [content model](../../../../../wiki/product/content-model.md).

use std::path::{Path, PathBuf};

use percent_encoding::percent_decode_str;

use crate::index::Index;
use crate::provider::PageKey;

/// Navigation target after link resolution (L2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Another page, optional heading slug.
    Page(PageKey, Option<String>),
    /// Same-page heading jump.
    Anchor(String),
    /// `http:` / `https:` / `mailto:` (confirm before open).
    External(String),
    /// Other URI schemes (`file:`, `javascript:`, custom) — never opened.
    Unsupported(String),
    /// Could not resolve; styled as broken.
    Unresolved(String),
}

/// Result of [`resolve`], including a non-broken missing-anchor notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveOutcome {
    /// Where to go.
    pub target: Target,
    /// Footer notice when the page resolved but the anchor slug was missing.
    pub notice: Option<String>,
}

/// Resolve link target `t` written in page `from` (content-model rules 1–6).
#[must_use]
pub fn resolve(t: &str, from: &PageKey, index: &Index) -> ResolveOutcome {
    let t = t.trim();
    if t.is_empty() {
        return unresolved(t);
    }

    // 1. `#anchor` only → same page.
    if let Some(anchor) = t.strip_prefix('#') {
        return same_page_anchor(from, anchor, index);
    }

    // 2. Scheme → external (http/https/mailto) or unsupported (anything else).
    if let Some(kind) = scheme_class(t) {
        return ResolveOutcome {
            target: match kind {
                SchemeClass::Openable => Target::External(t.to_owned()),
                SchemeClass::Unsupported => Target::Unsupported(t.to_owned()),
            },
            notice: None,
        };
    }

    let (path_part, anchor) = split_anchor(t);
    let path_part = strip_query(path_part);

    // 3–5: try as written, then URL-decoded once.
    if let Some(key) = resolve_path(path_part, from, index) {
        return page_with_anchor(key, anchor, index);
    }
    if let Some(decoded) = url_decode_once(path_part)
        && decoded != path_part
        && let Some(key) = resolve_path(&decoded, from, index)
    {
        return page_with_anchor(key, anchor, index);
    }

    // 6.
    unresolved(t)
}

fn unresolved(t: &str) -> ResolveOutcome {
    ResolveOutcome {
        target: Target::Unresolved(t.to_owned()),
        notice: None,
    }
}

fn same_page_anchor(from: &PageKey, anchor: &str, index: &Index) -> ResolveOutcome {
    let slug = slug_fragment(anchor);
    let notice = missing_anchor_notice(from, &slug, index);
    ResolveOutcome {
        target: Target::Anchor(slug),
        notice,
    }
}

fn page_with_anchor(key: PageKey, anchor: Option<&str>, index: &Index) -> ResolveOutcome {
    let Some(raw) = anchor.filter(|a| !a.is_empty()) else {
        return ResolveOutcome {
            target: Target::Page(key, None),
            notice: None,
        };
    };
    let slug = slug_fragment(raw);
    let notice = missing_anchor_notice(&key, &slug, index);
    ResolveOutcome {
        target: Target::Page(key, Some(slug)),
        notice,
    }
}

fn slug_fragment(raw: &str) -> String {
    let decoded = percent_decode_str(raw)
        .decode_utf8()
        .map_or_else(|_| raw.to_owned(), std::borrow::Cow::into_owned);
    crate::parse::github_slug(&decoded)
}

fn missing_anchor_notice(page: &PageKey, slug: &str, index: &Index) -> Option<String> {
    let p = index.pages.get(page)?;
    if p.headings.iter().any(|h| h.slug == slug) {
        return None;
    }
    Some(format!("heading #{slug} not found on this page"))
}

fn split_anchor(t: &str) -> (&str, Option<&str>) {
    match t.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (t, None),
    }
}

fn strip_query(path: &str) -> &str {
    path.split_once('?').map_or(path, |(p, _)| p)
}

/// Relative path when a target does not resolve to an indexed markdown page.
#[must_use]
pub fn unresolved_relative_path(t: &str, from: &PageKey) -> Option<PathBuf> {
    let t = t.trim();
    if t.is_empty() || scheme_class(t).is_some() || t.starts_with('#') {
        return None;
    }
    let (path_part, _) = split_anchor(t);
    let path_part = strip_query(path_part);
    if path_part.is_empty() {
        return None;
    }
    let joined = if path_part.starts_with('/') {
        PathBuf::from(path_part.trim_start_matches('/'))
    } else {
        let page_dir = from.relative_path.parent().unwrap_or_else(|| Path::new(""));
        page_dir.join(path_part)
    };
    normalize_dots(&joined)
}

/// Openable in the reader (confirm + system opener) vs never opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SchemeClass {
    Openable,
    Unsupported,
}

/// Classify a URI scheme, if `t` looks like one.
///
/// Openable: `http:`, `https:`, `mailto:`. Everything else with a scheme is unsupported
/// (custom schemes, `file:`, `javascript:`, `tel:`, …) and must never hit the opener.
fn scheme_class(t: &str) -> Option<SchemeClass> {
    let scheme = uri_scheme(t)?;
    Some(if is_openable_scheme(scheme) {
        SchemeClass::Openable
    } else {
        SchemeClass::Unsupported
    })
}

/// Leading URI scheme without the trailing `:`, if present.
#[must_use]
pub fn uri_scheme(t: &str) -> Option<&str> {
    let b = t.as_bytes();
    if b.is_empty() || !b[0].is_ascii_alphabetic() {
        return None;
    }
    let mut i = 1;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_alphanumeric() || c == b'+' || c == b'-' || c == b'.' {
            i += 1;
            continue;
        }
        if c == b':' {
            return Some(&t[..i]);
        }
        return None;
    }
    None
}

fn is_openable_scheme(scheme: &str) -> bool {
    scheme.eq_ignore_ascii_case("http")
        || scheme.eq_ignore_ascii_case("https")
        || scheme.eq_ignore_ascii_case("mailto")
}

fn url_decode_once(s: &str) -> Option<String> {
    let decoded = percent_decode_str(s).decode_utf8().ok()?;
    Some(decoded.into_owned())
}

/// Rules 3–4: relative to page dir, or root-relative when `t` starts with `/`; then path variants.
fn resolve_path(path_part: &str, from: &PageKey, index: &Index) -> Option<PageKey> {
    if path_part.is_empty() {
        return Some(from.clone());
    }

    let joined = if path_part.starts_with('/') {
        PathBuf::from(path_part.trim_start_matches('/'))
    } else {
        let page_dir = from.relative_path.parent().unwrap_or_else(|| Path::new(""));
        page_dir.join(path_part)
    };
    let norm = normalize_dots(&joined)?;
    lookup_variants(&norm, index)
}

fn lookup_variants(path: &Path, index: &Index) -> Option<PageKey> {
    // 4. as written → .md → t/README.md → t/index.md
    for candidate in [
        path.to_path_buf(),
        PathBuf::from(format!("{}.md", path.display())),
        path.join("README.md"),
        path.join("index.md"),
    ] {
        if let Some(key) = index.by_path.get(&candidate) {
            return Some(key.clone());
        }
    }
    None
}

fn normalize_dots(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::CurDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            std::path::Component::Normal(s) => out.push(s),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::FsProvider;
    use std::path::Path;

    fn index_at(rel: &str) -> Index {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        let provider = FsProvider::open(root).expect("fixture");
        Index::build(&provider).expect("index")
    }

    fn key(collection: &str, path: &str) -> PageKey {
        PageKey {
            collection_id: collection.into(),
            relative_path: PathBuf::from(path),
        }
    }

    #[test]
    fn query_suffix_stripped_before_resolve() {
        let index = index_at("../../fixtures/worked-example");
        let from = key("worked-example", "architecture/README.md");
        let out = resolve("design-system/tokens.md?edit=1", &from, &index);
        assert!(matches!(out.target, Target::Page(_, None)));
    }

    #[test]
    fn rule1_same_page_anchor() {
        let index = index_at("../../fixtures/broken-links");
        let from = key("broken-links", "README.md");
        let out = resolve("#existing-heading", &from, &index);
        assert_eq!(out.target, Target::Anchor("existing-heading".into()));
        assert!(out.notice.is_none());
    }

    #[test]
    fn missing_anchor_notice_not_broken() {
        let index = index_at("../../fixtures/broken-links");
        let from = key("broken-links", "README.md");
        let out = resolve("#no-such-heading", &from, &index);
        assert_eq!(out.target, Target::Anchor("no-such-heading".into()));
        assert!(out.notice.is_some());
    }

    #[test]
    fn rule2_external_schemes() {
        let index = index_at("../../fixtures/broken-links");
        let from = key("broken-links", "README.md");
        for t in ["https://example.com", "http://x.test", "mailto:a@b.c"] {
            let out = resolve(t, &from, &index);
            assert!(matches!(out.target, Target::External(_)), "{t}");
        }
    }

    #[test]
    fn rule2_unsupported_schemes() {
        let index = index_at("../../fixtures/broken-links");
        let from = key("broken-links", "README.md");
        for t in [
            "chatgpt-conversation://abc",
            "file:///tmp/x",
            "javascript:alert(1)",
            "tel:+15550100",
            "ftp://host/x",
        ] {
            let out = resolve(t, &from, &index);
            assert!(
                matches!(out.target, Target::Unsupported(_)),
                "{t} → {:?}",
                out.target
            );
        }
    }

    #[test]
    fn rule3_relative_and_root_relative() {
        let index = index_at("../../fixtures/broken-links");
        let from = key("broken-links", "subdir/page.md");
        let rel = resolve("../README.md", &from, &index);
        assert_eq!(
            rel.target,
            Target::Page(key("broken-links", "README.md"), None)
        );
        let root = resolve("/target.md", &from, &index);
        assert_eq!(
            root.target,
            Target::Page(key("broken-links", "target.md"), None)
        );
    }

    #[test]
    fn rule4_path_variants() {
        let index = index_at("../../fixtures/broken-links");
        let from = key("broken-links", "README.md");
        // as written
        assert_eq!(
            resolve("target.md", &from, &index).target,
            Target::Page(key("broken-links", "target.md"), None)
        );
        // add .md
        assert_eq!(
            resolve("target", &from, &index).target,
            Target::Page(key("broken-links", "target.md"), None)
        );
        // folder README
        assert_eq!(
            resolve("folder", &from, &index).target,
            Target::Page(key("broken-links", "folder/README.md"), None)
        );
        // folder index.md
        assert_eq!(
            resolve("indexed", &from, &index).target,
            Target::Page(key("broken-links", "indexed/index.md"), None)
        );
    }

    #[test]
    fn rule5_url_decode_retry() {
        let index = index_at("../../fixtures/broken-links");
        let from = key("broken-links", "README.md");
        let out = resolve("with%20space.md", &from, &index);
        assert_eq!(
            out.target,
            Target::Page(key("broken-links", "with space.md"), None)
        );
    }

    #[test]
    fn rule6_unresolved() {
        let index = index_at("../../fixtures/broken-links");
        let from = key("broken-links", "README.md");
        let out = resolve("./missing-nowhere.md", &from, &index);
        assert_eq!(
            out.target,
            Target::Unresolved("./missing-nowhere.md".into())
        );
    }

    #[test]
    fn worked_example_relative_link() {
        let index = index_at("../../fixtures/worked-example");
        let from = key("worked-example", "README.md");
        let out = resolve("architecture/README.md", &from, &index);
        assert_eq!(
            out.target,
            Target::Page(key("worked-example", "architecture/README.md"), None)
        );
    }
}
