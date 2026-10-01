---
id: WR-GUIDE-RELEASING
title: Releasing and upgrading
summary: How users upgrade or replace an installed wiki-reader, and how maintainers cut, dry-run, verify and replace a release.
status: draft
updated: 2026-10-01
related: [development]
---

# Releasing and upgrading

Two audiences: people who install wiki-reader and want a newer (or older) version, and maintainers who publish releases. Releases are built by [`.github/workflows/release.yml`](../../.github/workflows/release.yml) from `v*` tags and published under [GitHub Releases](https://github.com/luckgrid/wiki-reader/releases). Versions containing `-` (for example `v0.1.0-alpha.3`) are marked as prereleases automatically.

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
# Latest main
cargo install --locked --force --git https://github.com/luckgrid/wiki-reader wiki-reader

# A specific release (also how you roll back)
cargo install --locked --force --git https://github.com/luckgrid/wiki-reader --tag v0.1.0-alpha.3 wiki-reader
```

`--force` is what replaces the already-installed binary in `~/.cargo/bin`. Confirm with `which wiki-reader` that you are not still hitting a copy elsewhere.

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

`cargo uninstall wiki-reader` for a cargo install, or delete the binary you copied into your `PATH`. Remove the config and state directories above if you also want to drop settings and sessions.

## Cut a release (maintainers)

Cut a tag only from a commit whose CI is already green on `main`. The release workflow builds and smoke-tests `--version` but does not re-run the full test suite.

1. **Prepare.** Branch `release/vX.Y.Z`, bump `version` in the workspace `Cargo.toml` and refresh `Cargo.lock`, and update the roadmap note. Run `./scripts/check.sh`, open a PR, wait for green checks, and squash-merge.
2. **Optional dry run.** In GitHub, Actions → Release → *Run workflow* on the branch or `main`. It builds all three targets, names the packages `dry-run`, and uploads them as workflow artifacts. It does **not** create or touch a GitHub Release (the attach step only runs on a tag push). Use this after changing `release.yml` or its pinned actions.
3. **Tag the merge commit** as `v` plus the `Cargo.toml` version and push the tag. The workflow fails if the tag and the crate version differ.

   ```bash
   git tag vX.Y.Z <merge-commit>
   git push origin vX.Y.Z
   ```

4. **Wait for the three builds** (linux-x86_64, macos-arm64, macos-x86_64). Each builds with `--locked`, runs `--version`, packages `wiki-reader-<tag>-<platform>.tar.gz` plus a `.sha256`, and attaches both to the release.
5. **Write the release notes.** The workflow does not generate them:

   ```bash
   gh release edit vX.Y.Z --notes-file notes.md
   ```

   Repeat the binary caveats (unsigned, macOS quarantine, Linux glibc of `ubuntu-latest`).
6. **Verify.** The release should list 3 tarballs and 3 checksums. Download one, check it with `shasum -a 256 -c`, install it, and run `wiki-reader --version`.
7. **Record it.** Add a dated line to the current phase file under [roadmap](../roadmap/README.md) (dogfood notes or the relevant task row).

## Fix or replace a published release (maintainers)

Treat a published tag as immutable once anyone may have installed it, and prefer fixing forward: merge the fix and cut the next version (for example `v0.1.0-alpha.3`). Notes can always be corrected in place:

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
