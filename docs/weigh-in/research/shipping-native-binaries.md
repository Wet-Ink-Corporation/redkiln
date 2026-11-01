# Shipping native binaries in plugins: what applies to Weigh-In

The 2026-10-01 research on shipping a Rust binary in a Claude Code plugin now lives in temper's
repository, at [`docs/research/shipping-native-binaries/`](https://github.com/Wet-Ink-Corporation/temper/blob/main/docs/research/shipping-native-binaries/README.md), because most of it
is about temper 2's Rust port. It was written here (commits `60c5620` and `c54139a`) and
moved on 2026-10-02 ([#6](https://github.com/Wet-Ink-Corporation/weigh-in/issues/6)). These
are the findings that bear on Weigh-In, with where each one is acted on.

- **Bundling is right for Weigh-In.** The binary has to be on disk when a hook fires. A
  first-run download or build inside a short-lived hook fails offline, behind proxies, and
  silently. Weigh-In ships its binaries under `plugin/libexec/<target>/` and needs no Node,
  npm or network. The internal `marketplace` branch ([D-0009](decisions/build-decisions.md#d-0009))
  keeps that model for Claude Code installs.
- **Cowork runs code on x86_64 in both places**, the cloud workspace and the computer's shell,
  so the `x86_64-unknown-linux-musl` build covers it. No Linux arm64 build is needed.
- **Synced plugins get no npm install.** Claude Code installs a plugin's npm dependencies only
  when it copies the plugin into its cache, which a claude.ai-synced plugin never goes through.
  So in Cowork the binary must come from the zip itself, as Weigh-In's does.
- **Executable bits survive the upload and the sync** (observed; no documentation covers it).
  The launcher also runs `chmod +x` when the bit is missing.
- **Public release assets download in both Cowork places.** Only the repository's visibility
  keeps the SHA-256-pinned Cowork download ([D-0003](decisions/build-decisions.md#d-0003)) dormant
  ([D-0012](decisions/build-decisions.md#d-0012)).
- **Plugin size limits** are 200 MB uncompressed and 5,000 files, far above three bundled
  binaries of about 4.5 MB each.
- **Hooks fail silently when the binary is missing.** That is acceptable for a ledger that can
  lose a capture, but the user should be told
  ([#9](https://github.com/Wet-Ink-Corporation/weigh-in/issues/9)).
- **`sh` hooks fail on Windows without Git Bash**, with a non-blocking error
  ([#21](https://github.com/Wet-Ink-Corporation/weigh-in/issues/21)).
- **`${CLAUDE_PLUGIN_ROOT}` in synced skills has been unreliable.** Cowork's changelog fixed
  skill commands running with it blank; hooks received it in the probe. The Cowork route in
  `SKILL.md` already finds the binary without it. Watch for this when closing V1
  ([#16](https://github.com/Wet-Ink-Corporation/weigh-in/issues/16)).
- **Persistence of `${CLAUDE_PLUGIN_DATA}` in Cowork is undocumented and unverified.** Do not
  rely on it for a cached binary.
