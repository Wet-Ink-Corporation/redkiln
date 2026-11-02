#!/bin/sh
# Charter size check (field guide T6, WP-8). Codex loads every AGENTS.md from
# the repository root down to the working folder and cuts the chain at
# 32 KiB, so the deepest charter is lost first. For each folder holding an
# AGENTS.md, this sums that file and every AGENTS.md above it, and fails when
# a chain passes the budget. The frozen handover under references/ is not a
# charter and is left out. Usage: sh tools/charters/check.sh [budget-bytes]
set -eu

budget=${1:-24576}
root=$(git rev-parse --show-toplevel)
cd "$root"

# One charter per line, read whole, so a path with spaces or non-ASCII
# characters keeps its own bytes in its chain.
all=$(mktemp)
list=$(mktemp)
trap 'rm -f "$all" "$list"' EXIT
trap 'exit 1' HUP INT TERM
git -c core.quotePath=false ls-files -- 'AGENTS.md' '*/AGENTS.md' > "$all"
grep -v '^references/' "$all" > "$list" || [ "$?" -eq 1 ]
if ! grep -qx 'AGENTS.md' "$list"; then
  echo "charters: no root AGENTS.md is tracked" >&2
  exit 1
fi

status=0
while IFS= read -r charter; do
  dir=$(dirname "$charter")
  total=0
  chain=""
  while :; do
    if [ -f "$dir/AGENTS.md" ]; then
      size=$(wc -c < "$dir/AGENTS.md")
      total=$((total + size))
      chain="$dir/AGENTS.md $chain"
    fi
    [ "$dir" = . ] && break
    dir=$(dirname "$dir")
  done
  if [ "$total" -gt "$budget" ]; then
    echo "charters: $charter's chain is $total bytes, over the $budget-byte budget: $chain" >&2
    status=1
  else
    echo "charters: $charter $total bytes"
  fi
done < "$list"
exit "$status"
