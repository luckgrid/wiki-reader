---
id: WR-SPEC
title: Product spec
summary: User stories, prioritized requirements with acceptance criteria, and success measures for the wiki-reader reader.
status: draft
updated: 2026-09-28
related: [vision, content-model, ui-spec]
nav_order: 2
---

# Product spec

User stories, prioritized requirements with acceptance criteria, and success measures for the wiki-reader reader.

## Problem statement

Terminal markdown tools display files but don't support wiki browsing: links are inert, file opens pile up as tabs, and search is disconnected from navigation. Reading a collection properly still means leaving the terminal.

## Goals

1. Browse a markdown collection like a documentation site, inside a herdr pane at ≥ 80 columns.
2. Make every link followable by mouse and keyboard, with history.
3. Keep orientation visible: tree position, breadcrumbs, prev/next, "linked from".
4. Make search part of the sidebar, not a separate mode that spawns tabs.

## User stories

**Browsing**

- As a reader, I want selecting a page in the tree to replace the current page so that browsing feels like a wiki, not an editor.
- As a reader, I want back/forward (keys and clickable buttons) so that I can retrace my path.
- As a reader, I want breadcrumbs I can click so that I can jump up to a section.
- As a reader, I want prev/next links in the footer so that I can read a section front to back.
- As a reader, I want the side nav to show document titles in a site-like structure, and to reveal and highlight the page I'm on however I got there.
- As a keyboard user, I want Shift+←/→ to move between side nav and viewer and land back where my cursor was.
- As a reader, I want to open a page in a new tab only when I ask for it, as in a browser.

**Links**

- As a reader, I want to click a link in the article to follow it.
- As a keyboard user, I want Tab / Shift+Tab to move between links and actions (browser-style), arrows to move a cursor line, and Shift+arrows to skip whole blocks.
- As a reader, I want to see where a link goes (footer) before following it.
- As a reader, I want broken links to look broken and explain themselves when followed.

**Search**

- As a reader, I want a search overlay I can open from anywhere (hotkey or the ⌕ row at the top of the side nav) so that I can jump to any page, and picking a result opens it in the current view.
- As a reader, I want closing search to return me to exactly where my cursor was.

**Viewing**

- As a reader, I want to toggle rendered and raw markdown.
- As a reader, I want diagrams rendered in place.

**Edge cases**

- Empty collection → empty state naming the scanned root.
- Broken frontmatter → shown raw with a warning, never a crash.
- Anchor that doesn't exist → navigate to the page top + footer notice.
- File deleted while open → "page removed" state, with back still working.

## Requirements

### P0: reader shell (can't ship without)

| ID | Requirement | Acceptance criteria |
|----|-------------|---------------------|
| N1 | Single navigation path | Side nav, search result, link, breadcrumb, prev/next, back/forward, and start page all call `navigate(target, disposition)`. Invariant tests assert identical history entries. |
| N2 | Replace-by-default | With one tab, opening a page by side nav/search/link replaces the view and puts the old page in back history. No tab is created. |
| N3 | History | `Backspace`, `Alt+←/→` move back/forward; cursor line and scroll are restored on back; anchor jumps create entries. |
| N4 | Nav sync | After any navigation the current item is marked `●` and its ancestor groups expand. The side-nav cursor follows the focus rules in [UI spec](ui-spec.md). |
| T1 | Side nav construction | Titles, not filenames; root README first; folding rules and ordering exactly as in [content model](content-model.md), verified with fixture snapshots (incl. the worked example). |
| K1 | Pane focus | `Shift+←` focuses side nav, `Shift+→` viewer; click focuses; each pane restores its remembered cursor; defaults as in [UI spec](ui-spec.md). |
| K2 | Viewer cursor | `↑/↓` move the cursor line; `Shift+↑/↓` jump by content block; cursor survives raw/rendered toggle. |
| K3 | Viewer Tab cycle | `Tab`/`Shift+Tab` cycle links → block actions → footer prev/next, wrapping; start after the cursor line; the focused item is highlighted and its target shown in the status bar. |
| K4 | Side nav keys | `↑/↓` and `Tab/Shift+Tab` move item by item; `Shift+↑/↓` jump between group headers and the search row; `→/←` expand/collapse; `Enter` opens or toggles. |
| L1 | Links: mouse | Left-click anywhere on a link's text (including wrapped segments) follows it; clicking non-link text only moves the cursor. |
| L2 | Link resolution | Rules in [content model](content-model.md) (relative, extensionless, folder, anchor, root-relative). |
| L3 | Broken & external | Broken links are styled distinctly and explain themselves; external links confirm, then use the system opener. |
| H1 | Header | Root title + group breadcrumb trail (clickable; middle truncation keeps root and current); right icons `◫` toggle side nav and `✕` quit. |
| F1 | Viewer footer | Sticky prev/next bar at the bottom of the viewer pane, viewer width; clickable; in the Tab cycle; `[`/`]`. |
| F2 | Status bar | Full-width bottom bar: focused pane, path, line/%, words, reading time, updated, message area. |
| S1 | Search overlay | `/`, `Ctrl+k`, clicking or `Enter` on the side nav ⌕ row opens it; Pages/Text toggle; results replace the view and highlight the match; `Esc` restores prior focus and cursor. |
| V1 | Rendered view | Headings, emphasis, lists, task lists, tables, code, blockquotes/alerts, links, rules, frontmatter box. |
| V2 | Raw view | `r` toggles; highlighted markdown; same cursor line. |
| V3 | Live reload | External edits refresh the side nav and page within 1 s, keeping the cursor. |

