#!/usr/bin/env bash
# Check that all publicly re-exported Rust names from lib.rs are mentioned in API.md.
# Fails CI if something is exported but not documented.

set -euo pipefail

LIB="rs/tempo-reconcile/src/lib.rs"
API_DOC="docs-public/API.md"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

missing=0

# Extract bare names from `pub use ...` lines.
# Handles both `pub use foo::Bar;` and `pub use foo::{Bar, Baz};`.
names=$(grep -E '^pub use ' "$ROOT/$LIB" \
  | sed 's/.*:://g; s/[{};,]/ /g' \
  | tr ' ' '\n' \
  | sed 's/^ *//; s/ *$//' \
  | grep -E '^[A-Za-z_][A-Za-z0-9_]+$' \
  | grep -v '^$' \
  | sort -u || true)

for name in $names; do
  if ! grep -qF "$name" "$ROOT/$API_DOC"; then
    echo "MISSING in API.md: $name"
    missing=$((missing + 1))
  fi
done

if [ "$missing" -gt 0 ]; then
  echo ""
  echo "Found $missing Rust public item(s) not mentioned in $API_DOC."
  echo "Update docs-public/API.md to include documentation for all public Rust exports."
  exit 1
fi

echo "Rust docs sync OK: all public exports mentioned in API.md"
