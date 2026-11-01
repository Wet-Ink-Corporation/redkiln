//! `.kb` atoms rendered from ledger items.
//!
//! An atom is valid against Redkiln's `KbFrontmatter` (`src/schema/kb.ts`): `id`, `title`,
//! `kind`, `status`, `authority_tier`, `summary`, `depends_on`, `related`, `source_paths` and
//! `last_reviewed`, plus decision-only fields. The schema is `.passthrough()`, so the Weigh-In
//! lineage keys survive validation.
//!
//! The body of a decision atom is MADR 4 (context, drivers, considered options, outcome,
//! consequences, confirmation) plus "why this might be wrong". YAML strings are written as JSON
//! strings, which are valid YAML scalars, so no value can break the frontmatter.

use crate::card::{Card, CardOption};
use crate::event::Deferred;
use crate::marker::Door;
use crate::projection::{Decision, Item};
use std::fmt::{self, Display, Formatter};

/// Where a decision atom is written, relative to the repository root.
#[must_use]
pub fn decision_path(item: &Item) -> String {
    format!(
        ".kb/_intake/decisions/{}-{}.md",
        item.id,
        slug(&item.question)
    )
}

/// Where an open-question atom is written, relative to the repository root.
#[must_use]
pub fn open_question_path(item: &Item) -> String {
    format!(
        ".kb/_intake/open_questions/{}-{}.md",
        item.id,
        slug(&item.question)
    )
}

/// The `id` of the decision atom for an item.
#[must_use]
pub fn decision_atom_id(item: &Item) -> String {
    format!("kb-decision-{}", item.id)
}

/// The `id` of the open-question atom for an item.
#[must_use]
pub fn open_question_atom_id(item: &Item) -> String {
    format!("kb-open-question-{}", item.id)
}

/// Lowercase ASCII slug of at most 48 characters on a word boundary.
#[must_use]
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.len() <= 48 {
        return out;
    }
    // `out` holds only ASCII alphanumerics and `-`, so every offset is a char boundary and
    // `get` never misses; the fallbacks keep the function total without a panic path.
    let cut = out.get(..48).unwrap_or(&out);
    cut.rfind('-')
        .and_then(|i| cut.get(..i))
        .unwrap_or(cut)
        .to_owned()
}

/// A YAML-safe double-quoted scalar. JSON escaping covers C0 controls, quotes and backslashes;
/// YAML also refuses DEL, the C1 controls, U+FFFE/U+FFFF and treats U+2028/U+2029 as line breaks,
/// so those are escaped too.
fn q(s: &str) -> String {
    let json = serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    json.chars()
        .map(|c| {
            let u = u32::from(c);
            if (0x7f..=0x9f).contains(&u) || matches!(u, 0x2028 | 0x2029 | 0xfffe | 0xffff) {
                format!("\\u{u:04x}")
            } else {
                c.to_string()
            }
        })
        .collect()
}

fn list(items: &[String]) -> String {
    if items.is_empty() {
        "[]".into()
    } else {
        format!(
            "[{}]",
            items.iter().map(|s| q(s)).collect::<Vec<_>>().join(", ")
        )
    }
}

/// Reversibility for `KbFrontmatter`: one-way doors are `low`, unsure `medium`, two-way `high`.
#[must_use]
pub fn reversibility(door: Door) -> &'static str {
    match door {
        Door::OneWay => "low",
        Door::Unsure => "medium",
        Door::TwoWay => "high",
    }
}

/// Inputs the item alone does not carry.
#[derive(Debug, Clone, Default)]
pub struct RenderContext {
    /// ISO date (YYYY-MM-DD) for `last_reviewed`.
    pub date: String,
    /// Accepted atom ids that were MUSTs on this card.
    pub depends_on: Vec<String>,
    /// Extra source paths (ledger location, PR URL).
    pub source_paths: Vec<String>,
}

