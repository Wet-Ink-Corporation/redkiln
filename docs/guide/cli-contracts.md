# CLI contracts

Each product is a full CLI first (the harness-agnostic proposal), and siblings reach each other's domains through typed contracts, events or `--json` (blueprint design rules 4 and 6). The SPEC's `cli.md` fixes the result envelope and exit codes. This chapter is the behaviour those contracts need.

## Guardrails

### CL-1 Render, never parse

Typed values are the source; Markdown is a rendering. A human-facing artifact is a canonical JSON sidecar plus a rendered section stamped with its template and digest. At every entry, the renderer re-renders from the type and diffs; drift is recorded, restored from the type and gated, never parsed back into state. Text receipts between plugins are replaced by typed contracts.

- **Why.** Every system that parsed its own prose made comments load-bearing: redkiln-rs checkpoints grep prose, so a `Cargo.toml` comment "is kept because Checkpoint 5 greps the literal string"; the old audit's collator scraped criteria from Markdown where they were never written and scored 5 of 33 dimensions.
- **Evidence.** redkiln-rs ADR-0005, ADR-0189; blueprint design rule 6; redkiln-old scoresheet §7.

### CL-2 `--json` is a versioned contract

Every machine document carries its schema id (`<name>/<n>`), changes additively within a version, and is checked by golden fixtures. A consumer pins the version it reads. A CLI that "exits 0 whenever it writes a report and 1 only on its own errors" leaves policy to the caller (weigh-in `ci-check`). Streamed lines carry a discriminator and readers dispatch on it.

- **Evidence.** weigh-in `weighin.ci-check/1`, `crates/weighin/src/ci/report.rs`; anneal `status --json` (`schema: 1`, additive); assay `schemas/` and golden fixtures; SPEC CLI; prior art below (cargo `--format-version`).

### CL-3 A transport failure is never a verdict

Outcomes are typed so that "the reviewer said no" and "the reviewer never spoke" cannot share a value: `Result<Verdict, TransportError>`, and "could not evaluate" is distinct from "evaluated and failed" all the way to telemetry.

- **Why.** In the Node Redkiln one HTTP 529 produced a committed `changes-requested` seal whose only finding was "review failed to produce a digest"; a `not-ready` gate (exit 2) was sealed as `changes-requested`; a non-blocking failure was indistinguishable from a pass until `gate_result.basis` (evaluated, memoized, warned) existed, and then it was dropped again on one path.
- **Evidence.** redkiln-old `workflows/forge-implement.js:2150,2175-2181`, B2-03, A2-03, A3-02.

### CL-4 Unknown flags are errors, and every parsed flag is honoured

Parse strictly. A typo must not become a different command.

- **Why.** With `parseArgs strict:false`, `--commmit` became a dry run; `doctor --fix-ids --dry-run` parsed `dryRun` but never forwarded it, so ids were rewritten.
- **Evidence.** redkiln-old readiness review M2, M7.

### CL-5 One freshness contract for every read command

Commands that claim to reflect current state share one helper that decides when to rebuild. A malformed input fails every reader the same way.

- **Why.** Three Node read commands had three freshness rules despite identical docstrings; a malformed item made `reindex` exit 1 while `status` exited 0 with a clean board.
- **Evidence.** redkiln-old readiness review B2.

### CL-6 A back-reference keeps its slot when its target fails

In a batch format where `@N` refers to the Nth operation, a failed operation keeps its slot and a reference to it is refused; it never shifts later references onto the wrong item.

- **Evidence.** weigh-in CHANGELOG 0.6.0, #95.

### CL-7 Every declared field of a result is checked against what its consumers read

Outcomes with different shapes are different enum variants (`Halted`, `Completed`) with shared always-fields, so a consumer cannot read a field that only the success path carries.

- **Why.** A Node command gated on `designReview`, which only the success return carried, so it read `undefined` exactly when the design review had failed.
- **Evidence.** redkiln-old `AGENTS.md` ("The seam contract"), `workflows/forge-implement.js:2630`.

### CL-8 Defaults are chosen for the suite, not the dogfood repository

A default (a file-extension list, a template section a gate parses, a language assumption) is tested against at least one repository that is not the one it was built in, and a gate whose input is missing fails visibly, never degrades silently to advisory.

- **Why.** The Node evidence-citation guard accepted only web file extensions and rejected 566 of 990 Rust citations in happenstance; `templates/spec.md` never shipped the `## PR boundary` section a gate parsed, so every new repository started with two of four story checks silently off.
- **Evidence.** redkiln-old readiness ruling C1, C2, A4-01, A4-07.

## Patterns worth copying

- **Exit codes as a closed set the SPEC owns**, with "wrote a report" separate from "the report says no" (weigh-in `ci-check`; SPEC `cli.md`).
- **Answers from a UI are untrusted input**, validated against the card revision they were read from; a stale revision is refused, a partial batch fails the command after applying the rest, and a timed-out write says "may have been applied" and re-reads (weigh-in B-30, B-32, `crates/weighin-core/src/board.rs`).
- **Dated, version-keyed tables** for anything that depends on an external tool's version, refused when the version has no row ([HA-7](headless-agents.md#ha-7-pin-the-cli-version-key-version-sensitive-argv-by-it-and-refuse-an-unknown-pin)).
- **A "do" or "don't" code block in documentation is compiled or must fire the diagnostic it claims** (temper `tests/doc_examples.py`); every rule id cited anywhere exists (temper `tests/registry.py`). The SPEC's requirement ids deserve the same check.
- **Port a regex dialect, not just the patterns.** temper's Rust port reproduces JavaScript's `m` and `i` semantics and UTF-16 back-windows, because byte-identical parity was the only way to ship without re-litigating 56 rules (temper wi-962170 and following).

## Prior art

- clig.dev: stdout for data, stderr for messages, `--json` for scripts, additive changes, documented exit codes, crash-only recovery, `NO_COLOR`, `--no-input`. **Agree; adopt** `NO_COLOR` and crash-only re-runs as a hook rule. [Command Line Interface Guidelines](https://clig.dev/)
- Cargo: `cargo metadata` "is stable and versioned", and callers "should pass `--format-version` explicitly"; streamed messages dispatch on `reason`. **Adopt** a format-version argument on `--json` commands. [Cargo external tools](https://doc.rust-lang.org/cargo/reference/external-tools.html)
- kubectl names group and version explicitly because structures change between versions. **Agree**: our `schema` field. [kubectl resource printing](https://kubectl.docs.k8s.io/guides/resource_printing/fields/)
- gh's `--json <fields>` makes callers name what they read. **Optional**: field selection keeps agent contexts small. [gh formatting](https://cli.github.com/manual/gh_help_formatting)
- Rust API Guidelines: meaningful errors, sealed traits, private fields. **Adopt** `#[non_exhaustive]` on public error enums in kit and contract crates, so a new variant is not a break across separately versioned plugins. [API Guidelines checklist](https://rust-lang.github.io/api-guidelines/checklist.html)
- cargo-semver-checks lints rustdoc JSON before and after. **Adopt** on `*-contract` and kit crates against the last `rk-*` tag. [Toward fearless cargo update](https://predr.ag/blog/toward-fearless-cargo-update/)
- Anthropic recommends poka-yoke tool design (absolute paths only, closed enums). **Agree.** [Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)
