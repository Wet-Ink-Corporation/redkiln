//! Who gave a plan approval or a verdict (`actor`), and through what (`via`): the optional `by`
//! of `plan_approved` and `verdict_set` (#62).
//!
//! The key is `by`, not `actor`: an event is flattened into its envelope, whose `actor` (`cli`,
//! `hook`, ...) names the writer. Each part is written only when known, and `by` only when one
//! is, so an event recorded without them keeps the shape it had before they existed.

use serde::{Deserialize, Serialize};

/// Who gave an approval or a verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    /// A person.
    Human,
    /// A model.
    Model,
}

/// Through what an approval or a verdict was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    /// A Claude Code mod's buttons (the cockpit).
    Mod,
    /// A model's Bash tool call.
    Bash,
    /// A terminal, by hand.
    Terminal,
}

/// Who gave an approval or a verdict and through what, each part as far as its caller said.
///
/// At least one part is known: it is built with [`By::known`], and a stored `by: {}` does not
/// read ([`EmptyBy`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ByFields")]
pub struct By {
    /// Who gave it.
    #[serde(skip_serializing_if = "Option::is_none")]
    actor: Option<Actor>,
    /// Through what it was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    via: Option<Via>,
}

impl By {
    /// `Some` when `actor` or `via` is known; `None` when neither is, so nothing is written.
    #[must_use]
    pub const fn known(actor: Option<Actor>, via: Option<Via>) -> Option<Self> {
        if actor.is_none() && via.is_none() {
            None
        } else {
            Some(Self { actor, via })
        }
    }

    /// Who gave it, if known.
    #[must_use]
    pub const fn actor(self) -> Option<Actor> {
        self.actor
    }

    /// Through what it was given, if known.
    #[must_use]
    pub const fn via(self) -> Option<Via> {
        self.via
    }
}

/// `by` as stored, before it is checked to name at least one part.
#[derive(Deserialize)]
struct ByFields {
    #[serde(default)]
    actor: Option<Actor>,
    #[serde(default)]
    via: Option<Via>,
}

/// A stored `by` that names neither an `actor` nor a `via`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("by names neither an actor nor a via: leave it out")]
pub struct EmptyBy;

impl TryFrom<ByFields> for By {
    type Error = EmptyBy;

    fn try_from(fields: ByFields) -> Result<Self, Self::Error> {
        Self::known(fields.actor, fields.via).ok_or(EmptyBy)
    }
}

// Moved from `bench-ledger` (Move 2 step 3, PR 3.2) unchanged; the two tests that read a `by`
// through `bench-ledger`'s envelope stay there, beside the envelope.
#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Actor, By, Via};
    use crate::event::{BenchEvent as LedgerEvent, PlanApproved};

    #[test]
    fn without_an_actor_or_a_via_no_by_is_written() {
        let event = LedgerEvent::PlanApproved(PlanApproved {
            plan: "p.md".to_owned(),
            hash: "sha256:00".to_owned(),
            by: By::known(None, None),
        });
        let actor_only = By::known(Some(Actor::Human), None);

        assert_eq!(
            (
                serde_json::to_value(&event).expect("serialises"),
                serde_json::to_value(actor_only).expect("serialises"),
            ),
            (
                json!({"type": "plan_approved", "plan": "p.md", "hash": "sha256:00"}),
                json!({"actor": "human"}),
            ),
            "the shape an approval had before #62, and only the parts known"
        );
    }

    #[test]
    fn a_by_tells_who_gave_it_and_through_what() {
        let by = By::known(Some(Actor::Human), Some(Via::Terminal)).expect("both parts known");
        let via_only = By::known(None, Some(Via::Mod)).expect("one part known");

        assert_eq!(
            (by.actor(), by.via(), via_only.actor(), via_only.via()),
            (
                Some(Actor::Human),
                Some(Via::Terminal),
                None,
                Some(Via::Mod)
            ),
        );
    }
}
