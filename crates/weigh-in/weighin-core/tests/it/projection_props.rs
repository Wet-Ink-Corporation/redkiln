//! Properties of the projection fold over generated event sequences: raises, every later event
//! type, events for ids never raised, duplicate raises and positions out of order.

use crate::support::{CASES, Rng, TestResult, adversarial};
use std::collections::BTreeSet;
use weighin_core::card::{Card, CardOption, Recommendation};
use weighin_core::event::{
    Acknowledged, Blast, Channel, Decided, DefaultCause, Defaulted, Deferred, DismissReason,
    Dismissed, EventEnvelope, Grounded, LedgerEvent, Linked, Presented, Provenance, Raised,
    RecordWritten, Reopened, Route, Superseded, Triaged,
};
use weighin_core::id::question_key;
use weighin_core::marker::{Authority, Door, Kind};
use weighin_core::projection::Health;
use weighin_core::{Item, ItemId, Ledger, Status};

const SESSIONS: &[Option<&str>] = &[Some("s1"), Some("s2"), Some("s3"), None];

/// Four ids: the first three are raised now and then, the last one only ever by accident.
fn id_pool() -> Vec<ItemId> {
    [
        "Use Postgres?",
        "Keep the retry at three?",
        "Ship on Friday?",
        "Never raised?",
    ]
    .iter()
    .map(|q| ItemId::mint("s1", q))
    .collect()
}

fn card() -> Card {
    let option = |id: &str| CardOption {
        id: id.to_owned(),
        title: format!("Option {id}"),
        ..CardOption::default()
    };
    Card {
        options: vec![option("A"), option("B")],
        recommendation: Recommendation {
            option: "A".to_owned(),
            ..Recommendation::default()
        },
        ..Card::default()
    }
}

fn raised(rng: &mut Rng, item_id: ItemId) -> LedgerEvent {
    let question = adversarial(rng, 6);
    LedgerEvent::ItemRaised(Raised {
        item_id,
        kind: *rng.pick(&[Kind::Question, Kind::Assumption, Kind::Deferral, Kind::Call]),
        quote: question.clone(),
        question,
        did: None,
        flip: None,
        at: None,
        sym: None,
        blocks: rng.one_in(2).then(|| adversarial(rng, 3)),
        door_hint: *rng.pick(&[Door::OneWay, Door::TwoWay, Door::Unsure]),
        authority_hint: *rng.pick(&[Authority::Authority, Authority::Judgment]),
        channel: Channel::Marker,
        source_path: None,
        source_line: None,
    })
}

fn triaged(rng: &mut Rng, item_id: ItemId) -> LedgerEvent {
    LedgerEvent::ItemTriaged(Triaged {
        item_id,
        door: *rng.pick(&[Door::OneWay, Door::TwoWay, Door::Unsure]),
        blast: *rng.pick(&[Blast::File, Blast::Module, Blast::System, Blast::External]),
        authority: *rng.pick(&[Authority::Authority, Authority::Judgment]),
        route: *rng.pick(&[Route::Present, Route::Default, Route::Dismiss]),
        timing: None,
        resolve_by: None,
        reason: adversarial(rng, 3),
    })
}

fn decided(rng: &mut Rng, item_id: ItemId) -> LedgerEvent {
    LedgerEvent::ItemDecided(Decided {
        item_id,
        option: adversarial(rng, 2),
        decider: "human:owner".to_owned(),
        provenance: *rng.pick(&[Provenance::UserApproved, Provenance::UserDirected]),
        note: adversarial(rng, 3),
        tree_hash: None,
        matched_recommendation: *rng.pick(&[None, Some(true), Some(false)]),
        via: "cli".to_owned(),
        seconds_to_decide: rng.one_in(2).then(|| rng.next_u64()),
        reason: "decided".to_owned(),
    })
}

