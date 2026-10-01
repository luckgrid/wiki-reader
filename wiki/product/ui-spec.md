---
id: WR-UI
title: UI spec
summary: Layout, side nav, header/footers, focus and cursor model, keyboard and mouse behavior for the wiki-reader reader.
status: draft
updated: 2026-09-30
related: [spec, content-model]
nav_order: 4
---

# UI spec

Layout, side nav, header/footers, focus and cursor model, keyboard and mouse behavior for the wiki-reader reader.

## Layout

<!-- ui-diagram:start -->
```text
 Worked Example Wiki › Architecture › Design System › Token Projection    ○ ◫ ✕
┌Nav───────────────────────┐┌View──────────────────────────────────────────────┐
│▌⌕ Search…                ││                                                  │
│                          ││ ── frontmatter ▶ ───────────────────────         │
│  Worked Example Wiki     ││                                                  │
│  ▾ Architecture          ││▌# Token Projection                               │
│    Architecture Overview ││                                                  │
│    ▾ Design System       ││ The reusable adapter stays semantic-only;        │
│      Design System       ││ see [ADR-0003](../decisions/0003.md).            │
│▌     Token Projection    ││                                                  │
│    ▸ Wfos                ││ ## Linked from                                   │
│  ▸ Decisions             ││ • Worked Example Wiki                            │
│                          ││ • Architecture Overview                          │
└──────────────────────────┘└┤ ‹ Design System ├──────────────┤ Workflow OS › ├┘
 VIEW  · architecture/design-system/tokens.md · L5 9% · 2026-09-28 · draft
```
<!-- ui-diagram:end -->

