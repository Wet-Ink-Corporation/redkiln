# Plugins and surfaces

A plugin is a thin shell: manifest, hooks, skills, agents, a launcher and its binaries. `AGENTS.md`, CLI first, harness second (checked by `cargo xtask plugins`) says why it stays thin; this chapter is about what the shell must get right to load at all, on every surface where a person meets it. The org-sync spike (`redkiln-v1/org-sync-spike/README.md`, wi-f041fb) is the companion record for claude.ai.

## The surfaces

Each row was observed in at least one repository between September and November 2026. Surfaces change faster than any document; re-check before relying on a row.

| Surface | `CLAUDE_CODE_ENTRYPOINT` | Hooks | Skills | Notes |
| --- | --- | --- | --- | --- |
| Claude Code CLI and desktop Code tab | `cli` and friends | yes | yes | The only surface CI exercises |
| Claude Code cloud session | | yes | yes | Clones fresh; repository `enabledPlugins` and `extraKnownMarketplaces` are ignored, but `.claude/settings.json` hooks and `.claude/skills/` load; org-synced plugins also load, without `libexec/` builds |
| Projects thread | `remote_projects` | yes | yes | No `AskUserQuestion`; decisions arrive as later turns; the ledger dies with the task |
| Cowork | `remote_cowork` | in a scratch cloud workspace | yes | Edits to the user's folder go through the device shell, so `PostToolUse` never sees them; `!` injection is replaced by a placeholder; connected folders refuse deletes |
| Desktop chat with a folder | | none | yes | Turns the **whole plugin off** if it ships a mod; refuses commands with `disable-model-invocation: true`; a failed `!` injection stops the command loading |

Evidence: weigh-in B-04, D-0002, D-0018, B-34, B-35, B-12, CHANGELOG 1.1.1 to 1.1.3; anneal PR 140; temper `docs/troubleshooting.md:42-58`.

## Guardrails

### PS-1 No `bin/` in a plugin; binaries live in `libexec/<target>/` behind a launcher

