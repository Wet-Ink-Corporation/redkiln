# Weigh-In rubric, with its sources

Read this only when a filter in SKILL.md is unclear for a specific item. Every rule here has a
source; the research is in "Settle agent questions while the context is warm" (2026-09-26).

## Why filter hard

- Human–AI teams did worse than the better of the two alone on decision tasks (Hedges' g = −0.23,
  106 studies; Vaccaro et al., Nature Human Behaviour 2024). Ryan's attention belongs on value
  trade-offs and one-way doors, where his context is the stronger input.
- Agents told to ask over-ask; exploring first reached the same information with about half the
  questions (Ambig-SWE, ICLR 2026). Filter 2 (recoverable) is that finding.
- Users stop trusting a tool past about 10% effective false positives (Tricorder, CACM 2018). Target:
  ≥60% of presented items acted on, ≤10% dismissed as not useful.

## Filter 1: settled decisions

compound-engineering's settled-decisions protocol: classify conversation decisions as settled,
directive or unlabeled; an agent may not settle its own proposal; "never re-ask a settled decision";
research may contradict a settled decision "only on evidence". Accepted `.kb` decision atoms are
immutable in Redkiln: a contradiction becomes a supersede card that shows the evidence.

## Filter 3: authority and doors

Authority-bound (always a human): security, billing and money, data retention and residency,
migrations, public contracts and APIs, client-facing behaviour, product intent the user alone knows.
Everything else is judgment-bound: "uncertainty resolves to fixing … not escalating"
(compound-engineering, adjudicate-before-escalate, 2026-09-22).

Doors (Bezos, 2015 letter): one-way = "consequential and irreversible or nearly irreversible";
two-way = "changeable, reversible". Anchors for blast radius: file < module < system (touches more
than one component or a shared contract) < external (clients, data leaving the repo, money).

## Filter 4: decide now or defer

Waiting has value only if a specific fact would change the choice (value of information; Duke's
"is there some information I could find out that would change my mind?"). Options expire (Matts &
Maassen). "Last responsible moment" is widely criticised as licensed procrastination
(Wirfs-Brock 2011); decisions that fix conceptual integrity or spawn many follow-ons have their most
responsible moment early (Zimmermann 2024). A legitimate deferral names: the flip fact, the trigger
that brings it, a resolve-by, the default that applies if nobody acts, and an owner (Seeds: every
state change takes a reason).

## Filter 5: evasion patterns

| Pattern | Clears only if the card states | Source |
|---|---|---|
| Make it configurable | the second concrete value and who needs it now | Xu et al. FSE 2015: most parameters are never changed; wrongly kept defaults cause up to 53% of config errors |
| Behind a feature flag | flag kind, removal owner, removal trigger | Hodgson: flags are inventory with carrying cost; Uber Piranha found 17% stale |
| Interface to swap later | a second implementation within the planning horizon | Metz: the wrong abstraction costs more than duplication; Fowler: refactoring that eases change is not YAGNI |
| Revisit later / phase 2 / ticket | a passing decide-now test | Fowler's debt quadrant: only deliberate, prudent debt with a named payoff |
| Riskiest part last | its weakest "what would have to be true" tested first | Zimmermann: worst first |
| TODO / temporary | a ledger item with default and trigger, or removal | agent debt comments name problems without resolution (MSR 2026) |
| Both / it depends / needs a spike | a conditional pick "A unless X", or a time-boxed spike and the result that flips it | synthesis |

## Expressing risk

No risk matrices, heat colours or likelihood × impact scores: typical matrices compare fewer than
10% of random hazard pairs correctly and can rank risks backwards (Cox 2008); expert 90% intervals
held the truth only 55–78% of the time (Hubbard 2010). Use instead: MUSTs as pass/fail (Kepner-Tregoe),
consequences in natural units, reversibility × blast radius, two to four "what would have to be true"
conditions per option with the weakest flagged (Roger Martin), and a swing statement ("A wins unless
you weight X above Y").

## Presenting the recommendation

Pre-selecting is a default, and defaults move choices (d = 0.68 across 58 studies; Jachimowicz 2019);
disclosure does not weaken them (Bruns 2018). So pre-select only when earned: not a one-way door,
passes the evasion check, stable across re-runs. Always show the flip beside it ("A unless X").
Longer explanations raise trust without raising accuracy (Steyvers 2025): keep cards short.
First-person uncertainty lowered agreement and raised accuracy (Kim, FAccT 2024): say "I'm not sure"
when the runs split. Options-first displays cost about seven times the decision time (Ma, CHI 2025):
use them only for one-way doors.

## Sequencing

Upstream and one-way decisions first; regenerate cards that depend on them after they land; within a
tier, worst first. Batch to reduce context switches, not for "decision fatigue" (a 36-lab
preregistered test found d = 0.06; Vohs 2021).
