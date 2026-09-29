---
id: WR-CONTENT
title: Content model
summary: What counts as a page, link, ID, and relationship; frontmatter conventions wiki-reader reads.
status: draft
updated: 2026-09-28
related: [spec, ui-spec]
nav_order: 3
---

# Content model

What counts as a page, link, ID, and relationship; frontmatter conventions wiki-reader reads.

## Collection

A root directory (or several, see R25). A page is any `.md`/`.markdown` file not excluded by `.gitignore` or config `exclude` globs. Dot-directories are included unless ignored, since records often live in places like `.planning/`. Symlinked `.md` files are skipped (`follow_links` is off), so a page must be a regular file under the collection root.

## Page identity

- **Key:** `(collection_id, relative_path)`. Stable, and the thing everything references.
- **Title:** frontmatter `title` → first H1 → file stem.
- **ID (optional):** frontmatter `id`. If present, other pages can reference the page by ID.
- **Folder landing page:** `README.md` or `index.md` is its folder's landing page. How that shows in the side nav depends on the folding rules below.

## Frontmatter (all optional)

YAML (`---`) or TOML (`+++`). Unknown keys are kept and shown raw in a "metadata" disclosure.

| Key | Type | Used for |
|-----|------|----------|
| `id` | string | Addressable ID; strong context match |
| `title` | string | Display title |
| `summary` | string | Highlights, search results, hover |
| `status` | string | Highlights, tree badge |
| `tags` | list | Context matching |
| `work_units` | list | Context matching |
| `applies_to` | list of path globs | Context matching |
| `related` | list of paths/IDs | Explicit relationship edges |
| `owner`, `updated` | string | Highlights |
| `nav_order` | number | Sibling order in tree and prev/next |
| `nav_title` | string | Shorter title for tree, breadcrumbs, and prev/next |

## Edges (relationships)

| Kind | Detected from | Resolves to |
|------|---------------|-------------|
| `Link` | `[text](./other.md#anchor)` | page + anchor, or `Unresolved` |
| `WikiLink` | `[[other]]`, `[[other#h]]`, `[[other\|label]]` (P1) | page by stem/title/ID |
| `IdMention` | text matching configured `id_pattern` | pages whose `id` matches, else a "virtual" work-unit node |
| `FrontmatterRelated` | `related:` | page or ID |
| `Parent` | directory structure | folder index page |
| `External` | `http(s)://` | shown, never fetched |

Backlinks are the reverse of every edge except `Parent`. The index keeps a flat `Vec<Edge>` plus two maps (`by_from` and `by_to`), so a graph view (R24) is just another consumer.

**Virtual work-unit nodes.** An ID mentioned in many pages but defined by none still becomes a node. Selecting it lists every mention. For workstream-style repos this is often the most useful view.

## Parsing rules

- Frontmatter is split off before markdown parsing, so it's never interpreted as markdown.
- ID mentions inside fenced code blocks are **ignored by default** (configurable), to avoid counting example IDs.
- Anchors use GitHub-style slugging, which is what most collections are written against.
- Every edge records its source line, so raw view and `$EDITOR` jumps land in the right place.

## Link resolution rules

In order, for a link target `t` written in page `p`:

1. `#anchor` → same page, anchor by GitHub-style slug.
2. Scheme present (`http:`, `https:`, `mailto:`) → external.
3. Relative to `p`'s directory; then (if it starts with `/`) relative to the collection root.
4. Try as written, then with `.md`, then `t/README.md`, then `t/index.md`.
5. URL-decode (`%20`) and retry once.
6. Otherwise → `Unresolved(t)`: styled as broken, and explained in the footer when followed.

Anchors that don't exist resolve to the page with a footer notice. The link isn't treated as broken.

## Side nav tree construction

The side nav is built from the filesystem but **presented as a documentation site's navigation**. The same tree defines breadcrumbs ([UI spec](ui-spec.md)) and prev/next order.

### Labels

Each page's label is the first match of: frontmatter `nav_title` → `title` → first H1 → humanized filename (`02-token-projection.md` → "Token Projection"; numeric prefixes are stripped for display but still used for sorting).

Config (P1/P2):
```toml
[nav]
labels = "title"            # "title" | "filename" | "title+filename" (filename as dim alt text below)
```

### Folding rules (applied bottom-up, recursively)

Let a folder's **pages** be its markdown files, and its **children** be its pages plus its non-empty subfolders.

| Folder contains | Rendered as |
|-----------------|-------------|
| Nothing (after excludes) | Hidden |
| Only a README (`README.md` / `index.md`) | **Leaf link** to that README, labeled with its title |
| README + other children | **Collapsible group**: label = README title; first item = the README (landing page); then the other children |
| Other children, no README | **Collapsible group**: label = humanized folder name; no landing page |

The root folder is special: the root README is always the **first top-level item** (the wiki entry), and its title is the header's root breadcrumb.

**Worked example**

```
wiki/
├── README.md                    "Worked Example Wiki"
├── architecture/
│   ├── README.md                "Architecture Overview"
│   ├── design-system/
│   │   ├── README.md            "Design System"
│   │   └── tokens.md            "Token Projection"
│   └── wfos/
│       └── README.md            "Workflow OS"
└── decisions/
    ├── 0001-stack.md
    └── 0002-adapters.md
```

renders as

```
● Worked Example Wiki
▾ Architecture Overview
  Architecture Overview
  ▾ Design System
    Design System
    Token Projection
  Workflow OS
▸ Decisions
```

> [!NOTE]
> Group headers and their landing pages share a label by default (the MkDocs/Docusaurus convention). If that reads as noise, a later option `nav.landing_label = "Overview"` can relabel landing items.

### Group header behavior
Clicking or pressing `Enter` on a group header **toggles** it. The landing page is opened by its own item. Groups containing the current page auto-expand after navigation.

### Ordering (tree and prev/next)

First match wins:

1. **`SUMMARY.md`** (mdBook) or **`_sidebar.md`** (docsify) at the root: its nested link list defines order and grouping. Pages it doesn't list are appended in natural order under "Other pages".
2. **Frontmatter `nav_order`** among siblings; unordered siblings follow.
3. **Natural sort**: README/landing first, then `01-…` < `02-…` < `10-…`, case-insensitive; folders and files interleaved by name (config: `folders_first`).

**Prev/next** walk the rendered tree depth-first over *page items only*. Group headers aren't pages. So "next" after a group's landing page is its first child, and "next" after a group's last page is the item after the group.

## Derived stats

Word count excludes frontmatter and code blocks. Reading time is words / 230, rounded up. Both appear in the footer.
