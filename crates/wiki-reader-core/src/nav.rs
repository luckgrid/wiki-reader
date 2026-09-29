//! Link resolution, `NavTree` build (titles, folding, order), prev/next.
//!
//! See [content model](../../../wiki/product/content-model.md) and
//! [ADR-0008](../../../wiki/decisions/0008-side-nav-as-site-nav.md).

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
    /// `http:` / `https:` / `mailto:` (never fetched).
    External(String),
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

    // 2. Scheme → external.
    if has_scheme(t) {
        return ResolveOutcome {
            target: Target::External(t.to_owned()),
            notice: None,
        };
    }

    let (path_part, anchor) = split_anchor(t);

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
    let slug = crate::parse::github_slug(anchor);
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
    let slug = crate::parse::github_slug(raw);
    let notice = missing_anchor_notice(&key, &slug, index);
    ResolveOutcome {
        target: Target::Page(key, Some(slug)),
        notice,
    }
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

fn has_scheme(t: &str) -> bool {
    let b = t.as_bytes();
    starts_with_ignore_case(b, b"http:")
        || starts_with_ignore_case(b, b"https:")
        || starts_with_ignore_case(b, b"mailto:")
}

fn starts_with_ignore_case(hay: &[u8], prefix: &[u8]) -> bool {
    hay.len() >= prefix.len()
        && hay[..prefix.len()]
            .iter()
            .zip(prefix)
            .all(|(a, b)| a.to_ascii_lowercase() == *b)
}

fn url_decode_once(s: &str) -> Option<String> {
    let decoded = percent_decode_str(s).decode_utf8().ok()?;
    Some(decoded.into_owned())
}

/// Rules 3–4: relative (and root-relative when `t` starts with `/`), then path variants.
fn resolve_path(path_part: &str, from: &PageKey, index: &Index) -> Option<PageKey> {
    if path_part.is_empty() {
        return Some(from.clone());
    }

    let page_dir = from.relative_path.parent().unwrap_or_else(|| Path::new(""));

    let mut bases = vec![page_dir.to_path_buf()];
    if path_part.starts_with('/') {
        bases.push(PathBuf::new()); // collection root
    }

    let trimmed = path_part.trim_start_matches('/');
    for base in bases {
        let joined = if path_part.starts_with('/') {
            PathBuf::from(trimmed)
        } else {
            base.join(path_part)
        };
        let Some(norm) = normalize_dots(&joined) else {
            continue;
        };
        if let Some(key) = lookup_variants(&norm, index) {
            return Some(key);
        }
    }
    None
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
