//! Settling an item, or unsettling it: decide, defer, default, dismiss and reopen.

use anyhow::{Result, anyhow, bail};
use std::fmt::Write as _;
use weighin_core::event::{
    Decided, DefaultCause, Defaulted, Deferred, DismissReason, Dismissed, LedgerEvent, Provenance,
    Reopened,
};
use weighin_core::marker::{Authority, Door, authority_tripwire};
use weighin_core::{Item, Status};

use super::{Change, DecideOp, DeferOp, Session};
use crate::env;

impl Session<'_> {
    pub(super) fn defer(&mut self, d: &DeferOp) -> Result<String> {
        let item = self.item(&d.id)?;
        check_deferral(&item, d)?;
        let owner = d
            .owner
            .clone()
            .unwrap_or_else(|| self.env.owner().to_owned());
        self.write(
            &item.id,
            vec![LedgerEvent::ItemDeferred(Deferred {
                item_id: item.id.clone(),
                flip: d.flip.clone(),
                trigger: d.trigger.clone(),
                resolve_by: d.resolve_by.clone(),
                default_option: d.default.clone(),
                owner,
                unqualified: d.unqualified,
                reason: d.reason.clone(),
            })],
        )?;
        let mut receipt = format!(
            "deferred {} until {} (default {} by {})",
            item.id, d.trigger, d.default, d.resolve_by
        );
        if item.door() == Door::OneWay
            && let Some(path) = self.write_open_question(&item.id)?
        {
            write!(receipt, " · tracking atom {path}")?;
        }
        Ok(receipt)
    }

    pub(super) fn decide(&mut self, d: &DecideOp) -> Result<String> {
        let item = self.item(&d.id)?;
        let matched = check_answer(&item, &d.option)?;
        let provenance = match d.provenance.as_deref() {
            Some("user-approved") => Provenance::UserApproved,
            Some("user-directed") => Provenance::UserDirected,
            _ if matched == Some(true) => Provenance::UserApproved,
            _ => Provenance::UserDirected,
        };
        let now = env::now();
        let seconds = d.seconds.or_else(|| {
            item.presented_ts
                .as_deref()
                .and_then(|p| env::seconds_between(p, &now))
        });
        let tree = self.tree_hash();
        let decider = self.env.human();
        self.write(
            &item.id,
            vec![LedgerEvent::ItemDecided(Decided {
                item_id: item.id.clone(),
                option: d.option.clone(),
                decider,
                provenance,
                note: d.note.clone().unwrap_or_default(),
                tree_hash: tree,
                matched_recommendation: matched,
                via: d.via.clone().unwrap_or_else(|| "chat".into()),
                seconds_to_decide: seconds,
                reason: match matched {
                    Some(true) => "chose the recommended option".into(),
                    Some(false) => "chose against the recommendation".into(),
                    None => "answered".into(),
                },
            })],
        )?;
        self.decision_receipt(&item.id, &d.option, matched)
    }
}

