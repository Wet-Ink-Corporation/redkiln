# Releasing Weigh-In

This is the checklist each release follows, so that a cloud session and Ryan run the same steps.
README's [Releasing](../README.md#releasing) section describes what the workflow does. This page
is the order of steps and the checks around them.

Who does what: a session prepares and merges the bump PR and runs the checks. **Ryan pushes the
tag** (the cloud git proxy refuses tag pushes, and the `v*` tag ruleset limits who can create
them, #5). 1.0.0 is tagged only once every v1 gate is ticked
([#26](https://github.com/Wet-Ink-Corporation/weigh-in/issues/26)).

## 1. Before the bump

- [ ] Everything for the release has merged to `main`, and CI is green on `main`'s head.
- [ ] `## Unreleased` in `CHANGELOG.md` lists every user-visible change. Its opening paragraph says
      what the release is for.
- [ ] During the pilot (#23), nothing in the release changes capture (markers, the TODO scan, the
      Stop and PostToolUse hooks), unless Ryan has agreed to it. A capture bug that corrupts data
      goes to Ryan first.

## 2. The bump PR

One PR, titled `Release <version>: <what it is for>`:

- [ ] `version` in `Cargo.toml`, `plugin/.claude-plugin/plugin.json` and
      `plugin-mod/.claude-plugin/plugin.json` are equal and new.
      `Cargo.lock` is refreshed (`cargo update -p weighin -p weighin-core --offline`, or any
      build).
- [ ] `## Unreleased` is renamed to `## <version> — <YYYY-MM-DD>`, and a new empty `## Unreleased`
      sits above it. `scripts/changelog-section.sh <version>` prints the section.
- [ ] `plugin/libexec/cowork-fetch.sh` still holds exactly one `V=dev; S=none;` (`sh tests/cowork-fetch-check.sh`).
- [ ] The skill's eval cases pass, run by hand (D-0021; about $0.10 on your own credential):
      `claude plugin eval plugin --runs 1 --ablation none --max-cost-usd 1`
- [ ] If `plugin/libexec/cowork-fetch.sh` or the skill's Cowork route changed, one real Cowork
      sitting reads the script and its device shell call prints `B=<path>`.
- [ ] The dry run passes locally:
      ```sh
      cargo test --workspace                       # includes the 0.2.2 fixture tests (B-10)
      node .claude/temper/temper.mjs baseline --check
      cargo fmt --check
      cargo doc --workspace --no-deps
      cargo deny check
      ./scripts/build-plugin-bins.sh
      ./scripts/package-plugin.sh --no-download    # dist/weigh-in-plugin-<version>.zip
      claude plugin validate .
      ```
- [ ] CI is green on the PR (ci.yml on Linux, Windows and macOS, rust.yml's temper gate, the
      package check), no review thread is open, and the PR is merged with a merge commit (D-0020).

## 3. The tag (Ryan)

```sh
git fetch origin main
git tag -a v<version> <merge commit> -m "Weigh-In <version>"
git push origin v<version>
```

The workflow refuses a tag that is not on `main` or has no changelog section. A tag is never
moved or deleted; a bad release is fixed by a new patch release.

## 4. After the workflow

- [ ] The release workflow run is green, and the GitHub release has both zips (`weigh-in-plugin`
      and `weigh-mod-plugin`), the raw binaries
      (Linux musl, Windows msvc, macOS arm64) and `SHA256SUMS`, with the changelog section as its
      notes.
- [ ] The `marketplace` branch is at the new version (`plugin.json` there says `<version>`).
- [ ] `Wet-Ink-Corporation/weigh-in-plugin`'s default branch is at the new version, with
      binaries under `libexec/`. claude.ai's organization sync reads it (B-19).
- [ ] After the sync, a new session's SessionStart line names `<version>` (Ryan, on Windows and
      in Cowork).
- [ ] `docs/roadmap.md` "Where to pick up" names the release, and the issues it fixed are commented
      or closed. An issue that needs Ryan's re-check stays open until he has run it.
