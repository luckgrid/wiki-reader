---
id: WR-ADR-0004
title: "ADR-0004: Tiered diagram rendering"
summary: Render Mermaid in tiers — Kitty image, Unicode text, then source — with herdr-aware detection.
status: proposed
updated: 2026-10-02
related: []
---

# ADR-0004: Tiered diagram rendering

**Status:** Proposed (accept after the remaining P3-12a terminal checks) · **Date:** 2026-09-27

## Context

Diagrams are a key reason to leave the terminal today. Terminal image support varies, and herdr forwards only the Kitty graphics protocol. Sixel and iTerm2 images are dropped inside herdr panes.

## Decision

Per block: **image** (mermaid-rs-renderer → resvg → ratatui-image, Kitty protocol under herdr) → **text** (`mermaid-text`) → **source** with the reason shown. Detection:

1. Read the config request. `text` and `source` do not probe the terminal.
2. `$TMUX` set → text, including fallback from an explicit `image` request.
3. `HERDR_ENV=1` → run the Picker query at startup. Only a confirmed Kitty result enables images; every other result uses text. Never Sixel or iTerm2.
4. Otherwise → ratatui-image auto-detect.

The query runs after `ratatui::try_init` enters the alternate screen and before mouse capture, keyboard-enhancement queries and the first event read. Image setup failure degrades to text, then source.

## Consequences

- ➕ Readable diagrams everywhere; best quality where possible.
- ➖ Two rendering paths to test.
- `ratatui-image` 11.1.0 query timeouts can leave a worker blocked on stdin, so known-unsupported environments are not probed and the probe is never retried in-process.
- [P3-S1](../roadmap/spikes/p3-s1-image-protocol.md): Kitty detection and a real PNG are verified in Ghostty through herdr 0.9.0; direct Ghostty and a non-Kitty terminal remain before acceptance.
