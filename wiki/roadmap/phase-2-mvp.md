---
id: WR-ROADMAP-P2
title: Phase 2 — Wiki navigation MVP
summary: Custom nav order, backlinks, tabs, diagrams, config, and session restore.
status: active
updated: 2026-09-30
related: [phase-1-reader-shell, phase-3-alpha]
nav_order: 2
---

# Phase 2 — Wiki navigation MVP

Time box: ≈ 2 weeks.

## Exit criteria

All P0/P1 acceptance criteria in [product spec](../product/spec.md) pass; two weeks without opening a GUI markdown app for these collections.

The dogfood polish batch (P2-11…P2-24a) should land before the clock is judged, but the two weeks do **not** restart or extend. Clock started 2026-09-29; exit remains ≈ 2026-10-13. P2-24b (image diagrams) may slip to Phase 3 without blocking exit.

## Dogfood notes (operator)

Phase 2 stays `active` through two weeks of real use on a real collection. The two-week clock starts when you say so. Record anything that bites here (date + one line); feed that into Phase 3 scoping.

- 2026-09-29: clock started on a real collection
- 2026-09-29: v0.1.0-alpha.1 released (macOS arm64/x86_64, Linux x86_64); dogfood from the installed binary
- 2026-09-30: side-by-side review vs markdown-reader; UI/UX polish batch filed as P2-11…P2-22
- 2026-09-30: markdown element rendering (Mermaid text tier garbled, truncated tables, mid-word wrapping, raw syntax markers) filed as P2-23; formatted-text view and eye toggle as P2-24a; image diagrams as P2-24b (may slip to Phase 3)

## Tasks

