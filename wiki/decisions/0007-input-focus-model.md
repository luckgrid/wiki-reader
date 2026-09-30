---
id: WR-ADR-0007
title: ADR-0007: Input & focus model
summary: Browser-standard Tab cycling in the viewer, cursor-line arrows with Shift block jumps, Shift+Left/Right pane focus with per-pane cursor memory.
status: accepted
updated: 2026-09-28
related: []
---

# ADR-0007: Input & focus model

**Status:** Accepted · **Date:** 2026-09-28

## Context

Readers expect browser behavior for links (`Tab` cycles, `Enter` follows) and editor-like behavior for reading position (a cursor line, as in markdown-reader). The two must coexist, and moving between the side nav and viewer must not lose your place.

## Decision

- **Viewer:** `↑/↓` move a cursor line; `Shift+↑/↓` jump by content block. `Tab`/`Shift+Tab` cycle focusable items (links → block actions → footer prev/next) starting after the cursor line, and the cursor follows the focused item. Arrow movement clears item focus.
- **Side nav:** `↑/↓` and `Tab/Shift+Tab` both step items; `Shift+↑/↓` jump between group headers and the search row; `→/←` expand and collapse.
- **Pane focus:** `Shift+←` side nav, `Shift+→` viewer, or a click.
- **Cursor memory:** each pane restores its last position. Defaults are the top of the page (viewer) and the current page's item (side nav). If the current page changed while the side nav was unfocused, the side nav cursor jumps to the new current item.
- **Search:** a modal overlay (`/`, `Ctrl+k`, or the ⌕ row) that restores the prior focus and cursor on close.

## Consequences

- ➕ A familiar model with one visible "where am I" at any time.
- ➕ `Tab` is never overloaded for pane switching.
- ➖ Depends on the terminal and herdr delivering `Shift+arrows`; verified in the Phase 1 input spike, with remapping as the fallback.
- ➖ `Ctrl+Enter` isn't reliable without the kitty keyboard protocol, so new tabs use `t` and middle-click.
