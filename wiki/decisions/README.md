# Decisions

Architecture decision records. Accepted ADRs are immutable — supersede, don't edit; mechanical metadata or formatting fixes that don't change decision text are allowed.

**Dates:** ADR `Date:` / frontmatter `updated:` values are UTC. Roadmap and dogfood entries may use operator-local time; when they cite an ADR date (for example [ADR-0019](0019-theme-presets.md) as 2026-10-03 UTC while local notes say 2026-10-02), the ADR's UTC date wins and stays unchanged.

## Status

| ADR | Title | Status |
|-----|-------|--------|
| [0001](0001-rust-ratatui.md) | Rust + ratatui | accepted |
| [0002](0002-build-vs-fork.md) | Build on existing pieces, don't fork the app | accepted |
| [0003](0003-core-cli-first.md) | Core library + CLI first | superseded by [0006](0006-reader-first.md) |
| [0004](0004-diagram-rendering.md) | Tiered diagram rendering | accepted |
| [0005](0005-navigation-model.md) | Wiki navigation model | accepted |
| [0006](0006-reader-first.md) | Reader first; core stays terminal-free | accepted |
| [0007](0007-input-focus-model.md) | Input & focus model | accepted (in part superseded by [0015](0015-new-tab-combos-kitty-keyboard.md)) |
| [0008](0008-side-nav-as-site-nav.md) | Side nav as site navigation | accepted (label choices superseded by [0020](0020-nav-label-modes.md)) |
| [0009](0009-tui-tree-widget.md) | Adopt tui-tree-widget for side nav | superseded by [0010](0010-flat-side-nav-rows.md) |
| [0010](0010-flat-side-nav-rows.md) | Flat visible rows for side nav (supersedes ADR-0009) | accepted (in part superseded by [0013](0013-nav-labels-folder-names-and-titles.md)) |
| [0011](0011-renderer-source.md) | Renderer source (Phase 1) | accepted |
| [0012](0012-syntax-vs-formatted.md) | Syntax vs formatted rendered view | superseded by [0014](0014-remove-formatted-view-toggle.md) |
| [0013](0013-nav-labels-folder-names-and-titles.md) | Nav shows folder names and page titles, no Overview | accepted (label rules superseded by [0020](0020-nav-label-modes.md)) |
| [0014](0014-remove-formatted-view-toggle.md) | Rendered is the formatted view; remove the eye toggle | accepted |
| [0015](0015-new-tab-combos-kitty-keyboard.md) | New-tab combos via kitty keyboard disambiguation | accepted (Cmd dropped by [0016](0016-ctrl-only-new-tab-combos.md)) |
| [0016](0016-ctrl-only-new-tab-combos.md) | New-tab combos are Ctrl-only (no Cmd) | accepted |
| [0017](0017-static-local-images-only.md) | Images are static, local and never fetched | accepted |
| [0018](0018-config-write-path.md) | Options window writes the operator config file | accepted |
| [0019](0019-theme-presets.md) | luckgrid presets, and `herdr` follows herdr's config | accepted |
| [0020](0020-nav-label-modes.md) | Titles or actual filesystem names | accepted (chrome label scope superseded by [0021](0021-side-nav-only-label-mode.md)) |
| [0021](0021-side-nav-only-label-mode.md) | Nav label mode applies only to the side nav | accepted |
| [0022](0022-herdr-launcher-and-page-publishing.md) | herdr launcher opens an ordinary pane; page publishing is on by default | accepted |
| [0023](0023-lite-build-is-a-cargo-feature.md) | The lite build is a cargo feature, not a separate artifact | accepted |

## Related

- [Architecture](../architecture/README.md)
- [Roadmap](../roadmap/README.md)