fn deferred(rng: &mut Rng, item_id: ItemId) -> LedgerEvent {
    LedgerEvent::ItemDeferred(Deferred {
        item_id,
        flip: adversarial(rng, 2),
        trigger: adversarial(rng, 2),
        resolve_by: "before-release".to_owned(),
        default_option: "A".to_owned(),
        owner: "owner".to_owned(),
        unqualified: rng.one_in(2),
        reason: String::new(),
    })
}

fn dismissed(rng: &mut Rng, item_id: ItemId) -> LedgerEvent {
    let why = match rng.below(5) {
        0 => DismissReason::NotUseful,
        1 => DismissReason::Recoverable,
        2 => DismissReason::Moot,
        3 => DismissReason::NotMyCall("legal".to_owned()),
        _ => DismissReason::DuplicateOf("wi-000000".to_owned()),
    };
    LedgerEvent::ItemDismissed(Dismissed {
        item_id,
        why,
        note: String::new(),
        reason: "dismissed".to_owned(),
    })
}

/// The event types, so the hand-written sequences below can name them.
#[derive(Debug, Clone, Copy)]
enum Ev {
    Raise,
    Triage,
    Ground,
    Present,
    Decide,
    Defer,
    Default,
    Dismiss,
    Reopen,
    Supersede,
    Link,
    Record,
    Ack,
}

const ALL_EVENTS: [Ev; 13] = [
    Ev::Raise,
    Ev::Triage,
    Ev::Ground,
    Ev::Present,
    Ev::Decide,
    Ev::Defer,
    Ev::Default,
    Ev::Dismiss,
    Ev::Reopen,
    Ev::Supersede,
    Ev::Link,
    Ev::Record,
    Ev::Ack,
];

/// One event of any type about `item_id`.
fn event(rng: &mut Rng, item_id: ItemId) -> LedgerEvent {
    let ev = *rng.pick(&ALL_EVENTS);
    make(rng, ev, item_id)
}

/// An event of type `ev` about `item_id`, its other fields generated.
fn make(rng: &mut Rng, ev: Ev, item_id: ItemId) -> LedgerEvent {
    match ev {
        Ev::Raise => raised(rng, item_id),
        Ev::Triage => triaged(rng, item_id),
        Ev::Ground => LedgerEvent::ItemGrounded(Grounded {
            item_id,
            card: card(),
            reason: "grounded".to_owned(),
        }),
        Ev::Present => LedgerEvent::ItemPresented(Presented {
            item_id,
            surface: "chat".to_owned(),
            tree_hash: rng.one_in(2).then(|| "abc".to_owned()),
            reason: "shown".to_owned(),
        }),
        Ev::Decide => decided(rng, item_id),
        Ev::Defer => deferred(rng, item_id),
        Ev::Default => LedgerEvent::ItemDefaulted(Defaulted {
            item_id,
            option: "A".to_owned(),
            cause: *rng.pick(&[
                DefaultCause::TwoWay,
                DefaultCause::Expired,
                DefaultCause::Unattended,
            ]),
            reason: "defaulted".to_owned(),
        }),
        Ev::Dismiss => dismissed(rng, item_id),
        Ev::Reopen => LedgerEvent::ItemReopened(Reopened {
            item_id,
            note: String::new(),
            reason: "reopened".to_owned(),
        }),
        Ev::Supersede => LedgerEvent::ItemSuperseded(Superseded {
            item_id,
            by: adversarial(rng, 2),
            reason: "superseded".to_owned(),
        }),
        Ev::Link => LedgerEvent::ItemLinked(Linked {
            item_id,
            kind: (*rng.pick(&["raised-again", "commit", "pr"])).to_owned(),
            target: (*rng.pick(&["s1", "s2", "s4", "abc"])).to_owned(),
            reason: "linked".to_owned(),
        }),
        Ev::Record => LedgerEvent::RecordWritten(RecordWritten {
            item_id,
            path: ".kb/x.md".to_owned(),
            atom_id: "kb-x".to_owned(),
            atom_kind: (*rng.pick(&["decision", "open_question"])).to_owned(),
            reason: "recorded".to_owned(),
        }),
        Ev::Ack => LedgerEvent::ItemAcknowledged(Acknowledged {
            item_id,
            reason: "seen".to_owned(),
        }),
    }
}

