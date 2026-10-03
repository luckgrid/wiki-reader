---
id: WR-ADR-0021
title: "ADR-0021: Nav label mode applies only to the side nav"
summary: Filename mode changes the file tree, not header breadcrumbs or view footer labels.
status: accepted
updated: 2026-10-03
related: [0020-nav-label-modes]
---

# ADR-0021: Nav label mode applies only to the side nav

**Status:** Accepted · **Date:** 2026-10-03 · **Supersedes:** the mode-dependent breadcrumb/prev-next label rule in [ADR-0020](0020-nav-label-modes.md). Other label rules and config migration remain unchanged.

## Context

The operator clarified P3-16: filename mode is for the side-nav file tree only. Applying it to header breadcrumbs and the View footer replaces useful page titles with repeated README filenames.

## Decision

- `nav.labels` changes only side-nav page labels. Filesystem folder names remain literal in both modes.
- Header breadcrumbs and View footer links always use title-mode navigation labels: page title cascade, landing/root title fallbacks and curated SUMMARY/sidebar labels. Ancestor groups retain the title-mode hierarchy's group labels.
- Page order, navigation targets and breadcrumb hierarchy remain shared; selecting filename mode changes no destinations or ordering.
- The navigator caches the title-labelled tree separately from the side-nav tree and refreshes both on reindex. Switching labels rebuilds only the side-nav tree, not the header/footer labels; no tree is rebuilt on every frame.
- Regression tests cover switching both ways, initial filename config, stable chrome/navigation ordering and refreshed titles after reindex.

## Consequences

- Readers can inspect filenames in the side nav without losing title-based orientation elsewhere.
- A second cached label projection uses modest extra memory but avoids rebuilding navigation on each draw.
- Accepted ADR-0020 remains immutable; its broader label scope is superseded here.
