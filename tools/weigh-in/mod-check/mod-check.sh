#!/bin/bash
# Weigh-In mod checks (#114): `claude plugin validate` over the plugin, its plugin tests
# (plugin-mod/hooks/mod/mod.test.ts: `claude plugin test` finds tests only under the plugin's folder),
# and a strict tsc pass over the module and its tests against the plugin API types in
# tests/mod-types/. No model calls.
#
#   tests/mod-check.sh                                      # with `claude` and `tsc` on PATH
#   CLAUDE="npx --yes @anthropic-ai/claude-code@2.1.287" TSC="npx --yes -p typescript tsc" tests/mod-check.sh
set -euo pipefail
cd "$(dirname "$0")/.."
read -r -a claude <<< "${CLAUDE:-claude}"
read -r -a tsc <<< "${TSC:-tsc}"

"${claude[@]}" plugin validate --strict plugin-mod
"${claude[@]}" plugin test plugin-mod
"${tsc[@]}" -p tests/mod.tsconfig.json
