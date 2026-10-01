---
id: WR-ADR-0016
title: "ADR-0016: Drop Cmd/Ctrl+→ new-tab in the nav"
summary: Remove the Cmd/Ctrl+→ new-tab binding from the nav. Keep Cmd/Ctrl+Enter, Ctrl+click, and the kitty DISAMBIGUATE flags from ADR-0015.
status: accepted
updated: 2026-10-01
related: [0015-new-tab-combos-kitty-keyboard, 0007-input-focus-model]
---

# ADR-0016: Drop Cmd/Ctrl+→ new-tab in the nav

**Status:** Accepted · **Date:** 2026-10-01 · **Supersedes in part:** [ADR-0015](0015-new-tab-combos-kitty-keyboard.md) (Cmd/Ctrl+→ only)

## Context

[ADR-0015](0015-new-tab-combos-kitty-keyboard.md) bound Cmd/Ctrl+→ in the nav to `NewTab` alongside Cmd/Ctrl+Enter. Dogfood found Cmd/Ctrl+→ unreliable and not worth the extra binding: plain `→` already expands/opens into the view, and new tabs already have `t`, middle-click, Shift/Ctrl+click, Shift+Enter, and Cmd/Ctrl+Enter.

## Decision

- Remove the Nav `Cmd/Ctrl+→` → `NewTab` keymap entry.
- Keep `DISAMBIGUATE_ESCAPE_CODES`, Cmd/Ctrl+Enter (Nav and Viewer), Ctrl+click, and the other ADR-0015 fallbacks.

## Consequences

- ➕ One fewer modifier combo to teach and to keep working across terminals.
- ➕ Cmd/Ctrl+→ falls through to `NavExpand` (same as plain `→` via `AnyCode`).
- ➖ Users who memorised Cmd/Ctrl+→ for a new tab need Enter or click instead.
