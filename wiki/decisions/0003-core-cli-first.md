---
id: WR-ADR-0003
title: "ADR-0003: Core library + CLI first, TUI second"
summary: The index and context engine are a terminal-free library with a JSON CLI; the TUI is one client.
status: superseded
updated: 2026-09-28
related: []
---

# ADR-0003: Core library + CLI first, TUI second

**Status:** Superseded by [ADR-0006](0006-reader-first.md) · **Date:** 2026-09-27

## Context

The context engine is the riskiest, least-proven part. It must also serve agents, not just the TUI.

## Decision

`wiki-reader-core` has no terminal dependencies. The CLI (`index`, `context`, `backlinks`, `links`, `show`, all with `--json`) ships before the TUI. The TUI consumes the same `ContextSnapshot` type.

## Consequences

- ➕ Context quality can be tested with snapshot tests and spot checks before any UI exists.
- ➕ Agents get the same answers as the human.
- ➕ An external index provider or MCP server can be added without touching the UI.
- ➖ Slight up-front structure cost for a POC.
