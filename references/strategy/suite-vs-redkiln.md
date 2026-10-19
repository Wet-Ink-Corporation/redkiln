# One suite of plugins, or everything inside Redkiln?

*2026-10-12. Grounded in the six repositories configured for this project, read at their current heads.*

## The short answer

Build a **suite**: separate, independently installable plugins that get better when their siblings are installed, published from **one Redkiln marketplace**. Redkiln becomes two things at once: the name of the suite, and the optional conductor plugin that runs the others end to end, unattended. Do not fold weigh-in (or anything else) into redkiln-rs. Instead, have redkiln *read* what the others write.

This is mostly a matter of admitting what the repos already decided. Three of them were designed this way without saying so:

- **assay** ADR-01 (`assay/docs/architecture/07-decisions-assumptions.md`): "assay is its own repo and plugin; temper and Weigh-In contribute gates through two named adapters over documented JSON". The alternatives it rejected were building it into temper, and a monolith.
- **weigh-in's** own research recommended "D, built as C": a portable core plugin, with the Redkiln-native `decision` type layered on top *where Redkiln exists*, because IQBG client repos "will never run Redkiln" (`weigh-in/docs/research/notes/00-prior-weigh-in-brief-2026-09-12.html`). The report calls the Redkiln step "a mapping, not a rewrite" (`weigh-in/docs/research/report.md:504`).
- **anneal and weigh-in** already share a storage convention without a shared crate: a happenstance 0.3.2 SQLite store at `<git-common-dir>/<name>/`, TRUNCATE journal, `--store` then `$NAME_STORE` then git common dir then folder (`anneal/docs/ledger.md`). They also share the `plugin/libexec/<target>/` launcher pattern, the 1.98.1 toolchain pin and temper's gate.

The plan to move weigh-in into redkiln-rs is the outlier. It is also the step that creates the "take the whole system or nothing" problem you named.

## What each piece actually is today

| Repo | Job, in one line | Maturity | Couples to |
|---|---|---|---|
| temper | What good Rust is, and hooks that refuse anything else | Shipping; marketplace users; 2026-10 pilot on `main` | Nothing at runtime. Every other Rust repo here runs its gate |
| weigh-in | What the human decided: captures agents' assumptions and settles them at chat end | Shipping (v1 plan in flight) | Writes `.kb` atoms that pass Redkiln's `validate --kb` |
| anneal | How a session moves: grill, spec, plan, build, diagnose, verify, ship, with enforced gates | Pre-release (29 commits, active today) | Shares weigh-in's ledger design |
| assay | Whether a PR may merge: deterministic gates plus fanned-out reviewers and one required check | Spec v1.1 only, not implemented | Consumes temper and weigh-in JSON through adapters |
| redkiln-rs | A single binary that owns the loop headlessly: stages, gates, budgets, resume | Runbook and 112 ADRs; no kernel code yet; last commit 2026-09-11 | Was to absorb weigh-in (slice 3) |

Two shapes are mixed here. temper, weigh-in and anneal are **in-session plugins** (hooks and skills inside a Claude Code chat). assay and redkiln are **orchestrators** that run Claude headless from outside. That split is the natural seam, and folding the in-session tools into an orchestrator works against it.

## Suite versus monolith

| | Suite (recommended) | Everything in Redkiln |
|---|---|---|
| Adoption | A colleague installs only weigh-in, or only the Rust standard. Each piece is a one-line pitch | All or nothing. The heaviest piece sets the price of the lightest |
| Time to value | temper and weigh-in ship today and keep shipping | Waits on the least mature component (redkiln-rs has no kernel yet) |
| Client repos (IQBG) | Get the portable pieces | Get nothing unless they adopt the full loop |
| Integration | Needs written contracts, and contract tests to stop drift | Free inside one codebase |
| Duplication | Each plugin keeps its own ledger and CLI | One store, one CLI |
| Release coupling | Each plugin versions on its own; contracts version separately | One release train; any breakage blocks everything |
| Naming | Each needs a memorable name (see below) | One name, Redkiln |
| Risk if Redkiln stalls | The suite still works | Everything planned for it stalls too |

The monolith's real advantages are one ledger and zero contract drift. You can get most of the first without merging (below). The second you pay for with contract tests, a cost you already accepted in assay.

## What composition needs to actually work

These are the concrete pieces, in priority order.

