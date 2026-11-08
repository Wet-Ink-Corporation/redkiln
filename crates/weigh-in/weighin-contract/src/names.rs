//! Every `weighin.*` event type with its version (SPEC EV-01, EV-03, EV-06).
//!
//! One table names the 13 ledger events of [`LedgerEvent`](crate::event::LedgerEvent), each
//! version 1 and durable, plus the decision request's own type (DR-01). With the `core` feature
//! each payload is a Core [`Payload`](redkiln_core::envelope::Payload) under its name, so a
//! sibling writes and reads it through Core's envelope with no wrapper of its own.

use crate::event::{
    Acknowledged, Decided, Defaulted, Deferred, Dismissed, Grounded, Linked, Presented, Raised,
    RecordWritten, Reopened, Superseded, Triaged,
};

/// One event type: its full name and the version written today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventName {
    /// `weighin.<type>`.
    pub name: &'static str,
    /// The newest version, the one written (EV-06).
    pub version: u32,
}

/// A payload type and the event name it is written under.
pub trait Named {
    /// Its name and version.
    const NAME: EventName;
}

/// One table of `(payload, name)` for [`ITEM_EVENTS`], the [`Named`] impls and, with `core`, the
/// [`Payload`](redkiln_core::envelope::Payload) impls (ARC-15).
macro_rules! item_events {
    ($($payload:ty = $name:literal,)+) => {
        $(
            impl Named for $payload {
                const NAME: EventName = EventName { name: concat!("weighin.", $name), version: 1 };
            }

            #[cfg(feature = "core")]
            impl redkiln_core::envelope::Payload for $payload {
                const TYPE: &'static str = <$payload as Named>::NAME.name;
                const VERSION: u32 = <$payload as Named>::NAME.version;
                const CLASS: redkiln_core::envelope::Class = redkiln_core::envelope::Class::Durable;
            }
        )+

        /// The 13 ledger events, in [`LedgerEvent`](crate::event::LedgerEvent)'s order.
        pub const ITEM_EVENTS: [EventName; 13] = [$(<$payload as Named>::NAME),+];
    };
}

item_events! {
    Raised = "item_raised",
    Triaged = "item_triaged",
    Grounded = "item_grounded",
    Presented = "item_presented",
    Decided = "item_decided",
    Deferred = "item_deferred",
    Defaulted = "item_defaulted",
    Dismissed = "item_dismissed",
    Reopened = "item_reopened",
    Superseded = "item_superseded",
    Linked = "item_linked",
    RecordWritten = "record_written",
    Acknowledged = "item_acknowledged",
}

/// The decision request's event (DR-01): it raises an item that arrives grounded. Weigh-In does
/// not fold it yet; the first raiser arrives with Bench (Move 2).
pub const REQUEST_RAISED: EventName = EventName {
    name: "weighin.request_raised",
    version: 1,
};

/// The channel tag a decision request carries, `weighin.channel:request` (DR-01).
pub const REQUEST_CHANNEL: &str = "request";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EVENT_TYPES;

    #[test]
    fn every_ledger_event_is_named_by_its_serde_tag() {
        assert_eq!(EVENT_TYPES.len(), ITEM_EVENTS.len());
        for (tag, name) in EVENT_TYPES.iter().zip(ITEM_EVENTS) {
            assert_eq!(name.name, format!("weighin.{tag}"));
            assert_eq!(name.version, 1, "{tag}");
        }
    }

    #[test]
    fn the_request_is_a_weighin_type_no_ledger_event_uses() {
        assert!(REQUEST_RAISED.name.starts_with("weighin."));
        assert!(ITEM_EVENTS.iter().all(|e| e.name != REQUEST_RAISED.name));
    }
}
