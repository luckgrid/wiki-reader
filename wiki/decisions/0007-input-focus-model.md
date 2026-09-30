---
id: WR-ADR-0007
title: "ADR-0007: Input & focus model"
summary: Browser-standard Tab cycling in the viewer, cursor-line arrows with Shift block jumps, Shift+Left/Right pane focus with per-pane cursor memory.
status: accepted
updated: 2026-09-30
related: []
---

# ADR-0007: Input & focus model

**Status:** Accepted · **Date:** 2026-09-28 · **Updated:** 2026-09-30 (P2-22)

## Context

Readers expect browser behavior for links (`Tab` cycles, `Enter` follows) and editor-like behavior for reading position (a cursor line, as in markdown-reader). The two must coexist, and moving between the side nav and viewer must not lose your place.

## Decision

- **Viewer:** `↑/↓` move a cursor line; `Shift+↑/↓` jump by content block. `Tab`/`Shift+Tab` cycle focusable items (links → block actions → footer prev/next) starting after the cursor line, and the cursor follows the focused item. Arrow movement clears item focus. `f` focuses the footer next link (or prev on the last page). Activating a footer link (Enter or click) keeps focus on that side after the new page loads; if that side is missing, fall back to the other.
- **Side nav:** `↑/↓` and `Tab/Shift+Tab` both step items; `Shift+↑/↓` jump between group headers and the search row; `→` expands a group, steps into the first child when already expanded, or opens a page and moves focus to the viewer; `←` collapses / goes to parent. `Enter` opens a page but stays in the nav.
- **Pane focus:** `Shift+←` side nav, `Shift+→` viewer, or a click.
- **Cursor memory:** each pane restores its last position. Defaults are the top of the page (viewer) and the current page's item (side nav). If the current page changed while the side nav was unfocused, the side nav cursor jumps to the new current item.
- **Search:** a modal overlay (`/`, `Ctrl+k`, or the ⌕ row) that restores the prior focus and cursor on close.

## Consequences

- ➕ A familiar model with one visible "where am I" at any time.
- ➕ `Tab` is never overloaded for pane switching.
- ➕ Nav `→` and viewer `f` give a keyboard-only path nav → viewer → footer without Tab hunting.
- ➖ Depends on the terminal and herdr delivering `Shift+arrows`; verified in the Phase 1 input spike, with remapping as the fallback.
- ➖ `Ctrl+Enter` isn't reliable without the kitty keyboard protocol, so new tabs use `t` and middle-click.
