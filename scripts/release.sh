#!/usr/bin/env bash
set -euo pipefail
# Usage: ./scripts/release.sh 0.2.0
# Validates, creates release commit marker. Does NOT publish.
VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  echo "Usage: $0 <version>" >&2
  exit 1
fi
cd "$(dirname "$0")/.."
echo "Preparing release v$VERSION..."
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo build --release --workspace
echo "Release v$VERSION validation complete. Review changes and commit manually."
