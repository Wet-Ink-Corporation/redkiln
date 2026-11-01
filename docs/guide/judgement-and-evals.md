# Judgement and evals

Blueprint design rule 8 says a model's judgement does not drive control flow until it is scored. Every repository that tried to live by that rule learned the same things about what "scored" has to mean. This chapter covers reviewers, judges, classifiers and the evals that measure plugins.

## Guardrails

### JE-1 Judgement ships in shadow and is promoted on numbers

A model decision point first records what it would have decided, without acting. It is promoted only when it has enough labelled decisions and meets a calibration bar written down in advance, and promotion is a human config edit, never a button the model can press. Any failure of the judge (unreachable, timeout, unparseable) falls back to the safe default, never to a denial or an approval it did not make.

- **Evidence.** anneal Clef: shadow until 50 answered decisions, Brier ≤ 0.15 and an ask rate below 20%; false accepts weighted 5:1; no apply button in the cockpit (`crates/anneal-cli/AGENTS.md:271-299`, `docs/mods.md`); redkiln-rs ADR-0079, ADR-0090; assay QA-05.

### JE-2 A criterion with no positive class is not measurable, and a clean number gets its scorer read

Report "not measurable" rather than 0.00 when there is nothing to detect. Before publishing a clean score, read the scorer. Do not apply a remedy that the same measurement contraindicates.

- **Why.** A keyword deriver scored the `auth` facet 0/12 because `auth` is inside "authored"; a scorer comparing a JSON bool to the string `"true"` printed 0.00 where the truth was 0.60. The Node Redkiln's 46-agent audit published 66.7/100 from 5 of 33 dimensions; the corrected score was 51.8.
- **Evidence.** redkiln-rs Halts 9-13, F39, F40, ADR-0117; redkiln-old `references/audit/2026-08-30-state-of-the-system/scoresheet.md` §7.

### JE-3 Policy over typed findings, not the verdict string

A reviewer's findings are stable across runs; its verdict string is not. Decisions are computed by deterministic policy over typed findings (class, severity, evidence, fingerprint), and the reviewer's own "approved" or "changes requested" is an input, not the blocking bit.

