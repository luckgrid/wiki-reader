//! Pages, headings, edges (links/backlinks), and ID lookup.
//!
//! See [architecture overview](../../../wiki/architecture/overview.md).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::parse::{self, Diagnostic, Heading, MdLinkKind, ParsedPage};
use crate::provider::{CollectionProvider, PageKey};

/// Kind of relationship edge in the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    /// Markdown `[text](target)`.
    Link,
    /// `http:` / `https:` / `mailto:`.
    External,
    /// Frontmatter `related:`.
    FrontmatterRelated,
    /// Directory parent → child page.
    Parent,
    /// Reserved: `[[wikilink]]` (not emitted yet).
    WikiLink,
    /// Reserved: configured id-pattern mention (not emitted yet).
    IdMention,
}

/// One directed relationship.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// Source page.
    pub from: PageKey,
    /// Destination page when resolved at index time; `None` for external / unresolved paths.
    pub to: Option<PageKey>,
    /// Raw target string as written (path, URL, id, etc.).
    pub raw_target: String,
    /// Edge kind.
    pub kind: EdgeKind,
    /// 1-based source line when known (0 if not applicable).
    pub source_line: u32,
}

/// Indexed page record.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// Stable identity.
    pub key: PageKey,
    /// Display title: frontmatter `title` → first H1 → file stem.
    pub title: String,
    /// Optional frontmatter id.
    pub id: Option<String>,
    /// Headings from the body.
    pub headings: Vec<Heading>,
    /// Parse diagnostics carried forward.
    pub diagnostics: Vec<Diagnostic>,
    /// Parsed page (body, frontmatter, links) for downstream consumers.
    pub parsed: ParsedPage,
}

/// Collection index: pages, edges, and lookups.
#[derive(Debug, Clone, PartialEq)]
pub struct Index {
    /// All pages keyed by [`PageKey`].
    pub pages: HashMap<PageKey, Page>,
    /// Flat edge list.
    pub edges: Vec<Edge>,
    /// Edge indices by source page.
    pub by_from: HashMap<PageKey, Vec<usize>>,
    /// Edge indices by destination page (backlinks exclude [`EdgeKind::Parent`]).
    pub by_to: HashMap<PageKey, Vec<usize>>,
    /// Frontmatter `id` → page key.
    pub by_id: HashMap<String, PageKey>,
    /// Relative path string → page key (for quick path existence checks).
    pub by_path: HashMap<PathBuf, PageKey>,
}

impl Index {
    /// Build an index by listing and reading every page through `provider`.
    ///
    /// # Errors
    ///
    /// Returns when listing or reading pages fails.
    pub fn build(provider: &impl CollectionProvider) -> Result<Self, Error> {
        let metas = provider.list_pages()?;
        let mut pages = HashMap::with_capacity(metas.len());
        let mut by_id = HashMap::new();
        let mut by_path = HashMap::with_capacity(metas.len());

        for meta in metas {
            let source = provider.read(&meta.key)?;
            let parsed = parse::parse(&source);
            let title = title_for(&parsed, &meta.key.relative_path);
            let id = parsed.frontmatter.id.clone();
            if let Some(ref id) = id {
                by_id.insert(id.clone(), meta.key.clone());
            }
            by_path.insert(meta.key.relative_path.clone(), meta.key.clone());
            pages.insert(
                meta.key.clone(),
                Page {
                    key: meta.key,
                    title,
                    id,
                    headings: parsed.headings.clone(),
                    diagnostics: parsed.diagnostics.clone(),
                    parsed,
                },
            );
        }

        let mut edges = Vec::new();
        for page in pages.values() {
            push_link_edges(&mut edges, page, &by_path);
            push_related_edges(&mut edges, page, &by_id, &by_path);
            push_parent_edge(&mut edges, page, &by_path);
        }

        let (by_from, by_to) = build_edge_maps(&edges);

        Ok(Self {
            pages,
            edges,
            by_from,
            by_to,
            by_id,
            by_path,
        })
    }

    /// Backlink edge indices for `key` (excludes [`EdgeKind::Parent`]).
    #[must_use]
    pub fn backlinks(&self, key: &PageKey) -> Vec<usize> {
        self.by_to.get(key).cloned().unwrap_or_default()
    }

    /// Look up a page by frontmatter id.
    #[must_use]
    pub fn page_by_id(&self, id: &str) -> Option<&Page> {
        self.by_id.get(id).and_then(|k| self.pages.get(k))
    }
}

fn title_for(parsed: &ParsedPage, relative_path: &Path) -> String {
    if let Some(ref t) = parsed.frontmatter.title {
        return t.clone();
    }
    if let Some(ref h1) = parsed.h1 {
        return h1.clone();
    }
    relative_path.file_stem().map_or_else(
        || relative_path.display().to_string(),
        |s| s.to_string_lossy().into_owned(),
    )
}

fn push_link_edges(edges: &mut Vec<Edge>, page: &Page, by_path: &HashMap<PathBuf, PageKey>) {
    for link in &page.parsed.links {
        let kind = match link.kind {
            MdLinkKind::Link => EdgeKind::Link,
            MdLinkKind::External => EdgeKind::External,
        };
        // ponytail: coarse path hit for backlinks; full L2 rules land in P1-04 resolve().
        let to = (kind == EdgeKind::Link)
            .then(|| resolve_path_rough(&page.key, &link.target, by_path))
            .flatten();
        edges.push(Edge {
            from: page.key.clone(),
            to,
            raw_target: link.target.clone(),
            kind,
            source_line: link.source_line,
        });
    }
}

