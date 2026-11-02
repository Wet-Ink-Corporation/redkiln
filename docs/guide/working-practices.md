# Working practices

How agents and humans build the suite: testing that can actually fail, first contact with the real thing, review that finds defects, and keeping process from becoming the product. rust-craft (through temper) governs the Rust itself; this chapter is what the repositories learned around it.

## Guardrails

### WP-1 Every check has a positive control

A check whose green means both "found nothing wrong" and "looked at nothing" is not a check. Every ban or grep is paired with a control that must trip it; every coverage figure is a ratio with a pinned denominator and floor; every counter that should be zero has a fake that reads one.

- **Why.** redkiln-rs found eight shell shapes that pass vacuously, all live in checkpoints: `!grep` over a missing directory, a status-bearing command left of a pipe, `grep -qF "$(cat missing)"` (matches every line), `git ls-files` on an untracked path, `|| echo FAIL`, "no output means clean", `cmd | tee file` (truncates before reading). The Node Redkiln's coverage floors were unpinned constants nothing asserted.
- **Evidence.** redkiln-rs `runbook/halts.md:57-90`, `crates/rk-kernel/tests/it/main.rs:69-77`; redkiln-old B6-02, B6-04, B4-10.

### WP-2 A must-fail fixture must fail for the stated reason

A red-arm test asserts the specific diagnostic, not merely failure. Red for any other reason is not a pass.

- **Why.** A redkiln-rs fixture meant to fail clippy "passed the lint" because its red arm ran without the feature that reintroduces the defect.
- **Evidence.** redkiln-rs `@d6160ff`, `tests/fixtures/rk-banned-dep`.

### WP-3 Run the real thing once before freezing anything

Before a stage, format or interface is frozen, run one real dispatch, one real install, one real end-to-end lane. Fakes and mutants test the code you imagined.

- **Why.** Three times in redkiln-rs, running the real binary found what green fixtures and mutants missed: a self-transition only a replayed journal shows; 121 of 121 guards failing against an empty context; and the first real dispatch failing on a schema keyword after two months. In the Node Redkiln, a fix release broke the resume it enabled, because the new binary with old pinned packs was "exercised by no test".
- **Evidence.** redkiln-rs `@59be987`, `@ee893b6`, `@34c36fe`, `references/audit/2026-10-06-status/appendix.md:673`; redkiln-old readiness ruling C5.

### WP-4 Deny network to the process under test, not to the plumbing

A `--network none` container cannot check out or fetch. Fetch on the networked host, carry the caches into the container, then run the suites offline. A lockfile walk cannot see `std::net`, which needs no dependency.

- **Evidence.** redkiln-rs `.github/workflows/ci.yml:176-290`.

### WP-5 Determinism through bans, not review

Production code that must replay deterministically bans clocks, randomness, sleeps, `env::set_var`, unordered maps and direct process spawning in `clippy.toml`, with each warranted site carrying an approved `#[expect]`.

- **Evidence.** redkiln-rs `clippy.toml`, ADR-0076, ADR-0086, AC-242.

### WP-6 Brief implementers with what reviewers keep blocking on

When review rounds keep finding the same gaps, put them in the implementer's brief. For assay they were a worst-case complexity table for every loop over PR-controlled input, one test per refusal branch, and boundary tests at the cap and at cap+1.

- **Evidence.** assay `docs/handoff-2026-10-12.md`, #184, #281, #286, #289.

### WP-7 Adversarial review over green work, with a cap

A stage closes when its checks pass **and** a refuting round finds no blocker or major. Minor findings go to a follow-up issue rather than another round, and the follow-up debt is itself tracked.

- **Why.** Three verifiers re-deriving redkiln-rs Step 7i's passing work found 36 defects, 13 of them surviving mutants; an "obvious" one-line repair was wrong for 43 of 63 activities. assay's three spec-level adversarial rounds found forgeable decide-only, unauthenticated artifacts and a cost model wrong in the direction that matters, before code. But 47 of assay's 122 open issues are review follow-ups.
- **Evidence.** redkiln-rs `@5853aa8`, `@2ce5576`; assay `docs/review/02-regression-pass.md`, `03-adversarial-review-round-3.md`, issue listing.

### WP-8 Charters stay small enough to load whole

Root `AGENTS.md` and `CLAUDE.md` load on every session and count against context; Codex truncates the combined chain at 32 KiB, cutting the deepest (crate) charter first. Keep root charters to the rules an agent cannot discover, move decision logs to `docs/<plugin>/decisions.md` with a pointer, and repeat the H-rules only as a pointer where the hooks enforce them.

