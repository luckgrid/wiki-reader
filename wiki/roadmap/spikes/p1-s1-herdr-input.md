---
id: WR-SPIKE-P1-S1
title: P1-S1 Spike — herdr input
summary: Which keys and mouse events reach a TUI under herdr; implications for the P1-08/09 keymap.
status: done
updated: 2026-10-05
related: [phase-1-reader-shell, 0007-input-focus-model, 0015-new-tab-combos-kitty-keyboard, 0016-ctrl-only-new-tab-combos]
---

# P1-S1 Spike — herdr input

Timebox note for Phase 1. Sources: herdr 0.9.x docs ([Keyboard](https://herdr.dev/docs/keyboard/), [Troubleshooting](https://herdr.dev/docs/troubleshooting/), [Concepts](https://herdr.dev/docs/concepts/)), plus ADR-0007.

**Later:** the "do not bind Ctrl+Enter" guidance below is superseded in part by [ADR-0015](../../decisions/0015-new-tab-combos-kitty-keyboard.md) / [ADR-0016](../../decisions/0016-ctrl-only-new-tab-combos.md) (kitty disambiguation; Ctrl-only new-tab combos).

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
| `Ctrl+Enter` | Unreliable without kitty keyboard protocol | Phase 1: do **not** bind new-tab to Ctrl+Enter; use `t` + middle-click (ADR-0005/0007). Superseded later by [ADR-0015](../../decisions/0015-new-tab-combos-kitty-keyboard.md) / [ADR-0016](../../decisions/0016-ctrl-only-new-tab-combos.md) |

## Double Enter/Tab/Backspace

Old outer terminals can emit press+release as duplicate bytes under kitty keyboard reporting. Fixed in kitty ≥ 0.33, foot ≥ 1.20, Alacritty ≥ 0.15. If dogfooding shows double-fires, document the terminal upgrade rather than debouncing in-app first.

## Recommended Phase 1 bindings (draft)

- **Back:** `Backspace` (primary), `Alt+←` (secondary, when Option=Alt), `Alt+b` (Ghostty Option+← encoding — P1-R36)
- **Forward:** `Alt+→` / `Alt+f` (Ghostty Option+→)
- **Pane focus:** `Shift+←` / `Shift+→`, with `F6` fallback
- **Block jump:** `Shift+↑/↓`, with `Ctrl+↑/↓` fallback if needed
- **New tab:** `t` / middle-click — never `Ctrl+Enter` (Phase 1 draft; later [ADR-0015](../../decisions/0015-new-tab-combos-kitty-keyboard.md) / [ADR-0016](../../decisions/0016-ctrl-only-new-tab-combos.md))

## Real keyboard verification (Ghostty + herdr, macOS)

Ran `cargo run -p wiki-reader --example keylog` and the reader on a real collection in a herdr pane. Outer terminal = Ghostty via herdr 0.9.0. **Real keyboard** (not `send-keys`).

| Input | Result |
|-------|--------|
| Plain arrows | [x] reach the TUI |
| `Shift+↑/↓` | [x] reach the TUI |
| `Ctrl+↑/↓` | [x] reach the TUI |
| `Shift+←/→` / `F6` | [x] reach the TUI |
| `Backspace` | [x] |
| `Alt+↑/↓` | [x] arrive correctly |
| `Alt+Shift+↑/↓` | [x] arrive correctly — heading jump works |
| Click / wheel / middle-click | [x] |
| `Alt+←/→` | arrive as **Alt+b** / **Alt+f** (Ghostty Option encoding), not as arrows with ALT. Alt+→ (forward) never arrives as an arrow; Alt+b matched plain `b` (ToggleNav) before P1-R36 |
| Right-click | intercepted by herdr (its context menu); never reaches the app |
| `Ctrl+Enter` | still unreliable — keep unbound |

Also: `q` and `Ctrl+C` restore the terminal (alt-screen / raw mode) after running wiki-reader under herdr.

**Follow-ups:** P1-R36 done (modifier-strict plain keys + Alt+b/Alt+f Back/Forward). Enter/Tab in panes also require no Ctrl/Alt; Overlay search accepts AltGr / Option non-ASCII.

## Historical: synthetic herdr verification (2026-09-29)

Earlier pass used `herdr pane send-keys`, which injects events inside herdr and **bypasses** the outer terminal's key encoding (Ghostty Option-as-Alt, modified-key reporting). Kept for reference only.

| Input | Synthetic (send-keys) |
|-------|----------------------|
| Plain arrows | [x] `Up`/`Down`/`Left`/`Right` unmodified |
| `Shift+↑/↓` | [x] `SHIFT` on Up/Down |
| `Shift+←/→` / `F6` | [x] `SHIFT` on Left/Right; `F(6)` plain |
| `Backspace` | [x] |
| `Alt+←/→` | [x] `ALT` on Left/Right (synthetic only — not what Ghostty sends) |
| `Ctrl+↑/↓` | [x] `CONTROL` on Up/Down (block-jump fallback) |
| `Alt+Shift+↑/↓` | [x] `SHIFT \| ALT` |
| `Alt+[` / `Alt+]` | [ ] Arrive as plain `Char(']')` via `send-keys` (unreliable as fallback) |
| Click / wheel | [ ] not tried in synthetic pass |
| `Ctrl+Enter` | [x] Confirmed unreliable for binding — keep unbound |
