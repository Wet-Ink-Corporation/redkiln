# CI and release

redkiln's CI is one path-filtered job set with a single `ok` check (`AGENTS.md`). This chapter is what the repositories learned running theirs, and what prior art suggests adding. Workflow trust boundaries (re-runs, fork PRs, `pull_request_target`) are in [Trust and security](trust-and-security.md#ci-trust-boundaries).

## Guardrails

### CR-1 Claims about rulesets are checked, not trusted

A required check that no ruleset requires is not a gate. Read the live ruleset when a document claims one, and keep job names stable so required checks still match after runner moves.

- **Why.** temper's audit found its `main` ruleset required no status checks at all, while `AGENTS.md`, `CONTRIBUTING.md`, the plan and a closing comment all said it did; the pilots' rulesets did not require the gate either.
- **Evidence.** temper `docs/history/2026-10-audit/report.md:16,483-486`, wi-2d6d35, wi-1f933a, `@febcd6a`.

### CR-2 Repository tests parse the workflows

Workflows are code under test: a test parses the YAML and enforces that actions are pinned `owner/repo@<40 hex> # <tag>`, that decision-bearing jobs use only cache-free actions, that every job declares `permissions`, and that secret-bearing jobs carry the attempt guard. Add a static auditor (zizmor) for template injection and persisted credentials.

- **Why.** When Actions billing ran out, an owner-made runner migration brought in an unpinned, caching checkout action; two of assay's workflow tests failed at once and the change was reverted.
- **Evidence.** assay `crates/assay-cli/tests/it/workflow_cache.rs`, `release_workflow.rs:126-145`, #302, #304, `@f414239`.

### CR-3 CI flags, not fails, on changes only a human may make, where CODEOWNERS would deadlock

A raised baseline or a changed vendored copy is flagged loudly on the PR instead of failing, because CODEOWNERS deadlocks a single-owner repository (you cannot approve your own PR). The flag step runs before any vendored code, and the comment job runs no repository code.

- **Evidence.** temper wi-d23383, wi-bd1cde, `CHANGELOG.md:139,269-273`.

### CR-4 A release cannot ship without notes, off the default branch, red, or half-published

CI fails when a package's version has no CHANGELOG section; a release tag must be on `main`; the workflow refuses to publish when a required secret is missing; re-runs are idempotent (`gh release upload --clobber`); deploy pushes pin the host key. Release notes say how to upgrade when the engine or templates change.

- **Evidence.** weigh-in B-09, B-19, `.github/workflows/release.yml:124-151`; temper `tests/release-check.sh:74-110`; redkiln-old readiness ruling C3.

### CR-5 Binaries are pinned by digest, and should carry signed provenance

Consumers fetch release binaries by digest. A digest proves integrity, not origin; a checksum file from the same release catches corruption, not tampering. Add signed provenance (artifact attestations, or cosign keyless over the digest) and say which SLSA level the release targets. Build with `cargo auditable` so binaries already in users' `libexec/` can be checked against advisories published later.

- **Evidence.** assay `docs/operations/release.md`, `.github/actions/fetch-by-digest`; anneal PR 140 (same-release checksum); prior-art/platform.md §3.

### CR-6 Semver is defined by the impact on an adopted repository

A new check that fails only new or changed code is minor; one that fails untouched code is major; raising a minimum Claude Code or Node version is major, because it can stop hooks for anyone who has not upgraded.

- **Evidence.** temper `docs/history/decisions.md:130-133`, wi-22c6a1, wi-c038a8.

### CR-7 Plan for the CI vendor and the bill failing

Keep the required check's name independent of the runner, keep local gates runnable as the evidence of record, and make "minutes ran out" a documented mode, not an emergency migration.

- **Why.** Actions billing blocked assay's CI on 19 and 20 October (stopped from 17:58 on the 19th, and a re-run still refused on the 20th); an emergency move to another runner provider ended when its free minutes ran out and was reverted; weigh-in made the same round trip. Making the repositories public is what made minutes free.
- **Evidence.** assay `docs/handoff-2026-10-19.md`, decision 0006; weigh-in B-33, PRs #154, #157, #159.

## Patterns worth copying

