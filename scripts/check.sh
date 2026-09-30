#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked --quiet -p wiki-reader-tools --bin link-check
rumdl fmt --check .
rumdl check .

tree=$(cargo tree -p wiki-reader-core -e normal)
echo "$tree"
if echo "$tree" | grep -E '(^| )(ratatui|crossterm)( |$)'; then
  echo "wiki-reader-core must not depend on ratatui or crossterm (ADR-0006)" >&2
  exit 1
fi
