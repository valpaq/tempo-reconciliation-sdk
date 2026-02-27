#!/usr/bin/env bash
# Check that all exported types and values from index.ts are mentioned in API.md.
# Fails CI if something is exported but not documented.

set -euo pipefail

INDEX="ts/src/index.ts"
API_DOC="docs-public/API.md"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

missing=0

# Extract type names from multiline "export type { ... } from ..." blocks.
# awk collects everything between the first { and the matching }, strips commas.
types=$(awk '
  /^export type \{/ { inside=1; next }
  inside && /\}/ { inside=0; next }
  inside { gsub(/,/, ""); gsub(/^[[:space:]]+|[[:space:]]+$/, ""); if ($0 != "") print $0 }
' "$ROOT/$INDEX" || true)

for t in $types; do
  if ! grep -q "$t" "$ROOT/$API_DOC"; then
    echo "MISSING in API.md: type $t"
    missing=$((missing + 1))
  fi
done

# Extract value export names from single-line "export { foo, bar } from ..." lines.
exports=$(grep '^export {' "$ROOT/$INDEX" | grep -v '^export type' \
  | sed 's/^export {//;s/}.*//' | tr ',' '\n' \
  | sed 's/^ *//;s/ *$//' | grep -v '^$' || true)

for e in $exports; do
  if ! grep -q "$e" "$ROOT/$API_DOC"; then
    echo "MISSING in API.md: export $e"
    missing=$((missing + 1))
  fi
done

if [ "$missing" -gt 0 ]; then
  echo ""
  echo "Found $missing export(s) not mentioned in $API_DOC."
  echo "Update docs-public/API.md to include documentation for all public exports."
  exit 1
fi

echo "Docs sync OK: all exports mentioned in API.md"
