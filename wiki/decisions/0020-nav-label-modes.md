---
id: WR-ADR-0020
title: "ADR-0020: Titles or actual filesystem names"
summary: Two nav label modes, literal folder names and explicit migration from title+filename.
status: accepted
updated: 2026-10-05
related: [0008-side-nav-as-site-nav, 0013-nav-labels-folder-names-and-titles, 0021-side-nav-only-label-mode]
---

# ADR-0020: Titles or actual filesystem names

**Status:** Accepted · **Date:** 2026-10-03 · **Supersedes:** nav-label choices in [ADR-0008](0008-side-nav-as-site-nav.md) and the page/landing/folder label rules in [ADR-0013](0013-nav-labels-folder-names-and-titles.md). Tree grouping and ordering are unchanged; accepted ADRs remain immutable. · **Chrome label scope superseded by:** [ADR-0021](0021-side-nav-only-label-mode.md)

> Note: the mode-dependent breadcrumb/prev-next label rule here is superseded by [ADR-0021](0021-side-nav-only-label-mode.md); other label rules and config migration remain.

## Context

P3-16 dogfood requests two clear choices: readable page titles or truthful filesystem names. Humanized names hide extensions and numeric prefixes, and the third title-plus-filename mode adds visual noise. Filename mode must also show the real name of a landing page rather than force its title.

## Decision

- `nav.labels` supports `title` (default) and `filename` only.
- `title` uses `nav_title` → `title` → first H1 → humanized filename. Landing pages retain title mode's folder-name fallback; the root landing page falls back to the collection name.
- `filename` uses the on-disk filename, preserving case, numeric prefixes, separators and extensions, including `README.md` / `index.md` landing pages.
- Filesystem folder rows preserve their on-disk names in both modes, without humanization.
- Curated SUMMARY/sidebar link labels remain in title mode; filename mode uses page filenames. Synthetic group/part headings retain curated labels because they do not represent filesystem folders.
- Breadcrumbs and prev/next use the same mode-dependent tree labels.
- Existing `title+filename` values load as `title`, with one deprecation diagnostic per config load across all layers. Loading does not rewrite files. An Options label change writes only a supported value; setting one explicitly removes the warning on subsequent loads.

## Consequences

- Filename mode faithfully exposes names; title mode remains the default readable navigation.
- Landing filenames repeat in filename mode by design; their folder hierarchy distinguishes them.
- Old configs continue to load, but the dim filename suffix and its Options choice disappear.
