//! Pages, headings, edges (links/backlinks), and ID lookup.
//!
//! See [architecture overview](../../../wiki/architecture/overview.md).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::nav::{self, Target};
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
    /// Collection id (directory name by default).
    pub collection_id: String,
    /// All pages keyed by [`PageKey`].
    pub pages: HashMap<PageKey, Page>,
    /// Flat edge list (deterministic order).
    pub edges: Vec<Edge>,
    /// Edge indices by source page.
    pub by_from: HashMap<PageKey, Vec<usize>>,
    /// Edge indices by destination page (backlinks exclude [`EdgeKind::Parent`]).
    pub by_to: HashMap<PageKey, Vec<usize>>,
    /// Frontmatter `id` → page key.
    pub by_id: HashMap<String, PageKey>,
    /// Relative path string → page key (for quick path existence checks).
    pub by_path: HashMap<PathBuf, PageKey>,
    /// Build-time diagnostics (skipped pages, duplicate ids, ambiguous related).
    pub diagnostics: Vec<Diagnostic>,
}

impl Index {
    /// Build an index by listing and reading every page through `provider`.
    ///
    /// Unreadable or non-UTF-8 pages are skipped with a diagnostic. Only
    /// [`CollectionProvider::list_pages`] failure is fatal.
    ///
    /// # Errors
    ///
    /// Returns when listing pages fails.
    pub fn build(provider: &impl CollectionProvider) -> Result<Self, Error> {
        let (metas, mut diagnostics) = provider.list_pages()?;
        let collection_id = metas.first().map_or_else(
            || {
                provider
                    .root()
                    .file_name()
                    .map_or_else(|| "collection".into(), |n| n.to_string_lossy().into_owned())
            },
            |m| m.key.collection_id.clone(),
        );

        let mut pages: HashMap<PageKey, Page> = HashMap::with_capacity(metas.len());
        let mut by_id: HashMap<String, PageKey> = HashMap::new();
        let mut by_path: HashMap<PathBuf, PageKey> = HashMap::with_capacity(metas.len());

        for meta in metas {
            let source = match provider.read(&meta.key) {
                Ok(s) => s,
                Err(err) => {
                    diagnostics.push(Diagnostic {
                        message: format!("skipping {}: {err}", meta.key.relative_path.display()),
                    });
                    continue;
                }
            };
            let parsed = parse::parse(&source);
            let title = title_for(&parsed, &meta.key.relative_path);
            let id = parsed.frontmatter.id.clone();
            if let Some(ref id) = id {
                if let Some(existing) = by_id.get(id) {
                    diagnostics.push(Diagnostic {
                        message: format!(
                            "duplicate id `{id}`: keeping {}, ignoring {}",
                            existing.relative_path.display(),
                            meta.key.relative_path.display()
                        ),
                    });
                } else {
                    by_id.insert(id.clone(), meta.key.clone());
                }
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

        let mut index = Self {
            collection_id,
            pages,
            edges: Vec::new(),
            by_from: HashMap::new(),
            by_to: HashMap::new(),
            by_id,
            by_path,
            diagnostics,
        };

        let mut keys: Vec<PageKey> = index.pages.keys().cloned().collect();
        keys.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        let mut edges = Vec::new();
        for key in &keys {
            push_link_edges(&mut edges, key, &index);
            push_related_edges(&mut edges, key, &mut index);
            push_parent_edge(&mut edges, key, &index);
        }
        index.edges = edges;

        let (by_from, by_to) = build_edge_maps(&index.edges);
        index.by_from = by_from;
        index.by_to = by_to;
        Ok(index)
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

fn push_link_edges(edges: &mut Vec<Edge>, key: &PageKey, index: &Index) {
    let Some(page) = index.pages.get(key) else {
        return;
    };
    for link in &page.parsed.links {
        let kind = match link.kind {
            MdLinkKind::Link => EdgeKind::Link,
            MdLinkKind::External => EdgeKind::External,
        };
        let to = if kind == EdgeKind::Link {
            match nav::resolve(&link.target, key, index).target {
                Target::Page(dest, _) => Some(dest),
                Target::Anchor(_)
                | Target::External(_)
                | Target::Unsupported(_)
                | Target::Unresolved(_) => None,
            }
        } else {
            None
        };
        edges.push(Edge {
            from: key.clone(),
            to,
            raw_target: link.target.clone(),
            kind,
            source_line: link.source_line,
        });
    }
}

fn push_related_edges(edges: &mut Vec<Edge>, key: &PageKey, index: &mut Index) {
    let relateds = index
        .pages
        .get(key)
        .map(|p| p.parsed.frontmatter.related.clone())
        .unwrap_or_default();
    for related in relateds {
        let to = resolve_related(&related, key, index);
        edges.push(Edge {
            from: key.clone(),
            to,
            raw_target: related,
            kind: EdgeKind::FrontmatterRelated,
            source_line: 0,
        });
    }
}

/// ID → `nav::resolve` → unique stem match.
fn resolve_related(related: &str, from: &PageKey, index: &mut Index) -> Option<PageKey> {
    if let Some(key) = index.by_id.get(related) {
        return Some(key.clone());
    }
    if let Target::Page(key, _) = nav::resolve(related, from, index).target {
        return Some(key);
    }
    let matches: Vec<PageKey> = index
        .pages
        .keys()
        .filter(|k| k.relative_path.file_stem().and_then(|s| s.to_str()) == Some(related))
        .cloned()
        .collect();
    match matches.as_slice() {
        [one] => Some(one.clone()),
        [] => None,
        many => {
            let paths: Vec<_> = many
                .iter()
                .map(|k| k.relative_path.display().to_string())
                .collect();
            index.diagnostics.push(Diagnostic {
                message: format!(
                    "ambiguous related `{related}` matches {}; left unresolved",
                    paths.join(", ")
                ),
            });
            None
        }
    }
}

fn is_landing_name(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("README.md" | "README.markdown" | "index.md" | "index.markdown")
    )
}

fn push_parent_edge(edges: &mut Vec<Edge>, key: &PageKey, index: &Index) {
    let rel = &key.relative_path;
    let Some(dir) = rel.parent() else {
        return;
    };
    // Landing pages link to the parent folder's landing; others to nearest ancestor landing.
    let mut search = if is_landing_name(rel) {
        dir.parent()
    } else {
        Some(dir)
    };

    while let Some(folder) = search {
        let names = ["README.md", "README.markdown", "index.md", "index.markdown"];
        if folder.as_os_str().is_empty() {
            for name in names {
                if let Some(parent_key) = index.by_path.get(Path::new(name))
                    && parent_key != key
                {
                    edges.push(Edge {
                        from: parent_key.clone(),
                        to: Some(key.clone()),
                        raw_target: key.relative_path.display().to_string(),
                        kind: EdgeKind::Parent,
                        source_line: 0,
                    });
                    return;
                }
            }
            return;
        }

        for name in names {
            let candidate = folder.join(name);
            if let Some(parent_key) = index.by_path.get(&candidate)
                && parent_key != key
            {
                edges.push(Edge {
                    from: parent_key.clone(),
                    to: Some(key.clone()),
                    raw_target: key.relative_path.display().to_string(),
                    kind: EdgeKind::Parent,
                    source_line: 0,
                });
                return;
            }
        }
        search = folder.parent();
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
    use std::fs;
    use std::io::Write;

    fn worked_example() -> FsProvider {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example");
        FsProvider::open(root).expect("fixture")
    }

    #[test]
    fn builds_worked_example_pages_and_titles() {
        let index = Index::build(&worked_example()).expect("index");
        assert_eq!(index.pages.len(), 7);
        assert_eq!(index.collection_id, "worked-example");

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
    fn nested_landing_gets_ancestor_parent_edge() {
        let index = Index::build(&worked_example()).expect("index");
        let arch = PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("architecture/README.md"),
        };
        let ds = PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("architecture/design-system/README.md"),
        };
        let found = index
            .edges
            .iter()
            .any(|e| e.kind == EdgeKind::Parent && e.from == arch && e.to.as_ref() == Some(&ds));
        assert!(
            found,
            "design-system README should parent-link to architecture README"
        );
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

    #[test]
    fn skips_non_utf8_page_with_diagnostic() {
        let dir = tempfile::tempdir().expect("temp");
        fs::write(dir.path().join("README.md"), "# Ok\n").expect("readme");
        let mut bad = fs::File::create(dir.path().join("bad.md")).expect("create");
        bad.write_all(&[0xff, 0xfe, b'#', b' ', b'X', b'\n'])
            .expect("write");
        drop(bad);

        let provider = FsProvider::open(dir.path()).expect("open");
        let index = Index::build(&provider).expect("build");
        assert_eq!(index.pages.len(), 1);
        assert!(
            index
                .diagnostics
                .iter()
                .any(|d| d.message.contains("bad.md")),
            "expected skip diagnostic, got {:?}",
            index.diagnostics
        );
    }

    #[test]
    fn build_is_deterministic() {
        let a = Index::build(&worked_example()).expect("a");
        let b = Index::build(&worked_example()).expect("b");
        assert_eq!(a.edges, b.edges);
        assert_eq!(a.by_from, b.by_from);
        assert_eq!(a.by_to, b.by_to);
    }

    #[test]
    fn related_stem_resolves() {
        let dir = tempfile::tempdir().expect("temp");
        fs::create_dir(dir.path().join("product")).expect("dir");
        fs::write(
            dir.path().join("README.md"),
            "---\nrelated: [spec]\n---\n\n# Root\n",
        )
        .expect("readme");
        fs::write(dir.path().join("product/spec.md"), "# Spec\n").expect("spec");

        let provider = FsProvider::open(dir.path()).expect("open");
        let index = Index::build(&provider).expect("build");
        let root = PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: PathBuf::from("README.md"),
        };
        let spec = PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: PathBuf::from("product/spec.md"),
        };
        let found = index.edges.iter().any(|e| {
            e.kind == EdgeKind::FrontmatterRelated && e.from == root && e.to.as_ref() == Some(&spec)
        });
        assert!(found, "related: [spec] should resolve to product/spec.md");
    }

    #[test]
    fn anchor_only_link_is_not_self_edge() {
        let dir = tempfile::tempdir().expect("temp");
        fs::write(dir.path().join("README.md"), "# Hi\n\n[jump](#hi)\n").expect("readme");
        let provider = FsProvider::open(dir.path()).expect("open");
        let index = Index::build(&provider).expect("build");
        let self_edges: Vec<_> = index
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Link && e.to.as_ref() == Some(&e.from))
            .collect();
        assert!(
            self_edges.is_empty(),
            "unexpected self-edges: {self_edges:?}"
        );
        assert!(
            index
                .edges
                .iter()
                .any(|e| e.kind == EdgeKind::Link && e.raw_target == "#hi" && e.to.is_none())
        );
    }

    #[test]
    fn dogfood_wiki_builds_without_unresolved_related() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wiki");
        let provider = FsProvider::open(root).expect("wiki");
        let index = Index::build(&provider).expect("build");
        let unresolved: Vec<_> = index
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::FrontmatterRelated && e.to.is_none())
            .map(|e| format!("{} → {}", e.from.relative_path.display(), e.raw_target))
            .collect();
        assert!(
            unresolved.is_empty(),
            "unresolved related edges: {unresolved:?}"
        );
    }
}
