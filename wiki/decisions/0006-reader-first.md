---
id: WR-ADR-0006
title: ADR-0006: Reader first; core stays terminal-free
summary: Build the wiki reader first; defer the context engine and agent CLI, but keep the core terminal-free.
status: accepted
updated: 2026-09-28
related: []
---

# ADR-0006: Reader first; core stays terminal-free

**Status:** Accepted · **Date:** 2026-09-28 · **Supersedes:** [ADR-0003](0003-core-cli-first.md)

## Context

[ADR-0003](0003-core-cli-first.md) put the context engine and a JSON CLI first. Phase 0 showed that the most urgent, best-understood value is the **reading and navigation experience**. What the context sidebar should do is still unclear, and it may become a general widget/plugin sidebar.

## Decision

- v1 ships the reader: tree, reader, header, footer, sidebar search/outline, working links, and history.
- The context engine and `--json` CLI move to v2, alongside a right-hand widget slot.
- `wiki-reader-core` **still has no terminal dependencies**. Index, link resolution, nav order, and search live there, so v2 widgets, a CLI, or a uwiki provider can be added without touching the UI.

## Consequences

- ➕ The first build targets the pain actually felt.
- ➕ No speculative design locked in for the sidebar.
- ➖ Context quality isn't validated early; accepted, since it's no longer v1 scope.
