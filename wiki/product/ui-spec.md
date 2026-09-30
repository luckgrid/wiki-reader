---
id: WR-UI
title: UI spec
summary: Layout, side nav, header/footers, focus and cursor model, keyboard and mouse behavior for the wiki-reader reader.
status: draft
updated: 2026-09-29
related: [spec, content-model]
nav_order: 4
---

# UI spec

Layout, side nav, header/footers, focus and cursor model, keyboard and mouse behavior for the wiki-reader reader.

## Layout

```
┌ Header (full width) ─────────────────────────────────────────────────────────┐
│ Luckgrid Wiki › Architecture › Design System › Token Projection       ◫   ✕ │
├ Side nav ────────────────────┬ Viewer ───────────────────────────────────────┤
│ ⌕ Search…               /    │ # Token Projection                            │
│ ● Luckgrid Wiki              │                                               │
│ ▾ Architecture               │ The reusable adapter stays semantic-only; see │
│     Architecture Overview    │ [ADR-0003](../decisions/0003.md) for why.     │
│   ▾ Design System            │                                               │
│       Design System          │▌cursor line                                   │
│     ● Token Projection       │                                               │
│     Workflow OS              │                                               │
│ ▸ Decisions                  ├ Viewer footer (sticky) ───────────────────────┤
│ ▸ Workstreams                │ ‹ Design System                  Adapters ›   │
├──────────────────────────────┴───────────────────────────────────────────────┤
│ Status bar (full width): VIEWER · path · L42 38% · words · min · link target │
└──────────────────────────────────────────────────────────────────────────────┘
```

Five regions: **Header**, **Side nav**, **Viewer** (with its own sticky **Viewer footer**), and **Status bar**. The tab bar appears above the viewer only when two or more tabs are open. A right-hand widget slot is reserved for v2.

## Header (full width, 1 row)

- **Left:** the root entry page's title, then the breadcrumb trail through side-nav groups to the current page. Example: `Luckgrid Wiki › Architecture › Design System › Token Projection`. Segments follow the **side nav hierarchy** (groups), not raw directories, so folded folders ([content model](content-model.md)) don't produce extra crumbs. Each segment is clickable and opens that group's landing page. The trail truncates from the middle with `…` when narrow, always keeping the root and current page.
- **Right:** icon buttons. `◫` toggles the side nav, `✕` quits (saves session; same as `q`).
- Future: optional back/forward buttons (`‹ ›`). Back/forward are keyboard-only in v1.

## Side nav (left)

A file tree **presented as a documentation site's side nav**. Construction rules are in [content model](content-model.md). In short:

- The root entry page (root `README.md`/`index.md`) is the first item.
- Items show **document titles** (`nav_title` → `title` → first H1 → humanized filename), not filenames. A config option toggles filenames, or shows the filename as dim alt text below the title.
- A folder whose only page is its README is shown as a **single link**, not a collapsible. Folders with more content become **collapsible groups** with their README as the first item (the landing page).
- The current page is marked `●` and highlighted. Its ancestor groups auto-expand after every navigation.

**Search entry (top of the side nav).** The first row is `⌕ Search…`. Selecting it, clicking it, or pressing the search hotkey anywhere opens the **search overlay panel** (below). It's a nav stop for Shift+↑/↓ (see keyboard).

Future: this search row becomes a proper **side nav header**, and a **side nav footer** can hold widget actions or tabbed features (e.g. Pages / Outline).

## Search overlay panel

- Opens with `/` or `Ctrl-k` from anywhere, a click on the ⌕ row, or `Enter` on it.
- Floats over the side nav and viewer. Input at the top; results below, grouped by page (title, path, snippet, match count).
- A toggle (`Tab` inside the input, or clickable) switches between **Pages** (fuzzy title/path) and **Text** (full-text).
- `↑`/`↓` or mouse selects; `Enter` or click opens the result in the current view (replace + history), scrolls to the match and highlights it; `n`/`N` then cycle matches in the page.
- `Esc` or a click outside closes it and restores the previous focus and cursor.
- The last query and results are kept for the session.

## Viewer (center)

Rendered by default; `r` toggles raw. Both views share the **cursor line** (see Cursor model), so toggling keeps you on the same source line. The text column is capped at ~100 cols; tables and code may use the full width.

**Focusable items ("actions")** are what `Tab` cycles through, in document order:
1. Links (internal, anchor, external, broken)
2. Block actions (planned, P2-03): expand a truncated table, show collapsed frontmatter, expand a diagram, copy a code block
3. The viewer footer's ‹ Prev / Next › buttons (last in the cycle)

After the last item, `Tab` wraps to the first. The focused item renders inverted, and the status bar shows its target or action (`→ decisions/0003.md#context`, `↗ https://…`, `? not found: foo.md`, `expand table`).

**Links:** underlined; broken links in the error color with `?`; external links with `↗`. `Enter` or left-click follows. Middle-click or `t` opens in a new tab. External links ask `open https://… ? [y/N]` in the status bar, then use the system opener. Hovering (if the terminal reports motion) highlights the link and shows its target in the status bar.

**End of article:** "Linked from" list (backlinks), each a focusable link.

Future: a **sticky section header** at the top of the viewer showing the heading of the section in view.

## Viewer footer (sticky, 1 row, viewer width only)

`‹ Prev title` on the left, `Next title ›` on the right, following side nav order ([content model](content-model.md)). It stays pinned to the bottom of the viewer pane while the article scrolls. Both are clickable, reachable by `Tab`, and bound to `[` / `]`. A side with no prev/next is dim and skipped by `Tab`.

