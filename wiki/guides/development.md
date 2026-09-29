---
id: WR-GUIDE-DEV
title: Development
summary: Toolchain, checks, crate boundaries, dependency policy, docs conventions, and task workflow.
status: draft
updated: 2026-09-28
related: []
---

# Development

How to pick up work in this repo.

## Toolchain

- Rust from [`rust-toolchain.toml`](../../rust-toolchain.toml): channel `1.98.1`, with `rustfmt` and `clippy`.
- Edition 2024; `rustfmt.toml` sets `style_edition = "2024"`.
- Builds and CI use `--locked` against the committed `Cargo.lock`.

## Checks

Same three commands as CI:

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

ADR-0006 guard (core stays terminal-free):

```bash
cargo tree -p wiki-reader-core -e normal
# must not list ratatui or crossterm
```

Quickstart:

```bash
cargo run -p wiki-reader -- fixtures/worked-example
# q or Esc to quit
```

## Crate boundaries

| Crate | Responsibility |
|-------|----------------|
| `wiki-reader-core` | Provider, parse, index, nav, watch, config. **No** `ratatui` / `crossterm`. |
| `wiki-reader-render` | Markdown → `RenderedDoc` (lines, link spans, source map). |
| `wiki-reader` | Binary TUI: app state, layout, hit map, keymap. |

See [architecture overview](../architecture/overview.md) and [ADR-0006](../decisions/0006-reader-first.md).

## Dependency policy

- Pin versions in `Cargo.toml`; commit `Cargo.lock`; always `--locked`.
- Review `cargo tree` before adding a dep.
- Prefer stdlib / already-chosen crates from [prior art & libraries](../architecture/prior-art-and-libs.md).
- When porting code (e.g. markdown-reader render), keep copyright notices in the ported files and list them in [`THIRD_PARTY.md`](../../THIRD_PARTY.md).

## Docs and ADRs

- Wiki lives under `wiki/` with collection READMEs as indexes. Document standard is in [wiki/README.md](../README.md).
- ADRs under `wiki/decisions/` are immutable once accepted — supersede, don't edit.

## Task workflow

1. Open the current phase file under [roadmap](../roadmap/README.md) (Phase 1: [phase-1-reader-shell.md](../roadmap/phase-1-reader-shell.md)).
2. Pick an unchecked item; implement the smallest change that completes it.
3. Run the three checks above.
4. Check the box in the phase file when done. Leave exit criteria visible.

## Related

- [Roadmap](../roadmap/README.md)
- [Architecture](../architecture/overview.md)