claude.ai rejects a hosted plugin with a top-level `bin/`, because Claude Code puts it on PATH outside the approval surface. Packaging refuses `bin/` twice: when the zip is built and when the marketplace tree is built. `libexec/` takes an allowlist (the launchers `<binary>` and `<binary>.cmd`, `<target>/<binary>[.exe]`, and the root `<binary>.exe` that Windows exec-form hooks would need, which `cargo xtask plugins` does not allow yet, [Tension T9](README.md#tensions-with-the-plan), [HK-3](hooks.md#hk-3-hooks-are-exec-form-and-call-one-binary-by-path)), not whatever a job downloaded.

- **Why.** A rerun of weigh-in's release job downloaded every artifact of the run into `libexec/`, including a failed attempt's assets; the zip came out at 32 MB instead of 13.
- **Evidence.** weigh-in B-03, B-15, `scripts/marketplace-branch.sh`; redkiln `AGENTS.md`, Plugin rules.

### PS-2 No mod in a plugin that ships skills or commands

A plugin whose `hooks.json` names `modules` is switched off entirely in the desktop app's chats ("this plugin entry is off here"), skills and commands included, although the docs say chat ignores hooks. Mods ship in their own plugin, with their own copy of the launcher, because a plugin cannot reach another plugin's root. CI refuses `modules` in any plugin that is not a mod plugin, and refuses `types` in its `plugin.json`.

- **Why.** It broke temper, weigh-in and anneal in the same week, and was isolated by Ryan with two uploaded test copies. The weigh-in spike before it had defaulted to the convenient placement and left the failing surface to a later human run.
- **Evidence.** weigh-in B-35, B-29, `scripts/marketplace-branch.sh`; temper `tests/ci.sh:50-54`; anneal PR 194; upstream anthropics/claude-code#103029.

### PS-3 Duplicated launchers and versions are byte-checked

Every copy of the shared launcher is `cmp`-identical to `tools/launcher/`, and every version site of a package agrees (Cargo, each `plugin.json`, the CHANGELOG heading). Vendored copies of anything are checked in CI, never synced on trust. This applies in the repository that owns the copies; in an adopted repository a changed vendored copy is flagged, not failed ([CR-3](ci-and-release.md#cr-3-ci-flags-not-fails-on-changes-only-a-human-may-make-where-codeowners-would-deadlock)).

- **Why.** temper's byte check caught a CodeQL autofix that edited only the lens plugin's vendored copy of a file.
- **Evidence.** anneal `scripts/check-versions.sh`; temper `tests/ci.sh:40-48`, `@73f2ff8`; redkiln `.github/workflows/ci.yml` (launcher copies).

### PS-4 A marketplace install moves only when the version moves

The plugin cache is keyed on the version directory. A fix merged without a version bump never reaches existing installs, while new installs and org sync read the default branch directly. So the default branch must always be releasable, version sync is a test, and a tag is never cut while CI is red.

- **Evidence.** temper `AGENTS.md` ("`main` is what users get"); redkiln-old `test/version-sync.test.ts`, readiness ruling C3 (v0.15.0 tagged by hand on red CI).

### PS-5 The launcher picks the newest build, never recurses, and names what it skipped

The launcher chooses the newest build among the listed targets (`-nt`), so a stale native build cannot shadow a fresh cross-build; falls back to PATH with an environment marker that stops it re-executing itself; and, where a fallback finds a binary of another version, runs it only if `--version` matches and names the copies it skipped. The shared launcher does not do this yet: its PATH fallback runs any `<binary>` that is not itself, with no version check, so that check is a launcher follow-up.

- **Why.** Builds differ in journal mode, so an old build can reopen a ledger under WAL. weigh-in's Cowork fallback once picked a 0.2.1 build from a connected folder.
- **Evidence.** weigh-in B-05, #143, `plugin/libexec/weighin`; anneal `plugin/libexec/anneal:26-234`; redkiln `tools/launcher/README.md`.

### PS-6 A release is checked on every surface a person uses

CI and unattended sessions run only Claude Code and cloud sessions. The release checklist has a human ask for one skill by name in the CLI, the desktop Code tab, a new desktop chat with a folder connected, and Cowork, and record the result with the exact install path (marketplace branch, org-synced copy, uploaded zip).

- **Why.** The mod switch-off, the `disable-model-invocation` refusal, the failed injection and the binary-less synced copy were all invisible to CI. A Windows failure was once recorded as "does not reproduce" because the check ran on a different install than the one that failed.
- **Evidence.** anneal `docs/release-0.1.0.md:58-68`; weigh-in B-18, B-20; temper `AGENTS.md`, Release checklist step 3.

### PS-7 A skill injection always succeeds, and the skill reads a placeholder

A `!` injected command is written `… 2>&1 || echo "(<binary> did not run here)"`, and the skill treats that line as a possible placeholder. Cowork replaces injections with a placeholder anyway, and synced skills run them in cloud sessions but show them as literal text in local terminals.

- **Evidence.** weigh-in CHANGELOG 1.1.1, #163, wi-6b2fa5.

### PS-8 Frontmatter is identical on every surface, so choose it for the strictest one

A synced plugin is byte-identical everywhere, so frontmatter cannot vary per surface. `disable-model-invocation: true` makes desktop chats refuse the command; say "only when the user asks" in the description instead. Outside Claude Code only six frontmatter fields are accepted (`name`, `description`, `license`, `compatibility`, `metadata`, `allowed-tools`), and any other key is a hard error.

- **Evidence.** weigh-in B-34, `@82f3b4b`; Claude Code skills docs (prior-art/agents.md §6).

### PS-9 The skill never claims a write the CLI did not print

A skill that drives a CLI reports only what the CLI printed as written. With no binary it says once that nothing was recorded, shows the command and its error, and keeps every item open.

- **Why.** In the Windows desktop app every Weigh-In write failed, while the model "dismissed" and "decided" items in prose; the user believed they were recorded.
- **Evidence.** weigh-in #127, `plugin/skills/weigh-in/SKILL.md` ("Budget", "Without the binary").

### PS-10 A skill index lists only skills the installed plugin has

Naming a skill the plugin lacks sends the model to `Unknown skill`. Indexes are generated from `skills/<name>/SKILL.md` under the plugin root, and skills never claim to auto-load: scope them with `paths:` and say the hook enforces and the skill explains.

- **Evidence.** anneal `@0b7f89e`, `@9a10a43`; redkiln-old `skills/conventions/SKILL.md:1-23`.

## Patterns worth copying

- **One regenerated commit per marketplace branch.** weigh-in unpacks the release zips into a fresh repository, `git add -f` so a global gitignore cannot drop binaries, `update-index --chmod=+x` on the launcher and binaries whatever the zip kept, and force-pushes a single commit. Old binaries never pile up in history, and CI builds the same tree from every PR (weigh-in B-14, B-19, `scripts/marketplace-branch.sh`).
- **A launcher that downloads a checksummed release when it has no build.** anneal's launcher tries newest local build, PATH, a version-keyed cache, then the release asset plus `SHA256SUMS`, each try sized to its hook's timeout, `curl --proto '=https,file'`, a stale-after-2-minutes lock and a 2-minute failure back-off, never reading stdin (anneal `plugin/libexec/anneal:26-234`). Caveat: a checksum from the same release catches corruption, not tampering; sign or attest if this becomes the main route ([CR-5](ci-and-release.md#cr-5-binaries-are-pinned-by-digest-and-should-carry-signed-provenance)).
- **When the skill's shell cannot see the plugin folder, SessionStart copies the binary to where it can.** In the Windows desktop app the skill's shell could not see the install folder, so SessionStart copies the running binary to `<git-common-dir>/weighin/bin/<version>/` through a `.part` file and a rename and prints the path; copies are per version and never deleted (weigh-in B-26).
- **One read model serves every UI.** anneal's mod renders only `status --json` (versioned, additive), keeps no state beyond its last reading, drops sections whose types do not match and says so, and writes nothing: "hooks enforce, the mod shows and collects" (anneal `@581d927`, `docs/mods.md`).
- **One release repository can carry several plugins.** `weigh-in-plugin` lists `weigh` and `weigh-mod`; both load from org sync; each needs its own `libexec/` (weigh-in-plugin `.claude-plugin/marketplace.json`; org-sync spike).

## Gotchas

- **npm dependency install does not run for synced or Cowork plugins**; executable bits do survive upload and sync (temper `docs/research/shipping-native-binaries/report.md:126-160`).
- **Plugins do not reach cloud sessions through `.claude/settings.json`**, and installing from SessionStart loads only at the next start; the environment setup script is the route (temper `docs/troubleshooting.md:42-58`; weigh-in B-12).
- **A session started before a synced plugin arrived does not have it**, and the first session after an update runs the old hooks (weigh-in #113; org-sync spike).
- **Auto-update is off for non-official marketplaces** until the user turns it on (weigh-in CHANGELOG 0.4.1).
- **Plugin mods are behind a per-account rollout switch**: temper's lens did not load on the owner's Windows machine while it did on Linux at the same version (temper `docs/history/evals.md:77-82`).
- **A root `CLAUDE.md` is rejected when the repository root is the plugin root** (`claude plugin validate --strict`); in the monorepo the rule applies to each `plugins/<name>/` (temper `AGENTS.md`).
- **A workspace that looks like the plugin repository confuses `${CLAUDE_PLUGIN_ROOT}` readers**: an eval copy named `plugin/` was treated as anneal's own repository (anneal `@82bb54c`).
- **A plugin cannot declare that it needs another plugin's MCP server**; commands granted its tools broke on installs without it (redkiln-old `AGENTS.md:158-165`).
- **Plugin limits**: 5,000 files and 200 MB uncompressed per plugin; 25 marketplaces per account (org-sync spike).

## Open problems

- **Surface routing lives in untested skill prose.** Every surface fix added a branch to Weigh-In's `SKILL.md`; D-0021 (a tested fetch script and a SessionStart `route:` line) is designed, not built (weigh-in #171).
- **macOS and Linux desktop installs, and the Cowork download path, are unverified** (weigh-in `docs/roadmap.md`, Known gaps).
- **Windows never downloads** in anneal's `.cmd` launcher (anneal `plugin/libexec/anneal.cmd:1-11`).

## Prior art

- Agent Skills is an open standard with a progressive-disclosure budget: metadata about 100 tokens, `SKILL.md` under 5,000 tokens and 500 lines, references one level deep. **Adopt**: run `skills-ref validate` in the plugins CI job. [Agent Skills specification](https://agentskills.io/specification)
- Claude Code truncates a skill's `description` and `when_to_use` at 1,536 characters in the listing. **Adopt**: put the key use case first. [Claude Code skills](https://code.claude.com/docs/en/skills)
- uv's installer never needs to touch PATH and can be pinned by URL. **Agree**: we never ship `bin/` or install onto the system PATH (exporting a binary for one Claude session through `CLAUDE_ENV_FILE`, as temper's D17 does, is a different thing). [uv installation](https://docs.astral.sh/uv/getting-started/installation/)
- dist (cargo-dist) builds per-target archives with checksums and attestations. **We differ**: binaries must live inside the plugin folder; dist could still be the builder. [cargo-dist](https://github.com/axodotdev/cargo-dist/releases/tag/v0.33.1)