## Status bar (full width, 1 row)

Like markdown-reader's: focused region (`NAV`/`VIEWER`/`SEARCH`), relative path, cursor line and scroll %, word count, reading time, updated (git or mtime), and a **message area** for link targets, notices ("not found"), confirmations, and search match `n/m`. Lower-priority items drop first when narrow.

## Focus & cursor model

Two focusable panes: **Side nav** and **Viewer**. The search overlay is modal while open.

- `Shift+←` focuses the side nav; `Shift+→` focuses the viewer. Mouse click also focuses a pane.
- **Each pane remembers its cursor.** Switching returns to the last known position in that pane.
- **Defaults when a pane has no remembered position:**
  - Viewer: top of the current page.
  - Side nav: the item for the current page (e.g. when the root README is open, the top item).
- **Navigation updates cursors:**
  - Opening a page sets the viewer cursor to the anchor target, else to the remembered cursor for that history entry (back/forward), else the top.
  - If the current page changed while the side nav wasn't focused (via a link, search, or prev/next), refocusing the side nav puts its cursor on the new current page rather than its stale position. Otherwise it keeps its remembered position.
- The focused pane gets an accent border; the other gets a dim border.

## Keyboard

### Viewer

| Key | Action |
|-----|--------|
| `↑` / `↓` (`k` / `j`) | Move cursor line |
| `Shift+↑` / `Shift+↓` | Jump to previous / next **content block** (paragraph, list, code, table, quote, diagram). Headings are blocks, so this also lands on section starts. |
| `Alt+Shift+↑` / `↓` · `{` / `}` | Jump to previous / next **heading** (section skip). `{`/`}` is the fallback when Alt+Shift does not reach the TUI. |
| `PgUp` / `PgDn` · `Space` / `Shift+Space` | Page up / down |
| `Home` / `End` (`gg` / `G`) | Top / bottom |
| `Tab` / `Shift+Tab` | Next / previous focusable item (links, block actions, footer buttons) |
| `Enter` | Activate focused item. With no item focused and exactly one link on the cursor line, follow it. |
| `t` · middle-click | Open focused link in a new tab |
| `[` / `]` | Previous / next page |
| `Backspace` · `Alt+←` / `Alt+→` | Back / forward |
| `r` | Rendered / raw |
| `e` | Open in `$EDITOR` at cursor line |
| `y` / `Y` | Copy page path / focused link target (OSC 52) |

**Tab ↔ cursor interplay:** `Tab` focuses the first item *after* the cursor line, and the cursor line moves to that item. Moving the cursor with arrows clears item focus. This keeps one visible "where am I" at all times.

### Side nav

| Key | Action |
|-----|--------|
| `↑` / `↓` · `Tab` / `Shift+Tab` | Previous / next visible item (identical behavior) |
| `Shift+↑` / `Shift+↓` | Jump between **group headers and the search row** |
| `→` / `←` | Expand / collapse group (on a child: `←` goes to its parent group) |
| `Enter` · click | Page item: open (replace). Group header: toggle. Search row: open search overlay. |
| `t` · middle-click | Open item in a new tab |

### Global

| Key | Action |
|-----|--------|
| `Shift+←` / `Shift+→` | Focus side nav / viewer |
| `/` · `Ctrl+k` | Search overlay |
| `b` · click `◫` | Toggle side nav |
| `gt` / `gT` / `x` | Next / previous / close tab (when tabs exist) |
| `?` | Help overlay |
| `q` · click `✕` | Quit (saves session) |

### Terminal key caveats (verify in the Phase 1 input spike)
- `Shift+Tab` arrives as `BackTab`; fine everywhere.
- `Shift+arrows` are reported by most modern terminals (crossterm exposes the modifier). Confirm that herdr forwards them to the pane and doesn't bind them itself.
- `Ctrl+Enter` / `Shift+Enter` can't be told apart from `Enter` without the kitty keyboard protocol. Use `t` and middle-click for new tabs, and treat `Ctrl+Enter` as a bonus when the protocol is available.
- `Alt+arrows` may be taken by macOS terminal settings ("Option as Meta"). `Backspace` is the reliable back key.

## Mouse

Click to focus a pane; click items, links, breadcrumbs, prev/next, header icons, and search results; middle-click for a new tab; wheel scrolls the pane under the pointer. Hover works only where motion events arrive. Verify click, middle-click, wheel, and motion inside herdr panes.

## Responsive rules

| Width | Layout |
|-------|--------|
| ≥ 120 | Side nav 30 · Viewer flex |
| 80–119 | Side nav 26 · Viewer flex |
| < 80 | Side nav hidden; `◫`/`b` shows it as an overlay that closes after navigation |

## Future UI slots (designed for, not built)

| Slot | Purpose |
|------|---------|
| Viewer header | Sticky heading of the section in view |
| Side nav header | Search field (replacing the search row), mode tabs |
| Side nav footer | Widget actions, tabbed features (Pages / Outline / …) |
| Right widget sidebar | v2 widgets, incl. context engine ([context engine](../architecture/context-engine.md)) |
| Header ‹ › buttons | Optional back/forward |

## Theming
Semantic tokens only: `surface`, `surface.muted`, `border`, `border.focus`, `text`, `text.muted`, `text.alt` (filename alt text), `accent`, `cursor.line`, `focus.item`, `link`, `link.broken`, `link.external`, `heading.1..6`, `code.bg`, `match`, `status.ok/warn/error`.
