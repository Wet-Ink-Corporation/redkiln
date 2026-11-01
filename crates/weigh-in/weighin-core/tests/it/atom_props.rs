//! Properties of `.kb` atom rendering: whatever the item holds, the front matter keeps its shape
//! and carries exactly the expected fields, each reading back to the value it was generated from.

use crate::support::{CASES, Rng, TestResult, adversarial};
use serde_json::Value;
use std::collections::BTreeMap;
use weighin_core::atom::{RenderContext, render_decision, render_open_question, y_statement};
use weighin_core::card::{Card, CardOption, Criterion, Evidence, Recommendation};
use weighin_core::event::{
    Blast, Channel, Decided, Deferred, EventEnvelope, Grounded, LedgerEvent, Provenance, Raised,
    Route, Triaged,
};
use weighin_core::marker::{Authority, Door, Kind};
use weighin_core::{Item, ItemId, Ledger};

/// Keys every rendered atom carries (Redkiln's `KbFrontmatter`).
const REQUIRED_KEYS: &[&str] = &[
    "id",
    "title",
    "kind",
    "status",
    "authority_tier",
    "summary",
    "depends_on",
    "related",
    "source_paths",
    "last_reviewed",
];

fn text(rng: &mut Rng) -> String {
    adversarial(rng, 8)
}

fn texts(rng: &mut Rng) -> Vec<String> {
    (0..rng.below(3)).map(|_| text(rng)).collect()
}

fn option(rng: &mut Rng, id: &str) -> CardOption {
    CardOption {
        id: id.to_owned(),
        title: text(rng),
        summary: text(rng),
        holds_if: text(rng),
        if_wrong: text(rng),
        fails_must: rng.one_in(3).then(|| text(rng)),
        evasion: None,
    }
}

fn card(rng: &mut Rng) -> Card {
    Card {
        why: text(rng),
        door_note: None,
        timing: text(rng),
        evidence: (0..rng.below(3))
            .map(|_| Evidence {
                src: text(rng),
                text: text(rng),
            })
            .collect(),
        excerpt: None,
        options: vec![option(rng, "A"), option(rng, "B")],
        criteria: (0..rng.below(3))
            .map(|_| Criterion {
                name: text(rng),
                values: BTreeMap::new(),
            })
            .collect(),
        recommendation: Recommendation {
            option: "A".to_owned(),
            flip: text(rng),
            tradeoff: text(rng),
            why_wrong: text(rng),
            stability: None,
        },
        evasion: None,
    }
}

fn envelope(event: LedgerEvent, session: Option<String>, ts: String) -> EventEnvelope {
    EventEnvelope {
        event,
        actor: "test".to_owned(),
        session_id: session,
        worktree: None,
        branch: None,
        head: None,
        ts,
    }
}

fn raised(rng: &mut Rng, id: ItemId, question: String, door_hint: Door) -> LedgerEvent {
    LedgerEvent::ItemRaised(Raised {
        item_id: id,
        kind: *rng.pick(&[Kind::Question, Kind::Assumption, Kind::Deferral, Kind::Call]),
        quote: text(rng),
        question,
        did: rng.one_in(2).then(|| text(rng)),
        flip: None,
        at: rng.one_in(2).then(|| text(rng)),
        sym: None,
        blocks: None,
        door_hint,
        authority_hint: Authority::Authority,
        channel: Channel::Marker,
        source_path: rng.one_in(2).then(|| text(rng)),
        source_line: None,
    })
}

/// A decision, a deferral, or nothing (the item stays open and renders no atom).
fn outcome(rng: &mut Rng, id: ItemId) -> Option<LedgerEvent> {
    match rng.below(3) {
        0 => Some(LedgerEvent::ItemDecided(Decided {
            item_id: id,
            option: (*rng.pick(&["A", "B", "Z"])).to_owned(),
            decider: text(rng),
            provenance: *rng.pick(&[Provenance::UserApproved, Provenance::UserDirected]),
            note: text(rng),
            tree_hash: rng.one_in(2).then(|| text(rng)),
            matched_recommendation: *rng.pick(&[None, Some(true), Some(false)]),
            via: text(rng),
            seconds_to_decide: None,
            reason: text(rng),
        })),
        1 => Some(LedgerEvent::ItemDeferred(Deferred {
            item_id: id,
            flip: text(rng),
            trigger: text(rng),
            resolve_by: text(rng),
            default_option: text(rng),
            owner: text(rng),
            unqualified: false,
            reason: text(rng),
        })),
        _ => None,
    }
}

/// A folded item and the inputs the generator chose for it, so the expected front matter comes
/// from what was generated rather than from the item's own accessors.
struct Generated {
    item: Item,
    /// The triaged door, or the raise's hint when the item was never triaged.
    door: Door,
    /// The triaged blast radius, if the item was triaged.
    blast: Option<Blast>,
    /// The grounding card, if the item was grounded.
    card: Option<Card>,
}

