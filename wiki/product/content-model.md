---
id: WR-CONTENT
title: Content model
summary: What counts as a page, link, ID, and relationship; frontmatter conventions wiki-reader reads.
status: draft
updated: 2026-09-30
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
| `External` | `http(s)://`, `mailto:` | shown; confirm before open |
| `Unsupported` | other URI schemes | muted; never opened |

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
2. Scheme present: `http:` / `https:` / `mailto:` → external (confirm before open). Any other URI scheme (`file:`, `javascript:`, custom `…://…`) → unsupported: muted style, status `unsupported link scheme: …`, never passed to the system opener. Detection is RFC 3986-shaped (leading alpha, then alnum/`+`/`-`/`.` until `:`), so bare forms like `notes:2024.md` or a Windows drive letter (`C:\…`) are unsupported rather than relative file paths.
3. If `t` starts with `/`, resolve relative to the collection root; otherwise relative to `p`'s directory.
4. Try as written, then with `.md`, then `t/README.md`, then `t/index.md`.
5. URL-decode (`%20`) and retry once.
6. Otherwise → `Unresolved(t)`: styled as broken, and explained in the footer when followed.

Anchors that don't exist resolve to the page with a footer notice. The link isn't treated as broken.

**Query strings:** `?…` suffixes on path targets are stripped before rules 3–4 (e.g. `page.md?edit=1` resolves like `page.md`).

**Non-markdown files:** If the path points at an existing non-`.md` file under the collection, the link is styled as broken; following it leaves history unchanged and the status bar shows `not a markdown page`.

## Side nav tree construction

The side nav is built from the filesystem but **presented as a documentation site's navigation**. The same tree defines breadcrumbs ([UI spec](ui-spec.md)) and prev/next order.

### Labels

The label depends on `nav.labels`. With `title` (the default) or `title+filename` it is the first match of: frontmatter `nav_title` → `title` → first H1 → humanized filename. With `filename` it is always the humanized filename (`02-token-projection.md` → "Token Projection"; numeric prefixes are stripped for display but still used for sorting). Folder rows always use the folder name, and a folder's README (and the root README) always uses its title whatever the mode; the root falls back to the collection name, never "Readme".

Config (P1/P2):

```toml
[nav]
labels = "title"            # default; "title" | "filename" | "title+filename" (title with a dim filename suffix)
```

### Folding rules (applied bottom-up, recursively)

Let a folder's **pages** be its markdown files, and its **children** be its pages plus its non-empty subfolders.

| Folder contains | Rendered as |
|-----------------|-------------|
| Nothing (after excludes) | Hidden |
| Only a README (`README.md` / `index.md`) | **Collapsible group**: label = the humanized folder name; its only item is the README, labeled with its title |
| README + other children | **Collapsible group**: label = the humanized folder name; first item = the README (landing page), labeled with its title; then the other children |
| Other children, no README | **Collapsible group**: label = humanized folder name; no landing page |

The root folder is special: the root README is always the **first top-level item** (the wiki entry), and its title is the header's root breadcrumb.

**Worked example**

```text
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

renders as (annotated; `render_text` omits the ← markers)

```text
● Worked Example Wiki          ← landing / root entry
▾ Architecture                 ← group (folder name)
  Architecture Overview        ← landing page (its title)
  ▾ Design System              ← group
    Design System              ← landing page
    Token Projection           ← leaf
  ▸ Wfos                       ← group (folder with only a README)
▸ Decisions                    ← group (collapsed)
```

> [!NOTE]
> A group (folder name) and its landing page (README title) may read alike when the README title matches the folder. That is intentional: it keeps every row's label truthful and the tree uniform.

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
