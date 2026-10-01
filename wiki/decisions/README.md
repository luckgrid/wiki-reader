# Decisions

Architecture decision records. Accepted ADRs are immutable — supersede, don't edit; mechanical metadata or formatting fixes that don't change decision text are allowed.

## Status

| ADR | Title | Status |
|-----|-------|--------|
| [0001](0001-rust-ratatui.md) | Rust + ratatui | accepted |
| [0002](0002-build-vs-fork.md) | Build on existing pieces, don't fork the app | accepted |
| [0003](0003-core-cli-first.md) | Core library + CLI first | superseded by [0006](0006-reader-first.md) |
| [0004](0004-diagram-rendering.md) | Tiered diagram rendering | proposed |
| [0005](0005-navigation-model.md) | Wiki navigation model | accepted |
| [0006](0006-reader-first.md) | Reader first; core stays terminal-free | accepted |
| [0007](0007-input-focus-model.md) | Input & focus model | accepted |
| [0008](0008-side-nav-as-site-nav.md) | Side nav as site navigation | accepted |
| [0009](0009-tui-tree-widget.md) | Adopt tui-tree-widget for side nav | superseded |
| [0010](0010-flat-side-nav-rows.md) | Flat visible rows for side nav | accepted |
| [0011](0011-renderer-source.md) | Keep the current renderer; port patterns not modules | accepted |
| [0012](0012-syntax-vs-formatted.md) | Syntax vs formatted rendered view | superseded by [0014](0014-remove-formatted-view-toggle.md) |
| [0013](0013-nav-labels-folder-names-and-titles.md) | Nav shows folder names and page titles, no Overview | accepted |
| [0014](0014-remove-formatted-view-toggle.md) | Remove the syntax/formatted view toggle | accepted |
| [0015](0015-new-tab-combos-kitty-keyboard.md) | New-tab combos via kitty keyboard disambiguation | accepted |

## Related

- [Architecture](../architecture/README.md)
- [Roadmap](../roadmap/README.md)
