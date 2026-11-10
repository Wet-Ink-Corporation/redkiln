#!/bin/bash
# Bench cockpit checks (anneal's cockpit CI job, imported at Move 2): `claude plugin validate
# --strict` over the mod plugin, its plugin tests (plugins/bench-cockpit/hooks/mod/
# cockpit.test.ts: `claude plugin test` finds tests only under the plugin's folder), and a strict
# tsc pass over the module and its tests against the hand-written API types in
# tools/bench/cockpit-check/mod-types/. No model calls.
#
#   tools/bench/cockpit-check/cockpit-check.sh                   # with `claude` and `tsc` on PATH
#   CLAUDE="npx --yes @anthropic-ai/claude-code@2.1.316" TSC="npx --yes -p typescript tsc" tools/bench/cockpit-check/cockpit-check.sh
set -euo pipefail
cd "$(dirname "$0")/../../.."
read -r -a claude <<< "${CLAUDE:-claude}"
read -r -a tsc <<< "${TSC:-tsc}"

"${claude[@]}" plugin validate --strict plugins/bench-cockpit
"${claude[@]}" plugin test plugins/bench-cockpit
"${tsc[@]}" -p tools/bench/cockpit-check/tsconfig.json
