# wiki-reader

A terminal wiki reader for markdown collections. It browses like a documentation site (side nav, breadcrumbs, working links, back/forward, prev/next), sized to live in a herdr pane next to your work.

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ Project Wiki › Architecture › Design System › Token Projection          ◫  ✕ │
├──────────────────────────────┬───────────────────────────────────────────────┤
│ ⌕ Search…               /    │ # Token Projection                            │
│ ● Project Wiki               │                                               │
│ ▾ Architecture               │ The reusable adapter stays semantic-only; see │
│     Architecture Overview    │ [ADR-0003](../decisions/0003.md) for why.     │
│   ▾ Design System            │                                               │
│       Design System          │▌                                              │
│     ● Token Projection       │                                               │
│     Workflow OS              ├───────────────────────────────────────────────┤
│ ▸ Decisions                  │ ‹ Design System                  Adapters ›   │
├──────────────────────────────┴───────────────────────────────────────────────┤
│ VIEWER · architecture/design-system/tokens.md · L42 38% · 1,284 w · 6 min    │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Why not an existing reader?

Existing terminal markdown tools behave like editors or file browsers: opening a page spawns a tab, links are inert, and search lives in a modal. wiki-reader uses a **browser/wiki navigation model**. Every way of reaching a page replaces the current view and records history, and tabs are an opt-in secondary feature. See [ADR-0005](wiki/decisions/0005-navigation-model.md).

## Status

Phase 1 is closed. Phase 2 dogfood clock started 2026-09-29 on a real collection. P3-08 install path and release binaries are shipped; later Phase 3 rows wait on dogfood notes. The reader shell, backlinks, heading jumps, and `$EDITOR` support are shipped. Start at [wiki/README.md](wiki/README.md) and see the [roadmap](wiki/roadmap/README.md) for the next tasks.

## Install

From git (supported today):

```bash
cargo install --locked --git https://github.com/luckgrid/wiki-reader wiki-reader
```

Release binaries (macOS arm64 / x86_64, Linux x86_64) ship on `v*` tags under [GitHub Releases](https://github.com/luckgrid/wiki-reader/releases). Download the matching `.tar.gz`, verify the checksum, and put `wiki-reader` on your `PATH`:

```bash
shasum -a 256 -c wiki-reader-vX.Y.Z-<platform>.tar.gz.sha256
tar xf wiki-reader-vX.Y.Z-<platform>.tar.gz
```

Binaries are unsigned and not notarized. Browser downloads on macOS may be quarantined; clear with `xattr -d com.apple.quarantine path/to/wiki-reader` if Gatekeeper blocks them (`curl` downloads usually skip quarantine). The Linux binary is built on `ubuntu-latest` and links that runner's glibc, so older distros may need to build from source or use `cargo install`.

crates.io packaging metadata is prepared (`version` on path deps, repository/readme); the crates are not published yet.

## Upgrade

Check your version with `wiki-reader --version`. To upgrade, replace the old binary:

```bash
# cargo install: --force replaces the installed binary (add --tag vX.Y.Z-alpha.N to pin one)
cargo install --locked --force --git https://github.com/luckgrid/wiki-reader wiki-reader

# release tarball: verify and unpack as above, then copy over the old binary
install -m 0755 wiki-reader-vX.Y.Z-<platform>/wiki-reader ~/.local/bin/wiki-reader
```

Quit any running instance first. Your config and saved sessions are kept. Rolling back, uninstalling, resetting sessions, and replacing a published release are covered in [Releasing and upgrading](wiki/guides/releasing.md).

## Quickstart

```bash
cargo run -p wiki-reader -- fixtures/worked-example
# q to quit, ? for help
```

Or after install:

```bash
wiki-reader fixtures/worked-example
# q to quit, ? for help
```

## Checks

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked --quiet -p wiki-reader-tools --bin link-check
rumdl fmt --check .
rumdl check .
```

## Workspace

Cargo workspace with `wiki-reader-core` (terminal-free), `wiki-reader-render`, and the `wiki-reader` TUI binary. Docs live in `wiki/`. See [guides/development.md](wiki/guides/development.md).

## License

Copyright © 2026 LUCKGRID. Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. The two licenses are alternatives: users need to comply with only the one they choose.
