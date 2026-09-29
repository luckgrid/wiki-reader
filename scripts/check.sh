#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 scripts/link-check.py

tree=$(cargo tree -p wiki-reader-core -e normal)
echo "$tree"
if echo "$tree" | grep -E '(^| )(ratatui|crossterm)( |$)'; then
  echo "wiki-reader-core must not depend on ratatui or crossterm (ADR-0006)" >&2
  exit 1
fi