/// A sequence of up to 40 events, raises weighted up so most items exist.
fn events(rng: &mut Rng) -> Vec<EventEnvelope> {
    let pool = id_pool();
    (0..rng.below(41))
        .map(|_| {
            let id = rng.pick(&pool).clone();
            let event = if rng.one_in(4) {
                raised(rng, id)
            } else {
                event(rng, id)
            };
            EventEnvelope {
                event,
                actor: "test".to_owned(),
                session_id: rng.pick(SESSIONS).map(str::to_owned),
                worktree: None,
                branch: None,
                head: None,
                ts: adversarial(rng, 2),
            }
        })
        .collect()
}

/// Ascending positions with gaps, or, for one case in four, any positions at all.
fn positions(rng: &mut Rng, n: usize) -> Vec<u64> {
    let arbitrary = rng.one_in(4);
    let mut next = 0u64;
    (0..n)
        .map(|_| {
            if arbitrary {
                let any = rng.next_u64();
                *rng.pick(&[0, 1, 7, u64::MAX, any])
            } else {
                next = next.saturating_add(1 + u64::try_from(rng.below(3)).unwrap_or(0));
                next
            }
        })
        .collect()
}

fn fold(positions: &[u64], events: &[EventEnvelope]) -> Ledger {
    Ledger::fold(positions.iter().copied().zip(events))
}

#[test]
fn folding_is_total_deterministic_and_surfaces_orphans() {
    for case in 0..CASES {
        let mut rng = Rng::new(0x9e_0001, case);
        let events = events(&mut rng);
        let positions = positions(&mut rng, events.len());

        let ledger = fold(&positions, &events);
        let again = fold(&positions, &events);

        assert_eq!(ledger.items, again.items, "case {case}: items");
        assert_eq!(ledger.orphans, again.orphans, "case {case}: orphans");
        assert_eq!(
            ledger.head,
            positions.iter().copied().max().unwrap_or(0),
            "case {case}: head"
        );
        let mut raised = BTreeSet::new();
        let mut orphans = 0;
        for e in &events {
            if matches!(e.event, LedgerEvent::ItemRaised(_)) {
                raised.insert(e.event.item_id());
            } else if !raised.contains(e.event.item_id()) {
                orphans += 1;
            }
        }
        assert_eq!(ledger.orphans.len(), orphans, "case {case}: orphan count");
        assert_eq!(
            ledger.items.keys().collect::<BTreeSet<_>>(),
            raised,
            "case {case}: items are exactly the raised ids"
        );
    }
}

#[test]
fn queries_over_any_fold_are_total_and_consistent() {
    for case in 0..CASES {
        let mut rng = Rng::new(0x9e_0002, case);
        let events = events(&mut rng);
        let ledger = fold(&positions(&mut rng, events.len()), &events);
        let needle = if rng.one_in(2) {
            adversarial(&mut rng, 4)
        } else {
            let id = rng.pick(&id_pool()).to_string();
            id.get(rng.below(4)..3 + rng.below(8))
                .unwrap_or("wi-")
                .to_owned()
        };

        let open = ledger.open();

        assert!(open.iter().all(|i| i.status.is_open()), "case {case}: open");
        assert!(
            open.windows(2)
                .all(|w| w.first().map(|i| i.rank_key()) <= w.get(1).map(|i| i.rank_key())),
            "case {case}: open items are ranked"
        );
        assert!(
            ledger
                .unacknowledged_defaults()
                .iter()
                .all(|i| i.status == Status::Defaulted),
            "case {case}: unacknowledged defaults are defaulted"
        );
        if let Some(item) = ledger.resolve(&needle) {
            let n = needle.trim();
            let full = if n.starts_with("wi-") {
                n.to_owned()
            } else {
                format!("wi-{n}")
            };
            assert!(
                item.id.as_str().starts_with(&full),
                "case {case}: resolve({needle:?}) found {}",
                item.id
            );
        }
        let health = Health::of(&ledger);
        assert!(
            health.presented <= ledger.items.len(),
            "case {case}: presented"
        );
        assert!(
            ledger.open_in_session("s1").len() <= open.len(),
            "case {case}: session subset"
        );
        if let Some(item) = ledger.open_with_question(&needle) {
            assert!(item.status.is_open(), "case {case}: open_with_question");
        }
        if let Some(item) = ledger.any_with_question(&needle) {
            assert_eq!(
                item.question_key,
                question_key(&needle),
                "case {case}: any_with_question"
            );
        }
    }
}

