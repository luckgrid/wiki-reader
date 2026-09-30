---
id: WR-ROADMAP-P1
title: Phase 1 — Reader shell
summary: Browse a real collection end-to-end with correct navigation.
status: done
updated: 2026-09-29
related: [phase-2-mvp]
nav_order: 1
---

# Phase 1 — Reader shell

Browse a real collection end-to-end with correct navigation. Time box: ≈ 1–2 weeks.

## Exit criteria

Open a real collection (e.g. any docs folder, or `fixtures/worked-example`), follow ten links in a row (revisits allowed), go back ten times, never see a tab.

## Escape hatch

If porting the renderer takes more than ~3 days, temporarily depend on a simpler renderer (plain pulldown-cmark → styled lines) and port features incrementally. Navigation matters more than table polish.

## Tasks

| ID | Task | Reqs | Status | Notes |
|----|------|------|--------|-------|
| P1-00 | Scaffold | | done | 2b9fdb6 |
| P1-01 | FsProvider + discovery | | done | 7bc90c9 |
| P1-02 | Frontmatter + parse | | done | a907712 |
| P1-03 | Index | | done | f2752c2 |
| P1-04 | Link resolution | L2 | done | 216be37 |
| P1-05 | NavTree + fixture snapshots | T1 | done | a2d68c5 |
| P1-06 | navigate() + history + invariant tests | N1–N4 | done | ae5fd3a |
| P1-R1 | Parse robustness | F2,F7 | done | a0656a7 |
| P1-R2 | Index robustness + determinism | F3,F5,F8,F9 | done | 4ee9957 |
| P1-R3 | NavTree correctness | F1,F5,F6,T1 | done | e85c57b |
| P1-R4 | Navigator hardening | F4,F10,N1–N4 | done | 66866be |
| P1-R5 | Docs, spikes, housekeeping | | done | 20bdf18 |
| P1-R6 | Geometry: header hits + nav visibility | T1,T2 | done | c97963e |
| P1-R7 | Viewport + scroll follow | T3,T4 | done | c97963e |
| P1-R8 | Focus + nav state model | T6–T9 | done | c97963e |
| P1-R9 | K3 visuals, keymap, terminal safety | T5,T12,T13 | done | c97963e |
| P1-R10 | Docs, process, test hygiene | T10–T11,T14,T16–T18 | done | 5911ea5 |
| P1-R11 | Post-merge residuals + check.sh | N1–N6 | done | 1414d39 (`scripts/check.sh`) |
| P1-R12 | Renderer to V1 | V1 | done | 1414d39 (StyledLine) |
| P1-R13 | Cursor mapping + syntect raw | V2 | done | 1414d39 |
| P1-R14 | Search core + overlay | S1 | done | 1414d39 |
| P1-R15 | Watcher + reindex | V3 | done | 1414d39 |
| P1-R16 | Hygiene | | done | 1414d39 |
| P1-R17 | Renderer polish | V1 | done | 1414d39 |
| P1-R18 | Live reload hardening | V3 | done | 1414d39 |
| P1-R19 | Search overlay to S1 | S1 | done | 1414d39 |
| P1-R20 | Perf + docs | | done | 1414d39 |
| P1-R21 | Renderer perf + table align | V1 | done | 1414d39 |
| P1-R22 | Watcher correctness | V3 | done | 1414d39 |
| P1-R23 | Search match remap | S1 | done | 1414d39 |
| P1-R24 | Roadmap honesty + test time | | done | 1414d39 |
| P1-R25 | Source-map precision + table padding | V1,S1 | done | 17167ef |
| P1-R26 | Watcher FS heuristics | V3 | done | 8f81082 |
| P1-R27 | Roadmap honesty + exit checklist | | done | 35d711b |
| P1-R28 | Performance guard rails | V1,V2 | done | adae549 |
| P1-R29 | Perf guard finish (ceiling, stale highlight, docs) | V1,V2 | done | c1675a6 |
| P1-07 | Layout regions + hit map | | done | 5911ea5 |
| P1-07a | Core support APIs (breadcrumb, blocks, nav focus) | | done | 5911ea5 |
| P1-07b | TUI skeleton: regions, hit map, static content | | done | 5911ea5 |
| P1-07c | Side nav (`HitMap` flat rows; ADR-0010) | T1,N4 | done | 5911ea5 |
| P1-G | Live herdr key-log gate | | done | real keyboard 2026-09-29 (Ghostty + herdr); caveats in P1-S1 |
| P1-08 | Focus/cursor | K1–K4 | done | 5911ea5 |
| P1-08a | Pane focus (K1) + side-nav keys (K4) | K1,K4 | done | 5911ea5 |
| P1-08b | Viewer cursor (K2) + interim ViewerDoc | K2 | done | 5911ea5 |
| P1-08c | Viewer Tab cycle (K3) | K3 | done | 5911ea5 |
| P1-09 | Links, Tab/Enter/click | L1–L3 | done | 1414d39 |
| P1-10 | Search overlay | S1 | done | 1414d39 |
| P1-11 | Raw toggle | V2 | done | 1414d39 |
| P1-12 | Live reload | V3 | done | 1414d39 |
| P1-13 | Renderer port | V1 | done | 1414d39 |
| P1-S1 | Spike, herdr input | | done | real keyboard 2026-09-29; Alt+←/→ → Alt+b/f (P1-R36) |
| P1-S2 | Spike, tree widget decision | | done | ADR-0009 → superseded by ADR-0010 |
| P1-D1 | Docs: Phase 1 status + architecture honesty | | done | 269670b |
| P1-R36 | Modifier-strict keys + Alt+b/Alt+f | K1,K2 | done | 269670b Ghostty Option encoding |
| P1-R37 | Unsupported link schemes | L2 | done | 269670b muted; never open |
| P1-R38 | Full-width viewer cursor row | K2 | done | 269670b faint row background spans the viewer |

