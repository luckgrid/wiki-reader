---
id: WR-ROADMAP-P2
title: Phase 2 — Wiki navigation MVP
summary: Custom nav order, backlinks, tabs, diagrams, config, and session restore.
status: active
updated: 2026-10-03
related: [phase-1-reader-shell, phase-3-alpha, dogfood-log]
nav_order: 2
---

# Phase 2 — Wiki navigation MVP

Time box: ≈ 2 weeks.

**Status (2026-10-03): feature complete, dogfood hold.** [Phase 3](phase-3-alpha.md) is active. Phase 2 now accepts only fixes for things the dogfood clock turns up (a small PR with a test or snapshot and a dated line in the [dogfood log](dogfood-log.md)) until the ≈ 2026-10-13 verdict.

## Exit criteria

All P0/P1 acceptance criteria in [product spec](../product/spec.md) pass; two weeks without opening a GUI markdown app for these collections.

The dogfood polish batch (P2-11…P2-24a) should land before the clock is judged, but the two weeks do **not** restart or extend. Clock started 2026-09-29; exit remains ≈ 2026-10-13. P2-24b (image diagrams) may slip to Phase 3 without blocking exit.

## Dogfood log

Phase 2 stays `active` through two weeks of real use on a real collection. Clock started 2026-09-29; exit remains ≈ 2026-10-13. Dated bites, fixes, releases and decisions live in the [dogfood log](dogfood-log.md).

## Interim acceptance walk (2026-09-30)

Judged from tests, dogfood QA, and code — not a full interactive sweep of every row. Formal exit review ≈ 2026-10-13 re-confirms the adoption verdict.

### P0

| ID | Result | Notes |
|----|--------|-------|
| N1 | pass | navigate() invariant tests |
| N2 | pass | ordinary_navigation_does_not_create_tabs |
| N3 | pass | history + cursor restore tests |
| N4 | pass | nav sync / expand tests |
| T1 | pass | tree snapshots (page_label modes; default filename via P2-13) |
| K1–K4 | pass | focus/keymap/nav tests; sticky footer via P2-22 |
| L1–L3 | pass | link click / resolve / broken+external tests |
| H1 | pass | breadcrumb + header icon hits |
| F1–F2 | pass | footer prev/next; status bar |
| S1 | pass | search overlay; modes named Files/Content (was Pages/Text in older copy) |
| V1 | pass | rendered elements + P2-23/24a |
| V2 | pass | raw toggle |
| V3 | pass | watch/dirty reload tests |

### P1

| ID | Result | Notes |
|----|--------|-------|
| P1 | pass | SUMMARY.md / nav_order |
| B1 | pass | Linked from; formatted heading fixed #75 |
| TB | pass | tabs secondary |
| BA | pass | block actions in Tab cycle |
| D1 | partial | text Mermaid tier pass; **image tier → P3-12a…d** (exit-allowed slip) |
| R1 | pass | responsive nav |
| E1 | pass | `$EDITOR` |
| C1 | partial | keys/opener/excludes/nav.labels ship; **theme key still inert → P3-07/11** |
| M1 | pass | session restore |
| J1 | pass | heading jump |

### Adoption

