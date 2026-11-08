//! Triage's routing rule for `default`.
//!
//! Here so a sibling that triages can apply the same rule without Weigh-In's CLI (Weigh-In call
//! B). It is the route rule only: a deferred item whose
//! deadline passes is defaulted whatever its authority (`settle`), and a decision request's own
//! guards are `Request::check` (DR-04, `core` feature). A card's own guards are
//! [`Card::validate`](crate::Card::validate).

use crate::vocab::{Authority, Door};

/// Why an item may not be routed to `default`. Nothing was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DefaultRefused {
    /// The door is not two-way: a one-way item is never defaulted, and an unsure one counts as
    /// one-way (DR-02, DR-07, wi-4d8b6a).
    #[error("only a two-way door may be defaulted (door {})", .0.as_str())]
    Door(Door),
    /// The call needs the owner's authority, which triage's default route does not stand in for.
    #[error("only a judgment call may be defaulted ({})", .0.as_str())]
    Authority(Authority),
}

/// Whether triage may route an item to `default`: only a two-way door that is a judgment call.
///
/// # Errors
/// [`DefaultRefused::Door`] for a one-way or unsure door, checked first;
/// [`DefaultRefused::Authority`] for an authority-bound call.
///
/// # Examples
///
/// ```
/// use weighin_contract::guard::{DefaultRefused, may_route_to_default};
/// use weighin_contract::vocab::{Authority, Door};
///
/// assert_eq!(may_route_to_default(Door::TwoWay, Authority::Judgment), Ok(()));
/// assert_eq!(
///     may_route_to_default(Door::OneWay, Authority::Judgment),
///     Err(DefaultRefused::Door(Door::OneWay))
/// );
/// ```
pub fn may_route_to_default(door: Door, authority: Authority) -> Result<(), DefaultRefused> {
    match (door, authority) {
        (Door::OneWay | Door::Unsure, _) => Err(DefaultRefused::Door(door)),
        (Door::TwoWay, Authority::Authority) => Err(DefaultRefused::Authority(authority)),
        (Door::TwoWay, Authority::Judgment) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unsure_door_is_never_routed_to_default() {
        assert_eq!(
            may_route_to_default(Door::Unsure, Authority::Judgment),
            Err(DefaultRefused::Door(Door::Unsure))
        );
    }

    #[test]
    fn a_two_way_call_that_needs_the_owner_is_not_routed_to_default() {
        assert_eq!(
            may_route_to_default(Door::TwoWay, Authority::Authority),
            Err(DefaultRefused::Authority(Authority::Authority))
        );
    }

    #[test]
    fn the_door_is_named_before_the_authority() {
        assert_eq!(
            may_route_to_default(Door::OneWay, Authority::Authority),
            Err(DefaultRefused::Door(Door::OneWay))
        );
    }
}