/// The Y-statement used as the atom's summary.
#[must_use]
pub fn y_statement(item: &Item) -> String {
    let Some(dec) = &item.decision else {
        return item.question.clone();
    };
    let card = item.card.as_ref();
    let chosen = card.and_then(|c| c.option(&dec.option));
    let chosen_title = chosen.map_or_else(|| dec.option.clone(), |o| o.title.clone());
    let neglected: Vec<String> = card
        .map(|c| {
            c.options
                .iter()
                .filter(|o| o.id != dec.option)
                .map(|o| o.title.clone())
                .collect()
        })
        .unwrap_or_default();
    let why = card
        .map(|c| c.why.as_str())
        .filter(|w| !w.is_empty())
        .unwrap_or("the question an agent raised");
    let mut s = format!(
        "In the context of \"{}\", facing {}, we decided for {}",
        item.question,
        lower_first(clause(why)),
        chosen_title
    );
    if !neglected.is_empty() {
        s.push_str(" and neglected ");
        s.push_str(&neglected.join("; "));
    }
    if let Some(o) = chosen {
        let holds_if = clause(&o.holds_if);
        if !holds_if.is_empty() {
            s.push_str(", on the premise that ");
            s.push_str(holds_if);
        }
        let if_wrong = clause(&o.if_wrong);
        if !if_wrong.is_empty() {
            s.push_str(", accepting that if wrong: ");
            s.push_str(if_wrong);
        }
    }
    // `trim_end` ends on a character boundary, so the truncation cannot split a character.
    s.truncate(s.trim_end().len());
    s.push_str(full_stop(&s));
    s
}

/// `s` without its trailing whitespace and full stops, to sit before a comma or a full stop.
/// A trailing '?' or '!' stays.
fn clause(s: &str) -> &str {
    s.trim_end_matches(|c: char| c == '.' || c.is_whitespace())
}

/// The full stop that ends a sentence ending in `s`: none when `s` already ends in '.', '?' or
/// '!'.
fn full_stop(s: &str) -> &'static str {
    if s.ends_with(['.', '?', '!']) {
        ""
    } else {
        "."
    }
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f)
            if !s.starts_with("I ")
                && !s.chars().nth(1).is_some_and(|x| x.is_ascii_uppercase()) =>
        {
            f.to_lowercase().collect::<String>() + c.as_str()
        }
        Some(f) => f.to_string() + c.as_str(),
        None => String::new(),
    }
}

/// The item's own source path, then the caller's extras.
fn source_list(item: &Item, ctx: &RenderContext) -> Vec<String> {
    item.source_path
        .iter()
        .chain(&ctx.source_paths)
        .cloned()
        .collect()
}

/// A decision atom, written by its [`Display`] impl. Every write goes to the formatter with `?`,
/// so no failure is discarded; the only error source is the sink, and a `String` has none.
struct DecisionAtom<'a> {
    item: &'a Item,
    dec: &'a Decision,
    ctx: &'a RenderContext,
    card: Option<&'a Card>,
    chosen: Option<&'a CardOption>,
    title: String,
}

impl<'a> DecisionAtom<'a> {
    fn new(item: &'a Item, dec: &'a Decision, ctx: &'a RenderContext) -> Self {
        let card = item.card.as_ref();
        let chosen = card.and_then(|c| c.option(&dec.option));
        let title = match chosen {
            Some(o) => o.title.clone(),
            None => format!("{}: {}", item.question.trim_end_matches('?'), dec.option),
        };
        Self {
            item,
            dec,
            ctx,
            card,
            chosen,
            title,
        }
    }

    fn frontmatter(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let related: Vec<String> = self
            .item
            .records
            .iter()
            .filter(|r| r.atom_kind == "open_question")
            .map(|r| r.atom_id.clone())
            .collect();
        writeln!(f, "---")?;
        writeln!(f, "id: {}", q(&decision_atom_id(self.item)))?;
        writeln!(f, "title: {}", q(&self.title))?;
        writeln!(
            f,
            "kind: decision\nstatus: accepted\nauthority_tier: decision"
        )?;
        writeln!(f, "summary: {}", q(&y_statement(self.item)))?;
        writeln!(f, "depends_on: {}", list(&self.ctx.depends_on))?;
        writeln!(f, "related: {}", list(&related))?;
        writeln!(
            f,
            "source_paths: {}",
            list(&source_list(self.item, self.ctx))
        )?;
        writeln!(f, "last_reviewed: {}", q(&self.ctx.date))?;
        writeln!(f, "reversibility: {}", reversibility(self.item.door()))?;
        writeln!(f, "phase: null\nsupersedes: []\nsuperseded_by: null")?;
        self.decision_fields(f)?;
        writeln!(f, "---\n")
    }

    fn decision_fields(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let (item, dec) = (self.item, self.dec);
        writeln!(f, "weighin_item: {}", q(item.id.as_str()))?;
        writeln!(f, "question: {}", q(&item.question))?;
        writeln!(f, "door: {}", item.door().as_str())?;
        if let Some(b) = item.blast {
            writeln!(f, "blast_radius: {}", b.as_str())?;
        }
        writeln!(f, "decider: {}", q(&dec.decider))?;
        writeln!(f, "provenance: {}", dec.provenance.as_str())?;
        writeln!(f, "decided_at: {}", q(&dec.ts))?;
        if let Some(t) = &dec.tree_hash {
            writeln!(f, "tree_hash: {}", q(t))?;
        }
        if let Some(r) = self.card.map(|c| &c.recommendation) {
            writeln!(f, "recommended: {}", q(&r.option))?;
            if !r.flip.is_empty() {
                writeln!(f, "flip_condition: {}", q(&r.flip))?;
            }
        }
        Ok(())
    }