/// Triage, grounding and presentation, written out per status from the lifecycle documented on
/// `Status`. Each row is the status before; its columns are the status after triage, after
/// grounding and after presentation. An open item moves forward to the event's own stage and
/// never back; presenting shows any open item; a deferred item stays parked behind its deferral;
/// a closed item (decided, defaulted, dismissed, superseded) is left alone.
const PROGRESS: [(Status, [Status; 3]); 9] = [
    (
        Status::Raised,
        [Status::Triaged, Status::Grounded, Status::Presented],
    ),
    (
        Status::Triaged,
        [Status::Triaged, Status::Grounded, Status::Presented],
    ),
    (
        Status::Grounded,
        [Status::Grounded, Status::Grounded, Status::Presented],
    ),
    (
        Status::Presented,
        [Status::Presented, Status::Presented, Status::Presented],
    ),
    (
        Status::Deferred,
        [Status::Deferred, Status::Deferred, Status::Deferred],
    ),
    (
        Status::Decided,
        [Status::Decided, Status::Decided, Status::Decided],
    ),
    (
        Status::Defaulted,
        [Status::Defaulted, Status::Defaulted, Status::Defaulted],
    ),
    (
        Status::Dismissed,
        [Status::Dismissed, Status::Dismissed, Status::Dismissed],
    ),
    (
        Status::Superseded,
        [Status::Superseded, Status::Superseded, Status::Superseded],
    ),
];

/// One event folded onto an item that existed before it, as `walk` saw it.
struct Step<'a> {
    case: u64,
    position: u64,
    before: Status,
    had_card: bool,
    last_before: u64,
    reopened_before: u32,
    event: &'a LedgerEvent,
    after: &'a Item,
}