### P1: MVP polish

| ID | Requirement | Acceptance criteria |
|----|-------------|---------------------|
| P1 | Custom nav order | `SUMMARY.md`/`_sidebar.md` and `nav_order` respected ([content model](content-model.md)). |
| B1 | Linked from | Backlinks listed at the end of the article, focusable. |
| TB | Tabs (secondary) | `t`/middle-click opens a new tab; the tab bar appears only with ≥ 2 tabs; per-tab history. |
| BA | Block actions | Expand table, show frontmatter, expand diagram, copy code: all in the Tab cycle. |
| D1 | Diagrams | Tiered Mermaid per [ADR-0004](../decisions/0004-diagram-rendering.md). |
| R1 | Responsive | Side nav auto-hides below 80 cols; overlay via `◫`/`b`. |
| E1 | Open in editor | `e` at the cursor line. |
| C1 | Config | Keys, opener, theme, diagram mode, excludes, `nav.labels`. |
| M1 | Session | Restore last page, history, and both pane cursors per root. |
| J1 | Heading jump | `Alt+Shift+↑/↓` jump by heading (if the keys arrive reliably; else remap). |

### P2: design for, don't build

| ID | Consideration | Design implication now |
|----|---------------|------------------------|
| U1 | Sticky viewer section header | Renderer keeps a heading index by line; the viewer layout reserves an optional top row. |
| U2 | Side nav header/footer | Side nav is a column of (header?, list, footer?) sub-regions from the start. |
| U3 | Nav label options | `nav.labels = title | filename | title+filename`. |
| U4 | Header back/forward buttons | Header right/left slots are lists of icon buttons, not hard-coded. |
| W1 | Widget sidebar incl. context engine ([context engine](../architecture/context-engine.md)) | Optional right slot; widget trait gets read-only page + index. |
| W2 | Agent CLI (`--json`) | Core stays terminal-free ([ADR-0006](../decisions/0006-reader-first.md)). |
| W3 | External provider | `CollectionProvider` trait. |
| W4 | Design-system themes / shared design tokens | Semantic theme tokens only. |
| W5 | herdr integration | "Current page changed" event. |
| W6 | Link hover preview | Hit map already knows link targets. |

## Success measures

- **Browsing without tabs:** a full session (tree → link → back → search → next) never creates a tab unless asked.
- **Link reliability:** `--check-links`-style audit of fixtures: 100% of resolvable links are followable by click and by Tab+Enter.
- **Adoption:** after two weeks you don't open a GUI app for these collections.
- **Performance:** cold start < 300 ms for 1,000 pages; navigation < 50 ms excluding image diagrams.

## Open questions

| Question | Blocking? |
|----------|-----------|
| Do your collections use `SUMMARY.md`/`_sidebar.md`, numbered filenames, or neither? (drives P1 nav order) | No |
| External links: open directly, or confirm first? (default: confirm) | No |
| Should group headers also open their landing page on click (in addition to toggling)? Current spec: toggle only. | No |
| Does markdown-reader's crate expose a library API we can depend on instead of porting? | No |
