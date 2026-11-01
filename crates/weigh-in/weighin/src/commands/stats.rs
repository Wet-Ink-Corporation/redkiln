//! `stats`: the health numbers over the ledger and every committed journal.

use anyhow::Result;
use std::collections::BTreeMap;
use std::io::Write;
use weighin_core::projection::Health;
use weighin_core::{Ledger, Status};

use crate::JsonArgs;
use crate::env::Env;
use crate::{journal, ops};

/// The health numbers over the ledger and every committed journal (`journal.rs`).
pub(crate) fn stats(env: &Env, a: &JsonArgs, out: &mut impl Write) -> Result<()> {
    let ledger = journal::pool(ops::stored_events(env)?, journal::read(&env.root())?)?;
    let h = Health::of(&ledger);
    let reviews = Reviews::of(&ledger);
    if a.json {
        let json = StatsJson {
            health: &h,
            decided_via: decided_via(&ledger),
            reviews,
        };
        writeln!(out, "{}", serde_json::to_string(&json)?)?;
        return Ok(());
    }
    writeln!(
        out,
        "presented {} · action rate {} (target ≥60%) · not useful {} (target ≤10%) · overrides {} of {} · median time to decide {} · unqualified deferrals {} · unreviewed defaults {} · reviewed decisions {} (reopened {})",
        h.presented,
        percent(h.acted, h.presented),
        percent(h.not_useful, h.presented),
        h.chosen_against_recommendation,
        h.chosen_with_recommendation_known,
        h.median_seconds_to_decide
            .map_or_else(|| "—".to_owned(), |s| format!("{s}s")),
        h.deferred_unqualified,
        h.defaults_unacknowledged,
        reviews.decisions_reviewed,
        reviews.reopened_after_review
    )?;
    if !ledger.orphans.is_empty() {
        writeln!(
            out,
            "warning: {} event(s) for unknown items",
            ledger.orphans.len()
        )?;
    }
    Ok(())
}

/// What `stats --json` prints: the health numbers, and beside them the decisions still standing
/// by where they were taken, so pane, chat and board stay apart (B-30).
#[derive(serde::Serialize)]
struct StatsJson<'l> {
    #[serde(flatten)]
    health: &'l Health,
    decided_via: BTreeMap<&'l str, usize>,
    #[serde(flatten)]
    reviews: Reviews,
}

/// Reviews of standing decisions (D-0022), counted over every item's history.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
struct Reviews {
    /// Times a standing decision was reviewed and left standing (`item_acknowledged`).
    decisions_reviewed: u64,
    /// Reopens of a decision after it had been reviewed.
    reopened_after_review: u64,
}

impl Reviews {
    fn of(ledger: &Ledger) -> Self {
        ledger.items.values().fold(Self::default(), |r, i| Self {
            decisions_reviewed: r.decisions_reviewed.saturating_add(u64::from(i.reviews)),
            reopened_after_review: r
                .reopened_after_review
                .saturating_add(u64::from(i.reopened_after_review)),
        })
    }
}

/// The items in `ledger` decided and still standing, presented or not, counted by where they
/// were decided (the decision's `via`: `chat`, `board`, `pane`, `cli`).
fn decided_via(ledger: &Ledger) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    let standing = ledger
        .items
        .values()
        .filter(|i| i.status == Status::Decided)
        .filter_map(|i| i.decision.as_ref());
    for d in standing {
        let n: &mut usize = counts.entry(d.via.as_str()).or_default();
        *n = n.saturating_add(1);
    }
    counts
}

/// `n` of `d` as a whole percentage, rounded half to even as `{:.0}` rounds a float, or `—`
/// when there is nothing to divide by (or the count is too large to scale).
fn percent(n: usize, d: usize) -> String {
    rounded_percent(n, d).map_or_else(|| "—".to_owned(), |p| format!("{p}%"))
}

