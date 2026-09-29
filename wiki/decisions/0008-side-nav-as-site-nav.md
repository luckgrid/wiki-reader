---
id: WR-ADR-0008
title: ADR-0008: Side nav as site navigation
summary: The side nav is a curated site navigation derived from the file tree — titles not filenames, root entry first, README-only folders folded to links.
status: accepted
updated: 2026-09-28
related: []
---

# ADR-0008: Side nav as site navigation

**Status:** Accepted · **Date:** 2026-09-28

## Context
A raw file tree exposes filenames and `README.md` noise (every folder shows the same filename). Documentation sites instead show titles and treat a folder's README as its landing page.

## Decision
Build a `NavTree` from the filesystem with the rules in [content model](../product/content-model.md):
- Labels are document titles (`nav_title` → `title` → H1 → humanized filename). Filename display is a later config option (`title`, `filename`, or `title+filename` as alt text).
- The root README is the first item and supplies the header's root breadcrumb.
- Folders fold recursively: README-only → leaf link; README + more → group whose first item is the README landing page; no README → group labeled by folder name; empty → hidden.
- Order comes from `SUMMARY.md`/`_sidebar.md`, then `nav_order`, then natural sort.
- Breadcrumbs and prev/next are derived from the same `NavTree`, so all three stay consistent.

## Consequences
- ➕ Reads like a docs site; less noise; consistent breadcrumbs.
- ➕ Built in `core`, so a v2 CLI and widgets see the same structure.
- ➖ The tree no longer mirrors the filesystem 1:1. The status bar always shows the real path, and `y` copies it.
- ➖ Title extraction means reading every file's head at index time (cheap; cached by mtime).