Nav rows show page titles by default and folders show the folder name (see [Side nav](#side-nav-left)). `○`/`◉` is the syntax/formatted toggle (`v`), `◫` toggles the side nav (`b`), `✕` quits (`q`).

Regions:

- **Header:** one padded row with the breadcrumb on the left and the icon buttons on the right, directly above the panes. The last crumb is the current page and is drawn gray (read-only); the others are links.
- **Side nav:** a bordered pane titled `Nav` holding the search row and the page tree. Its width defaults to 26 (30 at ≥ 120 columns) and can be dragged.
- **View:** a bordered pane titled `View` with one blank row under the top border and the cursor line, which is marked `▌`. Its **prev/next footer is drawn on the pane's bottom border** and stays visible while the article scrolls.
- **Status bar:** one padded row directly under the panes.

The tab bar appears at the top of the viewer pane only when two or more tabs are open. A right-hand widget slot is reserved for Phase 4.

## Header (full width, 1 padded row)

- **Left:** the root entry page's title, then the breadcrumb trail through side-nav groups to the current page. Example: `Project Wiki › Architecture › Design System › Token Projection`. Segments follow the **side nav hierarchy** (groups), not raw directories, so folded folders ([content model](content-model.md)) don't produce extra crumbs. Each segment is clickable and opens that group's landing page. The trail truncates from the middle with `…` when narrow, always keeping the root and current page.
- **Right:** icon buttons. `○`/`◉` switches the rendered view between **syntax** (markdown markers visible, `○`) and **formatted** (markers hidden, `◉`; [ADR-0012](../decisions/0012-syntax-vs-formatted.md)). `◫` toggles the side nav. `✕` quits (saves session; same as `q`).
- Future: optional back/forward buttons (`‹ ›`). Back/forward are keyboard-only in v1.

## Side nav (left)

A file tree **presented as a documentation site's side nav**. Construction rules are in [content model](content-model.md). In short:

- The root entry page (root `README.md`/`index.md`) is the first item.
- Folders always show the **folder name**. Pages are labelled by the `nav.labels` option: **`title`** (the default: `nav_title` → `title` → first H1 → humanized filename), `filename` (humanized filename), or `title+filename` (title with a dim filename suffix). A folder's README and the root README always show their title (the root falls back to the collection name), never "Readme".
- Every folder with a README is a **collapsible group** (the folder name) whose first item is the README, labelled with its title. A folder holding only a README is still a group with that one item.
- Folder rows use their own color and nested rows indent two more columns per level. The selected row has a full-width background highlight and a `▌` marker; it follows the current page however you got there (a link, the footer, search, history). Ancestor groups auto-expand after every navigation.
- The pane is **resizable**: drag the divider between the nav and the viewer (clamped to 16–50 columns; the width is saved with the session and ignored below 80 columns).

**Search entry (top of the side nav).** The first row is `⌕ Search…`, drawn as a bar in both states; when it is the focused nav stop the bar and text turn the accent color. Selecting it, clicking it, or pressing the search hotkey anywhere opens the **search overlay panel** (below). It's a nav stop for Shift+↑/↓ (see keyboard).

Future: this search row becomes a proper **side nav header**, and a **side nav footer** can hold widget actions or tabbed features (e.g. Pages / Outline).

## Search overlay panel

- Opens with `/` or `Ctrl-k` from anywhere, a click on the ⌕ row, or `Enter` on it.
- Floats over the side nav and viewer. Input at the top; results below, grouped by page (title, path, snippet, match count).
- A toggle (`Tab` inside the overlay) switches between **Files** (fuzzy title/path) and **Content** (full-text). The header shows `Files | Content` with the active mode highlighted; Files is the default.
- Results use a larger centered pane (~80% × ~70%, up to ~100 cols). The list scrolls; `Home`/`End`/`PgUp`/`PgDn` and the mouse wheel jump or step selection. Selected row uses the cursor-line background; paths are dim, titles/snippets bright, with query matches underlined.
- `↑`/`↓` or mouse selects; `Enter` or click opens the result in the current view (replace + history), scrolls to the match and highlights it; `n`/`N` then cycle matches in the page.
- `Esc` or a click outside closes it and restores the previous focus and cursor.
- The last query and results are kept for the session.

## Help overlay panel

- Opens with `?` from Normal mode. Generated from the binding table in `keymap.rs` (same source as the live map).
- Sections: Global, Side nav, View, Chords, Search overlay. Each section is a full-width divider row with every binding on its own row. A header icon that also triggers an action (`◫`, `○ ◉`, `✕`) is shown beside its key. Key labels show config overrides when set.
- `↑`/`↓` / `PgUp`/`PgDn` / `g`/`G` and the mouse wheel scroll; `Enter` or a click on a row closes help and runs that action (display-only rows are not clickable).
- `Esc` or `?` closes without an action. Click outside dismisses.

## View (center)

Rendered by default; `r` toggles raw, and `v` (or the header eye) switches the rendered view between syntax and formatted (default syntax). All views share the **cursor line** (see Cursor model), so toggling keeps you on the same source line. The text column is capped at ~100 cols; tables and code may use the full width.

**Focusable items ("actions")** are what `Tab` cycles through, in document order:

1. Links (internal, anchor, external, broken)
2. Block actions: expand/collapse frontmatter (`▶`/`▼`, key-reachable and clickable from the first frame), copy a code block (OSC 52)
3. The View footer's ‹ Prev / Next › buttons (last in the cycle)

After the last item, `Tab` wraps to the first. The focused item renders inverted, and the status bar shows its target or action (`→ decisions/0003.md#context`, `↗ https://…`, `? not found: foo.md`, `copy code`).

**Links:** underlined; broken links in the error color with `?`; external links with `↗`. `Enter` or left-click follows. Middle-click or `t` opens in a new tab. External links ask `open https://… ? [y/N]` in the status bar, then use the system opener. Hovering (if the terminal reports motion) highlights the link and shows its target in the status bar.

**End of article:** "Linked from" list (backlinks), each a focusable link.

Future: a **sticky section header** at the top of the viewer showing the heading of the section in view.

## View footer (prev/next)

`┤ ‹ Prev title ├` on the left and `┤ Next title › ├` on the right, following side nav order ([content model](content-model.md)). They are outlined, padded buttons drawn on the **View pane's bottom border**, so they stay pinned while the article scrolls. The outline matches the pane border (teal when the View is focused, gray when the Nav is); a button reached with `Tab` or `f` fills peach with dark text. Each label is truncated with `…` to its half of the border so long titles cannot collide. Both are clickable and bound to `[` / `]`. A side with no prev/next draws nothing, so the border runs unbroken.

## Status bar (full width, 1 row)

Like markdown-reader's: focused region as a highlighted pill (`NAV`/`VIEW`), relative path, cursor line and scroll %, updated date and the page's frontmatter `status` (colored by value), word count, reading time, and a **message area** for link targets, notices ("not found"), confirmations, and search match `n/m`. Lower-priority items drop first when narrow.

## Focus & cursor model

Two focusable panes: **Side nav** and **View**. The search overlay is modal while open.

- `Shift+←` focuses the side nav; `Shift+→` focuses the View. Mouse click also focuses a pane.
- **Each pane remembers its cursor.** Switching returns to the last known position in that pane.
- **Defaults when a pane has no remembered position:**
  - Viewer: top of the current page.
  - Side nav: the item for the current page (e.g. when the root README is open, the top item).
- **Navigation updates cursors:**
  - Opening a page sets the viewer cursor to the anchor target, else to the remembered cursor for that history entry (back/forward), else the top.
  - If the current page changed while the side nav wasn't focused (via a link, search, or prev/next), refocusing the side nav puts its cursor on the new current page rather than its stale position. Otherwise it keeps its remembered position.
- The focused pane gets an accent border; the other gets a dim border.

## Keyboard

The tables below are generated from the binding table that also drives the `?` help overlay, so they cannot drift from it. `Esc` closes overlays; it does not quit.

<!-- keymap:start -->
Generated from `BINDINGS` in `crates/wiki-reader/src/tui/keymap.rs`, the same table the `?` help overlay shows. Edit the table there, then run `UPDATE_DOCS=1 cargo test -p wiki-reader keymap_docs`.

### Global

| Key | Action |
|-----|--------|
| `Alt+← / Alt+b` | Back |
| `Alt+→ / Alt+f` | Forward |
| `q` ✕ | Quit |
| `b` ◫ | Toggle side nav |
| `r` | Toggle raw / rendered |
| `v` ○ ◉ | Toggle syntax / formatted |
| `e` | Open in editor |
| `y` | Copy page path |
| `Y` | Copy focused link target |
| `t` | New tab |
| `x` | Close tab |
| `/` | Search |
| `Ctrl+k` | Search |
| `?` | Help |
| `n` | Next search match |
| `N` | Previous search match |
| `[` | Previous page |
| `]` | Next page |
| `Backspace` | Back |
| `Shift+←` | Focus side nav |
| `Shift+→` | Focus view |
| `F6` | Cycle pane focus |

### Side nav

| Key | Action |
|-----|--------|
| `Shift+↑ / Ctrl+↑` | Jump to previous group / search |
| `Shift+↓ / Ctrl+↓` | Jump to next group |
| `↑` | Previous nav row |
| `Shift+Tab` | Previous nav row |
| `↓` | Next nav row |
| `Tab` | Next nav row |
| `→` | Expand / open into view |
| `←` | Collapse / parent group |
| `Enter` | Open page (stay in nav) / toggle group |

### View

| Key | Action |
|-----|--------|
| `Alt+Shift+↑` | Previous heading |
| `Alt+Shift+↓` | Next heading |
| `{` | Previous heading |
| `}` | Next heading |
| `Shift+↑ / Ctrl+↑` | Previous block |
| `Shift+↓ / Ctrl+↓` | Next block |
| `k` | Cursor up |
| `↑` | Cursor up |
| `j` | Cursor down |
| `↓` | Cursor down |
| `PgUp` | Page up |
| `Shift+Space` | Page up |
| `Space` | Page down |
| `PgDn` | Page down |
| `Home` | Top of page |
| `End` | Bottom of page |
| `G` | Bottom of page |
| `Shift+Tab` | Previous focusable item |
| `Tab` | Next focusable item |
| `Enter` | Activate focused item |
| `f` | Focus footer prev/next |

### Chords

| Key | Action |
|-----|--------|
| `gg` | Top of page |
| `gt` | Next tab |
| `gT` | Previous tab |

### Search overlay

| Key | Action |
|-----|--------|
| `Esc` | Close search |
| `Enter` | Open selected result |
| `Tab` | Toggle Files / Content |
| `↑ / ↓` | Move selection |
| `Home / End` | Jump to first / last result |
| `PgUp / PgDn` | Page results |
<!-- keymap:end -->

### Notes

- **Tab and cursor:** `Tab` focuses the first item *after* the cursor line, and the cursor line moves to that item. Moving the cursor with arrows clears item focus (and sticky footer focus). Activating a footer link keeps focus on that side after the page changes, so you can step through pages quickly. This keeps one visible "where am I" at all times.
- **`Enter` in the viewer:** activates the focused item. With no item focused and exactly one link on the cursor line, it follows that link.
- **Side nav `→` / `←`:** `→` expands a group; on an expanded group it steps to the first child; on a page row it opens the page and focuses the viewer. `←` collapses, or goes to the parent group from a child.
- **Side nav `Enter` / click:** a page opens in place and the nav keeps focus; a group toggles; the search row opens the search overlay.
- **New tabs:** `t` or middle-click opens the focused link or nav row in a new tab.
- **Back / forward:** `Backspace` is primary. `Alt+b` / `Alt+f` are what macOS Ghostty sends for Option+←/→.
- **Heading jump:** `{` / `}` is the fallback when `Alt+Shift+↑/↓` does not reach the TUI.

### Terminal key caveats (verified, Ghostty + herdr, macOS)

- **K1:** Ghostty sends Option+←/→ as readline `Alt+b` / `Alt+f`, not as arrows with ALT. Bind `Alt+b`→Back and `Alt+f`→Forward (P1-R36); keep `Backspace` and `Alt+←/→` when those arrive as arrows.
- **K2:** Plain-letter bindings must ignore CONTROL/ALT (P1-R36). Before that fix, Alt+q quit, Alt+e opened the editor, Ctrl+b toggled the nav.
- **K3:** Right-click is reserved by herdr's context menu and never reaches the app — unused by wiki-reader.
- **K4:** Alt+↑/↓ and Alt+Shift+↑/↓ arrive correctly; heading jump works.
- `Shift+Tab` arrives as `BackTab`; fine everywhere.
- `Ctrl+Enter` / `Shift+Enter` can't be told apart from `Enter` without the kitty keyboard protocol. Use `t` and middle-click for new tabs.

## Mouse

Click to focus a pane; click items, links, breadcrumbs, prev/next, header icons, and search results; middle-click for a new tab; wheel scrolls the pane under the pointer. Hover works only where motion events arrive. Right-click is owned by herdr (see K3). Click, middle-click, and wheel verified inside herdr panes.

## Responsive rules

| Width | Layout |
|-------|--------|
| ≥ 120 | Side nav 30 (draggable, 16–50) · Viewer flex |
| 80–119 | Side nav 26 (draggable, 16–50) · Viewer flex |
| < 80 | Side nav hidden; `◫`/`b` shows it as an overlay that closes after navigation |

## Future UI slots (designed for, not built)

| Slot | Purpose |
|------|---------|
| Viewer header | Sticky heading of the section in view |
| Side nav header | Search field (replacing the search row), mode tabs |
| Side nav footer | Widget actions, tabbed features (Pages / Outline / …) |
| Right widget sidebar | Phase 4 widgets, incl. context engine ([context engine](../architecture/context-engine.md)) |
| Header ‹ › buttons | Optional back/forward |

## Theming

Semantic tokens only (`tui/theme.rs`): `surface`, `surface_muted`, `border`, `border_focus`, `text`, `text_muted`, `accent`, `cursor_line`, `search_box`, `tab_active`, `tab_inactive`, `focus_item`, `link`, `link_broken`, `link_external`, `link_unsupported`, `code_bg`, `quote_bar`, `heading[1..6]`, `alert[…]`. The `theme` config key is stored but not wired yet; presets arrive with [Phase 3](../roadmap/phase-3-alpha.md).
