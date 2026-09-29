# wiki-reader

> Working name. Rename freely; nothing depends on it yet.

A terminal wiki reader for markdown collections. It browses like a documentation site (side nav, breadcrumbs, working links, back/forward, prev/next), sized to live in a herdr pane next to your work.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ Luckgrid Wiki › Architecture › Design System › Token Projection       ◫   ✕ │
├──────────────────────────────┬───────────────────────────────────────────────┤
│ ⌕ Search…               /    │ # Token Projection                            │
│ ● Luckgrid Wiki              │                                               │
│ ▾ Architecture               │ The reusable adapter stays semantic-only; see │
│     Architecture Overview    │ [ADR-0003](../decisions/0003.md) for why.     │
│   ▾ Design System            │                                               │
│       Design System          │▌                                              │
│     ● Token Projection       │                                               │
│     Workflow OS              ├───────────────────────────────────────────────┤
│ ▸ Decisions                  │ ‹ Design System                  Adapters ›   │
├──────────────────────────────┴───────────────────────────────────────────────┤
│ VIEWER · architecture/design-system/tokens.md · L42 38% · 1,284 w · 6 min   │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Why not an existing reader?

Existing terminal markdown tools behave like editors or file browsers: opening a page spawns a tab, links are inert, and search lives in a modal. wiki-reader uses a **browser/wiki navigation model**. Every way of reaching a page replaces the current view and records history, and tabs are an opt-in secondary feature. See [ADR-0005](wiki/decisions/0005-navigation-model.md).

## Status

Phase 1 in progress (scaffolded). Start at [wiki/README.md](wiki/README.md).

## Quickstart

```bash
cargo run -p wiki-reader -- fixtures/worked-example
# q or Esc to quit
```

## Checks

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

## Workspace

Cargo workspace with `wiki-reader-core` (terminal-free), `wiki-reader-render`, and the `wiki-reader` TUI binary. Docs live in `wiki/`. See [guides/development.md](wiki/guides/development.md).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. See [THIRD_PARTY.md](THIRD_PARTY.md) for ported-code attributions.