    fn problem(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let item = self.item;
        writeln!(f, "# {}\n", self.title)?;
        writeln!(f, "## Context and problem statement\n")?;
        writeln!(f, "{}\n", item.question)?;
        if let Some(c) = self.card
            && !c.why.is_empty()
        {
            writeln!(f, "{}\n", c.why)?;
        }
        write!(
            f,
            "Raised by an agent ({}) as a {} and captured by Weigh-In as `{}`.",
            item.channel.as_str(),
            item.kind.as_str(),
            item.id
        )?;
        if let Some(at) = &item.at {
            write!(f, " Anchor: `{at}`.")?;
        }
        writeln!(f, "\n")
    }

    fn outcome(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let dec = self.dec;
        writeln!(f, "## Decision outcome\n")?;
        write!(
            f,
            "Chosen option: **{}**",
            self.chosen
                .map_or(dec.option.as_str(), |o| o.title.as_str())
        )?;
        match dec.matched_recommendation {
            Some(true) => f.write_str(", the recommended option.")?,
            Some(false) => write!(
                f,
                ", overriding the recommendation ({}).",
                self.card.map_or("?", |c| c.recommendation.option.as_str())
            )?,
            None => f.write_str(".")?,
        }
        writeln!(f, "\n")?;
        if !dec.note.is_empty() {
            writeln!(f, "Decider's note: {}\n", dec.note)?;
        }
        Ok(())
    }

    fn consequences(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Some(o) = self.chosen else {
            return Ok(());
        };
        writeln!(f, "### Consequences\n")?;
        let holds_if = clause(&o.holds_if);
        if !holds_if.is_empty() {
            writeln!(
                f,
                "- Good, because it holds if {holds_if}{}",
                full_stop(holds_if)
            )?;
        }
        let if_wrong = clause(&o.if_wrong);
        if !if_wrong.is_empty() {
            writeln!(
                f,
                "- Bad, because if wrong: {if_wrong}{}",
                full_stop(if_wrong)
            )?;
        }
        writeln!(f)
    }

    fn confirmation(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Some(c) = self.card else {
            return Ok(());
        };
        writeln!(f, "### Confirmation\n")?;
        if c.recommendation.flip.is_empty() {
            writeln!(f, "Revisit if the premises above stop holding.\n")?;
        } else {
            writeln!(f, "Revisit when: {}\n", c.recommendation.flip)?;
        }
        if !c.recommendation.why_wrong.is_empty() {
            writeln!(
                f,
                "## Why this might be wrong\n\n{}\n",
                c.recommendation.why_wrong
            )?;
        }
        Ok(())
    }

    fn provenance(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let (item, dec) = (self.item, self.dec);
        writeln!(f, "## Provenance\n")?;
        writeln!(
            f,
            "- Decided {} by {} ({}), via {}.",
            dec.ts,
            dec.decider,
            dec.provenance.as_str(),
            dec.via
        )?;
        if let Some(s) = &item.session_id {
            writeln!(f, "- Raised in session `{s}`.")?;
        }
        writeln!(
            f,
            "- Verbatim: {}",
            q(item.quote.lines().next().unwrap_or(""))
        )
    }
}

impl Display for DecisionAtom<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.frontmatter(f)?;
        self.problem(f)?;
        if let Some(c) = self.card {
            write_card_sections(f, c, &self.dec.option)?;
        }
        self.outcome(f)?;
        self.consequences(f)?;
        self.confirmation(f)?;
        self.provenance(f)
    }
}

/// Render the decision atom for a decided item.
#[must_use]
pub fn render_decision(item: &Item, ctx: &RenderContext) -> Option<String> {
    let dec = item.decision.as_ref()?;
    // `to_string` panics only when the `Display` impl errs, and this one errs only when its
    // sink does; a `String` sink never fails.
    Some(DecisionAtom::new(item, dec, ctx).to_string())
}

