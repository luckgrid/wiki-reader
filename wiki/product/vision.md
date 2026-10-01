---
id: WR-VISION
title: Vision
summary: Why wiki-reader exists, who it is for, and the principles that constrain it.
status: draft
updated: 2026-09-30
related: [spec, content-model, ui-spec]
nav_order: 1
---

# Vision

Why wiki-reader exists, who it is for, and the principles that constrain it.

## Problem

Knowledge that steers agent-heavy work lives in markdown collections: architecture notes, decisions, workstream records, specs. Terminal tools can display those files, but they don't let you **browse** them. In practice (see Phase 0 findings in [prior art](../architecture/prior-art-and-libs.md)):

- Opening a page spawns or keeps tabs, like a code editor, instead of replacing the view like a wiki.
- Links inside documents can't be clicked or followed, so a wiki's link structure is unusable.
- Search opens results in new tabs through a picker, instead of living next to the page tree.
- Nothing gives you site-style orientation: breadcrumbs, prev/next, "linked from".

So reading a wiki still means leaving the terminal.

## Product statement

A terminal pane that browses a markdown collection the way a good documentation site does, with a page tree on the left, a reader in the center, and navigation that just works with keyboard and mouse.

## Who it's for

A developer or architect working in herdr panes, with markdown collections spread across repos.

## Principles

1. **Wiki, not editor.** Opening something replaces the view and records history. Tabs are opt-in, as in a browser. Editing is `$EDITOR`'s job.
2. **One navigation path.** Tree clicks, search results, links, breadcrumbs, prev/next, and back/forward all go through one `navigate()` call, so they can't behave differently.
3. **Mouse and keyboard are equals.** Anything clickable is reachable by keys, and anything with a key binding has a clickable affordance where it makes sense (search icon, back/forward, prev/next, breadcrumbs, links).
4. **Links are first-class.** Every link is focusable, followable, and shows its target before you follow it.
5. **Build on what exists.** Port proven rendering, use proven crates, and write new code only for the navigation experience.
6. **Plain files are the source of truth.** No database. Wiki conventions (SUMMARY.md, index pages, frontmatter) are read, never required.
7. **Degrade, never break.** No graphics → text diagrams → source. Broken link → clear message, not a dead key.
8. **Pane-sized.** Usable at 80 columns in a split.

## Non-goals for v1

| Non-goal | Why |
|----------|-----|
| In-app editing | markdown-reader already does this well; we're a reader. |
| Context/widget right sidebar | The requirements aren't clear yet. Designed as a Phase 4 slot ([context engine](../architecture/context-engine.md), [integrations](../architecture/integrations.md)). |
| Agent CLI / JSON output | Follows the widget sidebar in Phase 4. The core stays terminal-free so it's easy to add. |
| Publishing / site export | mdBook, Zola, and similar tools own this. |
| External providers / theme tokens | Seams now, decisions after MVP. |

## What success looks like

- You browse your collections end-to-end (tree → page → link → back → search → next) without opening a tab unless you want one.
- You stop reaching for a GUI markdown app to read these collections.
