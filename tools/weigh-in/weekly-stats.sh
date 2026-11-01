#!/bin/sh
# Weekly Weigh-In numbers for #23, as markdown to paste into the issue: `weighin stats` for each
# folder given (a git repository or a Cowork folder), a pooled row, and the hook-error count of
# each ledger. A folder's row folds its ledger with the journals committed under
# `.kb/_ledger/journal/` by cloud sessions (B-21), whose ledgers are gone with their containers.
# Stats are cumulative over a ledger's life, so each week's comment carries totals to date; the
# change since last week is the difference between two comments.
#
#   scripts/weekly-stats.sh ~/src/weigh-in ~/src/other-repo ~/Cowork/folder
#
# The binary is $WEIGHIN, else the newest installed plugin's launcher, else `weighin` on PATH.
# Folders whose ledger was moved with `init --ledger` are read through that setting by weighin,
# but their hook-error count is looked for at the default location only.
#
# Each row is a folder's own ledger, so $WEIGHIN_STORE is ignored here: it names one ledger for
# every folder, which would repeat in each row and be counted again in the pooled row. To include
# a ledger kept outside a folder, set it with `init --ledger` in that folder instead.
set -eu
if [ -n "${WEIGHIN_STORE:-}" ]; then
  echo "note: ignoring \$WEIGHIN_STORE ($WEIGHIN_STORE); each folder is read through its own ledger" >&2
fi
unset WEIGHIN_STORE

if [ $# -eq 0 ]; then
  echo "usage: $0 <folder>..." >&2
  exit 2
fi

launcher=""
if [ -z "${WEIGHIN:-}" ]; then
  # The newest installed version, compared field by field (0.10.0 after 0.9.0), with no
  # `sort -V`, which not every sort has; set WEIGHIN to choose.
  launcher=$(for l in "$HOME"/.claude/plugins/cache/weigh-in/weigh/*/libexec/weighin; do
    [ -f "$l" ] || continue
    v=$(basename "$(dirname "$(dirname "$l")")")
    printf '%s\t%s\n' "$v" "$l"
  done | sort -t. -k1,1n -k2,2n -k3,3n | tail -n 1 | cut -f2-)
  # The organization-synced install (B-19) keeps the plugin outside the cache, under a folder
  # named for its upload: take the newest copy whose manifest names the `weigh` plugin and whose
  # libexec holds a built binary. Paths are read a line at a time, so a space in $HOME is kept.
  if [ -z "$launcher" ]; then
    launcher=$(find "$HOME/.claude" -path '*/libexec/weighin' -type f 2>/dev/null | while IFS= read -r l; do
      root=$(dirname "$(dirname "$l")")
      grep -qs '"name": *"weigh"' "$root/.claude-plugin/plugin.json" || continue
      for b in "$root"/libexec/*/weighin "$root"/libexec/*/weighin.exe; do [ -f "$b" ] && break; done
      [ -f "$b" ] || continue
      v=$(sed -n 's/.*"version": *"\([^"]*\)".*/\1/p' "$root/.claude-plugin/plugin.json" | head -n 1)
      printf '%s\t%s\n' "${v:-0}" "$l"
    done | sort -t. -k1,1n -k2,2n -k3,3n | tail -n 1 | cut -f2-)
  fi
fi
# Without a binary every row would read "stats failed"; say so once instead.
if [ -z "${WEIGHIN:-}" ] && [ -z "$launcher" ] && ! command -v weighin >/dev/null 2>&1; then
  echo "no Weigh-In binary found; set WEIGHIN=<path to weighin or its launcher>" >&2
  exit 1
fi
w() {
  if [ -n "${WEIGHIN:-}" ]; then "$WEIGHIN" "$@"
  elif [ -n "$launcher" ]; then sh "$launcher" "$@"
  else weighin "$@"
  fi
}

# One integer field from `stats --json`, which is a flat object of integers (`null` when unset).
field() {
  printf '%s' "$1" | sed -n "s/.*\"$2\":\([0-9]*\).*/\1/p"
}
pct() {
  if [ "$2" -eq 0 ]; then printf '—'; else awk -v n="$1" -v d="$2" 'BEGIN { printf "%.0f%%", 100 * n / d }'; fi
}

version=$(w --version 2>/dev/null || echo "weighin: not found")
echo "**Week of $(date -u +%Y-%m-%d)** · $version"
echo
echo "| Ledger | Presented | Action rate | Not useful | Overrides | Median to decide | Unqualified deferrals | Unreviewed defaults | Hook errors |"
echo "|---|---|---|---|---|---|---|---|---|"

tp=0 ta=0 tn=0 to=0 tk=0 td=0 tu=0 te=0
for dir in "$@"; do
  name=$(basename "$dir")
  if common=$(git -C "$dir" rev-parse --path-format=absolute --git-common-dir 2>/dev/null); then
    state="$common/weighin"
  else
    state="$dir/.weighin"
  fi
  # Reads never create a ledger; this only says "no ledger" rather than a row of zeros, unless the
  # config's `store` names the ledger elsewhere or the folder has committed journals (B-21).
  journals="$dir/.kb/_ledger/journal"
  if [ ! -f "$state/ledger.sqlite3" ] \
    && ! grep -qs '"store"' "$dir/.kb/_ledger/config.json" \
    && ! ls "$journals"/*.jsonl >/dev/null 2>&1; then
    echo "| $name | no ledger | | | | | | | |"
    continue
  fi
  if ! j=$(w --dir "$dir" stats --json 2>/dev/null); then
    echo "| $name | stats failed | | | | | | | |"
    continue
  fi
  p=$(field "$j" presented); a=$(field "$j" acted); n=$(field "$j" not_useful)
  o=$(field "$j" chosen_against_recommendation); k=$(field "$j" chosen_with_recommendation_known)
  m=$(field "$j" median_seconds_to_decide); d=$(field "$j" deferred_unqualified)
  u=$(field "$j" defaults_unacknowledged)
  log="$state/hook-errors.log"
  e=0
  [ -f "$log" ] && e=$(wc -l < "$log" | tr -d ' ')
  # Cloud sessions keep their hook errors beside their journals.
  for jl in "$journals"/*.hook-errors.log; do
    [ -f "$jl" ] && e=$((e + $(wc -l < "$jl" | tr -d ' ')))
  done
  echo "| $name | $p | $(pct "$a" "$p") | $(pct "$n" "$p") | $o of $k | ${m:+${m}s} | $d | $u | $e |"
  tp=$((tp + p)) ta=$((ta + a)) tn=$((tn + n)) to=$((to + o)) tk=$((tk + k))
  td=$((td + d)) tu=$((tu + u)) te=$((te + e))
done

echo "| **All** | **$tp** | **$(pct "$ta" "$tp")** | **$(pct "$tn" "$tp")** | $to of $tk | | $td | $tu | $te |"
echo
echo "Targets: action rate ≥60%, not useful ≤10% (above it, #25 comes into v1), at least 30 presented and two weeks."
echo "Sitting cost this week: _fill in_ (\`/cost\` at the end of a sitting, or the usage in its transcript)."
echo "Hook errors: _explain any non-zero count, or truncate the log once read._"
