//! Getting an item ready for the owner: triage routes it, grounding gives it a card, and
//! presenting records that it was shown.

use anyhow::{Result, anyhow, bail};
use weighin_core::Item;
use weighin_core::card::Card;
use weighin_core::event::{Blast, Grounded, LedgerEvent, Presented, Route, Triaged};
use weighin_core::marker::{Authority, Door};

use super::input::TriageOp;
use super::{Change, Session};

impl Session<'_> {
    pub(super) fn present(&mut self, id: &str, surface: Option<&str>) -> Result<String> {
        let item = self.item(id)?;
        if !item.status.is_open() {
            bail!("{} is {}, not open", item.id, item.status.as_str());
        }
        let tree = self.tree_hash();
        let surface = surface.unwrap_or("chat");
        self.write(
            &item.id,
            vec![LedgerEvent::ItemPresented(Presented {
                item_id: item.id.clone(),
                surface: surface.to_owned(),
                tree_hash: tree,
                reason: format!("shown in {surface}"),
            })],
        )?;
        Ok(format!("presented {} ({surface})", item.id))
    }
}

pub(super) fn triage_event(item: &Item, t: &TriageOp) -> Result<Change> {
    let blast = Blast::parse(&t.blast)
        .ok_or_else(|| anyhow!("blast must be file, module, system or external"))?;
    let route = match t.route.as_str() {
        "present" => Route::Present,
        "default" => Route::Default,
        "dismiss" => Route::Dismiss,
        _ => bail!("route must be present, default or dismiss"),
    };
    let door = Door::parse(&t.door);
    let authority = t
        .authority
        .as_deref()
        .map_or_else(|| item.authority(), Authority::parse);
    if route == Route::Default && (door != Door::TwoWay || authority != Authority::Judgment) {
        bail!(
            "{}: only a judgment-bound two-way door may be defaulted (door {}, {})",
            item.id,
            door.as_str(),
            authority.as_str()
        );
    }
    Ok(Change {
        event: LedgerEvent::ItemTriaged(Triaged {
            item_id: item.id.clone(),
            door,
            blast,
            authority,
            route,
            timing: t.timing.clone(),
            resolve_by: t.resolve_by.clone(),
            reason: t.reason.clone(),
        }),
        receipt: format!(
            "triaged {} · {} · {} · {}",
            item.id,
            door.as_str(),
            blast.as_str(),
            route_str(route)
        ),
    })
}

pub(super) fn ground_event(item: &Item, card: &Card, reason: Option<&str>) -> Result<Change> {
    card.validate().map_err(|e| anyhow!("{}: {e}", item.id))?;
    let earned = if card.recommendation_earned() {
        "recommendation earned"
    } else {
        "options first"
    };
    Ok(Change {
        event: LedgerEvent::ItemGrounded(Grounded {
            item_id: item.id.clone(),
            card: card.clone(),
            reason: reason.unwrap_or("grounded").to_owned(),
        }),
        receipt: format!(
            "grounded {} · {} options · {earned}",
            item.id,
            card.options.len()
        ),
    })
}

fn route_str(r: Route) -> &'static str {
    match r {
        Route::Present => "present",
        Route::Default => "default",
        Route::Dismiss => "dismiss",
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::ops::test_support::{ONE_WAY, after, id_of, raised};

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn triage_refuses_unknown_values_and_defaulting_a_one_way_door() -> Result<()> {
        let out = after(
            &raised(),
            &[
                serde_json::json!({"op": "triage", "id": "@1", "door": "one-way", "blast": "galaxy", "route": "present", "reason": "r"}),
                serde_json::json!({"op": "triage", "id": "@1", "door": "one-way", "blast": "file", "route": "later", "reason": "r"}),
                serde_json::json!({"op": "triage", "id": "@1", "door": "one-way", "blast": "file", "route": "default", "reason": "r"}),
            ],
        )?;

        let id = id_of(ONE_WAY);
        assert_eq!(
            out,
            [
                "✗ blast must be file, module, system or external".to_owned(),
                "✗ route must be present, default or dismiss".to_owned(),
                format!(
                    "✗ {id}: only a judgment-bound two-way door may be defaulted (door one-way, authority)"
                ),
            ]
        );
        Ok(())
    }
}
