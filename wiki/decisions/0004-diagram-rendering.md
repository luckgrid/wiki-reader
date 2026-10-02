---
id: WR-ADR-0004
title: "ADR-0004: Tiered diagram rendering"
summary: Render Mermaid in tiers — Kitty image, Unicode text, then source — with herdr-aware detection.
status: accepted
updated: 2026-10-02
related: []
---

# ADR-0004: Tiered diagram rendering

**Status:** Accepted · **Date:** 2026-09-27 · **Accepted:** 2026-10-02

## Context

Diagrams are a key reason to leave the terminal today. Terminal image support varies, and herdr forwards only the Kitty graphics protocol. Sixel and iTerm2 images are dropped inside herdr panes.

## Decision

Per block: **image** (mermaid-rs-renderer → resvg → ratatui-image, Kitty protocol under herdr) → **text** (`mermaid-text`) → **source** with the reason shown. Detection:

1. Read the config request. `text` and `source` do not probe the terminal.
2. `$TMUX` set → text, including fallback from an explicit `image` request.
3. `HERDR_ENV=1` → run the Picker query at startup. Only a confirmed Kitty result enables images; every other result uses text. Never Sixel or iTerm2.
4. Outside Herdr, iTerm2 is selected explicitly from `TERM_PROGRAM=iTerm.app`; its tested capability response incorrectly preferred Kitty, which iTerm2 did not render.
5. Otherwise, probe only when environment hints identify a known graphics-terminal candidate. Kitty and iTerm2 are supported; Sixel is experimental for the alpha; Halfblocks selects the text tier because its low-fidelity raster is worse for diagrams. Terminal.app and unknown terminals select text without probing.

The query runs after `ratatui::try_init` enters the alternate screen and before mouse capture, keyboard-enhancement queries and the first event read. Its default timeout is 250 ms and `WIKI_READER_IMAGE_QUERY_TIMEOUT_MS` is the diagnostic override; users can avoid the probe entirely with `diagrams = "text"` or `"source"`. Image setup failure degrades to text, then source.

## Consequences

- ➕ Readable diagrams everywhere; best quality where possible.
- ➖ Two rendering paths to test.
- `ratatui-image` 11.1.0 query timeouts can leave a worker blocked on stdin. A no-response pseudo-terminal hung in the following keyboard query at 50, 250 and 500 ms, so unknown environments are not probed and the probe is never retried in-process.
- `StatefulImage` with top `CropOptions` correctly clips a partially visible slot. Render the image first, then `Clear`, then popup content; this sequence and protocol replacement showed no stale graphics in Ghostty or Herdr.
- The terminal-image dependencies added about 50 locked packages in spike part 1. Mermaid plus `resvg` adds another 39. Production release binary after P3-12c: **+~6.7 MiB** (measured 2026-10-02 vs P3-12b: 9.7 MiB → 16.4 MiB).
- [P3-S1](../roadmap/spikes/p3-s1-image-protocol.md): real images are verified in Ghostty direct and through Herdr 0.9.0, forced iTerm2 works, and Terminal.app correctly maps to text. Mermaid rendering is a mixed go: compact diagrams are good and wide diagrams need per-block text fallback (legibility gate ≈ 0.55 scale).
