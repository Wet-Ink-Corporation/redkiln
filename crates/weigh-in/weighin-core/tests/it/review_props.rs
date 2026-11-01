//! Properties of decision ageing (D-0022) over generated histories: one item decided on some
//! day, reviewed a few times, perhaps reopened, and asked about on a later day.

use crate::support::{CASES, Rng};
use weighin_core::card::{Card, CardOption, Recommendation};
use weighin_core::event::{
    Acknowledged, Blast, Channel, Decided, EventEnvelope, Grounded, LedgerEvent, Provenance,
    Raised, Reopened, Route, Triaged,
};
use weighin_core::marker::{Authority, Door, Kind};
use weighin_core::{ItemId, Ledger};

const QUESTION: &str = "Keep holds per series?";
const DOORS: [Door; 3] = [Door::OneWay, Door::TwoWay, Door::Unsure];
const FLIPS: [&str; 3] = ["", "  ", "a tenant asks for per-record holds"];
const AGES: [u32; 5] = [0, 1, 30, 90, 365];

/// Day `n` counted from 2025-01-01 as an RFC 3339 stamp, walking month lengths: an oracle that
/// does not share the fold's arithmetic. Covers 2025 to 2028, a leap year included.
fn date(n: usize) -> String {
    let mut left = n;
    for year in 2025..=2028_u32 {
        let february = if year % 4 == 0 { 29 } else { 28 };
        let months = [31, february, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        for (month, len) in (1u32..).zip(months) {
            if left < len {
                return format!("{year}-{month:02}-{:02}T12:00:00Z", left + 1);
            }
            left -= len;
        }
    }
    panic!("day {n} is past 2028");
}

fn envelope(event: LedgerEvent, day: usize) -> EventEnvelope {
    EventEnvelope {
        event,
        actor: "test".to_owned(),
        session_id: Some("s1".to_owned()),
        worktree: None,
        branch: None,
        head: None,
        ts: date(day),
    }
}

fn raise(item_id: ItemId, door_hint: Door) -> LedgerEvent {
    LedgerEvent::ItemRaised(Raised {
        item_id,
        kind: Kind::Question,
        question: QUESTION.to_owned(),
        did: None,
        flip: None,
        at: None,
        sym: None,
        blocks: None,
        door_hint,
        authority_hint: Authority::Authority,
        channel: Channel::Marker,
        quote: QUESTION.to_owned(),
        source_path: None,
        source_line: None,
    })
}

fn ground(item_id: ItemId, flip: &str) -> LedgerEvent {
    let option = |o: &str| CardOption {
        id: o.to_owned(),
        title: o.to_owned(),
        ..CardOption::default()
    };
    LedgerEvent::ItemGrounded(Grounded {
        item_id,
        card: Card {
            options: vec![option("A"), option("B")],
            recommendation: Recommendation {
                option: "A".to_owned(),
                flip: flip.to_owned(),
                ..Recommendation::default()
            },
            ..Card::default()
        },
        reason: "grounded".to_owned(),
    })
}

fn triage(item_id: ItemId, door: Door) -> LedgerEvent {
    LedgerEvent::ItemTriaged(Triaged {
        item_id,
        door,
        blast: Blast::Module,
        authority: Authority::Authority,
        route: Route::Present,
        timing: None,
        resolve_by: None,
        reason: "triaged".to_owned(),
    })
}

fn decide(item_id: ItemId) -> LedgerEvent {
    LedgerEvent::ItemDecided(Decided {
        item_id,
        option: "A".to_owned(),
        decider: "human:owner".to_owned(),
        provenance: Provenance::UserApproved,
        note: String::new(),
        tree_hash: None,
        matched_recommendation: Some(true),
        via: "chat".to_owned(),
        seconds_to_decide: None,
        reason: "decided".to_owned(),
    })
}

fn acknowledge(item_id: ItemId) -> LedgerEvent {
    LedgerEvent::ItemAcknowledged(Acknowledged {
        item_id,
        reason: "still holds".to_owned(),
    })
}

fn reopen(item_id: ItemId) -> LedgerEvent {
    LedgerEvent::ItemReopened(Reopened {
        item_id,
        note: String::new(),
        reason: "reopened".to_owned(),
    })
}

/// What the generator chose, so the expected answer can be worked out without the fold.
struct History {
    door: Door,
    flip: &'static str,
    events: Vec<EventEnvelope>,
    last_look: usize,
    reopened: bool,
    reviews: u32,
}

/// Raised, grounded on a card with a generated flip, perhaps triaged to another door, decided,
/// reviewed up to three times and perhaps reopened, each on a day no earlier than the last.
fn history(rng: &mut Rng, id: &ItemId) -> History {
    let hint = *rng.pick(&DOORS);
    let triaged = rng.one_in(2).then(|| *rng.pick(&DOORS));
    let flip = *rng.pick(&FLIPS);
    let mut day = rng.below(400);
    let mut events = vec![
        envelope(raise(id.clone(), hint), day),
        envelope(ground(id.clone(), flip), day),
    ];
    events.extend(triaged.map(|door| envelope(triage(id.clone(), door), day)));
    events.push(envelope(decide(id.clone()), day));
    let mut reviews = 0;
    for _ in 0..rng.below(4) {
        day += rng.below(120);
        events.push(envelope(acknowledge(id.clone()), day));
        reviews += 1;
    }
    let last_look = day;
    let reopened = rng.one_in(4);
    if reopened {
        day += rng.below(30);
        events.push(envelope(reopen(id.clone()), day));
    }
    History {
        door: triaged.unwrap_or(hint),
        flip,
        events,
        last_look,
        reopened,
        reviews,
    }
}

#[test]
fn a_decision_is_due_exactly_when_two_way_flippable_standing_and_old_enough() {
    let id = ItemId::mint("s1", QUESTION);
    let mut due_seen = 0;
    for case in 0..CASES {
        let mut rng = Rng::new(0x9e_0a6e, case);
        let h = history(&mut rng, &id);
        let ledger = Ledger::fold((1u64..).zip(&h.events));
        let today = h.last_look + rng.below(500);
        let after_days = *rng.pick(&AGES);

        let got = ledger.due_for_review(&date(today), after_days);

        let age = u64::try_from(today - h.last_look).expect("days fit in u64");
        let expected = (after_days > 0
            && !h.reopened
            && h.door == Door::TwoWay
            && !h.flip.trim().is_empty()
            && age >= u64::from(after_days))
        .then_some(age);
        assert!(got.len() <= 1, "case {case}: one item, at most one entry");
        assert_eq!(got.first().map(|(_, age)| *age), expected, "case {case}");
        let item = ledger.get(&id).expect("the raise was folded");
        assert_eq!(item.reviews, h.reviews, "case {case}: reviews counted");
        assert_eq!(
            item.reopened_after_review,
            u32::from(h.reopened && h.reviews > 0),
            "case {case}: reopens after a review"
        );
        due_seen += usize::from(expected.is_some());
    }
    assert!(due_seen > 0, "some case came due");
}
