---
id: WR-ADR-0016
title: "ADR-0016: New-tab combos are Ctrl-only (no Cmd)"
summary: Drop Cmd from the new-tab combos. Ctrl+Enter (nav and view), Ctrl+→ (nav) and Ctrl+click open a new tab; the kitty DISAMBIGUATE flags from ADR-0015 stay.
status: accepted
updated: 2026-10-01
related: [0015-new-tab-combos-kitty-keyboard, 0007-input-focus-model]
---

# ADR-0016: New-tab combos are Ctrl-only (no Cmd)

**Status:** Accepted · **Date:** 2026-10-01 · **Supersedes in part:** [ADR-0015](0015-new-tab-combos-kitty-keyboard.md) (the Cmd/SUPER half of the combos)

## Context

[ADR-0015](0015-new-tab-combos-kitty-keyboard.md) bound `Cmd/Ctrl+Enter` and `Cmd/Ctrl+→` to open a new tab. Dogfooding in Ghostty on macOS showed that `Cmd+Enter` is taken by the terminal (it toggles full screen) and never reaches the app, while `Ctrl+Enter` works. Cmd can't be used on the mouse at all, so a Cmd requirement only added a binding that does the wrong thing on the platform it was meant for.

## Decision

- The matcher is Ctrl only (`Matcher::CtrlCode`). Cmd (SUPER) is not special: if a terminal ever delivers it, it behaves like the plain key.
- Bindings: `Ctrl+Enter` in the nav and in the view (opens the focused link), and `Ctrl+→` in the nav. The Enter combos are `NewTab`; `Ctrl+→` is `NewTabFocusView` (same, then focus moves into the View, like plain `→`). See the macOS caveat below. They match before plain `Enter` and `→`.
- Mouse: Shift+click and Ctrl+click open a new tab (SUPER is no longer in the mask).
- Keep `DISAMBIGUATE_ESCAPE_CODES`, `t`, middle-click, Shift+click and `Shift+Enter` from ADR-0015.

## Consequences

- ➕ The combos that work on macOS (Ctrl) are the documented ones; nothing relies on a modifier the OS owns.
- ➕ One modifier to teach, the same on every platform.
- ➖ `Ctrl+click` can still be stolen as right-click by some macOS hosts; `t`, middle-click and Shift+click remain the always-available routes.
- ➖ `Ctrl+→` never reaches the app on a default macOS setup: it is the system "Move right a space" shortcut (confirmed in Ghostty QA). It is kept for other platforms and for macOS users who disable that shortcut (System Settings → Keyboard → Keyboard Shortcuts → Mission Control). `Ctrl+Enter` is the combo that works everywhere.