- **Why.** Verdicts flipped on 2 of 4 items while detections flipped on 0 of 7 (redkiln-rs F27); a reproduced regression was approved 3 of 10 times (F29). In the Node Redkiln every gaming detector was advisory and only the verdict string carried weight (B5-01). One unchanged tree went approved, changes-requested, changes-requested, approved under temper (#136).
- **Evidence.** redkiln-rs ADR-0001, ADR-0074, F27, F29; redkiln-old B5-01; temper #136; assay's `decide` over rubric v1.

### JE-4 Repeat reviews of the same tree converge by rule

A minor-only `changes-requested` cannot overturn an approval of the same tree; it is kept in history, not acted on. The reviewer sees the earlier verdict as context. A review that objects only to code already on the base branch counts as approval.

- **Evidence.** temper wi-39a8f0, `scripts/lib/verdict.mjs:124-155`; assay wi-73d2d6.

### JE-5 A verdict is bound to the exact tree and recorded by the hook, not the agent

The reviewer is handed the tree fingerprint and must echo it; the hook validates and stores the verdict, and marks it stale when the tree moves. The fingerprint covers head, base, the diff, untracked files and every config file that steers the gate. Degenerate evidence is rejected: a `files_read` entry must name a real changed path, and an approval with no rated dimension, a blocker finding, a low score or a red gate is invalid.

- **Evidence.** temper `scripts/lib/hooks.mjs:1193-1247`, `scripts/lib/verdict.mjs:17-108`, `@e3eb3d5`, audit engine-1-12 and engine-1-15; anneal patch-id freshness ([GT-6](git.md#gt-6-patch-id-keeps-a-verdict-fresh-across-a-rebase-and-nothing-else)).

### JE-6 Evals are statistical, paired and grouped by purpose

Every case runs k times in both arms (with and without the plugin). Report intervals, not pass counts: a case that passes 3 of 3 in both arms carries no evidence (Newcombe interval −0.561 to 0.561). Group cases by what they show: **discriminating** (the plugin helps) and **regression** (the plugin never makes things worse). A case that stops separating the arms because the plain model improved moves to regression; it is not tuned until it separates again. New cases are candidates until an owner-approved run places them.

- **Evidence.** anneal `evals/README.md:109,229-231` (Wilson and Newcombe intervals); temper `docs/history/evals.md:5-60` (+0.42 to +0.57 discriminating mean); assay corpus acceptance (k = 3, no detected-to-missed flip, noise within the baseline's 95% Wilson bound).

### JE-7 Every paid eval round starts with a dry triage of its cases and graders

Before trusting an uplift number, check each grader against a real good and a real bad reply, and each case for answers it gives away.

- **Why.** In anneal's rounds r4 to r7, every triage fixed the harness first: graders that counted a refused Edit as success, a case that could never pass (`max: 0` and no `min`), a fixture whose fix was already committed, a plan that spelled out red/green steps for the baseline arm, "graders that the baseline passed anyway".
- **Evidence.** anneal #201, `@b843964`, `@d625764`.

### JE-8 Judges read the trace, not only the final message

A grader that sees only the last message penalises a correct run whose final message is a terse status block, and rewards a confident wrong one. Graders read tool calls from the trace; where the final message must carry the claim, the end-of-turn grammar carries it (temper added `Declined:` for exactly this).

- **Evidence.** temper `docs/history/evals.md:20,279`, #48; anneal #201.

### JE-9 Mine the corpus from your own review history, and keep labels out of reach

The best cases are PR heads where a reviewer later found a real defect, identified by PR, round, reviewed head and fixing commit; true negatives are rounds that drew no later finding. Labels live outside the tree the reviewer may read, and the corpus paths are trust paths so a PR cannot edit the labels it is judged by.

- **Evidence.** assay `corpus/dev/README.md`, `crates/assay-app/src/eval/corpus.rs`, decisions log 2026-10-14 (#230).

### JE-10 Model judgement does not decide authority-bearing steps at the cheapest tier

A step labelled as a relay that actually makes a judgement runs at the wrong tier and nothing notices. Route by what the step does, and treat "leaves behind something another step must trust" as a judgement.

- **Why.** The Node Redkiln's `seal:` step ran at haiku while writing a commit other runs depend on; a `gate:` relay classified failure scope, which authorised writes outside the slice.
- **Evidence.** redkiln-old `AGENTS.md:235-245`, `workflows/_lib/scan-dispatch.mjs`; RFC R13.

## Patterns worth copying

- **Fingerprints computed once, from defined fields, with length prefixes, failing closed.** assay hashes class, file, enclosing symbol, the first non-empty evidence line (normalised) and, for omissions only, the expected behaviour, each with a big-endian length prefix and an occurrence index. Wording and severity are excluded so rewordings match; evidence with no content gets no fingerprint and nothing can be waived against it (assay `crates/assay-domain/src/fingerprint.rs`).
- **The adversarial reviewer card names an escape-hatch taxonomy** (doubles standing in for a needed contract, fixture-pinned answers, flag-gated scenarios, unmounted deliverables) and denies that a gaming pattern merged earlier is precedent. Field reports called it "the single best part of the run" (redkiln-old `agents/redkiln-review.md:26-90`).
- **A model-emitted classification vector was unstable because of the schema, not the temperature.** Identical modal answers on 5/20 and 3/20 items against an 80% bar, corroborated across models at temperature 0; the fix made those axes human-sourced (redkiln-rs ADR-0090, F36). Before building on a model's classification, measure its stability.
- **A labelled seed with a minimum sample.** weigh-in's 80% precision bar is "in force" only once 10 items are captured, which avoids a 1-of-1 "100%" (weigh-in wi-a9df17, `eval/README.md`).
- **One sentence can move recall more than a model change.** Adding "a list of calls for the user gets one block per call" took Weigh-In's capture from 1 of 7 to 12 of 13 (weigh-in #68).

## Gotchas

- **The baseline model drifts under your evals.** A case that separated the arms in 8 of 8 paired runs on 2026-10-06 failed to in 3 of 3 on 2026-10-18 (temper `docs/history/evals.md:109`).
- **Headless runs cannot answer a hook's ask or run a slash command fairly**; a prompt starting `/temper:implement` reaches only the plugin arm, and evals without Bash never run the gate (temper `docs/history/evals.md:64-67,150-151`).
- **Skills under-trigger on ordinary phrasings** ("add a flag", "X should do Y but does Z, fix it"); widen triggers against observed misses, not guesses (anneal `@915a9e0`).
- **`claude plugin eval` hooks and scaffold scripts run on the host**, outside the eval sandbox, with your rights; its Linux sandbox denies writes under `/tmp` and its socket paths overflow the 107-byte limit with long workspace names (39 of 162 runs failed) (anneal `evals/README.md:92,138,212`, `@4420fcd`).
- **An implied public API break goes ahead before any question**, in both arms; H-16 is not applied unprompted (temper `docs/history/evals.md:110-111`).

## Open problems

- **The model side of Review is uncalibrated**: no live replay gate, an empty injection corpus, every case authored by one model (assay `corpus/inject/README.md`, #94, #104, #105).
- **Clef has never run live**; every point is in shadow with placeholder model ids (anneal #44).
- **Loop's review gate is uncalibrated** and quarantine is not wired into runs (redkiln-rs #13).
- **No executable witness for blockers**: a failing test run as a verifier-independent signal waits on a safe way to run model-authored code (assay `07-decisions-assumptions.md` §5; redkiln-rs ADR-0074).
- **Recall, not precision, limits capture** (weigh-in: precision 15/15, recall 15/27).

## Prior art

- Binary pass/fail judges calibrated against one principal expert; report true-positive and true-negative rates, never raw agreement; keep a held-out split. **Agree, and adopt the split and the rates.** [Using LLM-as-a-Judge (Hamel Husain)](https://hamel.dev/blog/posts/llm-judge/)
- Cohen's kappa corrects agreement for chance. **Adopt** in the replay gate's report. [Evaluating LLM-evaluators (Eugene Yan)](https://eugeneyan.com/writing/llm-evaluators/)
- Judges show position and verbosity bias, and favour their own generations. **Adopt**: where possible, the reviewer does not share model and prompt lineage with the implementer, and pairwise comparisons run in both orders. [MT-Bench (arXiv 2306.05685)](https://arxiv.org/abs/2306.05685v1), [Self-preference (arXiv 2404.13076)](https://arxiv.org/abs/2404.13076v1)
- Criteria drift: evaluation criteria cannot be fixed before judging, and a model swap moves them. **Adopt**: re-run the replay gate when the model changes, not only on prompt edits. [Who Validates the Validators? (arXiv 2404.12272)](https://arxiv.org/pdf/2404.12272)
- Separate capability evals from regression evals; give graders an "Unknown" option and grade dimensions separately; start with 20-50 cases from real failures. **Agree; adopt "Unknown".** [Demystifying evals for AI agents (Anthropic)](https://anthropic.com/engineering/demystifying-evals-for-ai-agents)
- SWE-bench Verified labelled each sample three times and kept the worst label. **Adopt**: label corpus cases more than once and drop disputed ones. [SWE-bench Verified](https://openai.com/index/introducing-swe-bench-verified/)
- Agents game editable tests; read-only tests stopped most of it without hiding them. **Adopt** read-only tests during the green step where a lane can enforce it. [ImpossibleBench (arXiv 2510.20270)](https://arxiv.org/abs/2510.20270v1)
