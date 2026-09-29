---
id: WR-ADR-0001
title: ADR-0001: Rust + ratatui
summary: Build wiki-reader in Rust with ratatui.
status: accepted
updated: 2026-09-28
related: []
---

# ADR-0001: Rust + ratatui

**Status:** Accepted · **Date:** 2026-09-27

## Context
We need a fast, single-binary TUI that runs well inside herdr panes, renders markdown and diagrams, and shares a core library with a CLI. The surrounding toolchain (luna, herdr) is Rust.

## Options

| | Rust + ratatui | Go + Bubble Tea + Glamour | Python + Textual |
|--|--|--|--|
| Markdown rendering | Port from markdown-reader | Glamour for free | Built-in Markdown widget |
| Mermaid | Pure-Rust text + image stack exists | Weak | Weak |
| Distribution | Single static binary | Single binary | Needs a Python env |
| Fit with toolchain | Same as herdr/luna | New language | New language |
| Layout control | Immediate-mode, explicit | Elm-style | CSS-like, rich |

## Decision
Rust + ratatui 0.30 + crossterm.

## Consequences
- ➕ The pure-Rust Mermaid pipeline and markdown-reader's code are directly reusable.
- ➕ Same stack as herdr, so fewer surprises inside it.
- ➖ Rendering isn't free the way Glamour is; mitigated by porting ([ADR-0002](0002-build-vs-fork.md)).
- ➖ Immediate-mode UI means we own scroll/focus state.
