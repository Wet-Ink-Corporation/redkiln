# Decision cards

Constraints in force (settled decisions, never re-open): the mechanism is a Redkiln-native decision item type built on a portable Claude Code plugin core; decided records land in the same .kb layout in every repo; judge models are Anthropic models only; the decider's agent work is about half interactive, half unattended. The decider is a very senior solo architect (Wet Ink Corporation, and a consulting firm, IQBG, with federal/state/local and commercial clients). He prefers long-term value and quality over shortcuts and dislikes recommendations that paper over problems or defer what can be settled now.


## wi-7c21e4: May Weigh-In publish ledger items from IQBG client repos to a claude.ai Artifact board?

Why it matters: Board items quote client code, requirements and your deliberations. The board keeps them on claude.ai, visible to your organisation, long after the session that raised them.

Door: one-way (Published excerpts can be deleted but not un-seen; a breach of client terms cannot be undone.). Blast radius: external. Resolve by: before slice 2 (the board) ships.

Evidence:
- [docs › artifacts] Artifacts need a claude.ai login and the Anthropic API (not Bedrock, Vertex or Foundry), and are unavailable in CMEK, HIPAA or ZDR organisations.
- [runtime › db] A board with a db store is organisation-internal and cannot be shared publicly. Any viewer can write the store, so answers read back are treated as untrusted.
- [session fact] Claude Code already sends the code in context to Anthropic during a session. The board adds persistence and an org-visible surface, not new processing.
- [settled · Q2] Records land in the same .kb layout everywhere, so the ledger stays in the repo whatever the board mode.

Options:
- **P. No hosted board anywhere** — Local review page and in-chat cards only, in every repo including your own. Holds if you never need the board from Cowork, away from your machine, or on your phone. If wrong: you lose the only surface that spans Claude Code and Cowork, for your own repos too.
- **Q. Artifact board in every repo** — One surface everywhere, including client repos, reachable from Cowork, the desktop app and your phone. Holds if every client's terms already allow Anthropic to process and store excerpts of their code. If wrong: client excerpts persisted on claude.ai against their terms, beyond recall from anyone who viewed them.
- **R. Per-repo board mode; client repos default to local** — board: artifact | local | off in .kb/_ledger/config.json. Wet Ink and Redkiln repos use the Artifact board; client repos use the local page and in-chat cards until you switch one. Holds if you can say at init whether a repo belongs to a client. If wrong: one extra setting per repo; a client repo you would have allowed runs without the board until switched.


## wi-a93f02: Should the ledger's event files (.kb/_ledger/events) be committed to git?

Why it matters: Committed events travel with branches, pull requests and machines. They also put raw agent deliberation into repo history, which a client may not want in theirs.

Door: two-way (You can stop committing at any time, but files already pushed stay in history.). Blast radius: system. Resolve by: before slice 1 ships (weighin init needs a default).

Evidence:
- [src/store/telemetry.ts:4] Redkiln already commits append-only JSONL telemetry, one partition per actor@worktree.
- [src/schema/config.ts:27] auto_stage_telemetry: Redkiln stages those partitions with git add by default.
- [design] The repo view folds events across git worktree list, so open items are visible across worktrees whether or not events are committed.
- [surfaces] Claude Code cloud sessions clone from the remote; an uncommitted ledger does not exist there.

Excerpt src/store/telemetry.ts:1:
```
/**
 * Telemetry sink (spec §4.5, §7, §8, §11).
 *
 * Events are append-only JSONL, one object per line, written to a per-actor /
 * per-worktree partition `${telemetry_dir}/${actor}@${worktree}.jsonl`. Every
 * record is validated against the `Event` schema before it is appended, so the
 * on-disk log always round-trips through the schema.
 */
```

Options:
- **P. Commit in your repos; client repos local by default** — The same repo class that sets the board mode sets this: your repos commit events, client repos keep them local unless you opt in. Holds if one own/client flag at init describes your repos well enough. If wrong: client repos lose open items in cloud sessions and across machines until you opt in.
- **Q. Never commit; only decided atoms reach git** — Event files stay local and gitignored everywhere; the curated .kb atoms are the only committed record. Holds if you work from one machine and never need open items in a cloud session or on a PR. If wrong: open items vanish on other machines and in cloud sessions; a lost clone loses the open ledger.
- **R. Commit by default; opt out per repo** — Every repo commits its event files; a repo that must not carry agent metadata excludes them with .git/info/exclude. Holds if most repos you work in tolerate agent metadata in history. If wrong: an opted-in client repo carries ledger files in pushed history; removing them needs a history rewrite.


## wi-4d8b6a: May unattended runs decide two-way, judgment-bound questions themselves and report them in a digest?

Why it matters: About half your agent work runs unattended. Without a rule, those runs either stall on every question or make silent assumptions, which are the buried questions this skill exists to surface.

Door: two-way (Any default can be reopened from the digest; reopening supersedes it.). Blast radius: system. Resolve by: before slice 1 ships.