1. **One marketplace.** Today temper, anneal and weigh-in are each their own marketplace. A single `Wet-Ink-Corporation/redkiln` marketplace listing every plugin is the cheapest, most visible tie: one `marketplace add`, then pick à la carte. Each repo keeps its own source; the marketplace only points at them.
2. **A short suite contract** (one document, versioned): the store-location rule all ledgers already follow; the `.kb` atom shapes; the JSON output envelope and exit codes (anneal's `data.store` / `data.source` style is a good base); and the end-of-turn grammar.
3. **Discovery and graceful degradation.** Each plugin checks for a sibling by probing its CLI or its store and reading a contract version. A missing sibling turns a feature off, never errors. Examples: weigh-in writes `.kb` atoms whether or not Redkiln is there; anneal's `verify` uses temper's gate when present and plain `cargo test` otherwise; assay's adapters already assume this.
4. **Living together in one session.** This is the risk most likely to bite. With everything installed, several `SessionStart` hooks inject context, and several `Stop` hooks can block a turn: temper's gate and review, weigh-in's capture, anneal's gates. Each also asks for its own end-of-message block. The suite needs a fixed hook order, a combined context budget, and an "all installed" end-to-end test. No repo has one today.
5. **Clear jobs where they overlap.** anneal and redkiln-rs overlap the most: both are about phases, gates and an event ledger, one in session and one headless. There are also three things called a gate (temper's gate, anneal's gates, assay's verdict). One way to split them: anneal owns *what the phases are* (its skills and playbooks); redkiln owns *running them unattended* and reuses anneal's playbooks instead of writing its own. That is the one decision here that changes redkiln-rs's design. It is yours to make.
6. **One ledger, later.** If one store matters, make it a contract, not a merge: every plugin appends to its own stream in a shared happenstance file under `<git-common-dir>/redkiln/`, and redkiln's kernel reads them all. Not needed for v1.

## Naming and identity

The constraints: "temper" collides with another Claude plugin; colleagues forget how to spell "assay" and "anneal"; Redkiln stays; each name has to stand alone and still point back to the suite.

**Where the thread lives.** It doesn't have to be in the word. Put it in four places: the marketplace (`name@redkiln`), one line in every README ("part of the Redkiln suite"), a shared visual mark (one kiln/flame glyph, a different colour or inner symbol per plugin), and the suite contract. Then each plugin's name only has to do one job: be memorable and say what it does.

**Rule for the names:** a word a colleague can spell after hearing it once, short enough to type as `/name:command`, and close to the job it names.

### Option A: everyday craft words (recommended)

Keep the heat-and-making theme, but only in common words.

| Today | Proposed | Why |
|---|---|---|
| temper | **rust-craft** | The standard is already called rust-craft; it says what it is and drops the collision. Commands read `/rust-craft:gate` |
| weigh-in | **Weigh-In** (keep) | Already plain English, already fits the theme (assayers weigh) |
| anneal | **Stoke** | Keep the fire going: moves work from phase to phase. Short, a verb, easy to spell |
| assay | **Proof** | Plain English for a test, and a proof mark is what a tested barrel gets. Verdict as noun: "did it pass Proof?" |
| redkiln-rs | **Redkiln** | The suite, and the conductor that fires the whole kiln unattended |

Tagline: *Redkiln. Craft it, weigh it, stoke it, prove it, fire it.*

### Option B: Redkiln product line

`Redkiln Craft`, `Redkiln Decide`, `Redkiln Flow`, `Redkiln Review`, with Redkiln as the conductor. Clearest suite tie, nothing to explain, but no individual personality, and longer names to type unless the plugin IDs drop the prefix.

### Option B, refined (Ryan's preference, 2026-10-12)

Ryan prefers B. "Redkiln Craft" doesn't fit, because Redkiln covers more than Rust and craft standards for other languages will follow. The proposal:

| Today | Display name | Plugin ID (what users type) |
|---|---|---|
| temper | **Redkiln Rust** | `rk-rust` (`/rk-rust:gate`) |
| weigh-in | **Redkiln Decide** (or keep Weigh-In as the product name) | `rk-decide` |
| anneal | **Redkiln Flow** | `rk-flow` |
| assay | **Redkiln Review** | `rk-review` |
| redkiln-rs | **Redkiln** (the conductor) | `redkiln` |

- **One plugin per language.** Name the plugin after the language and keep "craft" as the family word for the standards: the skill stays `rust-craft`, and later `ts-craft` or `py-craft` ship as Redkiln TypeScript or Redkiln Python. Users install only the languages they write, which is the whole point of the suite. Rule IDs and the skill name don't change.
- **No craft umbrella plugin yet.** temper's hooks, gate and scan are Rust-specific today (cargo, clippy, `.rs` edit gating). The parts that would carry over (skill-loaded gate, escape-hatch refusal, baseline ratchet, tree-bound reviewer verdict) become a shared engine once a second language exists, extracted from two real cases rather than designed for one.
- **Short `rk-` IDs.** Claude Code commands are `/plugin:skill`, so `/redkiln-review:review` is a lot to type. The `rk-` prefix is short, ties back to the suite and avoids the generic-word collisions that hit temper. The display name carries the full brand.

### Option C: keep the metallurgy, use the famous words

`Touchstone` (the standard: literally the stone gold is tested against), `Weigh-In`, `Anvil` (where work is shaped: the phases), `Crucible` or `Hallmark` (review: the stamp that certifies a piece). Richer story, still spellable. Longer to type, and "Hallmark" carries a well-known trademark.

### Things to check before choosing

- **Collisions.** A quick web search turned up nothing conclusive for these candidates; I did not do a real trademark or marketplace check. "Proof" and "Stoke" are generic words, which helps memory and hurts searchability. Do a marketplace check on the finalists.
- **Cost of renaming temper.** All of its users are internal (confirmed by Ryan, 2026-10-12), so there is no public install base to protect. The rename still touches the install ID, the `/temper:*` commands, `TEMPER_CLI`, and the vendored `.claude/temper/` path in adopted repos (weigh-in and anneal both run `node .claude/temper/temper.mjs`). That makes it a coordinated update across the internal repos plus a heads-up to colleagues, not an alias release. The only reason left to wait is the 2026-10 pilot running on `main` (wi-b63bd6), which freezes hook and gate behaviour, not names. Either rename during the pilot as a name-only change or right after it.
- **anneal and assay are cheaper still.** One is pre-release and the other is a spec. Every week that passes makes it dearer.

## Suggested order

1. Decide suite versus monolith, and the anneal/redkiln split (point 5 above).
2. Pick a naming scheme, then run a collision check on the finalists.
3. Stand up the Redkiln marketplace pointing at the existing repos under their current names. This is reversible and costs nothing.
4. Rename all three together (temper, anneal, assay) in one coordinated pass across the internal repos, timed around the pilot.
5. Write the suite contract (points 2 to 4), with an "all installed" e2e test.
6. Re-scope weigh-in's slice 3 from "move into redkiln-rs" to "redkiln reads weigh-in's ledger and atoms".
