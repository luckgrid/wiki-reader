---
id: WR-SPIKE-P1-S1
title: P1-S1 Spike — herdr input
summary: Which keys and mouse events reach a TUI under herdr; implications for the P1-08/09 keymap.
status: draft
updated: 2026-09-29
related: [phase-1-reader-shell, 0007-input-focus-model]
---

# P1-S1 Spike — herdr input

Timebox note for Phase 1. Sources: herdr 0.9.x docs ([Keyboard](https://herdr.dev/docs/keyboard/), [Troubleshooting](https://herdr.dev/docs/troubleshooting/), [Concepts](https://herdr.dev/docs/concepts/)), plus ADR-0007.

## How input reaches the pane

- In **terminal mode**, herdr forwards keys and mouse to the focused pane process (wiki-reader).
- Herdr owns only the **prefix** (default `ctrl+b`) and any **direct** chords bound in its config; everything else should pass through.
- Herdr is **mouse-native** at the chrome layer (pane focus, splits). With default `mouse_capture`, click/drag/wheel on a pane are forwarded when the app enables mouse reporting (crossterm/ratatui).

## Findings for the wiki-reader keymap

| Input | Expected under herdr | Notes for P1-08/09 |
|-------|----------------------|--------------------|
| Plain arrows | Reach the TUI | Safe for viewer cursor / nav step |
| `Shift+↑/↓` | Usually reach the TUI when the outer terminal reports modified keys | Needed for block jumps (ADR-0007). If missing, remap to `Ctrl+↑/↓` or `[{` / `]}` |
| `Shift+←/→` | Same as above | Pane focus swap. Fallback: `Ctrl-w` then `h`/`l`, or `F6` |
| `Alt+←/→` | Forwarded as CSI `1;3D` / `1;3C` when Option=Alt | herdr does **not** rewrite these (issue #1370). Prefer for **back/forward** only if the outer terminal is configured; else use `Backspace` / `Alt+←` with documented setup, or `Ctrl+o` / `Ctrl+i` |
| `Backspace` | Reaches the TUI | Good primary **back** binding |
| Click / wheel | Reach the TUI when mouse reporting is on | Hit map works; middle-click for new tab (ADR-0005) needs button reporting |
| `Ctrl+Enter` | Unreliable without kitty keyboard protocol | Do **not** bind new-tab to Ctrl+Enter; use `t` + middle-click (ADR-0005/0007) |

## Double Enter/Tab/Backspace

Old outer terminals can emit press+release as duplicate bytes under kitty keyboard reporting. Fixed in kitty ≥ 0.33, foot ≥ 1.20, Alacritty ≥ 0.15. If dogfooding shows double-fires, document the terminal upgrade rather than debouncing in-app first.

## Recommended Phase 1 bindings (draft)

- **Back:** `Backspace` (primary), `Alt+←` (secondary, when Option=Alt)
- **Forward:** `Alt+→` (when available)
- **Pane focus:** `Shift+←` / `Shift+→`, with `F6` fallback
- **Block jump:** `Shift+↑/↓`, with `Ctrl+↑/↓` fallback if needed
- **New tab:** `t` / middle-click — never `Ctrl+Enter`

## Verification still needed in a live herdr pane

Interactive confirm (½ day leftover): run wiki-reader under herdr, enable mouse, and log crossterm `KeyEvent` / `MouseEvent` for the chords above on the developer's outer terminal (Ghostty / kitty / iTerm). Update this note with per-terminal checkmarks when done; remaps above cover the failure modes already documented by herdr.