fn push_related_edges(
    edges: &mut Vec<Edge>,
    page: &Page,
    by_id: &HashMap<String, PageKey>,
    by_path: &HashMap<PathBuf, PageKey>,
) {
    for related in &page.parsed.frontmatter.related {
        let to = by_id
            .get(related)
            .cloned()
            .or_else(|| by_path.get(Path::new(related)).cloned())
            .or_else(|| {
                by_path
                    .get(&PathBuf::from(format!("{related}.md")))
                    .cloned()
            });
        edges.push(Edge {
            from: page.key.clone(),
            to,
            raw_target: related.clone(),
            kind: EdgeKind::FrontmatterRelated,
            source_line: 0,
        });
    }
}

/// Rough relative path lookup (subset of L2); P1-04 owns the full ordered rules.
fn resolve_path_rough(
    from: &PageKey,
    target: &str,
    by_path: &HashMap<PathBuf, PageKey>,
) -> Option<PageKey> {
    let path_part = target.split_once('#').map_or(target, |(p, _)| p);
    if path_part.is_empty() {
        return Some(from.clone());
    }
    let base = from.relative_path.parent().unwrap_or_else(|| Path::new(""));
    let joined = if path_part.starts_with('/') {
        PathBuf::from(path_part.trim_start_matches('/'))
    } else {
        base.join(path_part)
    };
    let joined = normalize_dots(&joined)?;
    lookup_path_variants(&joined, by_path)
}

fn lookup_path_variants(path: &Path, by_path: &HashMap<PathBuf, PageKey>) -> Option<PageKey> {
    if let Some(k) = by_path.get(path) {
        return Some(k.clone());
    }
    let with_md = PathBuf::from(format!("{}.md", path.display()));
    if let Some(k) = by_path.get(&with_md) {
        return Some(k.clone());
    }
    let readme = path.join("README.md");
    if let Some(k) = by_path.get(&readme) {
        return Some(k.clone());
    }
    let index = path.join("index.md");
    by_path.get(&index).cloned()
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

fn push_parent_edge(edges: &mut Vec<Edge>, page: &Page, by_path: &HashMap<PathBuf, PageKey>) {
    let Some(parent_dir) = page.key.relative_path.parent() else {
        return;
    };
    if parent_dir.as_os_str().is_empty() {
        // Page is at collection root — parent is the collection itself, no page edge.
        return;
    }
    // Parent folder landing page, if present.
    for name in ["README.md", "index.md"] {
        let candidate = parent_dir.join(name);
        if let Some(parent_key) = by_path.get(&candidate) {
            // Skip self (page is the landing page).
            if parent_key == &page.key {
                return;
            }
            edges.push(Edge {
                from: parent_key.clone(),
                to: Some(page.key.clone()),
                raw_target: page.key.relative_path.display().to_string(),
                kind: EdgeKind::Parent,
                source_line: 0,
            });
            return;
        }
    }
}

fn build_edge_maps(edges: &[Edge]) -> (HashMap<PageKey, Vec<usize>>, HashMap<PageKey, Vec<usize>>) {
    let mut by_from: HashMap<PageKey, Vec<usize>> = HashMap::new();
    let mut by_to: HashMap<PageKey, Vec<usize>> = HashMap::new();
    for (i, edge) in edges.iter().enumerate() {
        by_from.entry(edge.from.clone()).or_default().push(i);
        if edge.kind == EdgeKind::Parent {
            continue;
        }
        if let Some(ref to) = edge.to {
            by_to.entry(to.clone()).or_default().push(i);
        }
    }
    (by_from, by_to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::FsProvider;

    fn worked_example() -> FsProvider {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example");
        FsProvider::open(root).expect("fixture")
    }

    #[test]
    fn builds_worked_example_pages_and_titles() {
        let index = Index::build(&worked_example()).expect("index");
        assert_eq!(index.pages.len(), 7);

        let root_key = PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("README.md"),
        };
        let root = index.pages.get(&root_key).expect("root");
        assert_eq!(root.title, "Worked Example Wiki");
        assert_eq!(root.id.as_deref(), Some("FX-ROOT"));
        assert_eq!(
            index.page_by_id("FX-TOKENS").map(|p| p.title.as_str()),
            Some("Token Projection")
        );
    }

    #[test]
    fn records_link_and_related_edges() {
        let index = Index::build(&worked_example()).expect("index");
        let root_key = PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("README.md"),
        };
        let from_idxs = index.by_from.get(&root_key).cloned().unwrap_or_default();
        let kinds: Vec<_> = from_idxs.iter().map(|&i| index.edges[i].kind).collect();
        assert!(kinds.contains(&EdgeKind::Link));
        assert!(kinds.contains(&EdgeKind::FrontmatterRelated));
    }

    #[test]
    fn parent_edges_link_folder_landing_to_children() {
        let index = Index::build(&worked_example()).expect("index");
        let parent = PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("architecture/design-system/README.md"),
        };
        let child = PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("architecture/design-system/tokens.md"),
        };
        let found = index.edges.iter().any(|e| {
            e.kind == EdgeKind::Parent && e.from == parent && e.to.as_ref() == Some(&child)
        });
        assert!(found, "expected Parent edge design-system README → tokens");
    }

    #[test]
    fn backlink_from_tokens_to_design_system() {
        let index = Index::build(&worked_example()).expect("index");
        let tokens = PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("architecture/design-system/tokens.md"),
        };
        let ds = PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("architecture/design-system/README.md"),
        };
        let found = index.backlinks(&ds).iter().any(|&i| {
            let e = &index.edges[i];
            e.from == tokens && e.kind == EdgeKind::Link
        });
        assert!(found, "tokens.md should backlink to design-system README");
    }
}
