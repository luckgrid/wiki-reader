#!/usr/bin/env bash
# Regenerate THIRD-PARTY-LICENSES: licences of the Rust packages in the shipped dependency graph
# (cargo-about, see about.toml / about.hbs) plus the embedded Noto Sans font (SIL OFL 1.1).
# Needs `cargo install --locked cargo-about --features cli`. Run after any dependency change;
# `link-check` fails when a direct dependency is missing from the file.
set -euo pipefail
cd "$(dirname "$0")/.."

command -v cargo-about >/dev/null || {
  echo "cargo-about not found; install it with: cargo install --locked cargo-about --features cli" >&2
  exit 1
}

out=THIRD-PARTY-LICENSES
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

cargo about generate --locked about.hbs -o "$tmp"
{
  cat "$tmp"
  echo "================================================================================"
  echo "Noto Sans (embedded font, crates/wiki-reader-media/fonts/NotoSans.ttf)"
  echo "================================================================================"
  echo
  cat crates/wiki-reader-media/fonts/OFL.txt
} >"$out"
echo "wrote $out ($(wc -c <"$out" | tr -d ' ') bytes)"