- **Evidence.** prior-art/agents.md §6 (Codex `project_doc_max_bytes`, 32 KiB; Claude Code's memory docs advise under 200 lines per file, imports counted, which redkiln and assay meet today while assay's root charter is already about 30 KB). redkiln: `docs/decisions.md` and `tools/charters/check.sh` (24 KiB budget). See [Tension T6](README.md#tensions-with-the-plan), resolved.

### WP-9 An artifact with no reader is not produced

Every template, plan file and status field names its reader. If nothing reads it, it is not generated.

- **Why.** 370 of 430 `plan.md` files across three Node Redkiln repositories were the byte-identical template; a proposal to gate `plan` was reversed because it "would have authored ~430 essays with zero consumers". Companion `status:` frontmatter was deleted from 18 templates because nothing updated it and it looked authoritative.
- **Evidence.** redkiln-old `processes/story.yaml` (plan stage), field report finding #6.

### WP-10 Process weight is measured and capped

Track the ratio of docs-only commits, runbook length and time-in-stage with no code. When process outgrows the product, cut it.

- **Why.** redkiln-rs's phase-1 runbook reached 53,707 lines; docs-only commits rose to 70% by week 5; one stage was open 74 hours with no Rust; scope grew 114% in 24 days with nothing capping it.
- **Evidence.** redkiln-rs `references/audit/2026-10-06-status/appendix.md:539-562`, #72.

## Patterns worth copying

- **Read-only multi-agent audits with adversarial verification**: a detached worktree at a pinned SHA, survey agents, each headline claim attacked by two skeptics, a completeness critic, each finding re-checked at later SHAs before filing (redkiln-rs `references/audit/2026-10-06-status/README.md`).
- **Field reports are harvested into dated files with a disposition table**, re-verified at HEAD; an observation from an older version may motivate work but never proves a gap (redkiln-old `references/field-reports/`).
- **Mixed-version tests before any relaunch**: a new binary against old pinned state (redkiln-old readiness ruling C5).
- **Unattended merge bar**: every check green, no conflict, no open thread, a fresh-context approval cited on the PR, merge pinned to the head SHA; never release or `[hold]` PRs (wi-880b66, wi-9f5c91; assay handoff "Merge bar").
- **Stacked PRs while merges wait cause semantic conflicts**: two parsers of the same file ended up coexisting in assay. Prefer small PRs merged promptly over deep stacks (assay `docs/handoff-2026-10-19.md`).

## Gotchas

- **Test-first erodes under critical-path pressure**: assay's GitHub adapter was written before its tests and accepted retroactively (assay #53, PR #274).
- **Records drift from reality without a census**: redkiln-rs's trackers said "CI never ran" weeks after it had, and recorded a checkpoint as passed when only its mechanical block had (redkiln-rs #23, D21-40).
- **A verification note must name the exact install and surface** ([PS-6](plugins-and-surfaces.md#ps-6-a-release-is-checked-on-every-surface-a-person-uses)).
- **Single-machine evidence is lost evidence**: Loop's test-oracle corpus had no remote until it became a verified off-machine bundle (redkiln-rs #32, `CORPUS_PIN`).
- **Sessions must save their own transcripts** before ending; transcripts leave the sessions API before a later session can read them (weigh-in wi-48657b).

## Prior art

- Anthropic's long-running harness: an initializer, a JSON feature list where agents may only flip `passes`, a progress file, one feature at a time, an end-to-end check at each session start. **Agree; adopt** machine-owned plan state with one writable field per item, and a start-of-lane smoke check. [Effective harnesses for long-running agents](https://anthropic.com/engineering/effective-harnesses-for-long-running-agents)
- "'All tests pass' on its own tells you very little"; ask for the command and its output. **Adopt**: the Stop gate stores its output digest so a reported status can be checked. [Best practices](https://code.claude.com/docs/en/best-practices)
- Write down only what the agent cannot discover; if emphasis does not work, the line belongs in a hook. **Agree.** [Best practices](https://code.claude.com/docs/en/best-practices)
- The OpenHands SDK separates immutable agent configuration from an append-only event log. **Adopt**: record the prompt bundle hash and model in a lane's first event. [OpenHands SDK (arXiv 2511.03690)](https://arxiv.org/abs/2511.03690)
- Copilot's cloud agent never merges its own work. **We differ, deliberately** (wi-880b66), which is why the merge bar stays stricter than a human merge. [Copilot cloud agent](https://docs.github.com/en/copilot/concepts/agents/cloud-agent/risks-and-mitigations)