/// A raised item, folded: triaged, grounded and then decided or deferred, each at random.
fn item(rng: &mut Rng) -> TestResult<Generated> {
    const DOORS: [Door; 3] = [Door::OneWay, Door::TwoWay, Door::Unsure];
    let question = text(rng);
    let id = ItemId::mint("s", &question);
    let mut door = *rng.pick(&DOORS);
    let mut events = vec![raised(rng, id.clone(), question, door)];
    let mut blast = None;
    if rng.one_in(2) {
        door = *rng.pick(&DOORS);
        let radius = *rng.pick(&[Blast::File, Blast::System]);
        blast = Some(radius);
        events.push(LedgerEvent::ItemTriaged(Triaged {
            item_id: id.clone(),
            door,
            blast: radius,
            authority: Authority::Authority,
            route: Route::Present,
            timing: None,
            resolve_by: None,
            reason: text(rng),
        }));
    }
    let card = rng.one_in(2).then(|| card(rng));
    if let Some(c) = &card {
        events.push(LedgerEvent::ItemGrounded(Grounded {
            item_id: id.clone(),
            // The event and the expectation each keep their own copy of the card.
            card: c.clone(),
            reason: text(rng),
        }));
    }
    events.extend(outcome(rng, id));
    let session = rng.one_in(2).then(|| text(rng));
    let envelopes: Vec<EventEnvelope> = events
        .into_iter()
        .map(|e| envelope(e, session.clone(), text(rng)))
        .collect();
    let ledger = Ledger::fold((1u64..).zip(&envelopes));
    let item = ledger
        .items
        .into_values()
        .next()
        .ok_or("the raise was not folded into an item")?;
    Ok(Generated {
        item,
        door,
        blast,
        card,
    })
}

/// The wire spelling of a door, written out from the marker grammar.
fn door_spelling(door: Door) -> &'static str {
    match door {
        Door::OneWay => "one-way",
        Door::TwoWay => "two-way",
        Door::Unsure => "unsure",
    }
}

/// `KbFrontmatter`'s reversibility: a one-way door is `low`, unsure `medium`, two-way `high`.
fn reversibility_of(door: Door) -> &'static str {
    match door {
        Door::OneWay => "low",
        Door::Unsure => "medium",
        Door::TwoWay => "high",
    }
}

fn provenance_spelling(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::UserApproved => "user-approved",
        Provenance::UserDirected => "user-directed",
    }
}

fn blast_spelling(blast: Blast) -> &'static str {
    match blast {
        Blast::File => "file",
        Blast::Module => "module",
        Blast::System => "system",
        Blast::External => "external",
    }
}

/// A front-matter value: a string, or a list of strings.
fn v(s: impl Into<String>) -> Value {
    Value::String(s.into())
}

fn vs(items: &[String]) -> Value {
    Value::from(items.to_vec())
}

fn context(rng: &mut Rng) -> RenderContext {
    RenderContext {
        date: text(rng),
        depends_on: texts(rng),
        source_paths: texts(rng),
    }
}

/// The front matter's `key: value` pairs, checking its shape on the way: it opens on the first
/// line, closes on a `---` line, and each line is one key with a bare token or a JSON value that
/// carries no character YAML refuses or reads as a line break.
fn front_matter(atom: &str, case: u64) -> TestResult<BTreeMap<String, Value>> {
    let rest = atom
        .strip_prefix("---\n")
        .ok_or_else(|| format!("case {case}: no opening front matter in {atom:?}"))?;
    let (head, _body) = rest
        .split_once("\n---\n")
        .ok_or_else(|| format!("case {case}: the front matter never closes in {atom:?}"))?;
    let mut fields = BTreeMap::new();
    for line in head.split('\n') {
        assert!(
            !line.chars().any(|c| c.is_control()
                || matches!(c, '\u{2028}' | '\u{2029}' | '\u{fffe}' | '\u{ffff}')),
            "case {case}: raw control or line break in {line:?}"
        );
        let (key, value) = line
            .split_once(": ")
            .ok_or_else(|| format!("case {case}: {line:?} is not `key: value`"))?;
        assert!(
            !key.is_empty() && key.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "case {case}: key {key:?}"
        );
        let bare = !value.is_empty()
            && value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '-' || c == '_');
        let parsed = if bare {
            Value::String(value.to_owned())
        } else {
            serde_json::from_str(value)
                .map_err(|e| format!("case {case}: {key} is not JSON ({e}): {value:?}"))?
        };
        assert!(
            fields.insert(key.to_owned(), parsed).is_none(),
            "case {case}: {key} repeats"
        );
    }
    for key in REQUIRED_KEYS {
        assert!(fields.contains_key(*key), "case {case}: {key} is missing");
    }
    Ok(fields)
}

/// The item's own source path, then the context's extras.
fn sources(item: &Item, ctx: &RenderContext) -> Vec<String> {
    item.source_path
        .iter()
        .chain(&ctx.source_paths)
        .cloned()
        .collect()
}

