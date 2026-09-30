---
id: WR-SPIKE-P1-S1
title: P1-S1 Spike — herdr input
summary: Which keys and mouse events reach a TUI under herdr; implications for the P1-08/09 keymap.
status: accepted
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

## Live herdr verification (2026-09-29)

Ran `cargo run -p wiki-reader --example keylog` in a herdr pane. Outer terminal = Ghostty via herdr 0.9.0.

**Method note:** the 2026-09-29 keyboard rows were driven with `herdr pane send-keys`, which injects events inside herdr and **bypasses** the outer terminal's key encoding (Ghostty Option-as-Alt, modified-key reporting). Treat those ticks as **synthetic (send-keys)**. A **real keyboard** pass in Ghostty + herdr is still required before closing P1-G.

| Input | Synthetic (send-keys) | Real keyboard |
|-------|----------------------|---------------|
| Plain arrows | [x] `Up`/`Down`/`Left`/`Right` unmodified | [ ] to confirm |
| `Shift+↑/↓` | [x] `SHIFT` on Up/Down | [ ] to confirm |
| `Shift+←/→` / `F6` | [x] `SHIFT` on Left/Right; `F(6)` plain | [ ] to confirm |
| `Backspace` | [x] | [ ] to confirm |
| `Alt+←/→` | [x] `ALT` on Left/Right | [ ] to confirm |
| `Ctrl+↑/↓` | [x] `CONTROL` on Up/Down (block-jump fallback) | [ ] to confirm |
| `Alt+Shift+↑/↓` | [x] `SHIFT \| ALT` — used for P2-04 heading jump | [ ] to confirm |
| `Alt+[` / `Alt+]` | [ ] Arrive as plain `Char(']')` via `send-keys` (unreliable as fallback) | [ ] to confirm |
| Click / wheel | [ ] not tried live — hit-map / wheel **unit tests** cover geometry; mouse capture on in keylog | [ ] to confirm |
| `Ctrl+Enter` | [x] Confirmed unreliable for binding — keep unbound | [ ] to confirm |

Also: `q` and `Ctrl+C` restore the terminal (alt-screen / raw mode) after running wiki-reader under herdr (synthetic session; re-confirm on your machine).