| Measure | Result | Notes |
|---------|--------|-------|
| Two weeks without a GUI markdown app | pending | clock 2026-09-29 → ≈ 2026-10-13 |

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
| P2-R36 | Nav folder labels in `filename` mode; footer prev/next follows `nav.labels` | C1 / U3 | done | #82 |
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
| P2-24a | Formatted-text view and eye toggle | D1 / C1 | removed → P2-27 | #73; ADR-0012, superseded by ADR-0014 |
| P2-25 | View tabs: outlined buttons on the View's top border, styled like the prev/next footer; always shown (the current page's tab even when alone); filled peach only while the View has focus | TB | done | round 3 (PR 3) |
| P2-26 | Chrome: no `Nav`/`View` title tags; one warm colour for the status pill, nav folders, popups, tabs and footer links: the herdr tab peach `#f6c99f` | U3 | done | round 3; colours sampled from a herdr screenshot |
| P2-27 | Remove the eye / syntax-formatted toggle; Rendered is always formatted, raw shows syntax (key `v`, header icon, Help row, docs, `RenderOpts.formatted`, session field) | D1 | done | round 3; [ADR-0014](../decisions/0014-remove-formatted-view-toggle.md) supersedes ADR-0012; "Linked from" now hugs its list |
| P2-28 | Search and Help popups borrow markdown-reader's layout: full-width selected row, 2-col padding, peach border, mode + Tab hint in the title, `/` input with inline placeholder, footer with counts and key hints, content rows `[line] title path – snippet` in separate colours, Help tall and thin with icons joined to keys (`b / ◫`) and long group dividers; both panes behind go gray | U3 | done | round 3; status pill reads `SEARCH` / `HELP` while open |
| P2-29 | Shift+click (nav row, link, crumb, prev/next) and `Shift+Enter` (nav row) open in a new tab | TB | done | round 3; `Shift+Enter` needs a terminal that reports it (kitty keyboard protocol) |
| P2-30 | Raw view soft-wraps instead of running off the pane | D1 | done | round 3; display rows map to source lines (cursor, `$EDITOR`, copy, links, highlights) |
| P2-31 | Nav nested rows indent one more column (3 per level); search bar icon becomes a text-height `/` | U3 | done | round 3 |
| P2-32 | Frontmatter box rules span the full pane width; text colour no longer depends on the cursor line | D1 | done | round 3 |
| P2-R37 | Table cell wider than the pane no longer spills out above the table | D1 | done | `push_span` wrapped cell text at pane width |
| P2-R39 | Reopened nav starts mid-list: footer navigation while the nav was hidden scrolled the list against a 1-row viewport | N4 | done | round 4; the viewport only updates while the nav is drawn, and a list that fits always starts under the search bar |
| P2-33 | Search landing and popup polish: a content result keeps the page in place (centred only if off-screen), the phrase is highlighted in peach with the cursor inverted on it; cut-off rows end in `…`; bottom breathing room; Help dividers one row above, none below | U3 | done | round 4 |
| P2-34 | List items that start with inline code or a link emit the marker first; inline code no longer full-row shades adjacent items into one band | D1 | done | round 5 PR 1 |
| P2-35 | Consequence ➕/➖ emoji at item/paragraph start render as `+` / U+2212 `−` so they take the text colour | D1 | done | round 5 PR 1; mid-sentence emoji left alone (P2-41) |
| P2-36 | Heading ramp: H1 accent/teal, H2 peach, H3–H5 light gray; internal links teal; status pills use dedicated tokens | U3 | done | round 5 PR 1 |
| P2-37 | Linked from is a box-drawn pane: tag header, title links, optional summaries, dividers between entries | B1 | done | round 5 PR 2 |
| P2-38 | Narrow nav rows end in `…`; in `title+filename` the dim `(file)` suffix is dropped before the title is ellipsised | U3 | done | round 5 PR 3 |
| P2-39 | Search overlay: query row sits directly under the top border (no blank row) | U3 | done | round 5 PR 3 |
| P2-40 | New-tab combos: `Ctrl+Enter` (nav, view), `Ctrl+→` (nav), Ctrl+click; kitty DISAMBIGUATE flags ([ADR-0015](../decisions/0015-new-tab-combos-kitty-keyboard.md)); Cmd dropped ([ADR-0016](../decisions/0016-ctrl-only-new-tab-combos.md)) | TB | done | round 5 PR 4; ADR-0016 |
| P2-41 | Leading ➕/➖ at item/paragraph start only → `+` / U+2212; mid-sentence left alone | D1 | done | round 5 brief deltas |
| P2-42 | Viewer test: inline code does not full-row shade adjacent list items | D1 | done | round 5 brief deltas |
| P2-43 | Linked from: whole-entry LinkSpan (title+summary), side borders, BacklinkBorder/Tag/Summary styles, selection bg + teal ▌ (replaces left `│`) on Tab focus, plain first-paragraph summary fallback | B1 | done | round 5 brief deltas |
| P2-44 | Linked from Tab focus: selection bg + teal ▌ only; the reverse-video column cursor is hidden on the focused entry, and Tab moves the cursor column to the focused item | B1 | done | round 5 follow-ups |
| P2-45 | Linked from summary is one line ending in `…` (no orphan `.` or letter wraps) | B1 | done | round 5 follow-ups |
| P2-46 | Strong/emphasis/link spans inside quotes and alerts keep the quote bg (no black hole behind bold text) | D1 | done | round 5 follow-ups |
| P2-47 | Table links remapped onto laid-out cells, located in the cell text so links after a soft wrap land on their own glyphs; Tab no longer sticks on the top border | D1 | done | round 5 follow-ups |
| P2-48 | Table headers are bold text colour, not link-teal | D1 | done | round 5 follow-ups |
| P2-49 | Hit targets for links in the too-narrow unwrapped table dump (map cell offsets onto the `│ a \| b` rows) | D1 | deferred | very narrow panes only |
| P2-50 | Focused block actions (frontmatter toggle, code block titles, expand table/diagram, copy code) paint peach bg + dark text like the active tab and footer links; `focus_item` token removed | U3 | done | round 5 follow-ups (2) |
| P2-51 | H3–H6 spacing: two blank rows above, one below (was the reverse) | D1 | done | round 5 follow-ups (2) |
| P2-52 | Help overlay merges keys that do the same thing into one row joined with ` / ` (`↑ / Shift+Tab`, `k / ↑`, `/ / Ctrl+k`, `Ctrl+Enter / Shift+Enter`), driven by `help_entries()` so the generated keymap table matches; nav `Ctrl+→` opens the page in a new tab **and focuses the View** (`Action::NewTabFocusView`), `Ctrl+Enter` / `Shift+Enter` keep focus in the nav | TB / U3 | done | round 6 |
| P2-53 | Event loop applies every queued event (cap 256) before one redraw, and discards unread input when the terminal is restored; a wheel flick no longer lags behind the pointer or leaks mouse reports into the shell | K1 | done | round 6; found when the wheel kept scrolling after the user stopped |
| P2-54 | Heading colour and spacing: H3 and H4 share H2's peach (H5/H6 stay gray); H2 gets two blank rows above like H3–H6 (H1 unchanged) | D1 / U3 | done | round 6; supersedes the H3–H5 gray in P2-36 and widens P2-51 to H2–H6 |
| P2-55 | Copy path follows focus: with the nav focused, `y` copies the selected row (page file path or folder path for a group); with the View focused it copies the viewed page. Status line says what was copied. Default path is relative to the collection root; `copy.path = "relative" \| "absolute"` in config (options UI waits for P3-13) | K4 | done | dogfood request 2026-10-02; help label unchanged |
| P2-56 | Pie and Git graph diagrams (`fixtures/mermaid` themed.md) vanish after a theme change and reappear on scroll or page switch | D1 | done | 2026-10-03: preserve the palette-keyed natural-size cache during theme changes; clearing it could erase completed measurements before their re-layout notification and strand queue keys. Regression covers options theme cycling, queued work and repaint of Sequence/Pie/Git graph without scroll or page changes (fails before the fix). Operator verified theme switches in Ghostty and a Herdr pane; diagrams render after the switch |
| P2-57 | Docs audit and drift fixes | — | done | dogfood 2026-10; README/ui-spec ⚙, spec U3/U6–U9, modal viewers section, date drift, P3-07 Notes |
| P2-58 | Docs structure: short README Status, dogfood log as its own file, benchmarks as their own page | — | done | dogfood 2026-10; [dogfood-log.md](dogfood-log.md), [benchmarks.md](../architecture/benchmarks.md) |
| P2-59 | Narrow-terminal nav overlay: opaque inherited background, pinned header controls and outside-click dismissal | U2 / K1 | done | 2026-10-03 (v0.1.3): operator confirmed all three bugs on v0.1.2; pre-#121 `c69c348` renders reproduce bleed at 40/60 columns and regression tests fail for long breadcrumbs and outside clicks. Clear/fill only the overlay; reserve header icon columns and ellipsize overflow; outside clicks dismiss without activating underlying targets. Regression snapshots and mouse tests; post-fix terminal pass pending |
| P2-R38 | Copy fixes: empty table cells kept, wrapped cells rejoined, OSC 52 cap on the encoded payload (75 KB of text), held drag keeps scrolling | CP | done | round 2 follow-ups (PR 2) |
| P2-24b | Image diagrams and images | D1 | moved | slipped to Phase 3 as P3-12 (2026-09-30), now split into P3-12a…d; images follow [ADR-0017](../decisions/0017-static-local-images-only.md) |

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