/// `None` when `d` is zero or `n * 100` overflows `usize`.
fn rounded_percent(n: usize, d: usize) -> Option<usize> {
    let scaled = n.checked_mul(100)?;
    let quotient = scaled.checked_div(d)?;
    let remainder = scaled.checked_rem(d)?;
    // `remainder < d`, so `d - remainder` cannot underflow; compare halves without doubling.
    let rest = d.checked_sub(remainder)?;
    let up = remainder > rest || (remainder == rest && quotient % 2 == 1);
    quotient.checked_add(usize::from(up))
}

#[cfg(test)]
mod tests {
    use super::{decided_via, percent};
    use weighin_core::event::{Channel, Decided, Provenance, Raised, Reopened};
    use weighin_core::marker::{Authority, Door, Kind};
    use weighin_core::{EventEnvelope, ItemId, Ledger, LedgerEvent};

    #[test]
    fn percent_rounds_half_to_even_like_float_formatting() {
        assert_eq!(percent(1, 8), "12%");
        assert_eq!(percent(3, 8), "38%");
        assert_eq!(percent(1, 3), "33%");
        assert_eq!(percent(2, 3), "67%");
        assert_eq!(percent(5, 5), "100%");
        assert_eq!(percent(0, 4), "0%");
    }

    #[test]
    fn percent_of_nothing_is_a_dash() {
        assert_eq!(percent(0, 0), "—");
        assert_eq!(percent(3, 0), "—");
    }

    #[test]
    fn percent_matches_zero_decimal_float_formatting_for_small_ratios() {
        for d in 1..=400_u32 {
            for n in 0..=d {
                let float = format!("{:.0}%", 100.0 * f64::from(n) / f64::from(d));
                let n = usize::try_from(n).unwrap();
                let d = usize::try_from(d).unwrap();
                assert_eq!(percent(n, d), float, "{n}/{d}");
            }
        }
    }

    /// A ledger of `events`, each at the next position.
    fn ledger_of(events: Vec<LedgerEvent>) -> Ledger {
        let envelopes: Vec<EventEnvelope> = events
            .into_iter()
            .map(|event| EventEnvelope {
                event,
                actor: "test".into(),
                session_id: Some("s1".into()),
                worktree: None,
                branch: None,
                head: None,
                ts: "2026-10-19T00:00:00Z".into(),
            })
            .collect();
        Ledger::fold((1_u64..).zip(&envelopes))
    }

    #[test]
    fn standing_decisions_are_counted_by_where_they_were_taken() {
        let ids: Vec<ItemId> = ["a?", "b?", "c?", "d?", "e?"]
            .iter()
            .map(|q| ItemId::mint("s1", q))
            .collect();
        let mut events = Vec::new();
        for (id, via) in ids.iter().zip(["chat", "board", "pane", "pane", "pane"]) {
            events.push(LedgerEvent::ItemRaised(Raised {
                item_id: id.clone(),
                kind: Kind::Question,
                question: id.to_string(),
                did: None,
                flip: None,
                at: None,
                sym: None,
                blocks: None,
                door_hint: Door::TwoWay,
                authority_hint: Authority::Authority,
                channel: Channel::Marker,
                quote: id.to_string(),
                source_path: None,
                source_line: None,
            }));
            events.push(LedgerEvent::ItemDecided(Decided {
                item_id: id.clone(),
                option: "A".into(),
                decider: "human:ryan".into(),
                provenance: Provenance::UserDirected,
                note: String::new(),
                tree_hash: None,
                matched_recommendation: None,
                via: via.into(),
                seconds_to_decide: None,
                reason: "answered".into(),
            }));
        }
        events.extend(ids.last().map(|id| {
            LedgerEvent::ItemReopened(Reopened {
                item_id: id.clone(),
                note: String::new(),
                reason: "reopened".into(),
            })
        }));
        let ledger = ledger_of(events);

        let counted = decided_via(&ledger);

        assert_eq!(
            counted.into_iter().collect::<Vec<_>>(),
            [("board", 1), ("chat", 1), ("pane", 2)]
        );
    }
}
