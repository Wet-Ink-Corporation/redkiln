#!/bin/sh
# Bench's smoke test of the shared launcher (anneal's scripts/test-launcher.sh, retargeted at
# Move 2 to tools/launcher/launcher's wording, decision M2-E). Takes the path of a built bench
# binary, and runs plugins/bench/libexec/bench, a byte copy of tools/launcher/launcher.
#  1. A build in libexec/<target>/ runs with nothing on PATH, and `hook guard --fail-closed`
#     through it denies a force push with a PreToolUse deny decision.
#  2. With no build, the launcher falls back to a bench on PATH, also when its own directory
#     comes first on PATH.
#  3. With neither, every hook exits 0, a --fail-closed guard too (a missing install warns, it
#     does not block), silently except at SessionStart, where it says Bench is off as a
#     systemMessage; a plain call exits 127.
#  4. There is no bin/ directory anywhere in either plugin, and the launchers are executable
#     (hooks start them by path with no shell).
# anneal's case 5 (download a release build) went with the download (M2-E).
set -eu
built=${1:?usage: bench-test-launcher.sh <path to a built bench binary>}
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-gnu ;;
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  *) echo "skip: the sh launcher is not used on $(uname -s)"; exit 0 ;;
esac
sh_bin=$(command -v sh)
cp -R "$root/plugins/bench" "$work/plugin"
launcher="$work/plugin/libexec/bench"
empty="$work/empty-path"
mkdir -p "$empty"
# The launcher needs a few POSIX tools; give it only those, never a bench.
for tool in uname dirname basename chmod; do
  ln -s "$(command -v "$tool")" "$empty/$tool"
done

# 1. A built binary beside the launcher runs with nothing else on PATH.
mkdir -p "$work/plugin/libexec/$target"
cp "$built" "$work/plugin/libexec/$target/bench"
PATH="$empty" "$sh_bin" "$launcher" --help | grep -q '^bench:' || fail "--help through libexec/$target"
# The guard as hooks.json runs it; the ledger is named so no git is needed.
printf '%s' '{"tool_name":"Bash","tool_input":{"command":"git push -f origin main"}}' \
  | PATH="$empty" ANNEAL_STORE="$work/ledger.sqlite3" "$sh_bin" "$launcher" hook guard --fail-closed >"$work/deny" \
  || fail "hook guard through libexec/$target must exit 0 with its decision"
grep -q '"permissionDecision":"deny","permissionDecisionReason":"ERROR guard force-push: ' "$work/deny" \
  || fail "hook guard through libexec/$target must deny a force push to main"
rm -r "$work/plugin/libexec/$target"

# 2. No build: falls back to bench on PATH.
onpath="$work/on-path"
mkdir -p "$onpath"
cp "$built" "$onpath/bench"
PATH="$onpath:$empty" "$sh_bin" "$launcher" --version | grep -q '^bench ' || fail "PATH fallback"
# With the launcher's own directory first on PATH, the fallback skips the launcher and finds
# the bench after it, both when called by path and when called by name through PATH.
PATH="$work/plugin/libexec:$onpath:$empty" "$sh_bin" "$launcher" --version | grep -q '^bench ' \
  || fail "PATH fallback past the launcher's own directory"
PATH="$work/plugin/libexec/:$onpath:$empty" bench --version | grep -q '^bench ' \
  || fail "bench by name through PATH, the launcher's directory first"

# 3. Neither: hooks and guards exit 0, SessionStart says Bench is off, plain calls exit 127.
PATH="$empty" "$sh_bin" "$launcher" hook SessionStart </dev/null >"$work/hint" || fail "a hook with no binary must exit 0"
grep -q '^{"systemMessage":"bench is off: this plugin is a source copy with no bench builds' "$work/hint" \
  || fail "SessionStart with no binary must say bench is off as a systemMessage"
if command -v python3 >/dev/null 2>&1; then
  python3 -c 'import json, sys; json.load(sys.stdin)' <"$work/hint" || fail "the notice is not valid JSON"
fi
PATH="$empty" "$sh_bin" "$launcher" hook bootstrap </dev/null >"$work/other" || fail "a hook with no binary must exit 0"
[ ! -s "$work/other" ] || fail "other hooks with no binary must stay silent"
printf '%s' '{"tool_name":"Bash","tool_input":{"command":"ls"}}' \
  | PATH="$empty" "$sh_bin" "$launcher" hook guard --fail-closed >"$work/guard" 2>&1 \
  || fail "a --fail-closed guard with no binary must exit 0, not block"
[ ! -s "$work/guard" ] || fail "a guard with no binary must stay silent"
set +e
PATH="$empty" "$sh_bin" "$launcher" --help >/dev/null 2>&1
plain=$?
set -e
[ "$plain" = 127 ] || fail "a plain call with no binary must exit 127, got $plain"

# 4. No bin/ anywhere in either plugin, and the launchers keep their executable bit.
for plugin in bench bench-cockpit; do
  [ -z "$(find "$root/plugins/$plugin" -type d -name bin)" ] || fail "plugins/$plugin ships a bin/ directory"
  [ -x "$root/plugins/$plugin/libexec/bench" ] || fail "plugins/$plugin/libexec/bench is not executable"
done
echo "launcher ok"
