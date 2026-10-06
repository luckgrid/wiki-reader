---
id: WR-ADR-0011
title: "ADR-0011: Renderer source (Phase 1)"
summary: In-tree pulldown-cmark renderer; no markdown-tui-explorer library; sync watch without tokio.
status: accepted
updated: 2026-09-29
related: [0002-build-vs-fork]
---

# ADR-0011: Renderer source (Phase 1)

## Context

P1-13 needs a markdown → terminal layout pipeline. ADR-0002 asked whether **markdown-tui-explorer** (crates.io 1.34.75) exposes a library API.

## Inspection (2026-09-29)

- crates.io / `cargo search`: package exists at **1.34.75** (“terminal-based markdown file browser…”).
- docs.rs for 1.34.75 (and prior 1.34.x): page title states **“markdown-tui-explorer-… is not a library”** — no embeddable `[lib]` API for a `RenderedDoc`-style layout.
- Therefore we do **not** depend on it as a Cargo library. Porting selected MIT sources with attribution (`THIRD_PARTY.md`) remains the ADR-0002 escape hatch if the in-tree renderer stalls past ~3 days.

## Decision

1. **markdown-tui-explorer** is binary-oriented; we do **not** depend on it as a library.
2. Phase 1 ships an in-tree renderer in `wiki-reader-render`: **pulldown-cmark** (tables, task lists, strikethrough) → `StyledLine` / `StyleKind` + `LinkSpan` geometry + real `source_map`. Syntect for fenced code lands with P1-11.
3. **No async runtime** for P1-12 watch: `notify` + debounce on an `mpsc` channel polled from the TUI loop (no tokio in Phase 1).

## Consequences

- Ported markdown-reader code may be added later with `THIRD_PARTY.md` attribution if the in-tree renderer stalls.
- `wiki-reader-core` stays free of `ratatui` / `crossterm` (ADR-0006).
