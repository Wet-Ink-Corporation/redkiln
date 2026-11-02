#!/bin/sh
# ID-09 conformance: the patch-id method's pinned recipe gives one id whatever
# the user's git configuration, attributes, replace refs, grafts and diff
# environment, and gives no id when the diff cannot be taken whole.
#
# Builds a small repository whose change is sensitive to every input the
# recipe pins (two nearby hunks, content where myers and histogram differ, a
# block the indent heuristic slides, blank context lines, a rename, a
# non-ASCII path, a binary file, whitespace-only lines, a gitlink and a
# subdirectory). Each hostile case then checks two things: the recipe's id
# still equals expected.txt, and, as its control, a plain `git diff` under the
# same case gives another id than a plain `git diff` with none, so a case that
# changes nothing is caught. Cases that move only the diff's bytes (file
# order, abbreviation, mnemonic prefixes, patch-id's own settings) have no
# control, since patch-id --verbatim ignores them by construction, and nor
# does diff.srcPrefix, which git before 2.45 does not read. The case's -c
# overrides reach patch-id too, which is what makes the patchid.* case mean
# something. A missing object must give no id, never a smaller change's.
#
# Needs git 2.39 or later (--verbatim implies --stable) and POSIX sh.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
expected=$(cat "$here/expected.txt")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
trap 'exit 1' HUP INT TERM

# Nothing from the caller's git environment reaches the fixture.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY \
  GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_COMMON_DIR GIT_CONFIG GIT_CONFIG_PARAMETERS \
  GIT_CONFIG_COUNT GIT_DIFF_OPTS GIT_EXTERNAL_DIFF GIT_ATTR_SOURCE GIT_NAMESPACE \
  GIT_REPLACE_REF_BASE GIT_NO_REPLACE_OBJECTS GIT_CEILING_DIRECTORIES
export GIT_CONFIG_NOSYSTEM=1 GIT_ATTR_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null HOME="$work"
export GIT_AUTHOR_NAME=fixture GIT_AUTHOR_EMAIL=fixture@example.invalid
export GIT_COMMITTER_NAME=fixture GIT_COMMITTER_EMAIL=fixture@example.invalid
export GIT_AUTHOR_DATE='2026-01-01T00:00:00Z' GIT_COMMITTER_DATE='2026-01-01T00:00:00Z'

cafe="caf$(printf '\303\251').txt"
repo="$work/repo"
git init -q -b main "$repo"
cd "$repo"
git config advice.graftFileDeprecated false  # GIT_GRAFT_FILE prints it on every call
git commit -q --allow-empty -m root
root=$(git rev-parse HEAD)
mkdir sub
printf '{\n  one\n}\n\n{\n  three\n}\n' > blocks.src
printf 'line %s\n' 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 > notes.txt
printf '%s\n' a b c a b c X '}' '}' > algo.txt
printf 'inner\n' > sub/inner.txt
i=0; while [ "$i" -lt 400 ]; do printf 'bulk line %s of the change\n' "$i"; i=$((i + 1)); done > bulk.txt
printf 'moved content\none\ntwo\nthree\n' > old-name.txt
printf 'bonjour\n' > "$cafe"
printf '\000\001\002' > blob.bin
git add -A
git update-index --add --cacheinfo 160000,1111111111111111111111111111111111111111,module
git commit -q -m base
base=$(git rev-parse HEAD)

git checkout -q -b change
printf '{\n  one\n}\n\n{\n  two\n}\n\n{\n  three\n}\n' > blocks.src
printf 'line %s\n' 1 TWO 3 4 5 6 7 8 9 10 ELEVEN 12 13 14 15 16 > notes.txt
printf '%s\n' X a b c '}' Y a b '}' c > algo.txt
printf 'inner changed\n' > sub/inner.txt
sed 's/line/LINE/' bulk.txt > bulk.new && mv bulk.new bulk.txt
git mv old-name.txt new-name.txt
printf 'au revoir\n' >> "$cafe"
printf '\000\001\003' > blob.bin
printf '  trailing   \n\tindented\n' > white.yml
git add -A
git update-index --add --cacheinfo 160000,2222222222222222222222222222222222222222,module
git commit -q -m change
head=$(git rev-parse HEAD)
other=$(git commit-tree -p "$base" -m other "$base^{tree}")

dir=.

# The ID-09 recipe, flag for flag. Arguments are git -c overrides for the case.
# It prints the id, or exits non-zero when there is none: an ambiguous merge
# base, a failed diff or patch-id, or no change.
pinned() {
  (
    unset GIT_DIFF_OPTS GIT_EXTERNAL_DIFF
    GIT_GRAFT_FILE=/dev/null; export GIT_GRAFT_FILE
    cd "$dir"
    [ "$(git rev-parse --is-shallow-repository)" = false ] || exit 3
    bases=$(git "$@" -c core.useReplaceRefs=false merge-base --all "$base" "$head") || exit 3
    [ "$(printf '%s\n' "$bases" | wc -l)" -eq 1 ] || exit 3
    git "$@" -c core.quotePath=true -c diff.suppressBlankEmpty=false \
      -c core.useReplaceRefs=false \
      diff --no-color --no-ext-diff --no-textconv --no-renames --no-relative \
      --ignore-submodules=none --submodule=short --text --full-index -U3 \
      --inter-hunk-context=0 --diff-algorithm=myers --indent-heuristic \
      --src-prefix=a/ --dst-prefix=b/ -O/dev/null "$base...$head" > "$work/diff" 2>/dev/null \
      || exit 3
    git "$@" patch-id --verbatim < "$work/diff" > "$work/id" || exit 3
    [ "$(wc -l < "$work/id")" -eq 1 ] || exit 3
    cut -d' ' -f1 "$work/id"
  )
}

