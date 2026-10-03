---
id: WR-ROADMAP-DOGFOOD
title: Dogfood log
summary: Dated bites, fixes, releases and decisions from the Phase 2 dogfood clock, grouped by release cut.
status: active
updated: 2026-10-03
related: [phase-2-mvp, phase-3-alpha, phase-4-beta]
---

# Dogfood log

Phase 2 stays `active` through two weeks of real use on a real collection. Clock started 2026-09-29; exit remains ≈ 2026-10-13. Record anything that bites here (date + one line); feed that into Phase 3 (and later) scoping. Kind is one of `bite` / `fix` / `release` / `decision`. Dates use operator-local time (UTC−07:00 for these entries). Release and same-evening rework notes previously dated 2026-10-03 in UTC are recorded as 2026-10-02 here; accepted [ADR-0019](../decisions/0019-theme-presets.md) remains unchanged with its UTC date of 2026-10-03.

## v0.1.0-alpha.1

| Date | Kind | Note | Task IDs |
|------|------|------|----------|
| 2026-09-29 | decision | Clock started on a real collection | |
| 2026-09-29 | release | v0.1.0-alpha.1 released (macOS arm64/x86_64, Linux x86_64); dogfood from the installed binary | P3-08 |

## v0.1.0-alpha.2

| Date | Kind | Note | Task IDs |
|------|------|------|----------|
| 2026-09-30 | bite | Side-by-side review vs markdown-reader; UI/UX polish batch filed | P2-11…P2-22 |
| 2026-09-30 | bite | Markdown element rendering (Mermaid text tier garbled, truncated tables, mid-word wrapping, raw syntax markers); formatted-text view and eye toggle; image diagrams (may slip to Phase 3) | P2-23, P2-24a, P2-24b |
| 2026-09-30 | fix | Dogfood polish batch landed — P2-22 (#70), P2-20 (#71), P2-21 (#72), P2-24a (#73). P2-24b deferred to Phase 3. v0.1.0-alpha.2 gated on operator go-ahead after overlays | P2-20…P2-22, P2-24a |
| 2026-09-30 | decision | **Phase 2 feature exit** — all P2 tasks except P2-24b done; status stays `active` through the dogfood clock (≈ 2026-10-13), then flip to `done` | |
| 2026-09-30 | fix | Formatted-view heading hierarchy papercut — H2 rule + H3–H6 spacing (#75) | |
| 2026-09-30 | fix | Dogfood QA after polish batch — P2-22/20/24a pass; herdr keys (`?`/`f`/PageUp/Down/Home/End) pass; headings pass; Linked from backlinks heading ignored formatted mode — fixed in #75 (heading→list colour-match follow-up reverted as misdiagnosis) | |
| 2026-09-30 | fix | `scripts/check.sh` green on main tip after #75 (`d7cc560`); no new bites | |
| 2026-09-30 | decision | **Feature freeze** through ≈ 2026-10-13 — bug fixes / papercuts only; each small PR with a snapshot or test + dated dogfood line. Watch: heading hierarchy, light-terminal contrast (P3-07), nav labels / resize persistence, search ergonomics, herdr key delivery, formatted-mode checkbox/`[NOTE]`/quote-bar markers (known deviation) | |
| 2026-09-30 | release | v0.1.0-alpha.2 cut (#76); clock continues on alpha.2 | |
| 2026-09-30 | decision | Interim P0/P1 acceptance walk recorded on [phase-2-mvp](phase-2-mvp.md) (code + tests + dogfood); **status stays `active`** until ≈ 2026-10-13 for the two-weeks-without-GUI verdict, then flip to `done` | |
| 2026-09-30 | fix | Nav showed `Readme › Readme › Readme` and `Readme` rows with the `filename` default; footer prev/next ignored `nav.labels`; folders now use their folder name and the footer follows the label mode (#82) | P2-13, P2-R36 |
| 2026-09-30 | fix | Linked-from formatted underline uses dim `Rule` (same as H1/H2), not Heading colour | |
| 2026-09-30 | fix | Dogfood round 2 (PR 1): nav shows folder names and titles with no `Overview`/`●` ([ADR-0013](../decisions/0013-nav-labels-folder-names-and-titles.md)); nav highlight follows every navigation; search bar readable; frontmatter toggle works on first load (`▼` when open, YAML colored); outlined prev/next buttons; status pill + page status; `v` toggles formatted view; Help dividers/icons; wheel scrolls popups; header/status gaps removed. Column cursor and drag-select are PR 2 (below) | |
| 2026-09-30 | fix | Dogfood round 2 (PR 2): `←`/`→` move a sticky column cursor in the View (`L12:C5` in the status bar; `←` at column 0 focuses the nav); mouse drag selects text and copies it via OSC 52 (wrapped rows rejoined, gutters/borders dropped, tables tab-separated) | P2-R38 |

## v0.1.0-alpha.3

| Date | Kind | Note | Task IDs |
|------|------|------|----------|
| 2026-09-30 | fix | Dogfood round 3 (PR 3, from live-terminal notes + herdr/markdown-reader screenshots): tabs become outlined buttons on the View's top border (P2-25); `Nav`/`View` title tags dropped and yellow/peach re-sampled from herdr (P2-26); the eye toggle is removed and Rendered is the formatted view (P2-27, ADR-0014); Search and Help popups restyled after markdown-reader (P2-28); Shift+click / Shift+Enter open a new tab (P2-29); raw view soft-wraps (P2-30); nav indent and `/` search icon (P2-31); frontmatter rules full width and colour fixed (P2-32). Found while testing: a table cell wider than the pane leaked above the table (P2-R37) | P2-25…P2-32, P2-R37 |
| 2026-10-01 | fix | Dogfood round 4 (PR 3): reopened nav no longer starts mid-list (P2-R39); content-search results land in place with the phrase highlighted, `…` on cut-off result rows and bottom padding in Search, tighter Help dividers (P2-33) | P2-R39, P2-33 |
| 2026-10-01 | fix | Final review before alpha.3: manual pass in a real terminal found no visual bugs in rounds 2–4 (tabs, popups, content-search landing, raw wrap, hide/show nav, drag-select and paste). A code review then fixed: Help on a terminal under 22 columns panicked; a trailing space in a Content query lost the phrase highlight; clicking in the View left the search highlight; a resize or reload kept a stale selection; a raw-view resize re-read the file and dropped the syntax colours (now re-wraps in place); the raw status bar column restarted on each wrapped row | |
| 2026-10-01 | release | Dogfood rounds 2–4 merged (#83, #84, #85); v0.1.0-alpha.3 cut; clock continues on alpha.3 | |

## v0.1.0-alpha.4

| Date | Kind | Note | Task IDs |
|------|------|------|----------|
| 2026-10-01 | fix | Dogfood round 5 (PR 1): numbered/bulleted items that start with code or a link keep their marker first and no longer paint as one code band (P2-34); ADR ➕/➖ render as ASCII `+`/`-` so they take the text colour (P2-35); H1 teal, H2 peach, H3–H5 light gray, links teal; status pills keep their own colours (P2-36) | P2-34…P2-36 |
| 2026-10-01 | fix | Dogfood round 5 (PR 2): Linked from box-drawn pane with tag header, teal title links, optional summaries and dividers | P2-37 |
| 2026-10-01 | fix | Dogfood round 5 (PR 3): narrow nav rows end in `…` (drop the `(file)` suffix first in title+filename); Search query sits under the top border with no blank row | P2-38, P2-39 |
| 2026-10-01 | fix | Dogfood round 5 (PR 4): Cmd/Ctrl+Enter and Cmd/Ctrl+→ (nav) open a new tab via kitty `DISAMBIGUATE_ESCAPE_CODES`; Ctrl+click joins Shift+click ([ADR-0015](../decisions/0015-new-tab-combos-kitty-keyboard.md), P2-40). Ghostty QA: Cmd+Enter is full screen, Ctrl+Enter works, so the combos are Ctrl-only: `Ctrl+Enter` (nav and view) and `Ctrl+→` (nav) ([ADR-0016](../decisions/0016-ctrl-only-new-tab-combos.md)) | P2-40 |
| 2026-10-01 | fix | Dogfood round 5 brief deltas: leading ➕/➖ → `+`/U+2212 only at item/paragraph start (P2-41); viewer asserts inline code is not full-row shaded (P2-42); Linked from whole-entry hit/focus with selection bg, teal ▌ replacing left `│`, side borders, plain first-paragraph summary (P2-43) | P2-41…P2-43 |
| 2026-10-01 | fix | Dogfood round 5 follow-ups: Tab focus on a Linked-from entry paints selection + teal ▌ with no reverse-video column cursor, and the Tab cursor column follows the focused item (P2-44); summaries are one line ending in `…` (P2-45); bold/inline styles inside quotes and alerts keep the quote bg (P2-46); table links are remapped onto laid-out cells, including after a soft wrap, so Tab walks them row by row (P2-47); table headers are bold text, not link-teal (P2-48); links in the too-narrow unwrapped table dump have no hit targets yet (P2-49) | P2-44…P2-49 |
| 2026-10-01 | fix | Dogfood round 5 follow-ups (2, from Ghostty QA): focused block actions (frontmatter, code titles, expand/copy) use the active-tab/footer colours, dark text on peach, instead of yellow with white text (P2-50); H3 and under have two blank rows above and one below, not the reverse (P2-51) | P2-50, P2-51 |
| 2026-10-01 | release | Dogfood round 5 merged to main (#92, #93, #94; the stacked #88–#91 were closed as superseded by #92); v0.1.0-alpha.4 cut; clock continues on alpha.4 | |

## v0.1.0-alpha.4.1

| Date | Kind | Note | Task IDs |
|------|------|------|----------|
| 2026-10-01 | fix | Dogfood round 6: help window merges alternate keys onto one row and nav `Ctrl+→` opens the new tab and focuses its View (P2-52); mouse-wheel flicks no longer back up the event queue (P2-53); H3/H4 take H2's peach and H2 gets two blank rows above (P2-54). Not reproduced on demand: the runaway scroll, which the user saw once and which the fix targets by design (one redraw per batch, not per event) | P2-52…P2-54 |
| 2026-10-01 | decision | **Phase 2 feature complete; Phase 3 activated.** Nothing in scope is left: P2-24b (image diagrams) is P3-12 and P2-49 stays deferred. The adoption clock keeps running in the background and the formal exit verdict is still ≈ 2026-10-13; until then Phase 2 takes dogfood fixes only, and new features go to Phase 3 | |
| 2026-10-01 | release | Dogfood round 6 merged (#96); v0.1.0-alpha.4.1 cut (a point release, not alpha.5, since Phase 2 only takes fixes now); clock continues on alpha.4.1 | |

## v0.1.0-alpha.5

| Date | Kind | Note | Task IDs |
|------|------|------|----------|
| 2026-10-02 | release | P3-12c/d (Mermaid image tier + diagrams config) cut as v0.1.0-alpha.5; Phase 2 dogfood clock continues on alpha.5 | P3-12c, P3-12d |

## v0.1.0-alpha.5.1

| Date | Kind | Note | Task IDs |
|------|------|------|----------|
| 2026-10-02 | bite | Copy-path key (`y`) should copy what has focus, including a nav folder, from either pane. Same day, three larger requests went to Phase 3 | P2-55, P3-13, P3-14, P3-15 |
| 2026-10-02 | fix | P2-55 (copy path follows focus + `copy.path` config) merged (#104) | P2-55 |
| 2026-10-02 | release | v0.1.0-alpha.5.1 cut (a point release, not alpha.6, since Phase 2 only takes fixes now); clock continues on alpha.5.1 | |

## v0.1.1

| Date | Kind | Note | Task IDs |
|------|------|------|----------|
| 2026-10-02 | decision | Release tags now use `v0.1.N` for the whole alpha phase; `-alpha.N` and point-release forms stop with alpha.5.1; every `v0.1.*` tag is a GitHub prerelease. See [Releasing: Versioning](../guides/releasing.md#versioning) | |
| 2026-10-02 | release | **v0.1.1** cut, the first `v0.1.N` tag: P2-55 was already out; this adds the options window (P3-13), table and image viewers (P3-14/15), nav position (P3-11), luckgrid dark/light and herdr-following themes (P3-07, ADR-0019), `,` / `c` toggle and Help copy-path label. Phase 2 dogfood clock continues on v0.1.1 | P3-07, P3-11, P3-13, P3-14, P3-15 |
| 2026-10-03 | bite | Pie and Git graph diagrams (`fixtures/mermaid` themed.md) vanish after a theme change and reappear on scroll or page switch — slots left un-redrawn / un-requeued | P2-56 |
| 2026-10-03 | decision | Docs audit and drift fixes; docs structure (short README Status, dogfood log, benchmarks page) | P2-57, P2-58 |
| 2026-10-03 | decision | Phase 3 chrome / lite / viewer polish filed (nav label modes, tab/footer borders, footer ⚙/`?`, lite media crate, viewer sizing, carousel, diagram state toggle, screenshot refresh) | P3-16…P3-23 |
| 2026-10-03 | decision | Mirrored keybindings when nav is on the right filed for Phase 4 (low priority; needs ADR) | P4-06 |
| 2026-10-03 | fix | P2-56 implementation: preserve palette-keyed Mermaid sizes across options theme changes instead of globally clearing them. A quick switch away and back could erase completed sizes while their queue keys still said measuring. The regression fails before the fix and checks all three themed.md cards repaint, with new work queued and no scrolling or page switch. Operator verified theme switches in Ghostty and a Herdr pane; diagrams render after the switch | P2-56 |

## v0.1.2

| Date | Kind | Note | Task IDs |
|------|------|------|----------|
| 2026-10-03 | decision | Prepare v0.1.2 to put P2-56 in the installed dogfood binary, plus the docs filing from #117 (#119). Require green CI on the release merge commit before tagging. The adoption clock does not restart; Phase 2 stays on fixes-only hold until the ≈ 2026-10-13 verdict | P2-56, P2-57, P2-58, P3-08 |
| 2026-10-03 | release | [v0.1.2](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.2) published as a prerelease from #119 merge commit `13a1d2d` after its Ubuntu/macOS CI passed. All three release builds passed; three tarballs and three checksums present. macOS arm64 download checksum and binary version verified; Cargo dogfood install upgraded from tag v0.1.1 to v0.1.2 (`~/.cargo/bin/wiki-reader`, `--version` reports 0.1.2). Operator confirmed slow/quick dark/light/herdr theme switches on merged main in standalone Ghostty and Herdr before tagging. Clock continues without restarting | P2-56, P3-08 |

## Next release (unreleased)

| Date | Kind | Note | Task IDs |
|------|------|------|----------|
| 2026-10-03 | decision | Phase 3 execution order: nav labels → footer buttons → closed-box chrome → lite build → viewers → final assets. Three-row tab/footer strips approved. herdr API spike follows chrome alongside lite work; exit criteria recorded, with explicit deferral allowed. No release cut or installed-binary upgrade yet | P3-16…P3-23 |
| 2026-10-03 | fix | P3-16 implementation: title/filename only, literal filesystem folder names, filenames including README/index and extensions, legacy title+filename → title with one diagnostic per config load. Regression coverage includes config layers, curated navigation, index landings, footer/breadcrumb hits and 40/60/80/120-column snapshots. Automated checks recorded in the task row; operator manually confirmed filename labels in the side nav and title-based header breadcrumbs and View footer labels. P3-16's Batch A manual gate is satisfied; the remaining Ghostty/herdr pass covers P3-18/P3-17 only | P3-16 |
| 2026-10-03 | bite | Automated 40/60-column nav-overlay snapshots show article text bleeding through blank nav cells. Operator confirmed on v0.1.2, with long breadcrumbs hiding header controls and outside clicks leaving nav open (screenshots 2026-10-03 10:36). Pre-#121 `c69c348` renders reproduce bleed at both widths; header and dismissal regression tests also fail there. Separate Phase 2 fix, not a P3-16 regression | P2-59 |
| 2026-10-03 | decision | P3-16 scope corrected by operator: nav.labels affects only the side-nav file tree, never header breadcrumbs or View footer labels. ADR-0021 supersedes ADR-0020's chrome-label rule; cached title navigation and regression coverage keep chrome stable through switches and reindex | P3-16 |
| 2026-10-03 | fix | P2-59 (unreleased): narrow nav clears underlying text and fills with the inherited theme background; docked nav unchanged. Header icons reserve their columns, with ellipsized breadcrumbs and bounded hits. Outside left/middle clicks dismiss only, including queued clicks before redraw; popups keep precedence. Added 40/60 overlay snapshots and regression checks for inherited dark/light/herdr backgrounds, bounded header hits, dismiss-only clicks, queued clicks and popup precedence; 80/120 snapshots unchanged. `scripts/check.sh` passed (530 tests); post-fix operator terminal pass pending | P2-59 |

## Related

- [Phase 2](phase-2-mvp.md)
- [Phase 3](phase-3-alpha.md)
- [Phase 4](phase-4-beta.md)
- [Roadmap](README.md)
