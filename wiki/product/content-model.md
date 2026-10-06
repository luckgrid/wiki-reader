---
id: WR-CONTENT
title: Content model
summary: What counts as a page, link, ID, and relationship; frontmatter conventions wiki-reader reads.
status: active
updated: 2026-10-05
related: [spec, ui-spec]
nav_order: 3
---

# Content model

What counts as a page, link, ID, and relationship; frontmatter conventions wiki-reader reads.

## Collection

A root directory (or several, see R25). A page is any `.md`/`.markdown` file not excluded by `.gitignore` or config `exclude` globs. Dot-directories are included unless ignored, since records often live in places like `.planning/`. Symlinked `.md` files are skipped (`follow_links` is off), so a page must be a regular file under the collection root.

**Limits:** at most 50,000 pages per collection; each page file at most 8 MiB; frontmatter at most 64 KiB; YAML frontmatter at most 64 `*alias` markers (rejected before expand). Oversized or alias-heavy input is skipped or shown with a diagnostic, never a crash.

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
| `summary` | string | Highlights, search results; hover preview deferred ([P4-08](../roadmap/phase-4-beta.md)) |
| `status` | enum | Highlights, tree badge. Canonical values: `draft`, `proposed`, `accepted`, `active`, `done`, `deferred`, `superseded`, `historical` (see [wiki README](../README.md)) |
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
| `WikiLink` | `[[other]]`, `[[other#h]]`, `[[other\|label]]` | reserved — not emitted yet |
| `IdMention` | text matching configured `id_pattern` | reserved — not emitted yet |
| `FrontmatterRelated` | `related:` | page or ID |
| `Parent` | directory structure | folder index page |
| `External` | `http(s)://`, `mailto:` | shown; confirm before open |

Unsupported URI schemes (`file:`, `javascript:`, custom `…://…`) are link *targets* (muted, never opened), not a separate edge kind.

Backlinks are the reverse of every edge except `Parent`. The index keeps a flat `Vec<Edge>` plus two maps (`by_from` and `by_to`), so a graph view (R24) is just another consumer.

**Virtual work-unit nodes** (reserved — not emitted yet). An ID mentioned in many pages but defined by none would still become a node; selecting it would list every mention. For workstream-style repos this is often the most useful view once `IdMention` ships.

## Parsing rules

- Frontmatter is split off before markdown parsing, so it's never interpreted as markdown.
- ID mentions inside fenced code blocks are reserved (not emitted yet); the intended default is to ignore them (configurable) so example IDs are not counted.
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

The side nav is built from the filesystem but **presented as a documentation site's navigation**. The same hierarchy and page order define breadcrumbs ([UI spec](ui-spec.md)) and prev/next. Their labels always use title-mode navigation, independently of the side-nav label setting ([ADR-0021](../decisions/0021-side-nav-only-label-mode.md)).

### Labels

The side-nav page label depends on `nav.labels`; header breadcrumbs and View footer links are unaffected. With `title` (default), use frontmatter `nav_title` → `title` → first H1 → humanized filename. With `filename`, use the actual file-system name including its extension (`02-token-projection.md` stays `02-token-projection.md`), including landing pages (`README.md` / `index.md`). Folder rows preserve on-disk case, hyphens and numeric prefixes in both modes. In title mode, a folder's landing page falls back to the folder name and the root landing page to the collection name. Legacy `title+filename` maps to `title` with one warning per config load ([ADR-0020](../decisions/0020-nav-label-modes.md)).

For curated `SUMMARY.md` / `_sidebar.md` navigation, title mode preserves explicit link labels; filename mode uses actual page filenames. Synthetic part/group headings retain their curated labels: they are not filesystem folder rows.

Config (P1/P2):

```toml
[nav]
labels = "title"            # default; "title" | "filename"
```

### Folding rules (applied bottom-up, recursively)

Let a folder's **pages** be its markdown files, and its **children** be its pages plus its non-empty subfolders.

| Folder contains | Rendered as |
|-----------------|-------------|
| Nothing (after excludes) | Hidden |
| Only a README (`README.md` / `index.md`) | **Collapsible group**: label = on-disk folder name; its only item is the README, labeled by `nav.labels` |
| README + other children | **Collapsible group**: label = on-disk folder name; first item = the README (landing page), labeled by `nav.labels`; then the other children |
| Other children, no README | **Collapsible group**: label = on-disk folder name; no landing page |

The root folder is special: the root README is always the **first top-level item** (the wiki entry), and its title-mode label is always the header's root breadcrumb, even when the side nav shows `README.md`.

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
▾ architecture                 ← group (on-disk folder name)
  Architecture Overview        ← landing page (its title)
  ▾ design-system              ← group
    Design System              ← landing page
    Token Projection           ← leaf
  ▸ wfos                       ← group (folder with only a README)
▸ decisions                    ← group (collapsed)
```

> [!NOTE]
> A group (folder name) and its landing page (README title) may read alike when the README title matches the folder. That is intentional: it keeps every row's label truthful and the tree uniform.

### Group header behavior

Clicking or pressing `Enter` on a group header **toggles** it. The landing page is opened by its own item. Groups containing the current page auto-expand after navigation.

### Ordering (tree and prev/next)

First match wins:

1. **`SUMMARY.md`** (mdBook) or **`_sidebar.md`** (docsify) at the root: its nested link list defines order and grouping. Pages it doesn't list are appended in natural order under "Other pages".
2. **Frontmatter `nav_order`** among siblings; unordered siblings follow.
3. **Natural sort**: README/landing first, then `01-…` < `02-…` < `10-…`, case-insensitive; folders and files interleaved by name.

**Prev/next** walk the rendered tree depth-first over *page items only*. Group headers aren't pages. So "next" after a group's landing page is its first child, and "next" after a group's last page is the item after the group.

## Derived stats

Word count excludes frontmatter and code blocks. Reading time is words / 230, rounded up. Both appear in the footer.