## Phase 1 exit checklist

All operator items are ticked (2026-09-29). Phase status is `done`. Automated coverage is noted under each item; it does **not** close the item.

1. [x] `cargo run -p wiki-reader --example keylog` in a herdr pane on your real keyboard; tick the [P1-S1](spikes/p1-s1-herdr-input.md) spike note (P1-G).
   - Coverage: example exists; real keyboard 2026-09-29 in Ghostty + herdr (see spike for caveats → P1-R36).
2. [x] On a real collection (e.g. any docs folder, or `fixtures/worked-example`): follow ten links incl. anchors, go back ten times, confirm one tab and a restored cursor (revisits allowed; deepest page is ≤3 hops from README.md).
   - Coverage: `link_chain_ten_links_ten_backs_one_tab` on `wiki/`; operator-confirmed on a real collection 2026-09-29.
3. [x] Edit, rename, and delete a page from another terminal while it is open; confirm live reload / page-removed.
   - Coverage: watcher unit tests (atomic save, rename, delete); operator-confirmed 2026-09-29 on a scratch copy of `fixtures/worked-example` (edit, atomic save, rename, delete). Renaming the open page shows the page-removed placeholder; the nav row keeps its title label and now points at the renamed file.
4. [x] Resize below and above 80 columns; nav overlay / layout stays usable.
   - Coverage: snapshot + nav-visibility resize tests; operator-confirmed 2026-09-29.
5. [x] Open/close search; Esc restores the prior cursor.
   - Coverage: overlay unit tests; operator-confirmed 2026-09-29.
6. [x] Ctrl+C and `q` restore the terminal in herdr and in a plain terminal.
   - Coverage: confirmed on your machine 2026-09-29.

Also: both ignored release budgets pass (operator-confirmed 2026-09-29) — `cargo test -p wiki-reader-render --release -- --ignored` (render <20 ms / 50 KB) and `cargo test -p wiki-reader --release -- raw_load_budget --ignored` (raw UI path <50 ms / 50 KB, no sync highlight; syntect ~370 ms / 50 KB release runs off-thread and paints asynchronously). CI asserts a ~170 KB debug render finishes under a 10 s wall-clock ceiling (guards regression to per-call newline scans).

## Related

- [Product spec](../product/spec.md) P0 requirements
- [Architecture](../architecture/overview.md)
