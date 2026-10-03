#!/usr/bin/env bash
# Rust източниците на ytbuch са до 800 реда. Копираният Next.js не влиза тук.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LIMIT=800
fail=0

while IFS= read -r f; do
  n=$(wc -l < "$f")
  if [[ $n -gt $LIMIT ]]; then
    echo "filesize: FAIL ${f#"$ROOT"/} — $n реда (> $LIMIT)"
    fail=1
  fi
done < <(find "$ROOT/backend/src" -name '*.rs' -type f | sort)

if [[ $fail -ne 0 ]]; then
  exit 1
fi
echo "filesize: OK — Rust файловете в backend/src са ≤ $LIMIT реда"
