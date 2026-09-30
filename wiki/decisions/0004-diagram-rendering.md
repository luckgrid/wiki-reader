---
id: WR-ADR-0004
title: ADR-0004: Tiered diagram rendering
summary: Render Mermaid in tiers — Kitty image, Unicode text, then source — with herdr-aware detection.
status: proposed
updated: 2026-09-28
related: []
---

# ADR-0004: Tiered diagram rendering

**Status:** Proposed (confirm after the phase-2 spike) · **Date:** 2026-09-27

## Context

Diagrams are a key reason to leave the terminal today. Terminal image support varies, and herdr forwards only the Kitty graphics protocol. Sixel and iTerm2 images are dropped inside herdr panes.

## Decision

Per block: **image** (mermaid-rs-renderer → resvg → ratatui-image, Kitty protocol under herdr) → **text** (`mermaid-text`) → **source** with the reason shown. Detection:

1. Config `diagrams` override wins.
2. `$TMUX` set → text.
3. `HERDR_ENV=1` → Kitty if the outer terminal supports it (env hints / probe), else text. Never Sixel.
4. Otherwise → ratatui-image auto-detect.

## Consequences

- ➕ Readable diagrams everywhere; best quality where possible.
- ➖ Two rendering paths to test.
- Spike: verify ratatui-image detection inside herdr, and image placement and clearing while scrolling through a herdr pane.
