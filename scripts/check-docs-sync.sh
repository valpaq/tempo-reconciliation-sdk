#!/usr/bin/env bash
# Check that all exported types and values from each package's index.ts are mentioned in API.md.
# Fails CI if something is exported but not documented.

set -euo pipefail

API_DOC="docs-public/API.md"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

missing=0

check_index() {
  local index="$1"
  local label="$2"

  # Extract type names from multiline "export type { ... } from ..." blocks.
  # awk collects everything between the first { and the matching }, strips commas.
  local types
  types=$(awk '
    /^export type \{/ { inside=1; next }
    inside && /\}/ { inside=0; next }
    inside { gsub(/,/, ""); gsub(/^[[:space:]]+|[[:space:]]+$/, ""); if ($0 != "") print $0 }
  ' "$ROOT/$index" || true)

  for t in $types; do
    if ! grep -q "$t" "$ROOT/$API_DOC"; then
      echo "MISSING in API.md [$label]: type $t"
      missing=$((missing + 1))
    fi
  done

  # Extract value export names from single-line "export { foo, bar } from ..." lines.
  local exports
  exports=$(grep '^export {' "$ROOT/$index" | grep -v '^export type' \
    | sed 's/^export {//;s/}.*//' | tr ',' '\n' \
    | sed 's/^ *//;s/ *$//' | grep -v '^$' || true)

  for e in $exports; do
    if ! grep -q "$e" "$ROOT/$API_DOC"; then
      echo "MISSING in API.md [$label]: export $e"
      missing=$((missing + 1))
    fi
  done
}

check_index "ts/packages/sdk/src/index.ts" "sdk"
check_index "ts/packages/nonces/src/index.ts" "nonces"

if [ "$missing" -gt 0 ]; then
  echo ""
  echo "Found $missing export(s) not mentioned in $API_DOC."
  echo "Update docs-public/API.md to include documentation for all public exports."
  exit 1
fi

echo "Docs sync OK: all exports mentioned in API.md"