/// Every field a decision atom's front matter must carry, with its value, built from the
/// generated inputs. An item with no records relates to nothing.
fn expected_decision(
    generated: &Generated,
    ctx: &RenderContext,
) -> TestResult<BTreeMap<String, Value>> {
    let item = &generated.item;
    let decision = item
        .decision
        .as_ref()
        .ok_or("a decision atom for an undecided item")?;
    let chosen = generated
        .card
        .as_ref()
        .and_then(|c| c.options.iter().find(|o| o.id == decision.option));
    // The chosen option's title, or the question without its question marks and the answer.
    let title = chosen.map_or_else(
        || {
            format!(
                "{}: {}",
                item.question.trim_end_matches('?'),
                decision.option
            )
        },
        |o| o.title.clone(),
    );
    let mut expected = BTreeMap::from([
        ("id", v(format!("kb-decision-{}", item.id))),
        ("title", v(title)),
        ("kind", v("decision")),
        ("status", v("accepted")),
        ("authority_tier", v("decision")),
        ("summary", v(y_statement(item))),
        ("depends_on", vs(&ctx.depends_on)),
        ("related", vs(&[])),
        ("source_paths", vs(&sources(item, ctx))),
        ("last_reviewed", v(ctx.date.as_str())),
        ("reversibility", v(reversibility_of(generated.door))),
        ("phase", v("null")),
        ("supersedes", vs(&[])),
        ("superseded_by", v("null")),
        ("weighin_item", v(item.id.as_str())),
        ("question", v(item.question.as_str())),
        ("door", v(door_spelling(generated.door))),
        ("decider", v(decision.decider.as_str())),
        ("provenance", v(provenance_spelling(decision.provenance))),
        ("decided_at", v(decision.ts.as_str())),
    ]);
    if let Some(b) = generated.blast {
        expected.insert("blast_radius", v(blast_spelling(b)));
    }
    if let Some(t) = &decision.tree_hash {
        expected.insert("tree_hash", v(t.as_str()));
    }
    if let Some(c) = &generated.card {
        expected.insert("recommended", v(c.recommendation.option.as_str()));
        if !c.recommendation.flip.is_empty() {
            expected.insert("flip_condition", v(c.recommendation.flip.as_str()));
        }
    }
    Ok(expected
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect())
}

/// Every field an open-question atom's front matter must carry, with its value.
fn expected_open_question(
    generated: &Generated,
    ctx: &RenderContext,
) -> TestResult<BTreeMap<String, Value>> {
    let item = &generated.item;
    let d = item
        .deferral
        .as_ref()
        .ok_or("an open-question atom for an item never deferred")?;
    let summary = format!(
        "Open until {}: {}. Default if unresolved: {}.",
        d.trigger, item.question, d.default_option
    );
    Ok([
        ("id", v(format!("kb-open-question-{}", item.id))),
        ("title", v(item.question.as_str())),
        ("kind", v("open_question")),
        ("status", v("proposed")),
        ("authority_tier", v("note")),
        ("summary", v(summary)),
        ("depends_on", vs(&ctx.depends_on)),
        ("related", vs(&[])),
        ("source_paths", vs(&sources(item, ctx))),
        ("last_reviewed", v(ctx.date.as_str())),
        ("tracks", v(item.id.as_str())),
        ("resolve_by", v(d.resolve_by.as_str())),
        ("trigger", v(d.trigger.as_str())),
        ("default_if_unresolved", v(d.default_option.as_str())),
        ("owner", v(d.owner.as_str())),
        ("flip_condition", v(d.flip.as_str())),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect())
}

#[test]
fn decision_atoms_keep_their_front_matter_whatever_the_item_holds() -> TestResult {
    let mut rendered = 0u32;
    for case in 0..CASES {
        let mut rng = Rng::new(0xa7_0001, case);
        let generated = item(&mut rng)?;
        let item = &generated.item;
        let ctx = context(&mut rng);

        let Some(atom) = render_decision(item, &ctx) else {
            assert!(item.decision.is_none(), "case {case}: decided items render");
            continue;
        };

        rendered += 1;
        assert_eq!(
            front_matter(&atom, case)?,
            expected_decision(&generated, &ctx)?,
            "case {case}: front matter"
        );
        assert_eq!(
            render_decision(item, &ctx).as_ref(),
            Some(&atom),
            "case {case}: deterministic"
        );
    }
    assert!(rendered > 0, "no case rendered a decision atom");
    Ok(())
}

#[test]
fn open_question_atoms_keep_their_front_matter_whatever_the_item_holds() -> TestResult {
    let mut rendered = 0u32;
    for case in 0..CASES {
        let mut rng = Rng::new(0xa7_0002, case);
        let generated = item(&mut rng)?;
        let item = &generated.item;
        let ctx = context(&mut rng);

        let Some(atom) = render_open_question(item, &ctx) else {
            assert!(
                item.deferral.is_none(),
                "case {case}: deferred items render"
            );
            continue;
        };

        rendered += 1;
        assert_eq!(
            front_matter(&atom, case)?,
            expected_open_question(&generated, &ctx)?,
            "case {case}: front matter"
        );
        assert_eq!(
            render_open_question(item, &ctx).as_ref(),
            Some(&atom),
            "case {case}: deterministic"
        );
    }
    assert!(rendered > 0, "no case rendered an open-question atom");
    Ok(())
}
