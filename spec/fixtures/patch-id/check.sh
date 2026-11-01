#!/bin/sh
# ID-09 conformance: the patch-id method's pinned diff gives one id whatever
# the user's git configuration. Builds a small repository, takes the id once
# under an empty configuration and once under a hostile one, and checks both
# against expected.txt. Needs git 2.39 or later (--verbatim implies --stable).
set -eu

here=$(cd "$(dirname "$0")" && pwd)
expected=$(cat "$here/expected.txt")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null HOME="$work"
export GIT_AUTHOR_NAME=fixture GIT_AUTHOR_EMAIL=fixture@example.invalid
export GIT_COMMITTER_NAME=fixture GIT_COMMITTER_EMAIL=fixture@example.invalid
export GIT_AUTHOR_DATE='2026-01-01T00:00:00Z' GIT_COMMITTER_DATE='2026-01-01T00:00:00Z'

cafe="caf$(printf '\303\251').txt"
repo="$work/repo"
git init -q -b main "$repo"
cd "$repo"
printf 'fn main() {\n    let a = 1;\n\n    let b = 2;\n}\n' > main.src
printf 'first\nsecond\nthird\nfourth\nfifth\nsixth\nseventh\neighth\n' > notes.txt
printf 'moved content\none\ntwo\nthree\n' > old-name.txt
printf 'bonjour\n' > "$cafe"
printf '\000\001\002' > blob.bin
git add -A && git commit -q -m base
base=$(git rev-parse HEAD)

git checkout -q -b change
printf 'fn main() {\n    let a = 1;\n\n    let b = 3;\n    if a < b {}\n}\n' > main.src
printf 'first\nSECOND\nthird\nfourth\nfifth\nsixth\nSEVENTH\neighth\n' > notes.txt
git mv old-name.txt new-name.txt
printf 'au revoir\n' >> "$cafe"
printf '\000\001\003' > blob.bin
printf '  trailing   \n\tindented\n' > white.yml
git add -A && git commit -q -m change
head=$(git rev-parse HEAD)

# The ID-09 recipe, flag for flag. Extra arguments are git -c overrides.
pinned() {
  git "$@" -c core.quotePath=true -c diff.suppressBlankEmpty=false \
    diff --no-color --no-ext-diff --no-textconv --no-renames --no-relative \
    --ignore-submodules=none --submodule=short --binary -U3 \
    --inter-hunk-context=0 --diff-algorithm=myers --indent-heuristic \
    --src-prefix=a/ --dst-prefix=b/ -O/dev/null "$base...$head" \
  | git "$@" patch-id --verbatim | cut -d' ' -f1
}

clean=$(pinned)
hostile=$(pinned -c color.ui=always -c color.diff=always -c diff.renames=copies \
  -c diff.external=false -c diff.noprefix=true -c diff.mnemonicPrefix=true \
  -c diff.context=10 -c diff.interHunkContext=10 -c diff.algorithm=histogram \
  -c diff.indentHeuristic=false -c diff.suppressBlankEmpty=true \
  -c core.quotePath=false -c diff.relative=true -c diff.submodule=log \
  -c diff.orderFile=/dev/null -c patchid.stable=false -c patchid.verbatim=false)

# Control: the unpinned diff under the same configuration gives another id,
# so the configuration really is hostile.
loose=$(git -c diff.renames=copies -c diff.context=10 -c diff.noprefix=true \
  -c diff.algorithm=histogram -c core.quotePath=false \
  diff "$base...$head" | git patch-id --verbatim | cut -d' ' -f1)

status=0
if [ "$loose" = "$expected" ]; then
  echo "patch-id fixture: the control matched; the hostile configuration changes nothing" >&2
  status=1
fi
for got in "$clean" "$hostile"; do
  if [ "$got" != "$expected" ]; then
    echo "patch-id fixture: got $got, expected $expected" >&2
    status=1
  fi
done
if [ "$status" -eq 0 ]; then
  echo "patch-id fixture: ok ($expected)"
fi
exit "$status"
