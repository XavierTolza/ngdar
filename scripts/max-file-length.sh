#!/usr/bin/env bash
# Pre-commit hook: enforce max file length in src/
MAX=600
RC=0
shopt -s globstar nullglob
for f in src/**/*.rs; do
  [ -f "$f" ] || continue
  lines=$(wc -l < "$f")
  if [ "$lines" -gt "$MAX" ]; then
    echo "ERROR: $f has $lines lines (max $MAX)"
    RC=1
  fi
done
exit $RC
