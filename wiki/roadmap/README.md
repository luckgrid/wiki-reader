# Roadmap

Phased plan from completed Phase 0 through the MVP, alpha polish and beta. Each phase ends with a decision, not just a deliverable. Time boxes are guides for a solo builder.

Dated dogfood bites live in the [dogfood log](dogfood-log.md); audit findings are tracked in the [v0.1.5 audit](audit-v0.1.5.md).

## Phases

| Phase | Status | Goal | Shipped / next |
|-------|--------|------|----------------|
| Phase 0 | done | Try what exists | Findings shaped [ADR-0005](../decisions/0005-navigation-model.md) and [ADR-0006](../decisions/0006-reader-first.md) |
| Phase 1 | done | Reader shell (~1–2 weeks) | [phase-1-reader-shell.md](phase-1-reader-shell.md) |
| Phase 2 | feature complete; dogfood hold to ≈ 2026-10-13 | Wiki navigation MVP (~2 weeks) | Side nav, breadcrumbs, links, back/forward, prev/next, backlinks, heading jumps, tabs, search, help, column cursor + drag-select, soft-wrapped raw view, `$EDITOR`, config, session restore. Fixes only during the hold — [phase-2-mvp.md](phase-2-mvp.md) |
| Phase 3 | active | Alpha polish | Themes, nav position, options window, table and image/diagram viewers, Mermaid image tier, chrome pass (v0.1.3), herdr launcher and page publishing (v0.1.4), lite build, viewer sizing, carousel, diagram toggle, code viewer, shared toolbar icons and the status enum (v0.1.5), audit fixes for crashes and performance (v0.1.6), robustness (P3-29, v0.1.7). Exit decision waits on dogfooding the installed binary — [phase-3-alpha.md](phase-3-alpha.md) |
| Phase 4 | proposed | Beta: widget sidebar and agent surface | [phase-4-beta.md](phase-4-beta.md) |

## Phase 0 (done)

Used markdown-reader inside herdr. Keeper for quick reading; IDE-style model doesn't fit wiki browsing. Findings are in [prior art](../architecture/prior-art-and-libs.md).

## How to use the task files

See [guides/development.md](../guides/development.md). Set Status to `doing`/`done`; reference the ID in commit messages. Keep exit criteria visible at the top of each phase file.
