---
id: WR-INTEGRATIONS
title: Integration plan
summary: When and how to integrate uwiki, the Luckgrid design system, and herdr, and what to keep open now.
status: draft
updated: 2026-09-29
related: [overview, context-engine]
nav_order: 5
---

# Integration plan

The rule: **nothing here blocks the POC.** Each integration has a seam built in now and an adoption test to pass later.

## uwiki

**Seam now:** `CollectionProvider` trait ([architecture](overview.md)). All content access goes through it; the index never touches `std::fs` directly.

**Possible roles for uwiki, cheapest first:**

1. *Content only*: uwiki is just a collection wiki-reader reads. Zero integration work; works in the POC.
2. *Conventions*: wiki-reader adopts uwiki's frontmatter keys, ID grammar, and link style via config. Config-only.
3. *Index provider*: if uwiki builds an index (links, IDs, metadata), implement `UwikiProvider` and skip wiki-reader's own parsing for those collections.
4. *Library dependency*: depend on uwiki crates directly (only if it's Rust and the API is stable).

**Adopt at level 3–4 only if:** it removes code from wiki-reader, its interface is versioned, and wiki-reader still works on non-uwiki collections.

## Luckgrid design system

The design system is CSS/web-first. What transfers to a terminal is **semantic tokens**, not components.

**Seam now:** semantic theme tokens ([UI spec](../product/ui-spec.md)), loaded from a theme TOML.

**Later:** a small generator (in either repo) that projects design-system color/emphasis tokens into a wiki-reader theme TOML, quantized for 256-color and truecolor. Shared *primitives* (surface, action, focus states) map to border/emphasis rules conceptually, not by code sharing.

**Adopt when:** the MVP theme has stabilized and you want visual consistency across Luckgrid tools.

## herdr

herdr is the host environment, so it's the integration most likely to pay off early.

| Level | What | When |
|-------|------|------|
| Env awareness | Detect `HERDR_ENV`, choose Kitty-only graphics or the text tier | Phase 2 |
| Context signals (v2, widget sidebar) | `herdr pane list --workspace $HERDR_WORKSPACE_ID` → sibling cwds and agent states | Phase 3 |
| Publish state | `herdr pane report-metadata --token page=… --token wu=…` so herdr's sidebar shows what the wiki pane is on | Alpha |
| Plugin | A herdr plugin manifest with a pane entrypoint that opens wiki-reader as a split or popup for the current workspace | Alpha |

### Pane setup (today)

Install the binary ([Install](../../README.md#install)), then launch it in a herdr pane against a collection root:

```bash
wiki-reader ~/Workspaces/workstation
```

Ghostty (common outer terminal under herdr) encodes Option+← / Option+→ as readline `Alt+b` / `Alt+f`, not as arrows with ALT. wiki-reader binds those for Back/Forward ([P1-S1](../roadmap/spikes/p1-s1-herdr-input.md), P1-R36). Prefer `Backspace` for Back when Option is not Alt.

Check the herdr plugin manifest format and socket API against the installed version's docs when you get there. They are versioned and still moving.

## Agents (v2)

Deferred with the widget sidebar. `wiki-reader context --json` and `wiki-reader show <page> --section <heading>` are the agent interface. An MCP wrapper is an easy later add-on and doesn't change the core.