| ID | Task | Reqs | Status | Notes |
|----|------|------|--------|-------|
| P2-01 | Custom nav order via `SUMMARY.md` / `nav_order` | P1 | done | shipped in P1-R3 |
| P2-02 | "Linked from" backlinks | B1 | done | 47185bc |
| P2-03 | Block actions in the Tab cycle | BA | done | d8c63e2 |
| P2-04 | Heading jump | J1 | done | 19faa9e |
| P2-05 | Tabs as secondary | TB | done | 81ea91f |
| P2-06 | Mermaid tiers | D1 | done | 81ea91f text tier; image deferred |
| P2-07 | Responsive side nav | R1 | done | shipped in P1-R6 |
| P2-08 | `$EDITOR` | E1 | done | d130b92 |
| P2-09 | Config incl. `nav.labels` | C1 | done | 19db10d |
| P2-10 | Session restore | M1 | done | 36e00c2 |
| P2-R31 | Config trust merge (exclude union, keys merge) | C1 | done | 81ea91f |
| P2-R32 | Session autosave + test isolation | M1 | done | 81ea91f |
| P2-R33 | Block/copy polish | BA | done | 81ea91f |
| P2-R34 | `t` / middle-click open the right target | TB | done | focused link / nav row; all navigable hits |
| P2-R35 | Theme warn, dim labels, Mermaid cache, highlight flake | C1 / D1 | done | N2–N4 cleanups |
| P2-11 | Nav selected-row style | U3 | done | 09062f7 |
| P2-12 | Nav: no duplicate folder/landing row | U3 | done | 12728d1 |
| P2-13 | Nav labels: default back to filenames | C1 / U3 | done | 2cde12d |
| P2-14 | Resizable nav pane | R1 | done | 587beaa |
| P2-15 | Frontmatter as YAML props | — | subsumed → P2-23 | properties block in the viewer; stay collapsible (`ToggleFrontmatter`) |
| P2-16 | Viewer typography and spacing | — | subsumed → P2-23 | spacing between blocks; heading styling; snapshot updates |
| P2-17 | Chrome spacing | — | done | 91f7c32 |
| P2-18 | Viewer footer prev/next | — | done | cb8dbf3 |
| P2-19 | Viewer cursor line and tab visibility | — | done | 6d7448d |
| P2-20 | Help overlay (`?`) | — | done | #71; binding table + clickable help; done-via P3-06 |
| P2-21 | Search overlay layout and scroll | — | done | #72; larger pane, scroll + jumps, Files\|Content rename, styled results |
| P2-22 | Keyboard flow: nav to viewer to footer | — | done | #70; `→` opens page + focuses viewer; `f` focuses footer; sticky footer focus; ADR-0007 |
| P2-23 | Markdown syntax view: element styling (priority) | D1 | done | 09062f7 |
| P2-24a | Formatted-text view and eye toggle | D1 / C1 | done | ADR-0012; eye toggles syntax↔formatted; `r` stays Raw↔Rendered |
| P2-24b | Image diagrams and images | D1 | todo | after P2-24a; see [P2-24b scope](#p2-24b-scope). May slip to Phase 3 without blocking exit |

## P2-23 scope

Priority. The dogfood review showed the viewer is hard to read even as a markdown *source* view. This task makes the syntax view (markers stay visible) render every element neatly, using markdown-reader as the reference. The formatted-text view and the fancier diagram work wait for [P2-24a](#p2-24a-scope) / [P2-24b](#p2-24b-scope).

- **Text flow:** word-wrap at the pane width, never mid-word (the screenshots break words like `tu rning`). Consistent spacing between blocks.
- **Headings:** distinct H1–H6 styling, like markdown-reader's coloured heading bars.
- **Frontmatter:** a YAML properties block (keys and values aligned, lists rendered as lists), collapsible. This is P2-15.
- **Code blocks:** bordered or shaded block with a language label, syntect highlighting, wrap or horizontal scroll without breaking the frame, and a copy action.
- **Tables:** column widths fitted to the pane, cells wrapped instead of truncated with `…`, header row styled, rows separated. Fall back to an unwrapped dump with a note when minimum column widths do not fit. The screenshots showed cut-off cells and an "expand table" stub.
- **Lists and task lists:** bullets, numbering and nesting indent, checkboxes.
- **Links:** distinct link style, visible focus and hover state, broken-link marking, footnote references.
- **Blockquotes and alerts:** a left bar and tint for quotes; typed styling for note, warning and so on.
- **Inline:** bold, italic, strikethrough and inline code styled.
- **Diagrams (text tier):** the current Mermaid text output is garbled beyond a simple flow. The screenshots show overlapping boxes, labels split across lines, and output wider than the pane. Fix the text tier so it never overflows or overlaps. When a diagram can't fit, fall back to the fenced source with the reason shown.
- **Rules:** a full-width line.

Acceptance:

- Side-by-side with markdown-reader on the design-system collection pages shown in the review: every element above renders at least as readably.
- The `shell_{60,80,120}` snapshots and per-element snapshots cover each element.
- No regression in cursor, line selection, link following or block actions.

## P2-24a scope

After P2-23. Browser-style reading without syntax markers.

- **Formatted-text view:** render markdown as formatted text, with no visible syntax markers (`#`, `**`, backticks, fence lines, link brackets and URLs). Styling carries the meaning instead. Feasible in a terminal for everything except font size: headings differ by colour, weight, rules and spacing. The `source_map` must keep the cursor, line selection, link hits and block actions working on the formatted output.
- **Toggle:** an eye icon at the top right of the topbar switches between the syntax view and the formatted view. The same choice is a config option under P3-11. Decide the default, and record it in an ADR.

Acceptance: the eye toggle switches views without losing the cursor position.

## P2-24b scope

After P2-24a. Image diagrams and images; may slip to Phase 3 without blocking Phase 2 exit.

- **Diagrams like jcode:** the D1 image tier: mermaid-rs-renderer → resvg → Kitty, following jcode's pipeline, per [ADR-0004](../decisions/0004-diagram-rendering.md). Include scroll and clear behaviour, herdr/Kitty detection, a cache, and the tier-selection defaults. Confirm ADR-0004 after the spike.
- **Images:** local and remote images through the Kitty protocol, with alt-text fallback.

Acceptance: diagrams render as images where Kitty graphics is available, otherwise in the text tier.

## Proposed order (after Phase 1 exit)

1. P2-R31 → P2-R32 → P2-R33
2. P2-05 Tabs
3. P2-06 Diagrams

(Batch 1–2 done: P2-02/04/08/09/10/03.)

Dogfood polish batch (2026-09-30), in order:

1. Readability: P2-23 (includes P2-15, P2-16) → P2-11 → P2-24a (P2-24b after; may move to Phase 3)
2. Chrome: P2-17 → P2-18 → P2-19
3. Nav: P2-12 → P2-13 → P2-14
4. Keyboard flow: P2-22
5. Overlays: P2-20 → P2-21

## Related

- [Phase 1](phase-1-reader-shell.md)
- [Product spec](../product/spec.md) P1
