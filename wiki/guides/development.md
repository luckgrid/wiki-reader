---
id: WR-GUIDE-DEV
title: Development
summary: Toolchain, checks, crate boundaries, dependency policy, docs conventions, and task workflow.
status: draft
updated: 2026-09-30
related: []
---

# Development

How to pick up work in this repo.

## Toolchain

- Rust from [`rust-toolchain.toml`](../../rust-toolchain.toml): channel `1.98.1`, with `rustfmt` and `clippy`.
- Install [rumdl](https://github.com/rvben/rumdl) for Markdown linting and formatting: `uv tool install rumdl` (a prebuilt binary; `cargo install rumdl --locked` also works but compiles slowly). CI pins the version in `.github/workflows/ci.yml`.
- Edition 2024; `rustfmt.toml` sets `style_edition = "2024"`.
- Builds and CI use `--locked` against the committed `Cargo.lock`.

## Install

Supported path today — install the binary from git:

```bash
cargo install --locked --git https://github.com/luckgrid/wiki-reader wiki-reader
```

Tagged releases attach platform tarballs (macOS arm64 / x86_64, Linux x86_64) plus SHA256 checksums. To upgrade, replace or roll back an installed version, see [Releasing and upgrading](releasing.md), which also covers how maintainers cut, dry-run, verify and fix a release. The tag name must be `v` plus the workspace `Cargo.toml` version (for example `v0.1.0-alpha.3`).

crates.io metadata is prepared on the publishable crates; do not `cargo publish` until that is an intentional follow-up. `wiki-reader-tools` stays `publish = false`.

## Checks

Local mirror of CI (run before every push):

```bash
./scripts/check.sh
```

Optional: install the repo pre-push hook so those checks run automatically:

```bash
git config core.hooksPath .githooks
```

Same steps individually:

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked --quiet -p wiki-reader-tools --bin link-check
rumdl fmt --check .
rumdl check .
cargo tree -p wiki-reader-core -e normal  # must not list ratatui or crossterm (ADR-0006)
```

Quickstart:

```bash
cargo run -p wiki-reader -- fixtures/worked-example
# q to quit, ? for help
```

Herdr keymap check (P1-G entry criterion for P1-08):

```bash
cargo run -p wiki-reader --example keylog
# Run under herdr; tick wiki/roadmap/spikes/p1-s1-herdr-input.md
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
- Markdown is checked and formatted with [rumdl](https://github.com/rvben/rumdl); run `rumdl fmt .` when making documentation changes (`rumdl fmt --check .` only reports).
- No third-party source is vendored or ported. If that changes, keep copyright notices in the ported files and add a `THIRD_PARTY.md` in the same change.

## Docs and ADRs

- Wiki lives under `wiki/` with collection READMEs as indexes. Document standard is in [wiki/README.md](../README.md).
- ADRs under `wiki/decisions/` are immutable once accepted — supersede, don't edit; mechanical metadata or formatting fixes that don't change decision text are allowed.

## Task workflow

1. Open the current phase file under [roadmap](../roadmap/README.md) (Phase 2: [phase-2-mvp.md](../roadmap/phase-2-mvp.md)).
2. Pick a `todo` row; set Status to `doing`; implement the smallest change that completes it.
3. Run `./scripts/check.sh` (the same checks CI runs).
4. Set Status to `done`; reference the ID in commit messages. Leave exit criteria visible.

## Related

- [Roadmap](../roadmap/README.md)
- [Architecture](../architecture/overview.md)
