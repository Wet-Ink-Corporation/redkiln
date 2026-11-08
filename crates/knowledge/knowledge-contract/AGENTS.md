# Crate charter: `knowledge-contract`

## Purpose
Knowledge's atom contract (`references/docs/knowledge.md`, The atom model): the eight kinds, status, authority tiers, atom ids and the paths they set, the frontmatter writer and the frontmatter reader. Producers write atoms through it and readers decode them through it, so every atom has the same keys in the same order and every reader sees the same fields. Born at Move 1 step 4, ahead of the Knowledge plugin, because Weigh-In is the first producer; `knowledge validate`, `query` and the rest arrive with Knowledge.

## Layer
contract (Knowledge's). Pure: no file, clock, process or environment access. Writing the file and looking at what is already in `.kb/` is the producer's.

## Allowed dependencies
- Workspace crates: none. Core is allowed by the layer rules, but this crate stays light so pure domain crates (weighin-core) can link it; `crates/weigh-in/weighin/tests/it/atom_ids.rs` checks its ID-04 hashing against Core's.
- Third-party: `serde_json` (frontmatter strings are JSON strings), `sha2` (ID-04 hashes).
- Never: anything that does I/O, happenstance, tokio, or another plugin's crates.

## Public surface
`Kind`, `Status`, `AuthorityTier`, `Reversibility`, `AtomId` and `HashLen`, `Frontmatter` with `Field` and `Value`, `slug`, `yaml_str` and `yaml_list`, and the `read` module (`frontmatter`, `RawFrontmatter`). Workspace-internal, never published.

## Invariants
- An atom lives at `.kb/<kind>/<key>-<slug>.md`, where `<key>` follows `kb-<kind>-` in its id; status is a field, never a folder, so a path never moves (Knowledge call B).
- Ids already written are kept (`AtomId::kept`); new ones are `kb-<kind>-` and 10 hex of ID-04 over the domain `redkiln.id/knowledge.<kind>/1` and the producer's key, extended to 12 then 16 on a collision (call C).
- The frontmatter writes the ten required keys in their order, then the decision-only keys, then the producer's lineage keys; every free text is a JSON string, so no value breaks the YAML.
- The reader reads what the writer writes, and hand-edited atoms like it; it is not a YAML parser.

## Errors
None: constructors that can refuse return `Option` (`AtomId::kept`, `Kind::parse`, `read::frontmatter`).

## Test strategy
Unit tests beside each module, doctests on the public items. Weigh-In's atom goldens (`weighin-core` `atom`) pin the writer's bytes against 1.x's.

## Do not
- Write a kind, status or path outside this crate's types.
- Move or rename an atom once written.
