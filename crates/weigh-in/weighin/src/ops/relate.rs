//! Relating an item to others, or to the digest: supersede, link and acknowledge.

use anyhow::{Result, bail};
use weighin_core::event::{Acknowledged, LedgerEvent, Linked, Superseded};
use weighin_core::{Item, Status};

use super::Change;

pub(super) fn supersede_event(item: &Item, by: &str, reason: &str) -> Change {
    Change {
        event: LedgerEvent::ItemSuperseded(Superseded {
            item_id: item.id.clone(),
            by: by.to_owned(),
            reason: reason.to_owned(),
        }),
        receipt: format!("{} superseded by {by}", item.id),
    }
}

pub(super) fn link_event(item: &Item, kind: &str, target: &str, reason: Option<&str>) -> Change {
    Change {
        event: LedgerEvent::ItemLinked(Linked {
            item_id: item.id.clone(),
            kind: kind.to_owned(),
            target: target.to_owned(),
            reason: reason.unwrap_or("linked").to_owned(),
        }),
        receipt: format!("linked {} → {kind} {target}", item.id),
    }
}

/// Leave a default standing after the digest, or review a standing decision and leave it
/// standing (D-0022): both are `item_acknowledged`, so an older build still reads the event.
pub(super) fn ack_event(item: &Item, reason: Option<&str>) -> Result<Change> {
    let (default_reason, receipt) = match item.status {
        Status::Defaulted => (
            "left as defaulted after the digest",
            format!("acknowledged default on {}", item.id),
        ),
        Status::Decided => (
            "left standing after the digest",
            format!("reviewed decision on {}: still holds", item.id),
        ),
        other => bail!(
            "{} is {}, only defaults and decisions are acknowledged",
            item.id,
            other.as_str()
        ),
    };
    Ok(Change {
        event: LedgerEvent::ItemAcknowledged(Acknowledged {
            item_id: item.id.clone(),
            reason: reason.unwrap_or(default_reason).to_owned(),
        }),
        receipt,
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use weighin_core::event::LedgerEvent;

    use super::ack_event;
    use crate::ops::Session;
    use crate::ops::test_support::{ONE_WAY, TWO_WAY, after, env_in, grounded, id_of, run};

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn ack_on_a_decision_records_a_review_with_its_own_reason() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let env = env_in(dir.path(), "s1");
        let mut s = Session::open(&env, "t")?;
        run(
            &mut s,
            &serde_json::json!([
                {"op": "raise", "q": TWO_WAY, "door": "two-way"},
                {"op": "decide", "id": "@1", "option": "apply"}
            ]),
        )?;
        let item = s.ledger.get(&id_of(TWO_WAY)).expect("raised above");

        let change = ack_event(item, None)?;

        assert_eq!(
            change.receipt,
            format!("reviewed decision on {}: still holds", item.id)
        );
        assert!(
            matches!(&change.event, LedgerEvent::ItemAcknowledged(a) if a.reason == "left standing after the digest"),
            "{:?}",
            change.event
        );
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn ack_refuses_an_item_that_was_not_defaulted() -> Result<()> {
        let out = after(&grounded(), &[serde_json::json!({"op": "ack", "id": "@1"})])?;

        assert_eq!(
            out,
            [format!(
                "✗ {} is grounded, only defaults and decisions are acknowledged",
                id_of(ONE_WAY)
            )]
        );
        Ok(())
    }
}
