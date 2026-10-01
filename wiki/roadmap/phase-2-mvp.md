---
id: WR-ROADMAP-P2
title: Phase 2 — Wiki navigation MVP
summary: Custom nav order, backlinks, tabs, diagrams, config, and session restore.
status: active
updated: 2026-10-01
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
- 2026-09-30: dogfood polish batch landed — P2-22 (#70), P2-20 (#71), P2-21 (#72), P2-24a (#73). P2-24b deferred to Phase 3. v0.1.0-alpha.2 gated on operator go-ahead after overlays.
- 2026-09-30: **Phase 2 feature exit** — all P2 tasks except P2-24b done; status stays `active` through the dogfood clock (≈ 2026-10-13), then flip to `done`.
- 2026-09-30: formatted-view heading hierarchy papercut — H2 rule + H3–H6 spacing (#75)
- 2026-09-30: dogfood QA after polish batch — P2-22/20/24a pass; herdr keys (`?`/`f`/PageUp/Down/Home/End) pass; headings pass; Linked from backlinks heading ignored formatted mode — fixed in #75 (heading→list colour-match follow-up reverted as misdiagnosis)
- 2026-09-30: `scripts/check.sh` green on main tip after #75 (`d7cc560`); no new bites
- 2026-09-30: **feature freeze** through ≈ 2026-10-13 — bug fixes / papercuts only; each small PR with a snapshot or test + dated dogfood line. Watch: heading hierarchy, light-terminal contrast (P3-07), nav labels / resize persistence, search ergonomics, herdr key delivery, formatted-mode checkbox/`[NOTE]`/quote-bar markers (known deviation)
- 2026-09-30: v0.1.0-alpha.2 cut (#76); clock continues on alpha.2
- 2026-09-30: interim P0/P1 acceptance walk recorded below (code + tests + dogfood); **status stays `active`** until ≈ 2026-10-13 for the two-weeks-without-GUI verdict, then flip to `done`
- 2026-09-30: nav showed `Readme › Readme › Readme` and `Readme` rows with the `filename` default (P2-13), and the footer prev/next ignored `nav.labels`; folders now use their folder name and the footer follows the label mode (P2-R36, #82)
- 2026-09-30: Linked-from formatted underline uses dim `Rule` (same as H1/H2), not Heading colour
- 2026-09-30: dogfood round 2 (PR 1): nav shows folder names and titles with no `Overview`/`●` ([ADR-0013](../decisions/0013-nav-labels-folder-names-and-titles.md)); nav highlight follows every navigation; search bar readable; frontmatter toggle works on first load (`▼` when open, YAML colored); outlined prev/next buttons; status pill + page status; `v` toggles formatted view; Help dividers/icons; wheel scrolls popups; header/status gaps removed. Column cursor and drag-select are PR 2 (below)
- 2026-09-30: dogfood round 2 (PR 2): `←`/`→` move a sticky column cursor in the View (`L12:C5` in the status bar; `←` at column 0 focuses the nav); mouse drag selects text and copies it via OSC 52 (wrapped rows rejoined, gutters/borders dropped, tables tab-separated)
- 2026-09-30: dogfood round 3 (PR 3, from live-terminal notes + herdr/markdown-reader screenshots): tabs become outlined buttons on the View's top border (P2-25); `Nav`/`View` title tags dropped and yellow/peach re-sampled from herdr (P2-26); the eye toggle is removed and Rendered is the formatted view (P2-27, ADR-0014); Search and Help popups restyled after markdown-reader (P2-28); Shift+click / Shift+Enter open a new tab (P2-29); raw view soft-wraps (P2-30); nav indent and `/` search icon (P2-31); frontmatter rules full width and colour fixed (P2-32). Found while testing: a table cell wider than the pane leaked above the table (P2-R37)
- 2026-10-01: dogfood round 4 (PR 3): reopened nav no longer starts mid-list (P2-R39); content-search results land in place with the phrase highlighted, `…` on cut-off result rows and bottom padding in Search, tighter Help dividers (P2-33)
- 2026-10-01: final review before alpha.3: manual pass in a real terminal found no visual bugs in rounds 2–4 (tabs, popups, content-search landing, raw wrap, hide/show nav, drag-select and paste). A code review then fixed: Help on a terminal under 22 columns panicked; a trailing space in a Content query lost the phrase highlight; clicking in the View left the search highlight; a resize or reload kept a stale selection; a raw-view resize re-read the file and dropped the syntax colours (now re-wraps in place); the raw status bar column restarted on each wrapped row
- 2026-10-01: dogfood rounds 2–4 merged to main (#83, #84, #85); v0.1.0-alpha.3 cut; clock continues on alpha.3
- 2026-10-01: dogfood round 5 (PR 1): numbered/bulleted items that start with code or a link keep their marker first and no longer paint as one code band (P2-34); ADR ➕/➖ render as ASCII `+`/`-` so they take the text colour (P2-35); H1 teal, H2 peach, H3–H5 light gray, links teal; status pills keep their own colours (P2-36)
- 2026-10-01: dogfood round 5 (PR 2): Linked from is a box-drawn pane with a tag header, teal title links, optional summaries and dividers (P2-37)
- 2026-10-01: dogfood round 5 (PR 3): narrow nav rows end in `…` (drop the `(file)` suffix first in title+filename); Search query sits under the top border with no blank row (P2-38, P2-39)

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
| D1 | partial | text Mermaid tier pass; **image tier → P3-12** (exit-allowed slip) |
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
| P2-35 | Consequence ➕/➖ emoji render as ASCII `+`/`-` so they take the text colour | D1 | done | round 5 PR 1 |
| P2-36 | Heading ramp: H1 accent/teal, H2 peach, H3–H5 light gray; internal links teal; status pills use dedicated tokens | U3 | done | round 5 PR 1 |
| P2-37 | Linked from is a box-drawn pane: tag header, title links, optional summaries, dividers between entries | B1 | done | round 5 PR 2 |
| P2-38 | Narrow nav rows end in `…`; in `title+filename` the dim `(file)` suffix is dropped before the title is ellipsised | U3 | done | round 5 PR 3 |
| P2-39 | Search overlay: query row sits directly under the top border (no blank row) | U3 | done | round 5 PR 3 |
| P2-R38 | Copy fixes: empty table cells kept, wrapped cells rejoined, OSC 52 cap on the encoded payload (75 KB of text), held drag keeps scrolling | CP | done | round 2 follow-ups (PR 2) |
| P2-24b | Image diagrams and images | D1 | moved | slipped to Phase 3 as P3-12 (2026-09-30); needs ADR-0004 herdr/Kitty spike + deps |

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
