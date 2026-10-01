---
id: WR-ADR-0014
title: "ADR-0014: Rendered is the formatted view; remove the eye toggle"
summary: The eye toggle and the syntax rendered presentation are removed. Rendered always drops markdown markers; r (raw) is where the syntax lives. Supersedes ADR-0012.
status: accepted
updated: 2026-09-30
related: [0012-syntax-vs-formatted, 0011-renderer-source]
---

# ADR-0014: Rendered is the formatted view; remove the eye toggle

**Status:** Accepted · **Date:** 2026-09-30 · **Supersedes:** [ADR-0012](0012-syntax-vs-formatted.md)

## Context

[ADR-0012](0012-syntax-vs-formatted.md) added an eye control (`○`/`◉`, key `v`) that switched Rendered between **syntax** (markers visible, the default) and **formatted** (markers dropped). Dogfooding showed the toggle earns nothing next to `r`: `r` already shows the true markdown source, so a second "show the syntax" presentation inside Rendered is redundant, and the extra header icon, key, Help row, session field and layout branches all had to be kept in step.

## Decision

- Remove the eye icon, the `v` key, the `toggle_formatted_view` action name, the Help row, and the session's `formatted_view` field (old session files that still carry it load fine).
- **Rendered is always the formatted view.** Markers are dropped at layout time: no `#` on headings (H1/H2 get an underline rule, H3–H6 extra spacing), no code fences (a `── lang ──` label instead), no backticks on inline code. `RenderOpts.formatted` goes away because there is only one behaviour.
- **Raw (`r`) is where the markdown syntax is shown**, soft-wrapped to the pane.
- "Linked from" follows the same style: a title, a dim rule, then the list directly under it.
- P3-11 no longer owns a formatted-view config key.

## Consequences

- ➕ One rendered presentation and one switch (`r`): less state, fewer snapshots, a header with two icons (`◫`, `✕`).
- ➕ Link, block and source-map geometry no longer depends on a mode.
- ➖ Heading level without `#` relies on colour and spacing; code blocks rely on a label instead of fences.
- ➖ There is no rendered view that keeps markers. Anyone who wants them uses `r`.