/// Document sections shared by both atoms: drivers, every considered option, evidence. `chosen`
/// is the id of the option to mark, or empty when none is chosen.
fn write_card_sections(f: &mut Formatter<'_>, c: &Card, chosen: &str) -> fmt::Result {
    if !c.criteria.is_empty() {
        writeln!(f, "## Decision drivers\n")?;
        for cr in &c.criteria {
            writeln!(f, "- {}", cr.name)?;
        }
        writeln!(f)?;
    }
    writeln!(f, "## Considered options\n")?;
    for o in &c.options {
        write_option(f, o, chosen)?;
    }
    if !c.evidence.is_empty() {
        writeln!(f, "## Evidence\n")?;
        for e in &c.evidence {
            writeln!(f, "- `{}`: {}", e.src, e.text)?;
        }
        writeln!(f)?;
    }
    Ok(())
}

fn write_option(f: &mut Formatter<'_>, o: &CardOption, chosen: &str) -> fmt::Result {
    let mark = if o.id == chosen { " (chosen)" } else { "" };
    writeln!(f, "### {} · {}{}\n", o.id, o.title, mark)?;
    if !o.summary.is_empty() {
        writeln!(f, "{}\n", o.summary)?;
    }
    let holds_if = clause(&o.holds_if);
    if !holds_if.is_empty() {
        writeln!(f, "- Holds if {holds_if}{}", full_stop(holds_if))?;
    }
    let if_wrong = clause(&o.if_wrong);
    if !if_wrong.is_empty() {
        writeln!(f, "- If wrong: {if_wrong}{}", full_stop(if_wrong))?;
    }
    if let Some(m) = o.fails_must.as_deref().map(clause) {
        writeln!(f, "- Fails a MUST: {m}{}", full_stop(m))?;
    }
    writeln!(f)
}

/// A tracking atom for a deferred item, written by its [`Display`] impl.
struct OpenQuestionAtom<'a> {
    item: &'a Item,
    deferral: &'a Deferred,
    ctx: &'a RenderContext,
}

impl OpenQuestionAtom<'_> {
    fn frontmatter(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let (item, d) = (self.item, self.deferral);
        writeln!(f, "---")?;
        writeln!(f, "id: {}", q(&open_question_atom_id(item)))?;
        writeln!(f, "title: {}", q(&item.question))?;
        writeln!(
            f,
            "kind: open_question\nstatus: proposed\nauthority_tier: note"
        )?;
        writeln!(
            f,
            "summary: {}",
            q(&format!(
                "Open until {}: {}. Default if unresolved: {}.",
                d.trigger, item.question, d.default_option
            ))
        )?;
        writeln!(f, "depends_on: {}", list(&self.ctx.depends_on))?;
        writeln!(f, "related: []")?;
        writeln!(f, "source_paths: {}", list(&source_list(item, self.ctx)))?;
        writeln!(f, "last_reviewed: {}", q(&self.ctx.date))?;
        writeln!(f, "tracks: {}", q(item.id.as_str()))?;
        writeln!(f, "resolve_by: {}", q(&d.resolve_by))?;
        writeln!(f, "trigger: {}", q(&d.trigger))?;
        writeln!(f, "default_if_unresolved: {}", q(&d.default_option))?;
        writeln!(f, "owner: {}", q(&d.owner))?;
        writeln!(f, "flip_condition: {}", q(&d.flip))?;
        writeln!(f, "---\n")
    }
}

impl Display for OpenQuestionAtom<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let d = self.deferral;
        self.frontmatter(f)?;
        writeln!(f, "# {}\n", self.item.question)?;
        writeln!(
            f,
            "Deferred on purpose. Waiting for: {}. Resolve by: {}. If nobody acts, **{}** applies.\n",
            d.trigger, d.resolve_by, d.default_option
        )?;
        if let Some(c) = &self.item.card {
            write_card_sections(f, c, "")?;
        }
        Ok(())
    }
}

