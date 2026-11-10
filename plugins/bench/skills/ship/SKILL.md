---
name: ship
description: Use when a change needs a pull request, review verdict or depth, merge, schema migration, rollout, canary or rollback, even if it looks safe; decide GO or NO-GO only with a rollback ready. Do NOT use for proving work done or passing (use `verify`) or a failure with an unknown cause (use `diagnose`).
---

# Ship

## Overview

Takes a change that passed `verify` through review, merge, migration and rollout, deciding each step GO or NO-GO from exit codes, not by feel.

- You MUST write the rollback plan before any GO: no plan is NO-GO, and `bench rollout decide` exit 2 means run it.
- You MUST NOT merge on green tests alone: run `bench verdict valid --pr <n>`; the `merge-gate` hook denies `gh pr merge` without the review `bench ship skip-rule` wants.
- You MUST let `bench ship skip-rule` decide the review depth: exit 0 allows the light review, exit 1 needs the full one.
- You MUST record every migration wave with `bench migrate start` and `bench migrate done`; `bench migrate contract` is refused while a wave is open.
- You MUST keep a speed-up only when `bench perf noise` exits 0; neutral means revert.

## When to use

- A change passed `verify` and needs a pull request, review, merge, migration or rollout.
- Not for proving work done (use `verify`), nor for a failure nobody understands yet (use `diagnose`).

## Process

1. **Review depth.** Run `bench ship skip-rule`. It's working if you know the review it needs: exit 0 light, exit 1 full.
2. **Verdict.** Record green tests with `bench verdict set --pr <n> --sha HEAD --level tests-verified --source local` (exit 4: verdict recorded, status not posted; remove the cause or pass `--no-status`), get the review, then run `bench verdict valid --pr <n>`. It's working if it exits 0.
3. **Rollback plan.** In the pull request: the revert or flag that undoes the change, who runs it, and what it cannot undo. It's working if someone else could run it unaided.
4. **GO or NO-GO.** GO only with steps 2 and 3 done and `bench watch pr <n>` at exit 0; otherwise NO-GO. It's working if the decision names each exit code.
5. **Merge.** Run exactly `gh pr merge <n> --match-head-commit <HEAD> --repo <origin's host/owner/repo>`, alone. It's working if the `merge-gate` hook lets it through.
6. **Migrate.** For a schema or data change, follow references/migrate.md: expand, then waves with `bench migrate start` and `bench migrate done`, then `bench migrate contract`. It's working if contract exits 0.
7. **Roll out.** Before each stage of references/release.md, run `bench rollout decide --metrics <json> --bands <file>`. It's working if every widening follows an exit 0; on 1 hold, on 2 run the rollback plan.
8. **Speed claims.** Run `bench perf sample` on the base and `bench perf noise` on the change. It's working if every kept speed-up has an exit 0 behind it.

## Rationalizations

| Excuse | Reality |
|---|---|
| "It is one line, skip the review" | `bench ship skip-rule` decides the depth, not the author. |
| "We can always roll back" | Without a written plan nobody can at night. Write it first. |
| "The metrics look fine" | "Fine" is not a band. Run `bench rollout decide`. |
| "Contract now, the last wave is nearly done" | An open wave still reads the old shape. |
| "It felt faster" | Neutral means revert. |

## Red flags

- A GO with no rollback plan in the pull request.
- A merge without a fresh `bench verdict valid`.
- Widening a rollout that is on hold.
- Dropping the old shape before `bench migrate contract` exits 0.

## Verification

- `bench verdict valid --pr <n>` exits 0 for the merged head.
- `bench ledger query --tag migration:<id>` ends with `migrate.contract`.
- Every rollout stage has an `bench rollout decide` exit 0 behind it.
