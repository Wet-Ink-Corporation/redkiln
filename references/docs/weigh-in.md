<!-- Exported from the Claude Doc https://claude.ai/code/artifact/3f5a93c0-85de-4461-8d42-a143401634cc (revision 33) on 2026-10-19 as part of the project handover. The Doc is the original; this copy is frozen. Diagrams (1) could not be exported and show as [embedded content] placeholders; open the Doc to see them. -->

# Weigh-In: what the plugin should entail

Oct 18, 2026 · @Ryan

## Summary

Weigh-In should stay what 0.9.0 already is, the place where an agent's assumptions and questions are settled by you, and become the one place every Redkiln plugin sends a question for you. Capture, the sitting, the board and decision atoms carry over almost unchanged. What changes is who raises items, how they raise them, and where the answer travels.

- **One inbox for the suite.** Bench parking a call, Loop at a human gate, Review needing an owner ack and Rust asking to approve a lint exception all raise the same item. Today only anneal does, by piping text to `weighin batch` and parsing the receipt.
- **A typed decision request.** Every ask carries a question, two to four options, a recommendation, a door, a default, an optional deadline and the hash of what it is about. Siblings append it through `weighin-contract`, and Weigh-In's guards run inside that crate.
- **Answers reach the asker.** In chat, the end-of-chat sitting settles items as today. An unattended run takes the default on a two-way item and blocks on a one-way one (wi-4d8b6a), and its question reaches you through a shared rendezvous (pressure-test call B). A defaulted two-way decision yields a proposed atom flagged for your review, never an accepted one.
- **A slimmer binary.** About 1,700 lines of store, env and git code go to `redkiln-core`, and the 1,107-line atom writer goes to Knowledge. `weighin` keeps the ledger domain, capture, the sitting's operations and the board.
- **Calls.** Five, in the last section. Call A, the decision request's shape in Core's SPEC, is the one that is hard to undo, because every plugin's events will carry it.

## What the repos already do

Weigh-In is the most finished plugin in the suite, and four siblings already lean on it without a contract. The work is to give those links a typed shape, not to redesign the product.

