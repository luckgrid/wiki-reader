//! `NavTree` construction: labels, folding, ordering, prev/next.
//!
//! See [content model](../../../../../wiki/product/content-model.md) and
//! [ADR-0008](../../../../../wiki/decisions/0008-side-nav-as-site-nav.md).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::index::Index;
use crate::provider::PageKey;

use super::resolve::{Target, resolve};

/// Stable identity for a nav row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeId {
    /// A page row.
    Page(PageKey),
    /// A collapsible folder group (`relative` folder path from collection root).
    Group(PathBuf),
    /// Catch-all for pages not listed in `SUMMARY.md` / `_sidebar.md`.
    OtherPages,
}

/// One row in the side nav.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavItem {
    /// Clickable page.
    Page {
        /// Node id.
        id: NodeId,
        /// Page key.
        key: PageKey,
        /// Display label.
        label: String,
    },
    /// Collapsible group (may include a landing page as first child).
    Group {
        /// Node id.
        id: NodeId,
        /// Group header label.
        label: String,
        /// Nested items.
        children: Vec<NavItem>,
    },
}

/// Site-style navigation tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavTree {
    /// Top-level items (root README first when present).
    pub items: Vec<NavItem>,
}

impl NavTree {
    /// Build the nav tree from an index.
    #[must_use]
    pub fn build(index: &Index) -> Self {
        if let Some(items) = build_from_summary(index) {
            return Self { items };
        }
        Self {
            items: build_from_filesystem(index),
        }
    }

    /// Depth-first page keys only (group headers skipped).
    #[must_use]
    pub fn page_order(&self) -> Vec<PageKey> {
        let mut out = Vec::new();
        collect_pages(&self.items, &mut out);
        out
    }

    /// Previous page in tree order, if any.
    #[must_use]
    pub fn prev(&self, key: &PageKey) -> Option<PageKey> {
        let order = self.page_order();
        let i = order.iter().position(|k| k == key)?;
        i.checked_sub(1).map(|j| order[j].clone())
    }

    /// Next page in tree order, if any.
    #[must_use]
    pub fn next(&self, key: &PageKey) -> Option<PageKey> {
        let order = self.page_order();
        let i = order.iter().position(|k| k == key)?;
        order.get(i + 1).cloned()
    }

    /// Render a text snapshot (content-model style markers).
    ///
    /// `expanded` controls which groups show children. `current` marks `●` vs plain label.
    #[must_use]
    pub fn render_text(&self, expanded: &HashSet<NodeId>, current: Option<&PageKey>) -> String {
        let mut lines = Vec::new();
        render_items(&self.items, 0, expanded, current, &mut lines);
        lines.join("\n")
    }
}

fn collect_pages(items: &[NavItem], out: &mut Vec<PageKey>) {
    for item in items {
        match item {
            NavItem::Page { key, .. } => out.push(key.clone()),
            NavItem::Group { children, .. } => collect_pages(children, out),
        }
    }
}

fn render_items(
    items: &[NavItem],
    depth: usize,
    expanded: &HashSet<NodeId>,
    current: Option<&PageKey>,
    lines: &mut Vec<String>,
) {
    let indent = "  ".repeat(depth);
    for item in items {
        match item {
            NavItem::Page { key, label, .. } => {
                let marker = if current == Some(key) { "● " } else { "" };
                lines.push(format!("{indent}{marker}{label}"));
            }
            NavItem::Group {
                id,
                label,
                children,
            } => {
                let open = expanded.contains(id);
                let tri = if open { "▾ " } else { "▸ " };
                lines.push(format!("{indent}{tri}{label}"));
                if open {
                    render_items(children, depth + 1, expanded, current, lines);
                }
            }
        }
    }
}

/// Label for a page: `nav_title` → `title` → H1 → humanized filename.
#[must_use]
pub fn page_label(index: &Index, key: &PageKey) -> String {
    let Some(page) = index.pages.get(key) else {
        return humanize_filename(&key.relative_path);
    };
    if let Some(ref t) = page.parsed.frontmatter.nav_title {
        return t.clone();
    }
    if !page.title.is_empty() {
        return page.title.clone();
    }
    if let Some(ref h1) = page.parsed.h1 {
        return h1.clone();
    }
    humanize_filename(&key.relative_path)
}

