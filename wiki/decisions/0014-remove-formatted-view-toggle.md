---
id: WR-ADR-0014
title: "ADR-0014: Remove the syntax/formatted view toggle"
summary: The eye toggle and the formatted view are removed; Rendered keeps markers visible and r stays the only view switch. Supersedes ADR-0012.
status: accepted
updated: 2026-09-30
related: [0012-syntax-vs-formatted, 0011-renderer-source]
---

# ADR-0014: Remove the syntax/formatted view toggle

**Status:** Accepted · **Date:** 2026-09-30 · **Supersedes:** [ADR-0012](0012-syntax-vs-formatted.md)

## Context

[ADR-0012](0012-syntax-vs-formatted.md) added an eye control (`○`/`◉`, key `v`) that switched Rendered between **syntax** (markers visible, the default) and **formatted** (markers dropped). Dogfooding showed it earns nothing next to `r`: it only changes how headings and a few inline markers look, `r` already gives the true source, and the extra header icon, key, Help row, session field and layout branches all had to be kept in step.

## Decision

- Remove the eye icon, the `v` key, the `toggle_formatted_view` action name, the Help row, and the session's `formatted_view` field (old session files that still carry it load fine).
- Remove `RenderOpts.formatted` and its branches from the renderer. Rendered mode always keeps markdown markers visible, exactly the former *syntax* presentation.
- `r` (Raw ↔ Rendered) is the only view switch.
- P3-11 no longer owns a formatted-view config key.

## Consequences

- ➕ One rendered presentation: less state, fewer snapshots, a header with two icons (`◫`, `✕`).
- ➕ Block, link and source-map geometry no longer depends on a mode.
- ➖ No marker-free reading mode. A later reading mode would be a new decision with its own design.
