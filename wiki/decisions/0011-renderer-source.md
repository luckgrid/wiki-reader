---
id: WR-ADR-0011
title: ADR-0011 — Renderer source (Phase 1)
summary: In-tree pulldown-cmark renderer; no markdown-tui-explorer library; sync watch without tokio.
status: accepted
updated: 2026-09-29
related: [0002-build-vs-fork]
---

# ADR-0011 — Renderer source

## Context

P1-13 needs a markdown → terminal layout pipeline. ADR-0002 asked whether **markdown-tui-explorer** (crates.io 1.34.75) exposes a library API.

## Decision

1. **markdown-tui-explorer** is a binary-oriented crate; it does not expose a stable render library API suitable for embedding. We do **not** depend on it as a library.
2. Phase 1 ships an in-tree renderer in `wiki-reader-render`, starting from **pulldown-cmark** → wrapped plain lines + `LinkSpan` geometry (escape hatch from the phase roadmap). Features (tables, syntect, alerts) land incrementally after navigation works.
3. **No async runtime** for P1-12 watch: `notify` + debounce on an `mpsc` channel polled from the TUI loop (documented here; no tokio in Phase 1).

## Consequences

- Ported markdown-reader code may be added later with `THIRD_PARTY.md` attribution if the in-tree renderer stalls.
- `wiki-reader-core` stays free of `ratatui` / `crossterm` (ADR-0006).