/// Render the tracking atom for a one-way item deferred past its session.
#[must_use]
pub fn render_open_question(item: &Item, ctx: &RenderContext) -> Option<String> {
    let deferral = item.deferral.as_ref()?;
    // As in `render_decision`: the `String` sink cannot fail, so `to_string` cannot panic.
    Some(
        OpenQuestionAtom {
            item,
            deferral,
            ctx,
        }
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Recommendation;
    use crate::event::*;
    use crate::id::ItemId;
    use crate::marker::{Authority, Kind};
    use crate::projection::Ledger;

    const GOLDEN_DECISION: &str = r#"---
id: "kb-decision-wi-4dfe38"
title: "Common-dir store"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Where should the ledger live?\", facing it fixes the merge story, we decided for Common-dir store and neglected Committed files: with a colon, on the premise that open items can stay local, accepting that if wrong: a lost clone loses open items."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-09-26"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-4dfe38"
question: "Where should the ledger live?"
door: two-way
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-09-26T18:00:00Z"
tree_hash: "abc"
recommended: "A"
flip_condition: "others need open items"
---

# Common-dir store

## Context and problem statement

Where should the ledger live?

It fixes the merge story.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-4dfe38`. Anchor: `src/a.rs:3`.

## Considered options

### A · Common-dir store (chosen)

- Holds if open items can stay local.
- If wrong: a lost clone loses open items.

### B · Committed files: with a colon


## Decision outcome

Chosen option: **Common-dir store**, the recommended option.

### Consequences

- Good, because it holds if open items can stay local.
- Bad, because if wrong: a lost clone loses open items.

### Confirmation

Revisit when: others need open items

## Provenance

- Decided 2026-09-26T18:00:00Z by human:ryan (user-approved), via board.
- Raised in session `s`.
- Verbatim: "q: \"Where\" should the ledger live?"
"#;

    const GOLDEN_DECISION_RICH: &str = r#"---
id: "kb-decision-wi-4dfe38"
title: "Committed files: with a colon"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Where should the ledger live?\", facing it fixes the merge story, we decided for Committed files: with a colon and neglected Common-dir store, on the premise that files stay small, accepting that if wrong: merges conflict."
depends_on: ["kb-decision-z"]
related: ["kb-open-question-x"]
source_paths: ["docs/plan.md", "https://example.test/pr/1"]
last_reviewed: "2026-09-26"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-4dfe38"
question: "Where should the ledger live?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-09-26T18:00:00Z"
tree_hash: "abc"
recommended: "A"
flip_condition: "others need open items"
---

# Committed files: with a colon

## Context and problem statement

Where should the ledger live?

It fixes the merge story.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-4dfe38`. Anchor: `src/a.rs:3`.

## Decision drivers

- Merge story

## Considered options

### A · Common-dir store

Keep it beside the repository.

- Holds if open items can stay local.
- If wrong: a lost clone loses open items.
- Fails a MUST: must survive a clone.

### B · Committed files: with a colon (chosen)

- Holds if files stay small.
- If wrong: merges conflict.

## Evidence

- `src/a.rs:3`: the store path is derived here

## Decision outcome

Chosen option: **Committed files: with a colon**, overriding the recommendation (A).

Decider's note: Ship it, then measure.

### Consequences

- Good, because it holds if files stay small.
- Bad, because if wrong: merges conflict.

### Confirmation

Revisit when: others need open items

## Why this might be wrong

the store may move

## Provenance

- Decided 2026-09-26T18:00:00Z by human:ryan (user-approved), via board.
- Raised in session `sess-1`.
- Verbatim: "q: \"Where\" should the ledger live?"
"#;

    const GOLDEN_DECISION_NO_CARD: &str = r#"---
id: "kb-decision-wi-4dfe38"
title: "Where should the ledger live: Z"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Where should the ledger live?\", facing the question an agent raised, we decided for Z."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-09-26"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-4dfe38"
question: "Where should the ledger live?"
door: two-way
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-09-26T18:00:00Z"
tree_hash: "abc"
---

# Where should the ledger live: Z

## Context and problem statement

Where should the ledger live?

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-4dfe38`. Anchor: `src/a.rs:3`.

## Decision outcome

Chosen option: **Z**.

## Provenance

- Decided 2026-09-26T18:00:00Z by human:ryan (user-approved), via board.
- Raised in session `s`.
- Verbatim: "q: \"Where\" should the ledger live?"
"#;

    const GOLDEN_DECISION_OVERRIDE_NO_CARD: &str = r#"---
id: "kb-decision-wi-4dfe38"
title: "Where should the ledger live: A"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Where should the ledger live?\", facing the question an agent raised, we decided for A."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-09-26"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-4dfe38"
question: "Where should the ledger live?"
door: two-way
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-09-26T18:00:00Z"
tree_hash: "abc"
---

# Where should the ledger live: A

## Context and problem statement

Where should the ledger live?

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-4dfe38`. Anchor: `src/a.rs:3`.

## Decision outcome

Chosen option: **A**, overriding the recommendation (?).

## Provenance

- Decided 2026-09-26T18:00:00Z by human:ryan (user-approved), via board.
- Raised in session `s`.
- Verbatim: "q: \"Where\" should the ledger live?"
"#;

    const GOLDEN_OPEN_QUESTION: &str = r#"---
id: "kb-open-question-wi-4dfe38"
title: "Where should the ledger live?"
kind: open_question
status: proposed
authority_tier: note
summary: "Open until the pilot ends: Where should the ledger live?. Default if unresolved: A."
depends_on: []
related: []
source_paths: ["docs/plan.md"]
last_reviewed: "2026-09-26"
tracks: "wi-4dfe38"
resolve_by: "before-release"
trigger: "the pilot ends"
default_if_unresolved: "A"
owner: "human:ryan"
flip_condition: "a tenant needs per-record legal hold"
---

# Where should the ledger live?

Deferred on purpose. Waiting for: the pilot ends. Resolve by: before-release. If nobody acts, **A** applies.

## Considered options

### A · Common-dir store

- Holds if open items can stay local.
- If wrong: a lost clone loses open items.

### B · Committed files: with a colon


"#;

    const GOLDEN_OPEN_QUESTION_NO_CARD: &str = r#"---
id: "kb-open-question-wi-4dfe38"
title: "Where should the ledger live?"
kind: open_question
status: proposed
authority_tier: note
summary: "Open until the pilot ends: Where should the ledger live?. Default if unresolved: A."
depends_on: ["kb-decision-z"]
related: []
source_paths: ["docs/plan.md", "https://example.test/pr/1"]
last_reviewed: "2026-09-26"
tracks: "wi-4dfe38"
resolve_by: "before-release"
trigger: "the pilot ends"
default_if_unresolved: "A"
owner: "human:ryan"
flip_condition: "a tenant needs per-record legal hold"
---

# Where should the ledger live?

Deferred on purpose. Waiting for: the pilot ends. Resolve by: before-release. If nobody acts, **A** applies.

"#;

    fn raised_event(id: &ItemId) -> LedgerEvent {
        LedgerEvent::ItemRaised(Raised {
            item_id: id.clone(),
            kind: Kind::Question,
            question: "Where should the ledger live?".into(),
            did: None,
            flip: None,
            at: Some("src/a.rs:3".into()),
            sym: None,
            blocks: None,
            door_hint: Door::TwoWay,
            authority_hint: Authority::Authority,
            channel: Channel::Marker,
            quote: "q: \"Where\" should the ledger live?".into(),
            source_path: None,
            source_line: None,
        })
    }

    fn grounded_event(id: &ItemId) -> LedgerEvent {
        LedgerEvent::ItemGrounded(Grounded {
            item_id: id.clone(),
            card: Card {
                why: "It fixes the merge story.".into(),
                options: vec![
                    CardOption {
                        id: "A".into(),
                        title: "Common-dir store".into(),
                        holds_if: "open items can stay local".into(),
                        if_wrong: "a lost clone loses open items".into(),
                        ..Default::default()
                    },
                    CardOption {
                        id: "B".into(),
                        title: "Committed files: with a colon".into(),
                        ..Default::default()
                    },
                ],
                recommendation: Recommendation {
                    option: "A".into(),
                    flip: "others need open items".into(),
                    ..Default::default()
                },
                ..Default::default()
            },
            reason: "grounded".into(),
        })
    }

    fn decided_event(id: &ItemId) -> LedgerEvent {
        LedgerEvent::ItemDecided(Decided {
            item_id: id.clone(),
            option: "A".into(),
            decider: "human:ryan".into(),
            provenance: Provenance::UserApproved,
            note: String::new(),
            tree_hash: Some("abc".into()),
            matched_recommendation: Some(true),
            via: "board".into(),
            seconds_to_decide: Some(29),
            reason: "chose".into(),
        })
    }

    fn decided_item() -> Item {
        let id = ItemId::mint("s", "Where should the ledger live?");
        let env = |event| EventEnvelope {
            event,
            actor: "t".into(),
            session_id: Some("s".into()),
            worktree: None,
            branch: None,
            head: None,
            ts: "2026-09-26T18:00:00Z".into(),
        };
        let mut l = Ledger::default();
        l.apply(1, &env(raised_event(&id)));
        l.apply(2, &env(grounded_event(&id)));
        l.apply(3, &env(decided_event(&id)));
        l.get(&id).unwrap().clone()
    }

    #[test]
    fn decision_atom_has_every_required_field() {
        let item = decided_item();
        let text = render_decision(
            &item,
            &RenderContext {
                date: "2026-09-26".into(),
                ..Default::default()
            },
        )
        .unwrap();
        for f in [
            "id:",
            "title:",
            "kind: decision",
            "status: accepted",
            "authority_tier:",
            "summary:",
            "depends_on:",
            "related:",
            "source_paths:",
            "last_reviewed:",
            "reversibility: high",
        ] {
            assert!(text.contains(f), "missing {f}\n{text}");
        }
        assert!(text.contains("weighin_item: \"wi-"));
        assert!(text.contains("### A · Common-dir store (chosen)"));
        assert!(text.contains("Revisit when: others need open items"));
        // Quotes inside values are escaped, not breaking the YAML.
        assert!(text.contains("\\\"Where\\\""));
    }

    fn ctx() -> RenderContext {
        RenderContext {
            date: "2026-09-26".into(),
            ..Default::default()
        }
    }

    /// `decided_item` with every optional part of the atom switched on.
    fn rich_decided_item() -> Item {
        let mut item = decided_item();
        item.source_path = Some("docs/plan.md".into());
        item.blast = Some(Blast::Module);
        item.session_id = Some("sess-1".into());
        item.records.push(crate::projection::Record {
            path: ".kb/_intake/open_questions/x.md".into(),
            atom_id: "kb-open-question-x".into(),
            atom_kind: "open_question".into(),
        });
        item.records.push(crate::projection::Record {
            path: ".kb/_intake/decisions/y.md".into(),
            atom_id: "kb-decision-y".into(),
            atom_kind: "decision".into(),
        });
        if let Some(d) = item.decision.as_mut() {
            d.note = "Ship it, then measure.".into();
            d.matched_recommendation = Some(false);
            d.option = "B".into();
        }
        if let Some(c) = item.card.as_mut() {
            c.criteria.push(crate::card::Criterion {
                name: "Merge story".into(),
                ..Default::default()
            });
            c.evidence.push(crate::card::Evidence {
                src: "src/a.rs:3".into(),
                text: "the store path is derived here".into(),
            });
            c.recommendation.why_wrong = "the store may move".into();
            if let Some(o) = c.options.get_mut(0) {
                o.summary = "Keep it beside the repository.".into();
                o.fails_must = Some("must survive a clone".into());
            }
            if let Some(o) = c.options.get_mut(1) {
                o.holds_if = "files stay small.".into();
                o.if_wrong = "merges conflict.".into();
            }
        }
        item
    }

    fn deferred_item(with_card: bool) -> Item {
        let mut item = decided_item();
        item.decision = None;
        item.status = crate::projection::Status::Deferred;
        item.source_path = Some("docs/plan.md".into());
        if !with_card {
            item.card = None;
        }
        item.deferral = Some(Deferred {
            item_id: item.id.clone(),
            flip: "a tenant needs per-record legal hold".into(),
            trigger: "the pilot ends".into(),
            resolve_by: "before-release".into(),
            default_option: "A".into(),
            owner: "human:ryan".into(),
            unqualified: false,
            reason: "deferred".into(),
        });
        item
    }

    #[test]
    fn decision_atom_is_byte_exact() {
        let text = render_decision(&decided_item(), &ctx()).unwrap();
        assert_eq!(text, GOLDEN_DECISION);
    }

    #[test]
    fn decision_atom_with_every_optional_part_is_byte_exact() {
        let mut c = ctx();
        c.depends_on = vec!["kb-decision-z".into()];
        c.source_paths = vec!["https://example.test/pr/1".into()];
        let text = render_decision(&rich_decided_item(), &c).unwrap();
        assert_eq!(text, GOLDEN_DECISION_RICH);
    }

    #[test]
    fn decision_atom_without_a_card_is_byte_exact() {
        let mut item = decided_item();
        item.card = None;
        if let Some(d) = item.decision.as_mut() {
            d.matched_recommendation = None;
            d.option = "Z".into();
        }
        let text = render_decision(&item, &ctx()).unwrap();
        assert_eq!(text, GOLDEN_DECISION_NO_CARD);
    }

    #[test]
    fn decision_atom_overriding_without_a_card_names_a_question_mark() {
        let mut item = decided_item();
        item.card = None;
        if let Some(d) = item.decision.as_mut() {
            d.matched_recommendation = Some(false);
        }
        let text = render_decision(&item, &ctx()).unwrap();
        assert_eq!(text, GOLDEN_DECISION_OVERRIDE_NO_CARD);
    }

    #[test]
    fn a_decision_atom_needs_a_decision() {
        assert_eq!(render_decision(&deferred_item(true), &ctx()), None);
    }

    #[test]
    fn open_question_atom_is_byte_exact() {
        let text = render_open_question(&deferred_item(true), &ctx()).unwrap();
        assert_eq!(text, GOLDEN_OPEN_QUESTION);
    }

    #[test]
    fn open_question_atom_without_a_card_is_byte_exact() {
        let mut c = ctx();
        c.depends_on = vec!["kb-decision-z".into()];
        c.source_paths = vec!["https://example.test/pr/1".into()];
        let text = render_open_question(&deferred_item(false), &c).unwrap();
        assert_eq!(text, GOLDEN_OPEN_QUESTION_NO_CARD);
    }

    #[test]
    fn an_open_question_atom_needs_a_deferral() {
        assert_eq!(render_open_question(&decided_item(), &ctx()), None);
    }

    #[test]
    fn y_statement_is_byte_exact() {
        assert_eq!(
            y_statement(&decided_item()),
            r#"In the context of "Where should the ledger live?", facing it fixes the merge story, we decided for Common-dir store and neglected Committed files: with a colon, on the premise that open items can stay local, accepting that if wrong: a lost clone loses open items."#
        );
        assert_eq!(
            y_statement(&rich_decided_item()),
            r#"In the context of "Where should the ledger live?", facing it fixes the merge story, we decided for Committed files: with a colon and neglected Common-dir store, on the premise that files stay small, accepting that if wrong: merges conflict."#
        );
    }

    /// `decided_item` with its card's options edited by `edit`.
    fn decided_item_with(edit: impl FnOnce(&mut [CardOption])) -> Item {
        let mut item = decided_item();
        edit(&mut item.card.as_mut().expect("decided_item has a card").options);
        item
    }

    #[test]
    fn y_statement_trims_trailing_space_before_a_comma() {
        let item = decided_item_with(|o| o[0].holds_if = "open items can stay local. ".into());

        assert_eq!(
            y_statement(&item),
            r#"In the context of "Where should the ledger live?", facing it fixes the merge story, we decided for Common-dir store and neglected Committed files: with a colon, on the premise that open items can stay local, accepting that if wrong: a lost clone loses open items."#
        );
    }

    #[test]
    fn y_statement_does_not_add_a_full_stop_after_a_question_mark() {
        let item =
            decided_item_with(|o| o[0].if_wrong = "does a lost clone lose open items?".into());

        assert_eq!(
            y_statement(&item),
            r#"In the context of "Where should the ledger live?", facing it fixes the merge story, we decided for Common-dir store and neglected Committed files: with a colon, on the premise that open items can stay local, accepting that if wrong: does a lost clone lose open items?"#
        );
    }

    #[test]
    fn y_statement_ends_once_after_a_neglected_title_with_a_full_stop() {
        let item = decided_item_with(|o| {
            o[0].holds_if.clear();
            o[0].if_wrong.clear();
            o[1].title = "Keep v1.".into();
        });

        assert_eq!(
            y_statement(&item),
            r#"In the context of "Where should the ledger live?", facing it fixes the merge story, we decided for Common-dir store and neglected Keep v1."#
        );
    }

    #[test]
    fn option_lines_end_once_after_a_question_mark_or_trailing_space() {
        let item = decided_item_with(|o| {
            o[0].holds_if = "open items can stay local. ".into();
            o[0].if_wrong = "does a lost clone lose open items?".into();
            o[1].fails_must = Some("must survive a clone. ".into());
        });

        let text = render_decision(&item, &ctx()).expect("the item is decided");

        for line in [
            "- Good, because it holds if open items can stay local.\n",
            "- Bad, because if wrong: does a lost clone lose open items?\n",
            "- Holds if open items can stay local.\n",
            "- If wrong: does a lost clone lose open items?\n",
            "- Fails a MUST: must survive a clone.\n",
        ] {
            assert!(text.contains(line), "{line:?} missing from:\n{text}");
        }
    }

    #[test]
    fn whitespace_only_premises_are_omitted() {
        let item = decided_item_with(|o| {
            o[0].holds_if = "  ".into();
            o[0].if_wrong = " \n".into();
        });

        let summary = y_statement(&item);
        let text = render_decision(&item, &ctx()).expect("the item is decided");

        assert_eq!(
            summary,
            r#"In the context of "Where should the ledger live?", facing it fixes the merge story, we decided for Common-dir store and neglected Committed files: with a colon."#
        );
        for absent in ["Good, because", "Bad, because", "Holds if", "If wrong"] {
            assert!(!text.contains(absent), "{absent:?} in:\n{text}");
        }
    }

    #[test]
    fn yaml_hostile_characters_are_escaped() {
        let v = q("a\u{7f}b\u{85}c\u{2028}d\u{ffff}");
        assert_eq!(v, "\"a\\u007fb\\u0085c\\u2028d\\uffff\"");
    }

    #[test]
    fn slug_is_bounded() {
        assert_eq!(
            slug("Where should the ledger live?"),
            "where-should-the-ledger-live"
        );
        assert!(slug(&"word ".repeat(40)).len() <= 48);
    }
}
