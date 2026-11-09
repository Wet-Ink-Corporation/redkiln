#!/bin/sh
# Tests for the sh launcher. Run: sh tools/launcher/test.sh (CI runs it on Linux and macOS).
set -u
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)              first=x86_64-unknown-linux-musl;  second=x86_64-unknown-linux-gnu ;;
  Linux-aarch64|Linux-arm64) first=aarch64-unknown-linux-musl; second=aarch64-unknown-linux-gnu ;;
  Darwin-arm64)              first=aarch64-apple-darwin;       second="" ;;
  Darwin-x86_64)             first=x86_64-apple-darwin;        second="" ;;
  *) echo "launcher test: no targets for $(uname -s)-$(uname -m); skipped"; exit 0 ;;
esac
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failed=0
check() { # name, expected, actual
  if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: expected [$2], got [$3]"; failed=1; fi
}
# fake <path> <label> <exit>: a build that prints its label and arguments, then exits.
fake() {
  mkdir -p "$(dirname "$1")"
  printf '#!/bin/sh\necho "%s $*"\nexit %s\n' "$2" "$3" > "$1"
  chmod +x "$1"
}
plugin() { # a fresh libexec with the launcher installed as `demo`
  rm -rf "$work/libexec" "$work/path"
  mkdir -p "$work/libexec" "$work/path"
  cp "$here/launcher" "$work/libexec/demo"
  chmod +x "$work/libexec/demo"
}
run() { PATH="$work/path:/usr/bin:/bin" "$work/libexec/demo" "$@"; }

plugin
fake "$work/libexec/$first/demo" first 3
out=$(run a 'b c'); code=$?
check "runs this platform's build with its arguments" "first a b c" "$out"
check "passes the build's exit code through" 3 "$code"

if [ -n "$second" ]; then
  plugin
  fake "$work/libexec/$first/demo" first 0
  fake "$work/libexec/$second/demo" second 0
  touch -t 202001010000 "$work/libexec/$first/demo"
  check "a newer later target wins" "second x" "$(run x)"
  plugin
  fake "$work/libexec/$second/demo" second 0
  fake "$work/libexec/$first/demo" first 0
  touch -t 202001010000 "$work/libexec/$second/demo"
  check "the first target wins when newer" "first x" "$(run x)"
fi

plugin
fake "$work/path/demo" onpath 0
check "falls back to a build on PATH" "onpath x" "$(run x)"

plugin
out=$(PATH="$work/libexec:/usr/bin:/bin" "$work/libexec/demo" hook PreToolUse); code=$?
check "skips its own directory on PATH" "0:" "$code:$out"

plugin
out=$(run hook PreToolUse --fail-closed); code=$?
check "a hook with no build exits 0 silently" "0:" "$code:$out"
out=$(CLAUDE_CODE_ENTRYPOINT=cli run hook SessionStart); code=$?
case "$out" in
  '{"systemMessage":"demo is off: '*) shape=warns ;;
  *) shape="$out" ;;
esac
check "SessionStart with no build warns and exits 0" "0:warns" "$code:$shape"
out=$(CLAUDE_CODE_ENTRYPOINT=remote_cowork run hook SessionStart); code=$?
check "SessionStart in Cowork stays silent" "0:" "$code:$out"
run status 2>/dev/null; code=$?
check "a plain call with no build exits 127" 127 "$code"

# A source copy (no build for any platform) names the release repository and the build command;
# a copy with another platform's build only says this platform has none.
plugin
out=$(CLAUDE_CODE_ENTRYPOINT=cli run hook SessionStart)
case "$out" in
  *'source copy with no demo builds'*'Wet-Ink-Corporation/redkiln-plugins'*'cargo xtask build.'*) shape=source ;;
  *) shape="$out" ;;
esac
check "SessionStart in a source copy names the release repository" source "$shape"
err=$(run status 2>&1 >/dev/null)
case "$err" in
  *'source copy with no demo builds'*'cargo xtask build'*) shape=source ;;
  *) shape="$err" ;;
esac
check "a plain call in a source copy names the build command" source "$shape"
plugin
fake "$work/libexec/x86_64-pc-windows-msvc/demo.exe" other 0
out=$(CLAUDE_CODE_ENTRYPOINT=cli run hook SessionStart)
case "$out" in
  *'source copy'*) shape="$out" ;;
  '{"systemMessage":"demo is off: no demo build for '*'Install a release that ships one."}') shape=platform ;;
  *) shape="$out" ;;
esac
check "another platform's build is not a source copy" platform "$shape"

exit $failed
