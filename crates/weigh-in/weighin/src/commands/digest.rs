//! `digest`: the defaults taken without you, the standing decisions due a look (D-0022), and
//! the untriaged items from other sessions.

use anyhow::Result;
use std::io::Write;
use weighin_core::{Item, Status};

use super::{clip, write_line};
use crate::JsonArgs;
use crate::env::{self, Env};
use crate::ops::read_ledger;

/// `digest --json`: the defaults, the decisions due a look and the untriaged items.
#[derive(serde::Serialize)]
struct DigestJson<'l> {
    defaults: &'l [&'l Item],
    /// Present only when some decision is due, so a digest with none prints as it did before.
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    due_for_review: &'l [Due<'l>],
    untriaged_from_other_sessions: &'l [&'l Item],
}

/// A standing decision due a look: the item, its days since the last look and its flip.
#[derive(serde::Serialize)]
struct Due<'l> {
    age_days: u64,
    flip_condition: Option<&'l str>,
    #[serde(flatten)]
    item: &'l Item,
}

impl<'l> Due<'l> {
    fn of((item, age_days): (&'l Item, u64)) -> Self {
        Self {
            age_days,
            flip_condition: item.flip_condition(),
            item,
        }
    }
}

/// The `digest --json` line. It goes through `serde_json::Value`, whose maps are sorted, so
/// the keys at every level, items included, come out in alphabetical order.
fn digest_json(doc: &DigestJson<'_>) -> serde_json::Result<String> {
    serde_json::to_string(&serde_json::to_value(doc)?)
}

pub(crate) fn digest(env: &Env, a: &JsonArgs, out: &mut impl Write) -> Result<()> {
    let ledger = read_ledger(env)?;
    let defaults = ledger.unacknowledged_defaults();
    let due: Vec<Due<'_>> = ledger
        .due_for_review(&env::now(), env.config.caps.review_age_days)
        .into_iter()
        .map(Due::of)
        .collect();
    let untriaged: Vec<&Item> = ledger
        .open()
        .into_iter()
        .filter(|i| i.status == Status::Raised && env.session.as_deref() != i.session_id.as_deref())
        .collect();
    if a.json {
        let doc = DigestJson {
            defaults: &defaults,
            due_for_review: &due,
            untriaged_from_other_sessions: &untriaged,
        };
        writeln!(out, "{}", digest_json(&doc)?)?;
        return Ok(());
    }
    writeln!(
        out,
        "{} default(s) taken without you · {} untriaged item(s) from other sessions",
        defaults.len(),
        untriaged.len()
    )?;
    for i in defaults {
        // `unacknowledged_defaults` only returns items with a default.
        let Some(d) = i.default.as_ref() else {
            continue;
        };
        writeln!(
            out,
            "{} [{}] {} → took: {}",
            i.id,
            d.cause.as_str(),
            clip(&i.question, 90),
            clip(&d.option, 80)
        )?;
    }
    write_due(out, &due)?;
    for i in untriaged {
        write_line(out, i)?;
    }
    Ok(())
}

/// The decisions due a look, one line each under a heading; nothing when none is due.
fn write_due(out: &mut impl Write, due: &[Due<'_>]) -> std::io::Result<()> {
    if due.is_empty() {
        return Ok(());
    }
    writeln!(out, "Decisions due a look ({})", due.len())?;
    for d in due {
        let option = d.item.decision.as_ref().map_or("?", |c| c.option.as_str());
        writeln!(
            out,
            "{} [{}d] {} → {} · flips if: {}",
            d.item.id,
            d.age_days,
            clip(&d.item.question, 90),
            clip(option, 60),
            clip(d.flip_condition.unwrap_or(""), 80)
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{DigestJson, digest_json};
    use weighin_core::event::{Channel, Raised};
    use weighin_core::marker::{Authority, Door, Kind};
    use weighin_core::{EventEnvelope, ItemId, Ledger, LedgerEvent};

    fn raised_ledger() -> Ledger {
        let id = ItemId::mint("s1", "Where should the ledger live?");
        let mut l = Ledger::default();
        l.apply(
            1,
            &EventEnvelope {
                event: LedgerEvent::ItemRaised(Raised {
                    item_id: id,
                    kind: Kind::Question,
                    question: "Where should the ledger live?".into(),
                    did: None,
                    flip: None,
                    at: None,
                    sym: None,
                    blocks: None,
                    door_hint: Door::TwoWay,
                    authority_hint: Authority::Authority,
                    channel: Channel::Marker,
                    quote: "Where should the ledger live?".into(),
                    source_path: None,
                    source_line: None,
                }),
                actor: "test".into(),
                session_id: Some("s1".into()),
                worktree: None,
                branch: None,
                head: None,
                ts: "2026-09-26T00:00:00Z".into(),
            },
        );
        l
    }

    #[test]
    fn digest_json_prints_every_key_sorted_as_a_json_value_does() {
        let ledger = raised_ledger();
        let untriaged = ledger.open();
        assert_eq!(untriaged.len(), 1);
        let printed = digest_json(&DigestJson {
            defaults: &[],
            due_for_review: &[],
            untriaged_from_other_sessions: &untriaged,
        })
        .unwrap();
        let expected =
            serde_json::json!({ "defaults": [], "untriaged_from_other_sessions": untriaged })
                .to_string();
        assert_eq!(printed, expected);
        assert!(
            printed.starts_with(
                r#"{"defaults":[],"untriaged_from_other_sessions":[{"also_raised_in":[],"at":null,"#
            ),
            "{printed}"
        );
    }
}
