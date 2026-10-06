---
id: WR-GUIDE-DEV
title: Development
summary: Toolchain, checks, crate boundaries, dependency policy, docs conventions, and task workflow.
status: active
updated: 2026-10-05
related: []
---

# Development

How to pick up work in this repo.

## Toolchain

- Rust from [`rust-toolchain.toml`](../../rust-toolchain.toml): channel `1.98.1`, with `rustfmt` and `clippy`. The minimum supported version is the workspace `rust-version` (1.92, set by dependencies); CI compiles everything on it in the `msrv` job.
- Install [rumdl](https://github.com/rvben/rumdl) for Markdown linting and formatting: `uv tool install rumdl` (a prebuilt binary; `cargo install rumdl --locked` also works but compiles slowly). CI pins the version in `.github/workflows/ci.yml`.
- Edition 2024; `rustfmt.toml` sets `style_edition = "2024"`.
- Builds and CI use `--locked` against the committed `Cargo.lock`.

## Install

Supported path today — install the binary from git:

```bash
# Full build (default features: images / Mermaid raster)
cargo install --locked --git https://github.com/luckgrid/wiki-reader wiki-reader-tui

# Lite build (no image stack; ADR-0023)
cargo install --locked --no-default-features --git https://github.com/luckgrid/wiki-reader wiki-reader-tui
```

Tagged releases attach platform tarballs (macOS arm64 / x86_64, Linux x86_64) plus SHA256 checksums. To upgrade, replace or roll back an installed version, see [Releasing and upgrading](releasing.md), which also covers how maintainers cut, dry-run, verify and fix a release. The tag name must be `v` plus the workspace `Cargo.toml` version (for example `v0.1.N`).

crates.io metadata is prepared on the publishable crates; do not `cargo publish` until that is an intentional follow-up. `wiki-reader-tools` stays `publish = false`.

## Checks

Local mirror of CI (run before every push):

```bash
./scripts/check.sh          # default + lite
./scripts/check.sh default  # or: lite
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
cargo clippy --locked --all-targets --no-default-features -p wiki-reader-tui -p wiki-reader-render -p wiki-reader-media -- -D warnings  # lite
cargo test --locked --no-default-features -p wiki-reader-tui -p wiki-reader-render -p wiki-reader-media
cargo run --locked --quiet -p wiki-reader-tools --bin link-check
rumdl fmt --check .
rumdl check .
cargo tree -p wiki-reader-core -e normal  # must not list ratatui or crossterm (ADR-0006)
cargo tree --locked --no-default-features -p wiki-reader-tui -e normal  # must not list image, resvg, mermaid-rs-renderer, ratatui-image (ADR-0023)
```

Quickstart:

```bash
cargo run -p wiki-reader-tui -- fixtures/worked-example
# q to quit, ? for help
```

Herdr keymap check (historical Phase 1 entry criterion for P1-08; kept as a probe):

```bash
cargo run -p wiki-reader-tui --example keylog
# Run under herdr; see wiki/roadmap/spikes/p1-s1-herdr-input.md
```

### Orphaned-reader check (V22)

An orphaned reader used to burn ~100 %CPU inside `crossterm::event::poll` when the PTY master
closed (or the parent died) while the slave stayed open, and one startup race aborted the
process (SIGABRT) because `eprintln!` panics on a dead terminal. Now non-TTY stdio exits
immediately, a watchdog exits when the session leader is gone or stdin reports POLLHUP/ERR,
and error output never panics. The `check-orphan-exit` tool ([`crates/wiki-reader-tools/src/bin/check-orphan-exit/`](../../crates/wiki-reader-tools/src/bin/check-orphan-exit/main.rs)) runs the cases on
a PTY (master closed with the slave held, master closed during startup, session leader killed,
the same with `WIKI_READER_NO_WATCHDOG=1`, only the launcher exiting, no TTY) and fails unless the binary exits by itself with an exit code. `scripts/check.sh` and CI
(ubuntu and macOS) run it:

```bash
cargo build --locked -p wiki-reader-tui
cargo run --locked --quiet -p wiki-reader-tools --bin check-orphan-exit -- target/debug/wiki-reader
```

## Known issues

- **A reader can linger in macOS state `E` when its PTY master is never read (V23, P3-38).** If the
  terminal side keeps the master open but stops reading, the exiting process can stay in the
  uninterruptible `E` state (`(wiki-reader)` in `ps`) and ignores SIGKILL until the master is
  closed or read. A real terminal emulator always reads, so this has only shown up in test
  harnesses; `check-orphan-exit` drains the master for that reason. Cause not confirmed,
  fix not attempted: see [the register](../roadmap/audit-v0.1.8.md).

## Crate boundaries

| Crate | Responsibility |
|-------|----------------|
| `wiki-reader-core` | Provider, parse, index, nav, watch, config. **No** `ratatui` / `crossterm`. |
| `wiki-reader-render` | Markdown → `RenderedDoc` (lines, link spans, source map, media inventory). |
| `wiki-reader-media` | Image decode and Mermaid/SVG rasterisation behind `raster`; never depends on render. |
| `wiki-reader-tui` | Binary TUI (`[[bin]]` name `wiki-reader`): app state, layout, hit map, keymap. |
| `wiki-reader-tools` | Repo tooling (`link-check`); `publish = false`. |

See [architecture overview](../architecture/overview.md) and [ADR-0006](../decisions/0006-reader-first.md).

## Dependency policy

- Pin versions in `Cargo.toml`; commit `Cargo.lock`; always `--locked`.
- Review `cargo tree` before adding a dep.
- Prefer stdlib / already-chosen crates from [prior art & libraries](../architecture/prior-art-and-libs.md).
- Markdown is checked and formatted with [rumdl](https://github.com/rvben/rumdl); run `rumdl fmt .` when making documentation changes (`rumdl fmt --check .` only reports).
- No third-party source is vendored or ported. If that changes, keep copyright notices in the ported files and add the notices to `THIRD-PARTY-LICENSES` in the same change.
- **Licences and advisories:** [`deny.toml`](../../deny.toml) allows permissive licences only and records the two accepted unmaintained-crate advisories with a reason and a revisit date. `cargo deny check` runs in CI (`cargo install --locked cargo-deny` to run it locally; `scripts/check.sh` runs it when installed).
- **Third-party notices:** release tarballs ship [`THIRD-PARTY-LICENSES`](../../THIRD-PARTY-LICENSES) (every package in the shipped dependency graph, plus the embedded Noto Sans font under the SIL OFL). Regenerate it after any dependency change with `scripts/gen-third-party-licenses.sh` (needs `cargo install --locked cargo-about --features cli`); `link-check` fails when a direct dependency is missing from it.
- **Licence files in crates:** `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE` are copied into each published crate directory (crates.io packages only the crate). `link-check` fails when a copy differs from the root file.
- **Minimum Rust version:** the workspace `rust-version` is the lowest toolchain that compiles everything; the `msrv` CI job checks it. Raise it deliberately when a dependency needs a newer compiler.

## Docs and ADRs

- Wiki lives under `wiki/` with collection READMEs as indexes. Document standard is in [wiki/README.md](../README.md).
- ADRs under `wiki/decisions/` are immutable once accepted — supersede, don't edit; mechanical metadata or formatting fixes that don't change decision text are allowed.

## Screenshots

All documentation screenshots live in `wiki/assets/`: the images are inside the collection root so wiki pages can show them when the reader is opened on `wiki/` (images outside the root are not rendered, [ADR-0017](../decisions/0017-static-local-images-only.md)). The root README and the crates.io README link to the same files. They are not test fixtures (those live under `fixtures/` and stay tiny); retake them when the UI changes.

- `wiki-reader.png`: the main layout, in the root README and the [UI spec](../product/ui-spec.md).
- `wiki-reader-help.png`, `wiki-reader-search.png`: the Help and Search popups, in the UI spec.
- `wiki-reader-options.png`: the options window, in the UI spec and the [configuration guide](configuration.md).
- `wiki-reader-table-viewer.png`, `wiki-reader-table-viewer-filter.png`: the table viewer with the focused row expanded, and with a `/` filter applied, in the UI spec.
- `wiki-reader-diagram-viewer.png`, `wiki-reader-diagram-viewer-zoom.png`, `wiki-reader-media-viewer.png`: the diagram viewer at fit and at 200 % zoom, and the image viewer over a picture slot, in the UI spec.

## Task workflow

1. Open the current phase file under [roadmap](../roadmap/README.md). Active polish work is [Phase 3](../roadmap/phase-3-alpha.md); [Phase 2](../roadmap/phase-2-mvp.md) is done (closed 2026-10-06); dogfood findings go to Phase 3.
2. Pick a `todo` row; set Status to `doing`; implement the smallest change that completes it.
3. Run `./scripts/check.sh` (the same checks CI runs).
4. Set Status to `done`; reference the ID in commit messages. Leave exit criteria visible.

Task-table Status (`todo` / `doing` / `done`) is not the same as a page's frontmatter `status` (`draft` / `proposed` / `accepted` / …). See [wiki README](../README.md) for the document enum.

## Related

- [Roadmap](../roadmap/README.md)
- [Architecture](../architecture/overview.md)