/// Fold generated sequences one event at a time and hand `check` every event that lands on an
/// item raised earlier. A first raise must create a raised item, and no item ever disappears.
fn walk(seed: u64, mut check: impl FnMut(&Step<'_>)) -> TestResult {
    for case in 0..CASES {
        let mut rng = Rng::new(seed, case);
        let events = events(&mut rng);
        let mut ledger = Ledger::default();

        for (position, env) in (1u64..).zip(&events) {
            let id = env.event.item_id();
            let before = ledger
                .get(id)
                .map(|i| (i.status, i.card.is_some(), i.last_position, i.reopened));
            ledger.apply(position, env);
            match (before, ledger.get(id)) {
                // An orphan: the fold property above counts these.
                (None, None) => {}
                (Some(_), None) => {
                    return Err(format!("case {case}: {id} disappeared at {position}").into());
                }
                (None, Some(item)) => {
                    assert_eq!(item.status, Status::Raised, "case {case}: a first raise");
                    assert_eq!(item.raised_position, position, "case {case}: raised at");
                }
                (Some((before, had_card, last_before, reopened_before)), Some(after)) => {
                    check(&Step {
                        case,
                        position,
                        before,
                        had_card,
                        last_before,
                        reopened_before,
                        event: &env.event,
                        after,
                    });
                }
            }
        }
    }
    Ok(())
}

#[test]
fn bookkeeping_events_never_move_the_status() -> TestResult {
    let mut seen = 0u32;

    walk(0x9e_0003, |s| {
        if matches!(
            s.event,
            LedgerEvent::ItemRaised(_)
                | LedgerEvent::ItemLinked(_)
                | LedgerEvent::RecordWritten(_)
                | LedgerEvent::ItemAcknowledged(_)
        ) {
            seen += 1;
            assert_eq!(
                s.after.status,
                s.before,
                "case {}: {} at {}",
                s.case,
                s.event.event_type(),
                s.position
            );
        }
    })?;

    assert!(seen > 0, "the walk reached no bookkeeping event");
    Ok(())
}

#[test]
fn an_outcome_sets_its_own_status_from_any_status() -> TestResult {
    let mut from = BTreeSet::new();

    walk(0x9e_0005, |s| {
        let expected = match s.event {
            LedgerEvent::ItemDecided(_) => Status::Decided,
            LedgerEvent::ItemDeferred(_) => Status::Deferred,
            LedgerEvent::ItemDefaulted(_) => Status::Defaulted,
            LedgerEvent::ItemDismissed(_) => Status::Dismissed,
            LedgerEvent::ItemSuperseded(_) => Status::Superseded,
            _ => return,
        };
        from.insert(s.before);
        assert_eq!(
            s.after.status,
            expected,
            "case {}: {} at {} from {:?}",
            s.case,
            s.event.event_type(),
            s.position,
            s.before
        );
    })?;

    assert_eq!(from.len(), 9, "outcomes were folded from every status");
    Ok(())
}

#[test]
fn triage_grounding_and_presentation_follow_the_lifecycle_table() -> TestResult {
    let mut cells = BTreeSet::new();

    walk(0x9e_0006, |s| {
        let column = match s.event {
            LedgerEvent::ItemTriaged(_) => 0,
            LedgerEvent::ItemGrounded(_) => 1,
            LedgerEvent::ItemPresented(_) => 2,
            _ => return,
        };
        cells.insert((s.before, column));
        let expected = PROGRESS
            .iter()
            .find(|(before, _)| *before == s.before)
            .and_then(|(_, row)| row.get(column));
        assert_eq!(
            Some(&s.after.status),
            expected,
            "case {}: {} at {} from {:?}",
            s.case,
            s.event.event_type(),
            s.position,
            s.before
        );
    })?;

    assert_eq!(cells.len(), 27, "every cell of the table was exercised");
    Ok(())
}

#[test]
fn reopening_returns_to_raised_or_grounded_and_clears_the_outcome() -> TestResult {
    let mut with_card = BTreeSet::new();

    walk(0x9e_0007, |s| {
        if !matches!(s.event, LedgerEvent::ItemReopened(_)) {
            return;
        }
        with_card.insert(s.had_card);
        let item = s.after;
        let expected = if s.had_card {
            Status::Grounded
        } else {
            Status::Raised
        };
        assert_eq!(item.status, expected, "case {}: at {}", s.case, s.position);
        assert_eq!(
            (
                &item.decision,
                &item.default,
                &item.dismissed,
                &item.deferral
            ),
            (&None, &None, &None, &None),
            "case {}: reopen clears the outcome",
            s.case
        );
        assert_eq!(
            item.reopened,
            s.reopened_before + 1,
            "case {}: reopen count",
            s.case
        );
    })?;

    assert_eq!(with_card.len(), 2, "reopened both with and without a card");
    Ok(())
}

#[test]
fn every_event_but_a_raise_moves_the_last_position() -> TestResult {
    walk(0x9e_0008, |s| {
        let expected = if matches!(s.event, LedgerEvent::ItemRaised(_)) {
            s.last_before
        } else {
            s.position
        };
        assert_eq!(
            s.after.last_position,
            expected,
            "case {}: {} last position",
            s.case,
            s.event.event_type()
        );
    })
}

/// The status after each event of `sequence`, folded onto one item from position 1.
fn statuses(sequence: &[Ev]) -> TestResult<Vec<Status>> {
    let mut rng = Rng::new(0x9e_00ff, 0);
    let id = ItemId::mint("s1", "Use Postgres?");
    let mut ledger = Ledger::default();
    (1u64..)
        .zip(sequence)
        .map(|(position, ev)| {
            let env = EventEnvelope {
                event: make(&mut rng, *ev, id.clone()),
                actor: "test".to_owned(),
                session_id: Some("s1".to_owned()),
                worktree: None,
                branch: None,
                head: None,
                ts: format!("t{position}"),
            };
            ledger.apply(position, &env);
            let item = ledger
                .get(&id)
                .ok_or_else(|| format!("{ev:?} at {position}: no item"))?;
            Ok(item.status)
        })
        .collect()
}

#[test]
fn hand_written_sequences_reach_the_documented_statuses() -> TestResult {
    use Ev::{
        Ack, Decide, Default, Defer, Dismiss, Ground, Link, Present, Raise, Record, Reopen,
        Supersede, Triage,
    };
    use Status::{
        Decided, Defaulted, Deferred, Dismissed, Grounded, Presented, Raised, Superseded, Triaged,
    };
    let cases: [(&str, &[Ev], &[Status]); 8] = [
        (
            "the happy path",
            &[Raise, Triage, Ground, Present, Decide],
            &[Raised, Triaged, Grounded, Presented, Decided],
        ),
        (
            "a late triage does not move a grounded item back",
            &[Raise, Ground, Triage, Link, Ack, Record],
            &[Raised, Grounded, Grounded, Grounded, Grounded, Grounded],
        ),
        (
            "a deferred item stays parked",
            &[Raise, Present, Defer, Present, Triage, Ground],
            &[Raised, Presented, Deferred, Deferred, Deferred, Deferred],
        ),
        (
            "a dismissed item ignores progress, and its card decides the reopen",
            &[Raise, Dismiss, Triage, Ground, Present, Reopen],
            &[Raised, Dismissed, Dismissed, Dismissed, Dismissed, Grounded],
        ),
        (
            "reopening a decision without a card starts over",
            &[Raise, Triage, Decide, Reopen, Present],
            &[Raised, Triaged, Decided, Raised, Presented],
        ),
        (
            "defaults and supersessions reopen too",
            &[Raise, Default, Ack, Reopen, Ground, Supersede, Reopen],
            &[
                Raised, Defaulted, Defaulted, Raised, Grounded, Superseded, Grounded,
            ],
        ),
        (
            "a later outcome replaces an earlier one",
            &[Raise, Decide, Dismiss],
            &[Raised, Decided, Dismissed],
        ),
        (
            "a second raise changes nothing and a deferral can be decided",
            &[Raise, Raise, Defer, Decide],
            &[Raised, Raised, Deferred, Decided],
        ),
    ];

    for (name, sequence, expected) in cases {
        assert_eq!(statuses(sequence)?, expected, "{name}");
    }
    Ok(())
}

#[test]
fn replaying_every_raise_changes_no_item() {
    for case in 0..CASES {
        let mut rng = Rng::new(0x9e_0004, case);
        let events = events(&mut rng);
        let positions: Vec<u64> = (1u64..).take(events.len()).collect();
        let once = fold(&positions, &events);
        let raises: Vec<&EventEnvelope> = events
            .iter()
            .filter(|e| matches!(e.event, LedgerEvent::ItemRaised(_)))
            .collect();
        let mut replayed = once.clone();

        for (position, env) in (1000u64..).zip(raises) {
            replayed.apply(position, env);
        }

        assert_eq!(
            replayed.items, once.items,
            "case {case}: duplicate raises keep the first"
        );
        for item in replayed.items.values() {
            let unique: BTreeSet<&String> = item.also_raised_in.iter().collect();
            assert_eq!(
                unique.len(),
                item.also_raised_in.len(),
                "case {case}: each session once in also_raised_in"
            );
        }
    }
}
