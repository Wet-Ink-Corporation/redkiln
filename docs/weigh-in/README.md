# Weigh-In docs

| Where | What |
|---|---|
| [`research/report.md`](research/report.md) | *Settle agent questions while the context is warm* (2026-09-26): the research and the design, with dated amendments for what changed while building |
| [`research/notes/`](research/notes/) | The five research sweeps behind the report, and the 12 September brief that started it |
| [`research/shipping-native-binaries.md`](research/shipping-native-binaries.md) | (2026-10-01) What the research on shipping a Rust binary in a Claude Code plugin found for Weigh-In; the full report and notes moved to [temper](https://github.com/Wet-Ink-Corporation/temper/blob/main/docs/research/shipping-native-binaries/README.md) |
| [`decisions/`](decisions/README.md) | Every decision so far: settled questions, the board sitting, defaults, and the calls made while building |
| [`install.md`](install.md) | Installing, updating and removing a release: the organization sync of `weigh-in-plugin`, Claude Code from the `marketplace` branch, Cowork, the ledger, `hook-errors.log`, capture off, Windows `ENOENT` |
| [`releasing.md`](releasing.md) | The release checklist: the bump PR and its dry run, Ryan's tag, and the checks after the workflow |
| [`roadmap.md`](roadmap.md) | What is not yet verified, known gaps, and slice 2 |
| [`v1-plan.md`](v1-plan.md) | (2026-10-02) Evaluation of the repository at 0.2.2 and the plan to v1 in active use, starting with source control |

Research notes:

| File | Covers |
|---|---|
| [`00-prior-weigh-in-brief-2026-09-12.html`](research/notes/00-prior-weigh-in-brief-2026-09-12.html) | The earlier Weigh-In brief the research started from |
| [`01-prior-art-sweeps-and-review-uis.md`](research/notes/01-prior-art-sweeps-and-review-uis.md) | Prior art: end-of-session sweeps of open questions, and review surfaces outside the terminal |
| [`02-surfaces-and-token-mechanics.md`](research/notes/02-surfaces-and-token-mechanics.md) | Where cards can render (AskUserQuestion, Artifacts, Cowork) and token-efficient skill mechanics |
| [`03-decision-quality-methods.md`](research/notes/03-decision-quality-methods.md) | Decision-quality methods: doors, deferral tests, evasion, default effects |
| [`04-codebase-context-for-hitl.md`](research/notes/04-codebase-context-for-hitl.md) | Building cheap codebase context for a human decision |
| [`05-ledger-storage-and-lifecycle.md`](research/notes/05-ledger-storage-and-lifecycle.md) | Ledger storage, item lifecycle and decision records |