/// Humanize a filename or folder segment (`02-token-projection` → `Token Projection`).
#[must_use]
pub fn humanize_filename(path: &Path) -> String {
    let stem = path
        .file_stem()
        .or_else(|| path.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    humanize_stem(&stem)
}

fn humanize_stem(stem: &str) -> String {
    let stripped = strip_numeric_prefix(stem);
    stripped
        .split(['-', '_'])
        .filter(|p| !p.is_empty())
        .map(title_case)
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_numeric_prefix(stem: &str) -> &str {
    let bytes = stem.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'_') {
        &stem[i + 1..]
    } else {
        stem
    }
}

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = first.to_uppercase().collect::<String>();
    out.extend(chars.flat_map(char::to_lowercase));
    out
}

fn is_landing(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("README.md" | "index.md" | "README.markdown" | "index.markdown")
    )
}

// --- filesystem folding -------------------------------------------------------

#[derive(Debug, Default)]
struct DirNode {
    /// Markdown pages directly in this folder (relative paths).
    pages: Vec<PathBuf>,
    /// Child folders.
    dirs: BTreeMap<String, DirNode>,
}

fn build_from_filesystem(index: &Index) -> Vec<NavItem> {
    let mut root = DirNode::default();
    for key in index.pages.keys() {
        insert_page(&mut root, &key.relative_path);
    }
    fold_dir(Path::new(""), &root, index, true)
}

fn insert_page(dir: &mut DirNode, rel: &Path) {
    let parts: Vec<_> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.is_empty() {
        return;
    }
    let mut node = dir;
    for part in &parts[..parts.len() - 1] {
        node = node.dirs.entry(part.clone()).or_default();
    }
    node.pages.push(rel.to_path_buf());
}

fn fold_dir(dir_path: &Path, dir: &DirNode, index: &Index, is_root: bool) -> Vec<NavItem> {
    let collection_id = index
        .pages
        .values()
        .next()
        .map(|p| p.key.collection_id.clone())
        .unwrap_or_default();

    let mut landing: Option<PathBuf> = None;
    let mut other_pages: Vec<PathBuf> = Vec::new();
    for p in &dir.pages {
        if is_landing(p) {
            if landing.is_none() {
                landing = Some(p.clone());
            } else {
                other_pages.push(p.clone());
            }
        } else {
            other_pages.push(p.clone());
        }
    }

    // Child folder items (folded).
    let mut child_dir_items: Vec<(String, Vec<NavItem>, bool)> = Vec::new();
    for (name, child) in &dir.dirs {
        let child_path = dir_path.join(name);
        let folded = fold_dir(&child_path, child, index, false);
        if !folded.is_empty() {
            // Single leaf result from "only README" folding is one Page item.
            let is_single_leaf = folded.len() == 1 && matches!(folded[0], NavItem::Page { .. });
            child_dir_items.push((name.clone(), folded, is_single_leaf));
        }
    }

    if is_root {
        return fold_root(&collection_id, landing, other_pages, child_dir_items, index);
    }

    // Non-root folding rules.
    let has_other_children = !other_pages.is_empty() || !child_dir_items.is_empty();
    match (&landing, has_other_children) {
        (None, false) => Vec::new(), // hidden
        (Some(land), false) => {
            // Only README → leaf link
            let key = page_key(&collection_id, land);
            vec![NavItem::Page {
                id: NodeId::Page(key.clone()),
                label: page_label(index, &key),
                key,
            }]
        }
        (Some(land), true) => {
            // Group with landing first
            let key = page_key(&collection_id, land);
            let label = page_label(index, &key);
            let mut children = vec![NavItem::Page {
                id: NodeId::Page(key.clone()),
                label: label.clone(),
                key,
            }];
            children.extend(sorted_siblings(
                &collection_id,
                other_pages,
                child_dir_items,
                index,
            ));
            vec![NavItem::Group {
                id: NodeId::Group(dir_path.to_path_buf()),
                label,
                children,
            }]
        }
        (None, true) => {
            // Group without README — humanized folder name
            let folder_name = dir_path.file_name().map_or_else(
                || dir_path.display().to_string(),
                |s| s.to_string_lossy().into_owned(),
            );
            let label = humanize_stem(&folder_name);
            let children = sorted_siblings(&collection_id, other_pages, child_dir_items, index);
            vec![NavItem::Group {
                id: NodeId::Group(dir_path.to_path_buf()),
                label,
                children,
            }]
        }
    }
}

