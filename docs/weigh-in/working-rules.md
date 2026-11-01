@AGENTS.md

# Weigh-In

A decision ledger for agent sessions: a Rust CLI (`weighin`) over happenstance's SQLite event
store, shipped as a Claude Code / Cowork plugin whose skill is `/weigh-in`. Read `README.md` for
what it does, `docs/decisions/` before changing behaviour, and `docs/roadmap.md` for open items.

## Layout

- `crates/weighin-core` — no I/O: events, ids, markers, scanning, cards, projection, atoms, board format.
- `crates/weighin` — the CLI: store, git, context packs, hooks, the batch write path.
- `plugin/` — the `weigh` plugin: `skills/weigh-in/SKILL.md`, `hooks/hooks.json` (command hooks only), `libexec/` launcher, Cowork fetch script and built binaries, `evals/` (run by hand, not shipped).
- `plugin-mod/` — the `weigh-mod` plugin, Claude Code only: the mod (`hooks/mod/register.tsx`, `types/`, B-29, B-35). Packaging copies weigh's `libexec/` into its zip.
- `tests/` — the mod's checks (`mod-check.sh`) and the plugin API types they compile against, and the Cowork fetch script's check (`cowork-fetch-check.sh`).
- `scripts/` — `build-plugin-bins.sh` (Linux musl and Windows gnu), `build-plugin-bins.ps1` (Windows msvc), `package-plugin.sh` (zip for upload), `marketplace-branch.sh` (the `marketplace` branch commit, from that zip), `changelog-section.sh` (a version's `CHANGELOG.md` section: the release notes, and CI's check that a bump has its entry), `weekly-stats.sh` (`weighin stats` across folders, as markdown for #23), `seed.py` (drafts and reviews eval seed items, wi-100d6d).
- `docs/` — research report and notes, decision records, roadmap.

## Commands

```sh
cargo test --workspace
node .claude/temper/temper.mjs baseline --check   # clippy, rustdoc and temper's scan, against .claude/temper-baseline.json
cargo fmt --check
cargo doc --workspace --no-deps
cargo deny check                             # advisories, licences, sources (deny.toml)
./scripts/build-plugin-bins.sh               # binaries into plugin/libexec/<target>/
./scripts/package-plugin.sh --no-download    # dist/weigh-in-plugin-<version>.zip
claude plugin validate .                     # marketplace and plugin manifests
bash tests/mod-check.sh                      # the mod: validate --strict, claude plugin test, strict tsc
sh tests/cowork-fetch-check.sh               # the Cowork fetch script against a fake $HOME/mnt (D-0021)
claude plugin eval plugin --runs 1 --ablation none   # the skill's eval cases, by hand before a release
```

`.github/workflows/ci.yml` runs fmt, tests and docs on every push to `main` and every pull request,
on Linux, Windows and macOS; `.github/workflows/rust.yml` runs temper's lint ratchet (`baseline --check`)
and the rest of the temper gate, fmt, tests, docs and `cargo deny` included, on Linux. On a pull
request ci.yml's Linux leg and its deny job skip what rust.yml runs (wi-218cc3); pushes to `main`
and the release run them in full. Legacy findings are recorded in the baseline; only new ones fail. CI also checks the plugin package (stamp, versions,
changelog entry, `claude plugin validate`, the mod checks) and runs `cargo deny check` (`deny.toml`); the release
workflow runs all of it before it builds. Claude Code cloud sessions run
`.claude/hooks/session-start.sh`, which installs the toolchain and pre-builds the tests.
Dependabot (`.github/dependabot.yml`) opens weekly update PRs, holding back Cargo majors and
happenstance (B-01).

Toolchain is pinned to Rust 1.98.1 (edition 2024), the `redkiln` monorepo's pin, where Weigh-In
moves as `rk-weigh-in` 2.0 (B-36, superseding wi-b4c1d8's redkiln-rs destination). Line endings
are LF everywhere (`.gitattributes`).

## Working in this repository

- Every change reaches `main` through a pull request with green CI; open work is a GitHub issue
  under the `v1.0` milestone (D-0011), and `docs/roadmap.md` links them.
- Note user-visible changes under `## Unreleased` in `CHANGELOG.md`. A version bump renames that
  section to `## <version> — <date>`; the release notes are that section.
- End every working session here with `/weigh-in` (D-0010): settle what the session raised, so
  new decisions become `.kb` atoms. `docs/decisions/` stays the record of everything before.
  In cloud sessions the ledger (`.git/weighin/`) lives only as long as the container (D-0014),
  so settle or defer everything before the session ends. There the plugin is wired through
  `.claude/settings.json` and `.claude/hooks/session-start.sh`, because cloud sessions do not
  install plugins (B-12); on your own machine, install `weigh@weigh-in` as usual.
- In cloud sessions, commit `.kb/_ledger/journal/` with the session's work, including after a
  sitting that decided nothing. The hooks keep `<session>.jsonl` there current, and it is the
  only record of what the session's ledger presented, dismissed, deferred and defaulted, which
  is what the pilot is judged on (B-21, #23).
- Before a cloud session on this repository ends, add its own end-of-turn messages to
  `eval/seed.jsonl` (wi-48657b): blocks as capture items, the prose calls they missed, and a
  share of true negatives. Run `python3 scripts/seed.py save <transcript> --source <session id>`
  on the session's transcript under `~/.claude/projects/`, then read the drafts it appended:
  relabel prose calls as `capture` and redact (`eval/README.md`). A later session cannot rely on
  reading the transcript back. Ryan reviews drafts with `python3 scripts/seed.py review`.
- Run `git config core.hooksPath .githooks` once per clone: its pre-push hook refuses `archive/*`
  branches, which hold history that must stay local (B-06).

## Rules learned the hard way

- **No top-level `plugin/bin/`.** claude.ai rejects hosted plugins that ship one; binaries live in `plugin/libexec/` and are called by path (B-03).
- **No deletes beside the ledger.** Cowork's connected folders refuse deletes; the store runs under a TRUNCATE journal and never goes through happenstance's WAL open (B-01). Keep temporary files on the temp disk, not beside the ledger or in the repo.
- **Cowork is not Claude Code.** Skills and hooks run in a scratch cloud workspace (`CLAUDE_CODE_ENTRYPOINT=remote_cowork`); `!` injection does not run; edits to the user's folder go through the computer's shell, so `PostToolUse` never sees them. The skill's Cowork route runs `weighin` through that shell (D-0002).
- **Skill names must stand alone.** Cowork lists plugin skills by the skill's own name; Claude Code lists `/<plugin>:<skill>` and also resolves the bare name (D-0004).
- **Versions move together.** `Cargo.toml`, `plugin/.claude-plugin/plugin.json` and `plugin-mod/.claude-plugin/plugin.json` carry the same version; bump all three for any plugin change, or `/plugin marketplace update` will not pick it up.
- **One stamp, in the Cowork fetch script.** `plugin/libexec/cowork-fetch.sh` holds exactly one `V=…; S=…;` and `SKILL.md` none; `package-plugin.sh` stamps it in the zip only. The repository copy stays `V=dev; S=none;` (D-0021, checked by `tests/cowork-fetch-check.sh`).
- **No mod in `weigh`.** A desktop chat turns off a whole plugin that ships a hooks module, skills included; the mod lives in `weigh-mod` (B-35).
- **Rebuild every binary you keep** after a Rust change: an old build next to a new one reopens the ledger under its own journal mode (B-05).

## Decisions

Settled decisions are not reopened without new evidence. Record new ones in
`docs/decisions/` (board sittings as one file per `wi-` id, chat decisions in
`build-decisions.md`), saying who took them and what they supersede.

## Releasing

Bump both versions and move `## Unreleased` in `CHANGELOG.md` to the new version, merge to
`main`, then push the tag `v<version>`; the workflow refuses a tag that is not on `main` or has no
changelog entry. The workflow tests, builds Linux,
Windows and macOS binaries, packages the plugin and publishes the release. It then replaces the
`marketplace` branch (D-0009) and the default branch of `Wet-Ink-Corporation/weigh-in-plugin`,
which claude.ai's organization sync reads (B-19); that push needs the
`WEIGH_IN_PLUGIN_DEPLOY_KEY` secret. It stamps the Cowork download's checksum only while the
repository is public, as it is since B-33; otherwise it packages with `--no-download` (D-0007).

## temper (Claude Code specifics)

- The temper plugin enforces the rust-craft standard in this repository. It refuses Rust edits until the `temper:rust-craft` skill is loaded, refuses escape hatches before they are written, routes configuration changes, new dependencies, lint exceptions and removed tests to you for approval, and will not let a turn that changed Rust end until the gate is green and `temper:rust-reviewer` has approved the current tree. A turn can also end honestly with `Status: WIP` or `Status: BLOCKED`.
- Entry points: `/temper:implement <task>` (plan, then test-first implementation, then independent review), `/temper:review`, `/temper:gate`, `/temper:audit`, `/temper:setup`.
- Subagents: `temper:rust-architect` plans, `temper:rust-implementer` implements test-first, and `temper:rust-reviewer` gives an adversarial fresh-context review with a validated verdict. Each starts with the standard preloaded.
- Each crate's `CLAUDE.md` imports its `AGENTS.md` charter, so the charter loads when you read files in that crate.
