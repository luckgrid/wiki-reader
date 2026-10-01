---
id: WR-ADR-0013
title: "ADR-0013: Nav shows folder names and page titles, no Overview"
summary: Folders always read as the folder name, pages as their title (the new default), every README folder is a group; supersedes the landing-row "Overview" label in ADR-0010.
status: accepted
updated: 2026-09-30
related: [0008-side-nav-as-site-nav, 0010-flat-side-nav-rows]
---

# ADR-0013: Nav shows folder names and page titles, no Overview

**Status:** Accepted · **Date:** 2026-09-30 · **Supersedes:** the "Landing row label (P2-12)" section of [ADR-0010](0010-flat-side-nav-rows.md)

## Context

Dogfooding showed `Readme › Readme › Readme › Overview` in the breadcrumb and `Readme` / `Overview` rows in the nav. Filenames are the wrong label for index pages, and the `Overview` placeholder hid the page's real title. A folder whose only page was a README was folded into a single leaf, so the tree shape depended on how much a folder held.

## Decision

- **Folder rows** always show the folder name.
- **Page rows** show the page title (`nav_title` → `title` → first H1 → humanized filename). `title` becomes the default `nav.labels`; `filename` and `title+filename` stay available.
- A folder's **README / index** row and the **root README** always show their title, whatever the mode. The root falls back to the collection name, never "Readme".
- **Every folder with a README is a group** whose first child is that README, including a folder holding only a README. There is no `Overview` label.
- The breadcrumb follows the same labels. A README's crumb stands for its folder, so the folder is not repeated.

## Consequences

- ➕ Labels are truthful and the tree shape no longer depends on folder size.
- ➕ Prev/next and breadcrumbs read as titles.
- ➖ A group and its landing row can read alike when the README title matches the folder name.
- ➖ A README-only folder costs one extra row (group + page).
