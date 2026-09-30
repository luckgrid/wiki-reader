---
id: WR-ADR-0002
title: "ADR-0002: Build on existing pieces, don't fork the app"
summary: Build a new app shell and core; port rendering from markdown-reader instead of forking the whole app.
status: accepted
updated: 2026-09-28
related: []
---

# ADR-0002: Build on existing pieces, don't fork the app

**Status:** Accepted · **Date:** 2026-09-27

## Context

markdown-reader (MIT, Rust/ratatui) already provides a tree, rendered markdown, Mermaid, tables, and live reload. It covers most of the center pane. wiki-reader's value is the index, the context engine, and a three-column layout, none of which markdown-reader has.

## Options

**A. Fork markdown-reader.** Fastest to a first screen. We'd inherit a two-pane layout, tabs, a hybrid editor, and settings we don't need, and fight them to add two sidebars and an index. Syncing with upstream is ongoing work.

**B. New shell + core; depend on its published crates; port its render module.** Use `mermaid-text` as a dependency. Port rendering (wrapping, tables, code, frontmatter box) into `wiki-reader-render` with attribution. Everything else is ours.

**C. Wrap it.** Run markdown-reader in a pane and build only the sidebar as a separate process. Great for validation ([roadmap](../roadmap/README.md) phase 0), but two processes can't share selection or navigation.

## Decision

**C for phase 0, B for the product.** A is kept as an escape hatch if the render port overruns ~3 days (roadmap phase 2).

## Consequences

- ➕ The architecture fits the actual product (core + CLI + TUI).
- ➕ We only own rendering code we chose to take.
- ➖ The port costs a few days up front.
- Follow-up: check whether `markdown-tui-explorer` exposes a library API. If it does, depend on it instead of porting.
