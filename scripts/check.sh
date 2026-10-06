#!/usr/bin/env bash
# Usage: scripts/check.sh [all|default|lite]   (no args = all)
#   default  fmt, clippy, tests, orphan-exit check (V22), link-check, rumdl, ADR-0006 tree
#   lite     clippy, tests, and the no-image-deps tree check with --no-default-features (ADR-0023)
set -euo pipefail
cd "$(dirname "$0")/.."

mode=${1:-all}
case $mode in
all | default | lite) ;;
*)
  echo "usage: $0 [all|default|lite]" >&2
  exit 2
  ;;
esac

# Crates whose behaviour changes with the `media` feature.
lite_pkgs=(-p wiki-reader-tui -p wiki-reader-render -p wiki-reader-media)

if [[ $mode != lite ]]; then
  command -v rumdl >/dev/null || {
    echo "rumdl not found; install it with: uv tool install rumdl" >&2
    exit 1
  }

  cargo fmt --check
  cargo clippy --locked --all-targets -- -D warnings
  cargo test --locked
  command -v python3 >/dev/null || {
    echo "python3 not found; it runs scripts/check-orphan-exit.py (V22)" >&2
    exit 1
  }
  cargo build --locked -p wiki-reader-tui
  python3 scripts/check-orphan-exit.py target/debug/wiki-reader
  cargo run --locked --quiet -p wiki-reader-tools --bin link-check
  rumdl fmt --check .
  rumdl check .

  tree=$(cargo tree -p wiki-reader-core -e normal)
  echo "$tree"
  if echo "$tree" | grep -E '(^| )(ratatui|crossterm)( |$)'; then
    echo "wiki-reader-core must not depend on ratatui or crossterm (ADR-0006)" >&2
    exit 1
  fi
fi

if [[ $mode != default ]]; then
  cargo clippy --locked --all-targets --no-default-features "${lite_pkgs[@]}" -- -D warnings
  cargo test --locked --no-default-features "${lite_pkgs[@]}"

  tree=$(cargo tree --locked --no-default-features -p wiki-reader-tui -e normal)
  if echo "$tree" | grep -E '(^|[^A-Za-z0-9_-])(image|resvg|mermaid-rs-renderer|ratatui-image) v'; then
    echo "lite build must not pull image, resvg, mermaid-rs-renderer or ratatui-image (ADR-0023)" >&2
    exit 1
  fi
fi