# A plain diff, for the controls.
loose() {
  (cd "$dir" && git "$@" diff "$base...$head" 2>/dev/null | git patch-id --verbatim | cut -d' ' -f1)
}

plain=$(loose)
status=0

fail() { echo "patch-id fixture: $*" >&2; status=1; }

# case <label> [git -c args]: the recipe holds, and the case is hostile.
case_() {
  label=$1; shift
  got=$(pinned "$@") || got="no id"
  [ "$got" = "$expected" ] || fail "$label: got $got, expected $expected"
  [ "$(loose "$@")" != "$plain" ] || fail "$label: control unchanged, the case tests nothing"
}

# bytes <label> [git -c args]: the recipe holds; no control.
bytes() {
  label=$1; shift
  got=$(pinned "$@") || got="no id"
  [ "$got" = "$expected" ] || fail "$label: got $got, expected $expected"
}

bytes "empty configuration"
case_ color.ui -c color.ui=always
case_ diff.renames -c diff.renames=false
case_ diff.external -c diff.external=false
case_ diff.noprefix -c diff.noprefix=true
case_ diff.context -c diff.context=10
case_ diff.interHunkContext -c diff.interHunkContext=10
case_ diff.algorithm -c diff.algorithm=histogram
case_ diff.indentHeuristic -c diff.indentHeuristic=false
case_ diff.suppressBlankEmpty -c diff.suppressBlankEmpty=true
case_ core.quotePath -c core.quotePath=false
case_ diff.submodule -c diff.submodule=log
case_ diff.ignoreSubmodules -c diff.ignoreSubmodules=all

printf '* -diff\n' > "$work/attr-nodiff"
case_ "core.attributesFile -diff" -c core.attributesFile="$work/attr-nodiff"
printf 'notes.txt diff=upper\n' > "$work/attr-textconv"
case_ "textconv driver" -c core.attributesFile="$work/attr-textconv" -c diff.upper.textconv="tr a-z A-Z <"
printf 'algo.txt diff=bin\n' > "$work/attr-bin"
case_ "driver binary" -c core.attributesFile="$work/attr-bin" -c diff.bin.binary=true

printf '*.txt -diff\n' > .git/info/attributes
case_ "info/attributes"
rm .git/info/attributes

printf '*.src binary\n' > .gitattributes
case_ "worktree .gitattributes"
rm .gitattributes

dir=sub
case_ diff.relative -c diff.relative=true
dir=.

GIT_DIFF_OPTS=-u10; export GIT_DIFF_OPTS
case_ GIT_DIFF_OPTS
unset GIT_DIFF_OPTS

GIT_EXTERNAL_DIFF=false; export GIT_EXTERNAL_DIFF
case_ GIT_EXTERNAL_DIFF
unset GIT_EXTERNAL_DIFF

git replace "$head" "$other"
case_ "replace ref"
git replace -d "$head" > /dev/null

printf '%s %s\n' "$head" "$root" > .git/info/grafts
case_ "info/grafts"
rm .git/info/grafts

# A missing object late in the diff (white.yml sorts last, after enough bulk
# for git to have flushed output): the diff dies part way, and there must be
# no id, not the id of the files before it.
blob=$(git rev-parse "$head:white.yml")
obj=".git/objects/$(printf %s "$blob" | cut -c1-2)/$(printf %s "$blob" | cut -c3-)"
mv "$obj" "$work/blob"
if got=$(pinned); then fail "missing object: got $got, expected no id"; fi
mv "$work/blob" "$obj"

# no_id <label>: the recipe must refuse the current base and head.
no_id() {
  if got=$(pinned); then fail "$1: got $got, expected no id"; fi
}
keep_base=$base keep_head=$head
tree_base=$(git rev-parse "$base^{tree}") tree_head=$(git rev-parse "$head^{tree}")

# No change: no id.
head=$base
no_id "empty change"
head=$keep_head

# A criss-cross history has two merge bases: no id, and still none when a
# replace ref makes it look linear.
xa=$(git commit-tree -p "$keep_base" -m xa "$tree_base")
xb=$(git commit-tree -p "$keep_base" -m xb "$tree_base")
base=$(git commit-tree -p "$xa" -p "$xb" -m x1 "$tree_base")
head=$(git commit-tree -p "$xb" -p "$xa" -m x2 "$tree_head")
no_id "criss-cross"
git replace "$head" "$(git commit-tree -p "$xa" -m linear "$tree_head")"
no_id "criss-cross behind a replace ref"
git replace -d "$head" > /dev/null

# A shallow boundary can move the single merge base to an older commit, so a
# shallow repository has no id. Normally the base here is keep_base and the id
# is the expected one; with h1 shallow, the base would become root.
b1=$(git commit-tree -p "$keep_base" -m b1 "$tree_base")
h1=$(git commit-tree -p "$keep_base" -m h1 "$tree_head")
side=$(git commit-tree -p "$root" -m side "$(git rev-parse "$root^{tree}")")
base=$b1
head=$(git commit-tree -p "$h1" -p "$side" -m merged "$tree_head")
bytes "moved base, not shallow"
printf '%s\n' "$h1" > .git/shallow
no_id "shallow repository"
rm .git/shallow
base=$keep_base head=$keep_head

printf 'white.yml\n*\n' > "$work/order"
bytes diff.orderFile -c diff.orderFile="$work/order"
bytes diff.mnemonicPrefix -c diff.mnemonicPrefix=true
bytes diff.srcPrefix -c diff.srcPrefix=x/ -c diff.dstPrefix=y/
bytes core.abbrev -c core.abbrev=4
bytes patchid -c patchid.stable=false -c patchid.verbatim=false

if [ "$status" -eq 0 ]; then
  echo "patch-id fixture: ok ($expected)"
fi
exit "$status"