- **One aggregate `ok` check** that passes when every job that ran passed and treats legitimately skipped jobs as passing (redkiln `ci.yml`; the alls-green pattern).
- **Pinned toolchain versions bumped by hand, in their own PR**, including CI's MSRV job, which reads `rust-version` instead of hard-coding it (temper `AGENTS.md`; `@1398194`, the 1.3.0 MSRV job that failed every adopter).
- **A warm `target/` published by digest** for decision-bearing jobs, instead of the Actions cache; build and restore at the same absolute path (cargo fingerprints hold absolute paths) and pack with `--hard-dereference` (cargo hardlinks outputs) (assay ADR-21, `docs/operations/warm-target.md`).
- **A brownfield ratchet** that fails only on findings in added, modified or moved lines, or a per-crate count rising, and lowers but never raises without a human (temper `baseline --check`, wi-06d675, wi-cd9821).

## Gotchas

- **A ruleset cannot name the workflow's `GITHUB_TOKEN` as a bypass actor**, so a force-pushed marketplace branch has no ruleset (weigh-in B-14); there is no single-user bypass actor and no read-none default token (assay wi-66d1f8, decision 0007).
- **Agent sessions cannot edit `.github/`**, so workflow fixes go to the owner as a diff (anneal PR 140; assay "Protected files"). Plan the round trip into any PR that touches CI.
- **A GHCR package needs Actions write access set by hand** in its settings; publishing failed on every push for days until it was (assay #196).
- **CodeQL flags decision text in test messages as cleartext logging** (assay `@fd69f39`), and its autofix once edited a vendored copy only (temper `@73f2ff8`).
- **`npm publish tarballs/x.tgz` is read as a GitHub shorthand**; it needs `./` (temper `CHANGELOG.md:7`).
- **On 2-vCPU runners, test frameworks may silently run serially** (redkiln-old `CHANGELOG.md:780-823`).

## Prior art

- GitHub's hardening guide: pin actions by full SHA, read-only `GITHUB_TOKEN`, no untrusted code under `pull_request_target`, OIDC over long-lived secrets; workflow dependency lockfiles are arriving and also cover actions that composite actions pull in. **Agree; evaluate** the lockfile once documented. [Security hardening for GitHub Actions](https://docs.github.com/en/actions/security-for-github-actions/security-guides/security-hardening-for-github-actions)
- Required checks can be pinned to the GitHub App that must report them. **Adopt** for `ok` and Review's verdict check, so a token with `statuses: write` cannot spoof the gate (an owner-only ruleset change). [Changelog: required checks from the intended app](https://github.blog/changelog/2021-11-30-ensure-required-status-checks-provided-by-the-intended-app/)
- zizmor audits workflows for template injection and persisted credentials. **Adopt** in CI. [Securing workflows with zizmor (Packagist)](https://blog.packagist.com/securing-our-github-actions-workflows-with-zizmor/)
- OpenSSF Scorecard scores pinning, token permissions and dangerous workflows. **Adopt** as a periodic audit. [Scorecard check descriptions](https://devblogs.microsoft.com/nuget/openssf-scorecard-for-net-nuget/)
- SLSA levels; artifact attestations reach Build L2. **Adopt** a stated target level. [SLSA levels](https://slsa.dev/spec/v1.0/levels), [Artifact attestations](https://docs.github.com/actions/security-guides/using-artifact-attestations-to-establish-provenance-for-builds)
- cosign keyless signing of ORAS artifacts by digest. **Adopt** if attestations are unavailable. [Sigstore: signing other types](https://docs.sigstore.dev/cosign/other_types)
- cargo-auditable embeds the dependency list in the binary. **Adopt** for release builds (an H-16 tool decision). [cargo-auditable](https://github.com/rust-secure-code/cargo-auditable)
- release-plz supports per-package tags through `git_tag_name`, path-scoped changelogs and release PRs. **Evaluate** before writing release tooling in `xtask`; our `--` separator stays. [release-plz configuration](https://release-plz.dev/docs/config)
- Dependabot `cooldown` and `groups`. **Adopt** a few days of cooldown against freshly compromised releases. [Tame Dependabot](https://github.blog/security/application-security/tame-dependabot-group-your-updates-slow-the-cadence-keep-security-fast/)