fn fold_root(
    collection_id: &str,
    landing: Option<PathBuf>,
    other_pages: Vec<PathBuf>,
    child_dir_items: Vec<(String, Vec<NavItem>, bool)>,
    index: &Index,
) -> Vec<NavItem> {
    let mut items = Vec::new();
    if let Some(land) = landing {
        let key = page_key(collection_id, &land);
        items.push(NavItem::Page {
            id: NodeId::Page(key.clone()),
            label: page_label(index, &key),
            key,
        });
    }
    items.extend(sorted_siblings(
        collection_id,
        other_pages,
        child_dir_items,
        index,
    ));
    items
}

fn page_key(collection_id: &str, rel: &Path) -> PageKey {
    PageKey {
        collection_id: collection_id.to_owned(),
        relative_path: rel.to_path_buf(),
    }
}

fn sorted_siblings(
    collection_id: &str,
    pages: Vec<PathBuf>,
    child_dirs: Vec<(String, Vec<NavItem>, bool)>,
    index: &Index,
) -> Vec<NavItem> {
    // Build sortable entries: (sort_key, nav_order, item)
    struct Entry {
        sort_name: String,
        nav_order: Option<f64>,
        item: NavItem,
    }

    let mut entries = Vec::new();
    for p in pages {
        let key = page_key(collection_id, &p);
        let nav_order = index
            .pages
            .get(&key)
            .and_then(|pg| pg.parsed.frontmatter.nav_order);
        let sort_name = p
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        entries.push(Entry {
            sort_name,
            nav_order,
            item: NavItem::Page {
                id: NodeId::Page(key.clone()),
                label: page_label(index, &key),
                key,
            },
        });
    }
    for (name, folded, _leaf) in child_dirs {
        // folded is either one Page (leaf README) or one Group
        let item = folded
            .into_iter()
            .next()
            .expect("non-empty child_dir_items");
        let nav_order = match &item {
            NavItem::Page { key, .. } => index
                .pages
                .get(key)
                .and_then(|pg| pg.parsed.frontmatter.nav_order),
            NavItem::Group { children, .. } => children.first().and_then(|c| match c {
                NavItem::Page { key, .. } => index
                    .pages
                    .get(key)
                    .and_then(|pg| pg.parsed.frontmatter.nav_order),
                NavItem::Group { .. } => None,
            }),
        };
        entries.push(Entry {
            sort_name: name.to_lowercase(),
            nav_order,
            item,
        });
    }

    entries.sort_by(|a, b| match (a.nav_order, b.nav_order) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => nat_cmp(&a.sort_name, &b.sort_name),
    });
    entries.into_iter().map(|e| e.item).collect()
}

fn nat_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    // ponytail: chunk digits vs non-digits; enough for 01 < 02 < 10.
    let mut aa = a.chars().peekable();
    let mut bb = b.chars().peekable();
    loop {
        match (aa.peek().copied(), bb.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(ca), Some(cb)) if ca.is_ascii_digit() && cb.is_ascii_digit() => {
                let na = take_num(&mut aa);
                let nb = take_num(&mut bb);
                match na.cmp(&nb) {
                    std::cmp::Ordering::Equal => {}
                    other => return other,
                }
            }
            (Some(ca), Some(cb)) => {
                let oa = ca.to_ascii_lowercase().cmp(&cb.to_ascii_lowercase());
                aa.next();
                bb.next();
                if oa != std::cmp::Ordering::Equal {
                    return oa;
                }
            }
        }
    }
}

fn take_num(chars: &mut std::iter::Peekable<impl Iterator<Item = char>>) -> u64 {
    let mut n = 0u64;
    while let Some(c) = chars.peek().copied() {
        if let Some(d) = c.to_digit(10) {
            chars.next();
            n = n.saturating_mul(10).saturating_add(u64::from(d));
        } else {
            break;
        }
    }
    n
}

// --- SUMMARY.md / _sidebar.md -------------------------------------------------

