---
id: WR-ADR-0015
title: "ADR-0015: New-tab combos via kitty keyboard disambiguation"
summary: Enable DISAMBIGUATE_ESCAPE_CODES so Cmd/Ctrl+Enter and Cmd/Ctrl+→ open new tabs; Ctrl+click joins Shift+click. Supersedes ADR-0007's "no Ctrl+Enter" consequence in part.
status: accepted
updated: 2026-10-01
related: [0007-input-focus-model]
---

# ADR-0015: New-tab combos via kitty keyboard disambiguation

**Status:** Accepted · **Date:** 2026-10-01 · **Supersedes in part:** [ADR-0007](0007-input-focus-model.md) (Ctrl+Enter reliability)

## Context

[ADR-0007](0007-input-focus-model.md) avoided binding `Ctrl+Enter` because ordinary terminals cannot tell it from plain Enter. Dogfood round 5 wants browser-style new-tab combos: Ctrl+click, plus Cmd/Ctrl+Enter and Cmd/Ctrl+→ in the nav. Crossterm can report SUPER and disambiguated modifiers only after the kitty keyboard protocol's `DISAMBIGUATE_ESCAPE_CODES` flag is pushed. `REPORT_ALL_KEYS_AS_ESCAPE_CODES` would change typing and is not wanted.

Mouse events still cannot carry SUPER (Cmd+click never arrives). On some macOS hosts Ctrl+click is remapped to right-click and never reaches the app.

## Decision

- When `supports_keyboard_enhancement()` is true, push `KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES` at startup; pop it on every exit path (Drop, panic hook, `$EDITOR` suspend/resume). Do not push `REPORT_ALL_KEYS_AS_ESCAPE_CODES`.
- Bind Cmd/Ctrl+Enter and Cmd/Ctrl+→ (Nav) and Cmd/Ctrl+Enter (Viewer) to `NewTab`, matching before plain Enter / AnyCode(Right).
- Treat mouse modifiers `SHIFT | CONTROL | SUPER` like today's Shift+click (including SearchResult hits). Document that Cmd+click is impossible via crossterm mouse and Ctrl+click may be stolen on macOS.
- Keep `t`, middle-click, and Shift+Enter as always-available fallbacks.

## Consequences

- ➕ Cmd/Ctrl+Enter and Cmd/Ctrl+→ open a new tab on terminals that speak the kitty protocol (Ghostty, Kitty, herdr when forwarded).
- ➕ Fallback is clean when enhancement is unsupported: existing bindings keep working.
- ➖ A global input-encoding change; must pop flags on every restore path or nested shells keep sticky keyboard mode.
- ➖ Ctrl+click on macOS may never arrive; Cmd+click cannot.