Evidence:
- [settled · Q1] Your agent work is about half interactive, half unattended.
- [ce-sweep SKILL.md] "With no usable blocking-question tool, behave as non-interactive"; only authority-bound items are escalated to the human.
- [docs › sdk user input] AskUserQuestion is unavailable to subagents and in claude -p runs.
- [Amazon 2015 letter] Two-way-door decisions "can and should be made quickly by high judgment individuals or small groups".

Options:
- **P. Yes for every item, one-way included** — Unattended runs decide everything and report it all in the digest. Holds if you are willing to review one-way calls after the fact. If wrong: an irreversible choice is made with nobody asked. FAILS MUST: One-way doors always get a human (principle 1 of the 12 September brief).
- **Q. No; every item waits for you** — Unattended runs park each question until you answer, using the SDK defer loop. Holds if unattended runs can wait hours for you. If wrong: runs stall, and the slice 3 defer loop becomes a prerequisite for slice 1.
- **R. Yes for two-way items; one-way items block** — Unattended runs take a logged default on two-way, judgment-bound items and list it in the digest. One-way items are marked did: blocked and the run continues with independent work. Holds if runs classify door type correctly most of the time. If wrong: a misclassified one-way item gets a default; the digest shows it and a reopen supersedes it.


## wi-e15c77: Should Redkiln's .kb contract reserve .kb/_ledger/ and accept Weigh-In's intake keys in slice 1?

Why it matters: Decides whether every repo has one .kb footprint from day one, and whether the later native decision type is a mapping or a migration.

Door: two-way (A contract note and a kb-ingest change; both can be revised.). Blast radius: module. Resolve by: before slice 1 ships.

Evidence:
- [src/store/validate-kb.ts:84] validate --kb already skips _-prefixed directories, so .kb/_ledger/ is invisible to atom validation today.
- [src/schema/kb.ts:61] KbFrontmatter is .passthrough(), so Weigh-In keys such as weighin_item, door and flip_condition survive validation without a schema change.
- [src/schema/kb.ts:8] The schema comment already names tracks and resolution_ref as keys seen on open_question atoms.
- [settled · Q2] Same .kb layout everywhere.
- [gap] How kb-ingest handles open_question atoms whose status changes often is unverified.

Excerpt src/store/validate-kb.ts:79:
```
function collectAtomFiles(kbDir: string): string[] {
  const files: string[] = [];
  const walk = (dir: string): void => {
    for (const ent of readdirSync(dir, { withFileTypes: true })) {
      if (ent.isDirectory()) {
        if (ent.name.startsWith("_")) continue;
        walk(join(dir, ent.name));
```

Options:
- **P. Leave it unspecified until slice 3** — Write to .kb/_ledger/ without documenting it; settle the contract with the native type. Holds if nothing in slice 1 depends on the contract. If wrong: slice 1 writes to an undocumented path that a later kb-ingest may reject. Evasion flag: Revisit later, with no trigger, default or owner.
- **Q. Own .weighin/ folder until slice 3** — Keep the ledger outside .kb until the native decision type lands, then move it. Holds if Redkiln's .kb contract is still changing week to week. If wrong: two footprints per repo, and a migration of every ledger at slice 3.
- **R. Yes: reserve it and teach kb-ingest now** — Document .kb/_ledger/ as reserved in the .kb README and teach kb-ingest the intake keys in slice 1. Holds if the key names survive slice 1 use without much churn. If wrong: a key rename touches the README and kb-ingest, not stored atoms (passthrough keeps them valid).


## wi-2b90f3: Should Weigh-In run reliance drills, occasionally marking a deliberately weaker option as recommended, to check that you still review?

Why it matters: You expect to accept the recommendation most of the time. Endorsed defaults are strong and disclosure does not weaken them, so an unmeasured rubber stamp is the main way this skill could make your decisions worse.

Door: two-way (Drills can be switched off at any sitting.). Blast radius: module. Resolve by: before slice 2 metrics ship.

Evidence:
- [Jachimowicz 2019] Defaults work mainly through perceived endorsement (d = 0.68 across 58 studies).
- [Bruns 2018] Disclosing a default does not weaken it.
- [Vaccaro 2024] Human–AI teams did worse than the better of the two on decision tasks (g = −0.23, 106 studies).
- [Ma 2025] Options-first deliberative display raised accuracy from 52.4% to 59.8% but median time per decision from 33 s to 236 s.

Options:
- **P. Never pre-select; options first on every item** — Show options and evidence before any recommendation, on every item. Holds if you accept roughly seven times longer per decision. If wrong: every sitting slows down, and the habit of running /weigh-in erodes.
- **Q. No drills; watch override rate and time-to-decide** — Trust the metrics every sitting already records. Holds if override rate and time-to-decide are enough signal that you still review. If wrong: a rubber-stamp habit goes unseen, because a low override rate looks the same as good recommendations.
- **R. Yes, two-way items only, 1 in 30 for 60 days** — A drill marks a weaker option as recommended on a two-way item and is disclosed before the sitting writes anything back. Holds if one extra careful look per 30 items is worth a measured check on your review habit. If wrong: mild annoyance; a missed drill is reverted before write-back, so nothing wrong is recorded.
