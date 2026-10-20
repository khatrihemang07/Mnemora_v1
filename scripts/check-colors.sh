#!/usr/bin/env bash
# Fails if a raw colour literal appears in src/ outside the token files (ui-system.md).
# Excluded: token definitions, the AppMark brand-mark exception, and test files.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

PATTERN='#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(|\boklch\('

hits=$(grep -rnE --include='*.svelte' --include='*.ts' --include='*.css' "$PATTERN" src \
  --exclude='tokens.css' --exclude='AppMark.svelte' --exclude='*.test.ts' --exclude='bindings.ts' || true)

if [ -n "$hits" ]; then
  echo "Raw colours found (use tokens from src/lib/theme/tokens.css):" >&2
  echo "$hits" | cut -d: -f1,2 >&2
  exit 1
fi
echo "check-colors: ok"
