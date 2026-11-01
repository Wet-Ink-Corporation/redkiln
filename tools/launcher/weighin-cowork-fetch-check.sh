#!/bin/sh
# Checks plugin/libexec/cowork-fetch.sh, the script the skill sends to Cowork's device shell
# (D-0021), against a fake $HOME with connected folders under mnt/ and a stand-in curl.
# Each case runs the script as package-plugin.sh would stamp it. Needs sh and sha256sum only.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
script="$root/plugin/libexec/cowork-fetch.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failed=0

fail() { echo "FAIL: $*" >&2; failed=1; }

# The repository copy holds exactly one stamp, and it is the dev one.
[ "$(grep -c 'V=[^;]*; S=[^;]*;' "$script")" = 1 ] || fail "cowork-fetch.sh must hold exactly one 'V=…; S=…;'"
grep -q 'V=dev; S=none;' "$script" || fail "the repository copy of cowork-fetch.sh must stay 'V=dev; S=none;'"
if grep -q 'sha256sum -c' "$root/plugin/skills/weigh-in/SKILL.md"; then
  fail "SKILL.md still carries the fetch script inline; it must Read cowork-fetch.sh"
fi

# A stand-in weighin that reports version $1.
fake_bin() {
  mkdir -p "$(dirname "$2")"
  printf '#!/bin/sh\necho "weighin %s"\n' "$1" > "$2"
  chmod +x "$2"
}

# The release asset the stand-in curl serves, and its SHA-256.
asset="$work/asset"
fake_bin 1.2.0 "$asset"
asset_sha=$(sha256sum "$asset" | cut -d' ' -f1)

# A stand-in curl on PATH: copies $asset to the -o target, or fails when $CURL_FAILS is set.
# It records each call in $CURL_LOG.
mkdir -p "$work/path"
cat > "$work/path/curl" <<EOF
#!/bin/sh
echo "\$*" >> "\$CURL_LOG"
[ -z "\${CURL_FAILS:-}" ] || exit 22
while [ \$# -gt 0 ]; do
  if [ "\$1" = -o ]; then cp "$asset" "\$2"; exit 0; fi
  shift
done
exit 2
EOF
chmod +x "$work/path/curl"

# run <case> <version> <sha>: a fresh $HOME, the stamped script, its output in $out.
run() {
  home="$work/$1"
  mkdir -p "$home/mnt"
  sed "s/V=[^;]*; S=[^;]*;/V=$2; S=$3;/" "$script" > "$work/$1.sh"
  CURL_LOG="$work/$1.curl"
  : > "$CURL_LOG"
}
go() {
  out=$(HOME="$home" PATH="$work/path:$PATH" CURL_LOG="$CURL_LOG" sh "$work/$1.sh" 2>&1) || fail "$1: exited non-zero: $out"
}
expect() { case "$out" in *"$2"*) ;; *) fail "$1: expected '$2' in: $out" ;; esac; }
refuse() { case "$out" in *"$2"*) fail "$1: did not expect '$2' in: $out" ;; esac; }

# 1. dev, a copy in a connected folder: used whatever its version, no download.
run dev-copy dev none
fake_bin 1.1.3 "$home/mnt/repo/plugin/libexec/x86_64-unknown-linux-musl/weighin"
go dev-copy
expect dev-copy "B=$home/mnt/repo/plugin/libexec/x86_64-unknown-linux-musl/weighin"
expect dev-copy "weighin 1.1.3"
[ ! -s "$CURL_LOG" ] || fail "dev-copy: curl ran with S=none"

# 2. a release with no checksum (--no-download): the matching copy wins, another is skipped.
run match 1.2.0 none
fake_bin 1.1.3 "$home/mnt/a/plugin/libexec/x86_64-unknown-linux-musl/weighin"
fake_bin 1.2.0 "$home/mnt/b/libexec/x86_64-unknown-linux-musl/weighin"
go match
expect match "skipped $home/mnt/a/plugin/libexec/x86_64-unknown-linux-musl/weighin (weighin 1.1.3; this release is 1.2.0)"
expect match "B=$home/mnt/b/libexec/x86_64-unknown-linux-musl/weighin"
[ ! -s "$CURL_LOG" ] || fail "match: curl ran with S=none"

# 3. no binary anywhere.
run none 1.2.0 none
go none
expect none "B= (no Weigh-In binary)"

# 4. a stamped release: downloads, checks the SHA-256, and installs into ~/.weighin/<version>/.
run download 1.2.0 "$asset_sha"
go download
expect download "B=$home/.weighin/1.2.0/weighin"
expect download "weighin 1.2.0"
grep -q 'releases/download/v1.2.0/weighin-x86_64-unknown-linux-musl' "$CURL_LOG" || fail "download: wrong URL: $(cat "$CURL_LOG")"
[ ! -e "$home/.weighin/1.2.0/weighin.part" ] || fail "download: left the .part file"

# 5. the next sitting in the same chat: the installed binary is reused, no download.
: > "$CURL_LOG"
go download
expect download "B=$home/.weighin/1.2.0/weighin"
[ ! -s "$CURL_LOG" ] || fail "download (again): curl ran though the binary was installed"

# 6. a checksum that does not match: nothing is installed, and a matching copy is used instead.
run mismatch 1.2.0 0000000000000000000000000000000000000000000000000000000000000000
fake_bin 1.2.0 "$home/mnt/repo/plugin/libexec/x86_64-unknown-linux-musl/weighin"
go mismatch
expect mismatch "download failed or did not match the pinned SHA-256"
expect mismatch "B=$home/mnt/repo/plugin/libexec/x86_64-unknown-linux-musl/weighin"
[ ! -x "$home/.weighin/1.2.0/weighin" ] || fail "mismatch: installed a binary that failed its checksum"

# 7. the download fails (no network): says so, and ends with no binary when there is no copy.
run offline 1.2.0 "$asset_sha"
CURL_FAILS=1 HOME="$home" PATH="$work/path:$PATH" CURL_LOG="$CURL_LOG" sh "$work/offline.sh" > "$work/offline.out" 2>&1 \
  || fail "offline: exited non-zero"
out=$(cat "$work/offline.out")
expect offline "download failed or did not match the pinned SHA-256"
expect offline "B= (no Weigh-In binary)"
refuse offline "B=$home/.weighin"

# 8. a copy that does not run is skipped and named.
run broken 1.2.0 none
mkdir -p "$home/mnt/repo/plugin/libexec/x86_64-unknown-linux-musl"
printf 'not a binary\n' > "$home/mnt/repo/plugin/libexec/x86_64-unknown-linux-musl/weighin"
go broken
expect broken "(does not run; this release is 1.2.0)"
expect broken "B= (no Weigh-In binary)"

[ "$failed" = 0 ] || exit 1
echo "cowork-fetch: ok"