| Repo | What it does today | Worth lifting |
| --- | --- | --- |
| weigh-in (0.9.0, 2026-10-14) | The v1.0 candidate: 17.5k lines of Rust in `weighin-core` and `weighin`, 274 tests. Command hooks capture `weigh-in` blocks and TODOs at zero tokens; `/weigh-in` filters, asks up to four cards and records each answer; decisions land as `.kb` atoms with a `Weigh-In:` trailer. Works in Claude Code, Cowork through the device shell (D-0002), Projects tasks with tappable cards (#126) and the Windows desktop app (#127). Capture precision 15 of 15 on the eval seed; the two-week pilot (#23) closes on or after 2026-10-20 | Nearly all of it: the item lifecycle (13 event types), the CLI guards, the marker grammar, the board exchange format, the eval seed and the `Health` metrics |
| weigh-in-plugin | The release-only repository claude.ai org sync reads: one force-pushed commit per release with about 17.8 MB of binaries (B-19) | The pattern, generalised into the suite's one release repository (Blueprint call 11) |
| anneal | `orchestrate park` raises and defers through `weighin batch`, finding the binary on `PATH`, and parses the receipt from text lines (`src/weighin/receipt.rs`). The ruled-before hook (D6) reads accepted decision atoms before each prompt. Its skills must never emit `weigh-in` blocks, or the Stop hook would raise them twice | The two uses, as the first `weighin-contract` callers: raise-and-defer, and "was this ruled before" |
| assay | Specified a `weighin ci-check --json --base <ref>` read (D-WI, never filed as #44) with reason codes `weigh-in-open-blocking` and `weigh-in-pending-triage` and an owner `/assay weigh-in-ack`. Its own decisions are Weigh-In atoms, and an unattended run took 88 defaults in one journal | `ci-check` as the read API for Review and Loop |
| redkiln-rs (build/phase-0-to-1) | Human gates are a record `(token, tree_hash, deadline)` with options from `questions.toml` and typed attachments, answered only by `redkiln answer` (ADR-0009). The operator-surface proposal calls its gate view "the weigh-in inbox". The build itself used Weigh-In for approvals (wi-ef7b00) | The gate record as the decision request's shape: deadline, tree hash and attachments |
| temper | H-16 calls (dependencies, lint exceptions, removed tests) stop the agent and wait for an answer in chat. 41 Weigh-In atoms record temper's own decisions, but temper reads none of them at runtime | H-16 asks as decision requests, so an approved lint exception has a record and a hash |

Two facts shape the design. Every sibling that talks to Weigh-In today does it through text, a subprocess or a file it parses. And the long-term plan in weigh-in's own docs, slice 3, was to dissolve into Redkiln as a `decision` item type, which the suite decision replaces (Blueprint call 7).

## The decision model

Weigh-In keeps one item model for everything, and a sibling's request is an item that arrives already grounded. An agent's marker starts as a bare question that Weigh-In triages and grounds into a card; a plugin's request arrives with the card filled in, so it skips straight to being presented.

### Two ways in, one lifecycle

|  | Captured item (today) | Decision request (new) |
| --- | --- | --- |
| Raised by | An agent's `weigh-in` block, an "Open questions" bullet, a TODO, the sitting's sweep, `weighin raise` | Bench, Loop, Review or Rust through `weighin-contract` |
| Channel tag | `marker`, `scan`, `sweep`, `manual` | `request`, plus `plugin:<name>` |
| Card | Built in the sitting by triage and grounding | Supplied by the raiser; Weigh-In validates it |
| Door and authority | Hints, judged in triage | Stated by the raiser; `unsure` is treated as one-way |
| Lifecycle | raised, triaged, grounded, presented, then decided, deferred, defaulted or dismissed, then recorded | The same, starting at presented |
| Who reads the answer | The agent, through the atom and the trailer | The raiser, from the `weighin.item_decided` event |

### What a request carries

The fields extend today's `Raised` and `Card` events; nothing existing changes meaning.

| Field | Meaning | Comes from |
| --- | --- | --- |
| Question and options | Two to four options, each with when it holds and what it costs if wrong | Today's `CardOption` and its 2 to 4 guard |
| Recommendation | One option, which may not fail a MUST or be evasive | Today's CLI guards |
| Door, authority, blast | One-way or two-way; whose call it is; how far a wrong answer reaches | Today's triage fields |
| Default | The option an unattended run takes on a two-way item. Never set on one-way | wi-4d8b6a |
| Deadline | When the asker stops waiting and takes the default or gives up | Loop's gate record (ADR-0017, ADR-0198) |
| Subject | Kind, hash method and hash of what is being decided about | Core's approval record; the pressure test's hash table |
| Blocks | What waits on it: `merge`, `release`, `run`, `none` or `unknown` | assay's D-WI enum, plus `run` for Loop |
| Trust needed | `witnessed` for a local gate, `attested` for CI or an unattended run, except a run on the machine that holds the board, where a witnessed answer on the board or in a sitting suffices. Client repositories have no attested channel, so an unattended client run stops at any gate that needs one | Pressure-test call C |
| Raiser and correlation | Plugin, run or session, and the raiser's own key, so a re-raise finds its item | Today's content-hash `wi-` id, keyed on the raiser's key. New wi- ids lengthen to 10 hex characters, old 6-hex ids are kept, and CI checks for duplicates across the suite, as Knowledge call C requires |

### Who asks what

| Raiser | Asks when | Typical door | Blocks | With nobody there |
| --- | --- | --- | --- | --- |
| Bench | A one-way call during grill or plan; a fan-out task parks after three rounds | Either | `none` or `run` | Not applicable: Bench is in chat |
| Loop | A human gate in an activity; a policy check it cannot pass | Either | `run` | Two-way takes the default at the deadline; one-way stops the run and waits at the rendezvous |
| Review | An owner ack, a waiver, a finding marked needs-owner | One-way | `merge` | The check stays pending; nothing is defaulted |
| Rust | An H-16 approval: a new dependency, a lint exception, a removed test | One-way | `none` in chat, `merge` in CI | CI fails the gate with the item's id |

### Outcomes that become policy

A decision today is once: it settles one item. Some answers are rules ("never add a dependency without a licence check"). An `item_decided` event gains an applies field, `once` or `always`, as ACP's once-or-always outcomes do. A decision that always applies carries a Confirmation, the check that enforces it (MADR's field), and Knowledge serves it to later sessions as a constraint atom, using the same `applies` and `confirmation` keys its atom schema proposes. This is pressure-test recommendation 15; the field can land now and the enforcement later.

## Surfaces

The surfaces stay as they are in 0.9.0, and each one learns to show requests beside captured items. The one new surface is a rendezvous, so a question asked where you are not can be answered where you are.

| Surface | Today | In the suite |
| --- | --- | --- |
| Capture hooks | SessionStart primes the convention; Stop and SubagentStop raise marker blocks; PostToolUse scans written text. Exec form, 10 s timeout, never fail a session (B-16) | Unchanged until 1.0.0 (wi-7c3aa0). Then SessionStart injects less when it sees siblings in the log, from the suite's shared context budget. Stop never blocks, so it spends none of the eight-block budget other plugins share |
| The sitting (`/weigh-in`) | Sweep, five filters, up to four cards per ask, record each answer, close. Projects tasks use tappable cards; Cowork and the Windows desktop run the binary through the device shell | Requests join the same queue, ranked first by what they block: `merge` and `run` ahead of `none`. A request's card is shown as the raiser wrote it, with the raiser named |
| The board | `board.html` published as an Artifact with a `db` capability; answers validated as untrusted and checked against `card_rev` | Grows into the suite board on `redkiln-view`, as the visualization study recommends: decisions, Loop gates and Review owner actions in one Needs Ryan view, all answered through Weigh-In's path. That view is Weigh-In's tab on the one suite board Artifact, beside Backlog's tabs. Client repositories get no Artifact and no self-hosted web; their decisions are taken in the in-session sitting, the CLI or the suite mod pane (wi-7c21e4) |
| The mod (#112) | Planned: a band, a local board pane, `/wi` commands, decisions surfaced on edits (D-0017) | Folds into the one suite mod, as one tab |
| The CLI | `open`, `show`, `context`, `raise`, `batch`, `decide`, `defer`, `digest`, `stats`, `board`, `journal` | Adds `ci-check --json`, versioned, for Review and Loop. `batch` stays for skills and shells; plugins use the contract crate |

### Where an unattended question is answered

The log is per clone, so an answer given on your laptop never reaches a run in a container (pressure-test gap 1). A request raised with nobody present is published to the shared record, and Weigh-In imports the answer back as an event. Weigh-In stays the item's owner; the rendezvous is only where the question and answer travel.

| Asker runs in | Question goes to | You answer by | Trust of the answer |
| --- | --- | --- | --- |
| Your chat (Claude Code, Cowork, Projects) | The sitting in that chat | A card | Witnessed |
| Loop on your own machine | The local log | The board or the next sitting on that machine. A witnessed answer there suffices for an unattended gate on that machine | Witnessed |
| A cloud session or Actions, own repository with Backlog | A GitHub issue of a Decision type, linked to the run | A comment command or label from a maintainer | Attested |
| A cloud session or Actions, own repository without Backlog | A ref in the ledger repository | The board, which writes the answer with its deploy key | Attested |
| Review in CI | A pending check and the PR summary | A comment command such as `/review weigh-in-ack`, read by the Review App | Attested |
| Anything on a client repository | Nowhere off the machine | Only on the machine that holds the board. An unattended run stops at any gate that needs an attested answer, the review gate included, until someone answers locally | Witnessed |

Renovate's dependency dashboard is the precedent for the GitHub row: an issue with checkboxes works as an approval board on a phone, with nothing to host. The answer's trust level is what lets a gate accept it (pressure-test call C): a one-way request from CI accepts only an attested answer.

## Data, events and hand-offs

Weigh-In keeps the decision domain and gives away its plumbing. About 1,700 lines move to Core and 1,107 to Knowledge, and a new contract crate gives siblings a typed way in.

### Crates

| Crate | Holds | Built from |
| --- | --- | --- |
| `weighin-contract` (new) | Request and answer types, the `weighin.*` event names with a version each, the guards (2 to 4 options, a valid recommendation, one-way never defaulted), and the JSON types of `open --json` and `ci-check --json`. Depends only on Core | `event.rs`, `card.rs` and the guards now in `ops.rs` |
| `weighin-core` | The projection fold, triage and cards, the marker keys and the TODO scanner, the board exchange format | Today's `weighin-core` minus `atom.rs` |
| `weighin` (binary) | Hooks, the sitting's operations, context packs, the CLI | Today's `weighin` minus `store.rs`, `env.rs`, `git.rs` and `journal.rs` |

### What moves out

| Today | Lands in | Note |
| --- | --- | --- |
| `store.rs`: the TRUNCATE opening, WAL conversion, `user_version`, the 0.1.x and 0.2.2 fixtures | `redkiln-core` | Core takes weigh-in's WAL conversion and anneal's typed errors |
| `env.rs` store resolution and the override chain (B-02) | `redkiln-core` | The override for synced Cowork folders becomes one suite override |
| `git.rs`: common dir, worktree, tree hash per ADR-0105 clause 4 | `redkiln-core` | One worktree id spelling for every plugin |
| `journal.rs` and the cloud journal hook (B-21) | Core's export checkpoint at Stop and pre-push | Where journals live is pressure-test call A |
| `atom.rs`: the `.kb` writer and MADR body | `knowledge-contract` | Weigh-In calls it to write each decision atom. If Knowledge calls B and C pass, atoms move to .kb/decision/ with kb-decision-<10 hex> ids, so the wi- id in today's file names stops being the key |
| The `weigh-in` block's shell (fence, `key: value`) | Core SPEC, end-of-turn grammar | Weigh-In keeps its key set |
| `.kb/_ledger/config.json` | `.redkiln/suite.toml` for repo class and store; a `[weigh-in]` table for owner, board mode and caps | A missing config means client (pressure-test recommendation 11) |

### Events

Weigh-In's 13 event types become `weighin.item_raised` through `weighin.item_acknowledged` in the shared log, with the `item:`, `session:`, `worktree:` and `channel:` tags they carry now and a `plugin:weighin` tag. Weigh-In reads only its own types and skips the rest. Every write stays DCB-guarded (`NotYetRaised`, `ItemUnchangedSince`), now through Core's helpers. The `Weigh-In: <id> decided <X>` commit trailer stays, since Loop treats trailers as tier-1 authority.

### Moving the data

The existing ledger at `<git-common-dir>/weighin/ledger.sqlite3` and the committed journals are imported once into `<git-common-dir>/redkiln/events.sqlite3` and never deleted, because Cowork folders refuse deletes. The happenstance move from 0.3.2 to 0.4.1 happens once, in Core. Ryan's own redkiln-rs checkout has a live Weigh-In ledger, so the import has a real test case beyond the fixtures.

## Prior art and patterns

The approval systems agree on four ideas: a durable wait with a deadline and a named fallback, an answer validated before it is accepted, an identity the requester cannot forge, and a choice between once and always. Weigh-In already has the first two inside one chat; the suite needs them across machines.

| Prior art | What it is | Pattern to take | What to leave |
| --- | --- | --- | --- |
| [Temporal's approval pattern](https://docs.temporal.io/design-patterns/approval) | A workflow blocks on a condition with a timeout until a signal or update carries the approval | Every wait has a maximum and a fallback path, such as reject or escalate. An update with a validator tells the approver at once whether the answer was accepted. Duplicate deliveries need idempotency keys | Running a workflow engine. The event log and the raiser's correlation key give the same guarantees |
| [LangGraph interrupts](https://docs.langchain.com/oss/python/langgraph/human-in-the-loop) | `interrupt()` pauses a graph, a checkpointer saves state under a thread id, and `Command(resume=…)` continues it | The raiser resumes from its own checkpoint, keyed by the request's correlation id. The node reruns on resume, so side effects before the ask must be idempotent | An in-process graph. Loop's question may be answered on another machine, by another run |
| [ACP permission requests](https://agentclientprotocol.com/protocol/tool-calls) | Agent Client Protocol's `session/request_permission`, with option kinds `allow_once`, `allow_always`, `reject_once`, `reject_always` | Once or always as the scope of a decision, so an answer can become policy | A prompt per tool call, which interrupts more than it decides |
| [GitHub environments](https://docs.github.com/en/enterprise-server@3.1/actions/deployment/targeting-different-environments/using-environments-for-deployment) | A job that names an environment pauses until one of up to six required reviewers approves | An approval bound to a GitHub identity the agent does not hold, which is what "attested" means | Environments are repository settings only the owner can change, and they carry no options or recommendation |
| [Renovate's Dependency Dashboard](https://docs.renovatebot.com/key-concepts/dashboard/) | One issue listing pending updates, released by ticking a checkbox | An issue as a phone-friendly approval queue with nothing to host. Its docs warn that people forget to check the dashboard, so SessionStart should name what waits | One dashboard issue per repository; a request per issue links better to its run |
| MADR 4 | The decision-record template Weigh-In's atoms already follow, Confirmation section included (`atom.rs:369`) | Today the Confirmation is a prose review trigger. For a decision that always applies it should name the check that enforces it | Nothing; it is already the format |
| HumanLayer (per the pressure test, not re-checked today) | An SDK for human approval of agent actions | Its per-tool-call approvals were deprecated and plan-level checkpoints survived, which supports a batched sitting over interruptions | Per-call approval |

## How it composes with the rest of the suite

Weigh-In owns the decision domain and nothing else: siblings ask through its contract and read its answer as an event, and none of them settles a question for you on its own.

[embedded content: Weigh-In's place in the suite · 5 raisers, 3 places to answer]

Questions arrive from five kinds of asker, go out to wherever you can answer, and come back as one event the asker and Knowledge both read.

- **Bench** raises a one-way call it parks during grill or plan, and fan-out rulings (`orchestrate.ruled`) as two-way items. Its ruled-before hook asks Knowledge, not Weigh-In, whether something was decided before.
- **Loop** raises a request at each human gate, with a deadline and its subject's hash: the file sha256 for a plan, design or artifact approval, and the tree hash only for a gate on this snapshot. It takes the default on a two-way item at the deadline, stops on a one-way one, and resumes when the answer's event arrives. `redkiln answer` stays as a fallback (ADR-0009).
- **Review** reads `ci-check --json` as a gate and raises owner acks. A one-way request from CI accepts only an attested answer.
- **Redkiln Rust** raises H-16 approvals, so an approved lint exception or dependency has a record bound to the change fingerprint.
- **Knowledge** writes each decision as an atom through `knowledge-contract` and answers the sitting's "already decided" filter, which today searches `.kb` files through context packs.
- **Backlog**, where a repository has it, is where an unattended request becomes a Decision issue, and a deferral with a trigger becomes an issue, as the Backlog study proposed.
- **Core** owns the store, the envelope with origin and trust level, the end-of-turn grammar's shell, the export checkpoint and the repo-class gate that keeps client items on the machine.

## Calls for Ryan and next steps

Five calls shape the plugin. Each has a recommendation, and only A is hard to undo, because every plugin's events will carry its shape.

| # | Call | Recommendation | Door |
| --- | --- | --- | --- |
| A | Put the decision request record (fields above) in Core's SPEC v1 as the one shape for every plugin's human gate | Yes | One-way once events are written |
| B | Siblings raise items through `weighin-contract`, with the guards inside the crate; anneal's subprocess bridge retires and `weighin batch` stays for skills and shells | Yes | Two-way |
| C | Build `weighin ci-check --json` in Weigh-In 1.1, before the monorepo move: versioned, with `blocks` as `merge`, `release`, `run`, `none` or `unknown`, and assay's reason codes | Yes, and file assay's D-WI contract with it | Two-way |
| D | Same question as Knowledge call G, which owns it: an `applies` field (`once` or `always`) and a named Confirmation check on `item_decided`. | See Knowledge G | See Knowledge G |
| E | Ship 1.0.0 from the weigh-in repository as planned; Weigh-In moves into the monorepo as 2.0.0 with the one-time store import, after the pilot report | Yes | Two-way |

These calls are already asked elsewhere and decide part of this plugin, so they are not repeated here. They are Blueprint call 7 (Loop's gates become Weigh-In requests; slice 3 is superseded) and Blueprint call 9 (install IDs: Weigh-In installs as `rk-weigh-in`, and `/weigh-in` stays the skill name). The pressure test's call B (the unattended rendezvous) and call C (trust levels) are also asked there. Decision authority per person (pressure-test recommendation 20) waits until colleagues join.

### Suggested next steps

- Ship 1.0.0 once week 2 of the pilot (#23) closes, on or after 2026-10-20, with no new code (B-23).
- Ryan decides calls A, B, C and E (D is Knowledge call G), plus Blueprint call 7 and the pressure test's calls B and C.
- Write the decision request record into Core's SPEC draft, taking Loop's gate fields and assay's D-WI enums.
- File D-WI in weigh-in and build `ci-check` for 1.1.
- Spike the rendezvous: Loop raises a request in a cloud session, it is answered by a comment on a Decision issue, and the answer imports back as an attested `weighin.item_decided`.
- When anneal moves into Bench, port its park bridge onto `weighin-contract` as the first sibling caller.
