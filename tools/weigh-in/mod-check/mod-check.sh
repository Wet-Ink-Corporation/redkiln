#!/bin/bash
# Weigh-In mod checks (#114): `claude plugin validate` over the plugin, its plugin tests
# (plugins/weigh-in-mod/hooks/mod/mod.test.ts: `claude plugin test` finds tests only under the
# plugin's folder), and a strict tsc pass over the module and its tests against the plugin API
# types in tools/weigh-in/mod-check/mod-types/. No model calls.
#
#   tools/weigh-in/mod-check/mod-check.sh                   # with `claude` and `tsc` on PATH
#   CLAUDE="npx --yes @anthropic-ai/claude-code@2.1.286" TSC="npx --yes -p typescript tsc" tools/weigh-in/mod-check/mod-check.sh
set -euo pipefail
cd "$(dirname "$0")/../../.."
read -r -a claude <<< "${CLAUDE:-claude}"
read -r -a tsc <<< "${TSC:-tsc}"

"${claude[@]}" plugin validate --strict plugins/weigh-in-mod
"${claude[@]}" plugin test plugins/weigh-in-mod
"${tsc[@]}" -p tools/weigh-in/mod-check/mod.tsconfig.json
