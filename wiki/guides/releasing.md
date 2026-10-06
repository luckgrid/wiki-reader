---
id: WR-GUIDE-RELEASING
title: Releasing and upgrading
summary: How users upgrade or replace an installed wiki-reader, and how maintainers cut, dry-run, verify and replace a release.
status: active
updated: 2026-10-06
related: [development]
---

# Releasing and upgrading

Two audiences: people who install wiki-reader and want a newer (or older) version, and maintainers who publish releases. Releases are built by [`.github/workflows/release.yml`](../../.github/workflows/release.yml) from `v*` tags and published under [GitHub Releases](https://github.com/luckgrid/wiki-reader/releases). Tags containing `-` (such as `v0.2.0-rc.1`) are marked as GitHub prereleases automatically; every other tag is a normal release (see [Versioning](#versioning)).

## Versioning

Release tags are `v` plus the workspace `Cargo.toml` version, and the workflow fails if the two differ.

- **Pre-1.0: `0.1.x`.** Every release is `v0.1.N`, and N goes up by one each time (`v0.1.1`, `v0.1.2`, …). A fix and a feature release are numbered the same way; there are no `-alpha.N` suffixes and no point-release numbers like `.4.1` any more. The first release in this scheme is `v0.1.1`.
- **Prerelease flag.** Only tags with a `-` suffix are marked as GitHub prereleases. `v0.1.1` to `v0.1.10` were marked that way while the project was in its alpha phase, and keep the flag as history; `v0.1.11`, the first release published to crates.io, and later tags are normal releases.
- **Older tags stay as they are.** `v0.1.0-alpha.1` through `v0.1.0-alpha.5.1` were published before this rule and are never renamed. Semver orders them below `0.1.1`, so `cargo install` and upgrades behave normally.
- **Where the number is used.** `--version`, the tarball names (`wiki-reader-v0.1.N-<platform>.tar.gz`) and `cargo install --tag v0.1.N`.

## Upgrade or replace an installed version

Check what you have first:

```bash
wiki-reader --version
which wiki-reader
```

Upgrade the **same way you installed**. `cargo install` writes `~/.cargo/bin/wiki-reader`; release tarballs are usually copied to `~/.local/bin/wiki-reader`. If both exist, the first match on `PATH` is the one you run — so a successful cargo upgrade can still leave `wiki-reader --version` on the old tarball binary. Prefer one install location; remove the other copy if you switch methods.

Quit any running wiki-reader before replacing the binary.

### Installed with cargo

```bash
# Latest release on crates.io (also how you roll back: add --version 0.1.N)
cargo install --locked --force wiki-reader-tui

# Latest main
cargo install --locked --force --git https://github.com/luckgrid/wiki-reader wiki-reader-tui

# A specific release from git; tags up to v0.1.10 use the old package name
cargo install --locked --force --git https://github.com/luckgrid/wiki-reader --tag v0.1.N wiki-reader-tui
cargo install --locked --force --git https://github.com/luckgrid/wiki-reader --tag v0.1.10 wiki-reader
```

The package was renamed `wiki-reader-tui` because `wiki-reader` is taken on crates.io; the installed command is still `wiki-reader`. `cargo install --list` shows which package name your copy was installed under.

`--force` is what replaces the already-installed binary in `~/.cargo/bin`. Confirm with `which wiki-reader` that you are not still hitting a copy elsewhere.

Rolling back to an older tag can surface `unknown key` diagnostics for config keys the older binary does not know yet. The keys stay in the file; clear or ignore the diagnostics, or keep a newer binary.

### Installed from a release tarball

Prefer this path if that is how you installed: overwrite the same file `which wiki-reader` points at.

1. Download the tarball and its `.sha256` for your platform from the release page (macOS arm64, macOS x86_64, or Linux x86_64).
2. Verify and unpack:

   ```bash
   shasum -a 256 -c wiki-reader-vX.Y.Z-<platform>.tar.gz.sha256
   tar xf wiki-reader-vX.Y.Z-<platform>.tar.gz
   ```

3. Replace the old binary with the new one, in the same directory as before (any directory on your `PATH` works):

   ```bash
   install -m 0755 wiki-reader-vX.Y.Z-<platform>/wiki-reader ~/.local/bin/wiki-reader
   ```

4. Confirm with `wiki-reader --version`. On macOS, if Gatekeeper blocks a browser-downloaded binary, run `xattr -d com.apple.quarantine ~/.local/bin/wiki-reader` (binaries are unsigned and not notarized).

To go back to an older release, repeat the steps with that release's tarball (or the `--tag` form above).

### Config and saved sessions

- Config lives in `$XDG_CONFIG_HOME/wiki-reader/config.toml` (default `~/.config/wiki-reader/config.toml`) plus an optional `.wiki-reader.toml` in each collection root. Upgrading does not touch either.
- Saved sessions (open tabs, history, cursor, nav state) live in `$XDG_STATE_HOME/wiki-reader/` (default `~/.local/state/wiki-reader/`), one `<hash>.toml` file per collection root. New fields have defaults, so sessions from older versions load. If a session cannot be restored, wiki-reader starts clean and shows a notice.
- To reset saved sessions, delete the files in that state directory. Compatibility of an *older* binary with a session written by a *newer* one is not guaranteed; if a downgrade misbehaves on restore, delete the state files.

### Uninstall

`cargo uninstall wiki-reader-tui` for a cargo install (`cargo uninstall wiki-reader` if `cargo install --list` shows the old package name), or delete the binary you copied into your `PATH`. Remove the config and state directories above if you also want to drop settings and sessions.

## Cut a release (maintainers)

Cut a tag only from a commit whose CI is already green on `main`. The release workflow builds and smoke-tests `--version` but does not re-run the full test suite.

1. **Prepare.** Branch `release/vX.Y.Z` (for example `release/v0.1.N`), then bump every place that carries the version. Items marked "asserted" are checked by `link-check`:

   - workspace `version` in root `Cargo.toml` (asserted)
   - the three path-dependency versions in `[workspace.dependencies]` (`wiki-reader-core`, `wiki-reader-media`, `wiki-reader-render`) (asserted)
   - `Cargo.lock` (refresh by building or testing)
   - `integrations/herdr/herdr-plugin.toml` (asserted)
   - the Status line in root `README.md` (must name `v` + workspace version; asserted)
   - a matching `## v0.1.N` heading in the [dogfood log](../roadmap/dogfood-log.md) when the release is recorded (may be at most one patch ahead of the workspace; asserted)
   - [wiki/roadmap/README.md](../roadmap/README.md) phase table / notes as needed
   - Phase 3 intro, Done list, and P3-08 release list in [phase-3-alpha.md](../roadmap/phase-3-alpha.md)
   - the audit register "Shipped in" line when the release closes audit work
   - version/commit labels in [benchmarks.md](../architecture/benchmarks.md) when numbers are re-measured

   Run `./scripts/check.sh`, open a PR, wait for green checks, and squash-merge.
2. **Optional dry run.** In GitHub, Actions → Release → *Run workflow* on the branch or `main`. It builds all three targets, names the packages `dry-run`, and uploads them as workflow artifacts. It does **not** create or touch a GitHub Release (the attach step only runs on a tag push). Use this after changing `release.yml` or its pinned actions.
3. **Publish to crates.io** (from the merge commit, on a clean checkout). Needs a crates.io token with `publish-new` / `publish-update` scope from `cargo login`. Dry-run first, then publish all workspace crates in dependency order:

   ```bash
   cargo publish --workspace --dry-run --locked
   cargo publish --workspace --locked
   ```

   A published version cannot be overwritten, only yanked, so confirm the dry run, `cargo deny check` and green CI first. Then tag the **same** commit.
4. **Tag the merge commit** as `v` plus the `Cargo.toml` version and push the tag. The workflow fails if the tag and the crate version differ.

   ```bash
   git tag vX.Y.Z <merge-commit>
   git push origin vX.Y.Z
   ```

5. **Wait for the three builds** (linux-x86_64, macos-arm64, macos-x86_64). Each builds with `--locked`, runs `--version`, packages `wiki-reader-<tag>-<platform>.tar.gz` plus a `.sha256`, and attaches both to the release.
6. **Write the release notes.** The workflow does not generate them:

   ```bash
   gh release edit vX.Y.Z --notes-file notes.md
   ```

   Repeat the binary caveats (unsigned, macOS quarantine, Linux glibc of `ubuntu-latest`).
7. **Verify.** The release should list 3 tarballs and 3 checksums. Download one, check it with `shasum -a 256 -c`, install it, and run `wiki-reader --version`.
8. **Record it.** Add a dated line to the [dogfood log](../roadmap/dogfood-log.md) (and update the relevant task row under [roadmap](../roadmap/README.md) when a release closes work).
9. **Sweep for drift.** In the publication PR (dogfood log release row, plus the new tag in the P3-08 release list), search the current-facing docs for the previous version and for stale release words, and fix what the search finds. History files (the dogfood log, closed task rows, audit "Shipped in" lines) keep their old versions on purpose.

   ```bash
   # replace 0.1.N with the previous release
   git grep -n -E 'v?0\.1\.N\b' -- README.md SECURITY.md integrations wiki/guides wiki/product wiki/architecture
   git grep -n -i -E 'pre-?release|current release is' -- README.md SECURITY.md integrations wiki/guides wiki/product
   ```

   Also check that the README Status line, the install examples and the screenshots still describe the shipped UI, and that the known-issues list in [development](development.md#known-issues) is current.

## Fix or replace a published release (maintainers)

Treat a published tag as immutable once anyone may have installed it, and prefer fixing forward: merge the fix and cut the next version (for example `v0.1.N+1` after a bad `v0.1.N`). Notes can always be corrected in place:

```bash
gh release edit vX.Y.Z --notes-file notes.md
```

If only one asset is bad, re-run the failed build job from the workflow run (a re-run of a tag-push run attaches again and replaces assets with the same name), or upload a replacement by hand:

```bash
gh release upload vX.Y.Z wiki-reader-vX.Y.Z-<platform>.tar.gz wiki-reader-vX.Y.Z-<platform>.tar.gz.sha256 --clobber
```

Only for a release that is broken and that nobody has used yet: delete it and its tag, merge the fix, and tag again.

```bash
gh release delete vX.Y.Z --cleanup-tag
```

Deleting a release or a tag, and pushing a tag, are outward-facing and hard to undo. Do them deliberately, and tell anyone who may have installed the removed version to reinstall.

## Related

- [Development](development.md) — toolchain, checks, crate boundaries
- [Roadmap](../roadmap/README.md)