/// The option `option` of an open item, checked against its card: `Some(matched)` says whether
/// it is the recommendation, `None` that the item has no card.
fn check_answer(item: &Item, option: &str) -> Result<Option<bool>> {
    if !item.status.is_open() {
        bail!(
            "{} is {}, not open; reopen it first",
            item.id,
            item.status.as_str()
        );
    }
    let Some(card) = &item.card else {
        return Ok(None);
    };
    let opt = card.option(option).ok_or_else(|| {
        anyhow!(
            "{} has no option `{option}` (options: {})",
            item.id,
            card.options
                .iter()
                .map(|o| o.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    if let Some(m) = &opt.fails_must {
        bail!("{}: option {option} fails a MUST ({m})", item.id);
    }
    Ok(Some(card.recommendation.option == option))
}

fn check_deferral(item: &Item, d: &DeferOp) -> Result<()> {
    if !item.status.is_open() {
        bail!("{} is {}, not open", item.id, item.status.as_str());
    }
    if let Some(card) = &item.card
        && card.option(&d.default).is_none()
    {
        bail!("{}: default `{}` is not an option", item.id, d.default);
    }
    // A bare "defer" from the user is recorded as unqualified: it names no flip fact and is
    // counted against the action rate.
    for (k, v) in [
        ("flip", if d.unqualified { &d.trigger } else { &d.flip }),
        ("trigger", &d.trigger),
        ("resolve_by", &d.resolve_by),
    ] {
        if v.trim().is_empty() {
            bail!("a deferral needs {k}: waiting without a named fact is the can being kicked");
        }
    }
    Ok(())
}

pub(super) fn default_event(
    item: &Item,
    option: &str,
    cause: &str,
    reason: &str,
) -> Result<Change> {
    let cause = DefaultCause::parse(cause)
        .ok_or_else(|| anyhow!("cause must be two-way, expired or unattended"))?;
    if item.door() != Door::TwoWay {
        bail!(
            "{} is a {} door; only a two-way door is defaulted (triage it first)",
            item.id,
            item.door().as_str()
        );
    }
    if cause != DefaultCause::Expired && item.authority() != Authority::Judgment {
        bail!(
            "{} is authority-bound; only its resolve-by expiring may default it (triage it as judgment if that is wrong)",
            item.id
        );
    }
    Ok(Change {
        event: LedgerEvent::ItemDefaulted(Defaulted {
            item_id: item.id.clone(),
            option: option.to_owned(),
            cause,
            reason: reason.to_owned(),
        }),
        receipt: format!("defaulted {} to {option} ({})", item.id, cause.as_str()),
    })
}

pub(super) fn dismiss_event(
    item: &Item,
    why: &str,
    note: Option<&str>,
    reason: &str,
) -> Result<Change> {
    let why = DismissReason::parse(why).ok_or_else(|| anyhow!("why must be not-useful, recoverable, moot, already-decided:<ref>, duplicate-of:<id> or not-my-call:<who>"))?;
    // Papering-over guard: an authority-sounding item is "already decided" only by an accepted
    // .kb decision or by the user's own words choosing it.
    if let DismissReason::AlreadyDecided(r) = &why
        && let Some(word) = authority_tripwire(&format!("{} {}", item.question, item.quote))
        && !(r.starts_with("kb-") || r.starts_with("chat:"))
    {
        bail!(
            "{} is authority-bound (`{word}`): cite the accepted atom (already-decided:kb-…) or the user's words choosing it (already-decided:chat:\"…\"). \"Assume and keep going\" delegates the work, not this decision; present it instead",
            item.id
        );
    }
    let receipt = format!("dismissed {} ({})", item.id, why.label());
    Ok(Change {
        event: LedgerEvent::ItemDismissed(Dismissed {
            item_id: item.id.clone(),
            why,
            note: note.unwrap_or_default().to_owned(),
            reason: reason.to_owned(),
        }),
        receipt,
    })
}

pub(super) fn reopen_event(item: &Item, note: Option<&str>, reason: &str) -> Result<Change> {
    if item.status.is_open() && item.status != Status::Deferred {
        bail!("{} is already open", item.id);
    }
    Ok(Change {
        event: LedgerEvent::ItemReopened(Reopened {
            item_id: item.id.clone(),
            note: note.unwrap_or_default().to_owned(),
            reason: reason.to_owned(),
        }),
        receipt: format!("reopened {}", item.id),
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::ops::Session;
    use crate::ops::test_support::{ONE_WAY, TWO_WAY, after, env_in, grounded, id_of, run};

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn defaults_and_dismissals_print_their_cause() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let env = env_in(dir.path(), "s1");
        let batch = serde_json::json!([
            {"op": "raise", "q": TWO_WAY, "door": "two-way", "authority": "judgment"},
            {"op": "default", "id": "@1", "option": "apply", "cause": "two-way", "reason": "r"},
            {"op": "raise", "q": "Which colour for the badge?"},
            {"op": "dismiss", "id": "@2", "why": "moot", "reason": "r"}
        ]);

        let out = run(&mut Session::open(&env, "t")?, &batch)?;

        let two_way = id_of(TWO_WAY);
        let badge = id_of("Which colour for the badge?");
        assert_eq!(
            out,
            [
                format!("✓ raised {two_way} (raised)"),
                format!("✓ defaulted {two_way} to apply (two-way)"),
                format!("✓ raised {badge} (raised)"),
                format!("✓ dismissed {badge} (moot)"),
            ]
        );
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn decide_refuses_an_unknown_option_and_one_that_fails_a_must() -> Result<()> {
        let out = after(
            &grounded(),
            &[
                serde_json::json!({"op": "decide", "id": "@1", "option": "Z"}),
                serde_json::json!({"op": "decide", "id": "@1", "option": "C"}),
            ],
        )?;

        let id = id_of(ONE_WAY);
        assert_eq!(
            out,
            [
                format!("✗ {id} has no option `Z` (options: A, B, C)"),
                format!("✗ {id}: option C fails a MUST (one schema)"),
            ]
        );
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn defer_refuses_a_missing_flip_and_a_default_that_is_not_an_option() -> Result<()> {
        let out = after(
            &grounded(),
            &[
                serde_json::json!({"op": "defer", "id": "@1", "trigger": " ", "resolve_by": "soon", "default": "A"}),
                serde_json::json!({"op": "defer", "id": "@1", "flip": "f", "trigger": "t", "resolve_by": "soon", "default": "Z"}),
            ],
        )?;

        let id = id_of(ONE_WAY);
        assert_eq!(
            out,
            [
                "✗ a deferral needs flip: waiting without a named fact is the can being kicked"
                    .to_owned(),
                format!("✗ {id}: default `Z` is not an option"),
            ]
        );
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn default_refuses_an_unknown_cause_and_a_one_way_door() -> Result<()> {
        let out = after(
            &grounded(),
            &[
                serde_json::json!({"op": "default", "id": "@1", "option": "A", "cause": "whenever", "reason": "r"}),
                serde_json::json!({"op": "default", "id": "@1", "option": "A", "cause": "two-way", "reason": "r"}),
            ],
        )?;

        let id = id_of(ONE_WAY);
        assert_eq!(
            out,
            [
                "✗ cause must be two-way, expired or unattended".to_owned(),
                format!(
                    "✗ {id} is a one-way door; only a two-way door is defaulted (triage it first)"
                ),
            ]
        );
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn dismiss_refuses_an_unknown_why() -> Result<()> {
        let out = after(
            &grounded(),
            &[serde_json::json!({"op": "dismiss", "id": "@1", "why": "bored", "reason": "r"})],
        )?;

        assert_eq!(
            out,
            [
                "✗ why must be not-useful, recoverable, moot, already-decided:<ref>, duplicate-of:<id> or not-my-call:<who>"
            ]
        );
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn reopen_refuses_an_open_item() -> Result<()> {
        let out = after(
            &grounded(),
            &[serde_json::json!({"op": "reopen", "id": "@1"})],
        )?;

        assert_eq!(out, [format!("✗ {} is already open", id_of(ONE_WAY))]);
        Ok(())
    }
}
