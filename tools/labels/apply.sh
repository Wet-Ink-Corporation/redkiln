#!/usr/bin/env bash
# Apply the suite label manifest to a repository: create missing labels, update colour and
# description of existing ones. Never deletes or renames a label; retiring a repository's old
# labels is a separate, reviewed step per repository (Backlog call A, wi-cef669).
# Usage: tools/labels/apply.sh <owner/repo> [manifest]   (needs gh and python3 with PyYAML)
# DRY_RUN=1 prints what it would do.
set -euo pipefail
repo=${1:?usage: apply.sh <owner/repo> [manifest]}
manifest=${2:-$(dirname "$0")/../../.github/labels.yml}
python3 "$(dirname "$0")/check.py" "$manifest" >/dev/null
existing=$(gh api --paginate "repos/$repo/labels" --jq '.[].name')
python3 -c 'import sys, yaml
for l in yaml.safe_load(open(sys.argv[1]))["labels"]:
    print("\t".join((l["name"], l["color"], l["description"])))' "$manifest" |
while IFS=$'\t' read -r name color description; do
  if grep -qxF "$name" <<<"$existing"; then
    verb=update; method=PATCH; path="repos/$repo/labels/$(python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "$name")"
  else
    verb=create; method=POST; path="repos/$repo/labels"
  fi
  if [[ -n "${DRY_RUN:-}" ]]; then echo "$verb $name"; continue; fi
  if [[ $verb == create ]]; then
    gh api -X POST "$path" -f name="$name" -f color="$color" -f description="$description" --silent
  else
    gh api -X PATCH "$path" -f color="$color" -f description="$description" --silent
  fi
  echo "$verb $name"
done