fn build_from_summary(index: &Index) -> Option<Vec<NavItem>> {
    let collection_id = index.pages.values().next()?.key.collection_id.clone();
    let summary_key = ["SUMMARY.md", "_sidebar.md"].into_iter().find_map(|name| {
        let key = page_key(&collection_id, Path::new(name));
        index.pages.get(&key).map(|_| key)
    })?;

    // SUMMARY.md itself is not a content page for nav — but it was discovered as .md.
    // Prefer reading its body from the index page record.
    let summary_page = index.pages.get(&summary_key)?;
    let links = &summary_page.parsed.links;
    if links.is_empty() {
        return None;
    }

    let mut listed = HashSet::new();
    let mut items = Vec::new();
    // Root README first if present and not already first in SUMMARY.
    let root_readme = page_key(&collection_id, Path::new("README.md"));
    if index.pages.contains_key(&root_readme) {
        items.push(NavItem::Page {
            id: NodeId::Page(root_readme.clone()),
            label: page_label(index, &root_readme),
            key: root_readme.clone(),
        });
        listed.insert(root_readme);
    }

    for link in links {
        let outcome = resolve(&link.target, &summary_key, index);
        let Target::Page(key, _) = outcome.target else {
            continue;
        };
        if key.relative_path == summary_key.relative_path {
            continue;
        }
        if listed.contains(&key) {
            continue;
        }
        listed.insert(key.clone());
        items.push(NavItem::Page {
            id: NodeId::Page(key.clone()),
            label: page_label(index, &key),
            key,
        });
    }

    let mut others: Vec<PageKey> = index
        .pages
        .keys()
        .filter(|k| !listed.contains(k) && k.relative_path != summary_key.relative_path)
        .cloned()
        .collect();
    others.sort_by(|a, b| {
        nat_cmp(
            &a.relative_path.to_string_lossy(),
            &b.relative_path.to_string_lossy(),
        )
    });
    if !others.is_empty() {
        let children = others
            .into_iter()
            .map(|key| NavItem::Page {
                id: NodeId::Page(key.clone()),
                label: page_label(index, &key),
                key,
            })
            .collect();
        items.push(NavItem::Group {
            id: NodeId::OtherPages,
            label: "Other pages".into(),
            children,
        });
    }

    Some(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::FsProvider;

    fn index_at(rel: &str) -> Index {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        Index::build(&FsProvider::open(root).unwrap()).unwrap()
    }

    #[test]
    fn humanize_strips_numeric_prefix() {
        assert_eq!(
            humanize_filename(Path::new("02-token-projection.md")),
            "Token Projection"
        );
        assert_eq!(humanize_filename(Path::new("0001-stack.md")), "Stack");
    }

    #[test]
    fn worked_example_tree_snapshot() {
        let index = index_at("../../fixtures/worked-example");
        let tree = NavTree::build(&index);
        let mut expanded = HashSet::new();
        expanded.insert(NodeId::Group(PathBuf::from("architecture")));
        expanded.insert(NodeId::Group(PathBuf::from("architecture/design-system")));
        // Decisions stays collapsed.
        let root = PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("README.md"),
        };
        let text = tree.render_text(&expanded, Some(&root));
        insta::assert_snapshot!(text);
    }

    #[test]
    fn worked_example_prev_next() {
        let index = index_at("../../fixtures/worked-example");
        let tree = NavTree::build(&index);
        let order = tree.page_order();
        assert_eq!(order[0].relative_path, PathBuf::from("README.md"));
        let tokens = PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("architecture/design-system/tokens.md"),
        };
        let prev = tree.prev(&tokens).expect("prev");
        assert_eq!(
            prev.relative_path,
            PathBuf::from("architecture/design-system/README.md")
        );
    }

    #[test]
    fn summary_md_orders_listed_pages() {
        let index = index_at("../../fixtures/summary-md");
        let tree = NavTree::build(&index);
        let labels: Vec<_> = tree
            .page_order()
            .iter()
            .map(|k| page_label(&index, k))
            .collect();
        // Root README first, then SUMMARY order Two, One, Three, then orphan under Other.
        assert_eq!(
            labels,
            vec!["Summary Root", "Two", "One", "Three", "Orphan Page"]
        );
        assert!(tree.items.iter().any(|i| matches!(
            i,
            NavItem::Group {
                id: NodeId::OtherPages,
                ..
            }
        )));
    }

    #[test]
    fn deep_tree_folds_nested_groups() {
        let index = index_at("../../fixtures/deep-tree");
        let tree = NavTree::build(&index);
        let order = tree.page_order();
        assert_eq!(order[0].relative_path, PathBuf::from("README.md"));
        assert!(
            order
                .iter()
                .any(|k| k.relative_path.as_path() == Path::new("l1/l2/l3/leaf.md"))
        );
    }
}
