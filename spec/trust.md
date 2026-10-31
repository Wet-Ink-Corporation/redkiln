# Trust levels

An event's `actor` is text its writer asserts. Anything that can run a binary can write it: the agent through Bash, a hook, or a contributor who commits a journal. Loop's unattended agent runs with the same rights as the hooks, so nothing in the actor stops it recording its own approval, and a fork pull request can carry a forged journal into CI. Approvals are bound to content (design rule 9); trust levels say who can be believed to have given them (pressure test, Trust: who said so).

## The levels

**TR-01 (one-way, OW-9).** Every event carries `trust`, an object with a `level` and the evidence for it. There are three levels, lowest first:

| Level | Produced by | May satisfy |
| --- | --- | --- |
| `asserted` | Any write through a CLI or a hook, and every imported journal line | Nothing that gates: context, display and telemetry only |
| `witnessed` | An answer taken through a surface the agent cannot drive (TR-03) | Local gates: Bench's plan approval, Weigh-In one-way items in a chat, Loop's gates on the machine that holds the board (TR-07) |
| `attested` | A record written by, or verifiable against, an identity the agent and the pull request's author do not hold (TR-04) | Merge gates in CI, and Loop's human gates when it runs unattended away from the board |

Trust levels are in the first release because adding them later would leave every earlier event `asserted` forever (pressure-test call C).

| Key | Type | Meaning |
| --- | --- | --- |
| `level` | string | `asserted`, `witnessed` or `attested` |
| `via` | string, absent for `asserted` | The surface or mechanism (TR-03, TR-04) |
| `by` | string or absent | The identity the surface reports, such as a GitHub login, where it reports one |
| `ref` | string or absent | Where the evidence is: a tool use id, a board submission id, a decision card's message id, a check run id, a ref and commit |
| `claimed` | string or absent | On an imported event, the level its journal line carried (TR-06) |

**TR-02.** A reader shows `actor` as a claim unless the trust level backs it: `human:ryan` at `asserted` reads as "says Ryan".

## Witnessed

**TR-03.** An event is `witnessed` only when the code that writes it took the answer from the surface itself, never from the agent's account of it. The registered `via` values:

| `via` | The answer comes from | Read by |
| --- | --- | --- |
| `ask-user-question` | An `AskUserQuestion` tool result | A hook, from its own hook input |
| `board` | A board submission, validated as untrusted and checked against the card's revision | The plugin's board import |
| `decision-card` | A Claude Projects decision card | The plugin's card import |
| `mod-pane` | A click in the suite mod's pane | The mod |
| `github-review` | A pull request review by a named user, read locally from GitHub | The plugin, with its own GitHub credentials |

A consequence for Weigh-In: an answer given on an `AskUserQuestion` card becomes `witnessed` only if the `PostToolUse` hook records it from the tool result. When the agent records the same answer with `weighin decide`, the event is `asserted`. A person typing a command in their own terminal is also `asserted`, since the binary cannot tell their shell from the agent's.

## Attested

**TR-04.** An event is `attested` when its `ref` lets a reader verify it against an identity outside the agent's and the author's reach:

| `via` | What it is | Verified by |
| --- | --- | --- |
| `github-app` | A check run, or a comment command a maintainer posted, read and recorded by the Review App | The App's check run on the commit |
| `ledger-cas` | A compare-and-set on the ledger repository, made with its App or deploy key | The ref's commit in the ledger repository |
| `signed-commit` | A commit signed by a key on the decider's GitHub account | The commit's verified signature |

## Who may set a level

**TR-05.** No CLI argument, environment variable, stdin field or config key raises an event above `asserted`. Only the code paths in TR-03 and TR-04 set `witnessed` or `attested`, and a binary that writes on behalf of another (through a contract crate) writes `asserted`.

**TR-06.** Every imported event is stored as `asserted`, whatever its journal line says; the line's level is kept in `trust.claimed` (JR-12). A gate that needs `attested` may accept an imported event by verifying its `ref` against the source itself, at read time. A `witnessed` claim cannot be re-verified away from the machine that witnessed it, so it stays `asserted` there.

**TR-07.** Each gate states the level it needs, and accepts an event only at that level or higher. On the machine that holds the board, a `witnessed` answer given on the board or in a sitting satisfies a gate that would otherwise need `attested`, because there the board is the human's own surface (weigh-in.md, Where an unattended question is answered).

**TR-08.** A `client` repository has no attested channel, since nothing leaves the machine (ST-13). An unattended run in one stops at any gate that needs `attested`, the review gate included, until someone answers on the machine that holds the board.

**TR-09.** `redkiln-guard`'s shared rule pack denies an agent's Bash call that would write an event whose actor is `human:*` (a `decide` or `approve` subcommand, a raw `append`). This closes the cheapest forgery; trust levels close the rest.
